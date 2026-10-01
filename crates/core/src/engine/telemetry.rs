//! Telemetry: SPEC.md §8. Turning what a device sends into state.
//!
//! A spec's `telemetry.updates` is a list of rules. Every inbound message is
//! offered to every rule, whether it answers a command or arrived on its own,
//! because a reply to a query carries the same data as a pushed change. A rule
//! that matches produces assignments to state paths; the paths and values are
//! templates over the rule's captures, rendered by the one template renderer,
//! and each value is converted to the type the spec's `state` declares for
//! that path.

use std::collections::BTreeMap;

use regex::Regex;
use serde_json::{Map, Value};

use super::template::{render, sole_value, Values};
use crate::catalog::{ParamSpec, ParamType, Params, StateField};

/// One assignment: a state path and the value to put there.
#[derive(Debug, Clone)]
struct Assign {
    path: String,
    value: String,
    /// Wire text to state value, for enumerations and booleans spelled in the
    /// device's own words. A wire value missing from the map is not assigned.
    map: Option<Map<String, Value>>,
}

#[derive(Debug)]
enum Matcher {
    /// The whole message matches; captures are `{1}`, `{2}`, ...
    Message(Regex),
    /// The first line matches `header`; `line` is applied to every other line.
    Lines { header: Regex, line: Regex },
    /// The first line matches `header`; the other lines are `name: value`.
    Fields {
        header: Regex,
        fields: BTreeMap<String, Assign>,
    },
    /// An OSC message whose address matches; arguments are `{arg0}`, `{arg1}`.
    Osc(Regex),
    /// The JSON reply to an HTTP request whose path matches; `json` names the
    /// values to take, by JSON path.
    HttpJson {
        path: Regex,
        json: BTreeMap<String, String>,
        /// A JSON path to an array: the rule matches once per element, and
        /// `json` paths are taken from the element.
        each: Option<String>,
    },
    /// The XML reply to an HTTP request whose path matches; every element
    /// named `element` is one match, its attributes the captures.
    HttpXml { path: Regex, element: String },
}

#[derive(Debug)]
struct Rule {
    matcher: Matcher,
    assign: Vec<Assign>,
}

/// An inbound message, as the rules see it.
pub(crate) enum Inbound<'a> {
    Text(&'a str),
    Osc {
        address: &'a str,
        args: &'a [Value],
    },
    /// An HTTP reply, with the path and query it answered.
    Http {
        path: &'a str,
        body: &'a [u8],
    },
}

#[derive(Debug, Default)]
pub(crate) struct Telemetry {
    /// Sent after connecting, and again every `renew_every` ms if set.
    pub(crate) subscribe: Vec<Value>,
    pub(crate) renew_every: Option<u64>,
    /// Queries sent on a schedule; their replies go through the rules.
    pub(crate) poll: Vec<Value>,
    pub(crate) poll_every: Option<u64>,
    rules: Vec<Rule>,
    types: Vec<(Vec<String>, String)>,
}

fn regex(v: &Value, what: &str) -> Result<Regex, String> {
    let s = v
        .as_str()
        .ok_or(format!("telemetry: {what} must be a string"))?;
    Regex::new(s).map_err(|e| format!("telemetry: {what} '{s}': {e}"))
}

fn assign(path: &str, v: &Value) -> Result<Assign, String> {
    Ok(match v {
        Value::String(value) => Assign {
            path: path.into(),
            value: value.clone(),
            map: None,
        },
        Value::Object(o) => Assign {
            path: path.into(),
            value: o
                .get("value")
                .and_then(Value::as_str)
                .ok_or(format!("telemetry: '{path}' needs a value"))?
                .into(),
            map: o.get("map").and_then(Value::as_object).cloned(),
        },
        _ => {
            return Err(format!(
                "telemetry: '{path}' must be a template or {{value, map}}"
            ))
        }
    })
}

fn items(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(a)) => a.clone(),
        Some(one) => vec![one.clone()],
        None => Vec::new(),
    }
}

impl Telemetry {
    pub(crate) fn parse(
        spec: Option<&Value>,
        state: &BTreeMap<String, StateField>,
    ) -> Result<Telemetry, String> {
        let types: Vec<(Vec<String>, String)> = state
            .iter()
            .map(|(k, f)| (k.split('.').map(str::to_string).collect(), f.kind.clone()))
            .collect();
        let Some(spec) = spec else {
            return Ok(Telemetry {
                types,
                ..Telemetry::default()
            });
        };
        let every = |v: Option<&Value>| v.and_then(|x| x.get("every_ms")).and_then(Value::as_u64);
        let mut t = Telemetry {
            subscribe: items(spec.get("subscribe").and_then(|s| s.get("send"))),
            renew_every: every(spec.get("subscribe")),
            poll: items(spec.get("poll").and_then(|s| s.get("send"))),
            poll_every: every(spec.get("poll")),
            rules: Vec::new(),
            types,
        };
        for rule in spec
            .get("updates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let mut assigns = Vec::new();
            for (path, v) in rule
                .get("state")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                assigns.push(assign(path, v)?);
            }
            let matcher = if let Some(m) = rule.get("match") {
                Matcher::Message(regex(m, "match")?)
            } else if let Some(a) = rule.get("address") {
                Matcher::Osc(regex(a, "address")?)
            } else if let Some(p) = rule.get("path") {
                let path = regex(p, "path")?;
                if let Some(json) = rule.get("json").and_then(Value::as_object) {
                    let json = json
                        .iter()
                        .map(|(k, v)| {
                            v.as_str()
                                .map(|s| (k.clone(), s.to_string()))
                                .ok_or(format!("telemetry: json '{k}' must be a JSON path"))
                        })
                        .collect::<Result<_, _>>()?;
                    let each = rule
                        .get("json_each")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    Matcher::HttpJson { path, json, each }
                } else if let Some(e) = rule.get("xml_each").and_then(Value::as_str) {
                    Matcher::HttpXml {
                        path,
                        element: e.to_string(),
                    }
                } else {
                    return Err("telemetry: a path rule needs json or xml_each".into());
                }
            } else if let Some(h) = rule.get("header") {
                let header = regex(h, "header")?;
                if let Some(line) = rule.get("each_line") {
                    Matcher::Lines {
                        header,
                        line: regex(line, "each_line")?,
                    }
                } else if let Some(fields) = rule.get("fields").and_then(Value::as_object) {
                    let mut out = BTreeMap::new();
                    for (name, v) in fields {
                        let a = match v {
                            Value::String(path) => Assign {
                                path: path.clone(),
                                value: "{value}".into(),
                                map: None,
                            },
                            Value::Object(o) => Assign {
                                path: o
                                    .get("path")
                                    .and_then(Value::as_str)
                                    .ok_or(format!("telemetry: field '{name}' needs a path"))?
                                    .into(),
                                value: "{value}".into(),
                                map: o.get("map").and_then(Value::as_object).cloned(),
                            },
                            _ => return Err(format!("telemetry: field '{name}' is malformed")),
                        };
                        out.insert(name.clone(), a);
                    }
                    Matcher::Fields {
                        header,
                        fields: out,
                    }
                } else {
                    return Err("telemetry: a header rule needs each_line or fields".into());
                }
            } else {
                return Err("telemetry: a rule needs match, address, header or path".into());
            };
            t.rules.push(Rule {
                matcher,
                assign: assigns,
            });
        }
        // Every path a rule writes must be declared, so its type is known.
        for rule in &t.rules {
            let paths: Vec<&str> = match &rule.matcher {
                Matcher::Fields { fields, .. } => {
                    fields.values().map(|a| a.path.as_str()).collect()
                }
                _ => rule.assign.iter().map(|a| a.path.as_str()).collect(),
            };
            for path in paths {
                let shape = placeholder_to_star(path);
                if t.kind_of(&shape).is_none() {
                    return Err(format!(
                        "telemetry: '{path}' is not declared in the spec's state"
                    ));
                }
            }
        }
        Ok(t)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rules.is_empty() && self.subscribe.is_empty() && self.poll.is_empty()
    }

    /// The declared type of a concrete or starred path.
    fn kind_of(&self, path: &str) -> Option<&str> {
        let parts: Vec<&str> = path.split('.').collect();
        self.types
            .iter()
            .find(|(pattern, _)| {
                pattern.len() == parts.len()
                    && pattern
                        .iter()
                        .zip(&parts)
                        .all(|(p, s)| p == "*" || p == s || *s == "*")
            })
            .map(|(_, k)| k.as_str())
    }

    /// The state patch this message produces, if any rule matches.
    pub(crate) fn apply(&self, message: &Inbound) -> Option<Value> {
        let mut patch = Value::Object(Map::new());
        let mut any = false;
        for rule in &self.rules {
            match (&rule.matcher, message) {
                (Matcher::Message(re), Inbound::Text(text)) => {
                    if let Some(caps) = re.captures(text.trim_end()) {
                        any |= self.assign_all(&rule.assign, &captures(&caps), &mut patch);
                    }
                }
                (Matcher::Lines { header, line }, Inbound::Text(text)) => {
                    let mut lines = text.lines();
                    if !lines.next().is_some_and(|h| header.is_match(h.trim_end())) {
                        continue;
                    }
                    for l in lines {
                        if let Some(caps) = line.captures(l.trim_end()) {
                            any |= self.assign_all(&rule.assign, &captures(&caps), &mut patch);
                        }
                    }
                }
                (Matcher::Fields { header, fields }, Inbound::Text(text)) => {
                    let mut lines = text.lines();
                    if !lines.next().is_some_and(|h| header.is_match(h.trim_end())) {
                        continue;
                    }
                    for l in lines {
                        let Some((name, value)) = l.split_once(':') else {
                            continue;
                        };
                        let Some(a) = fields.get(name.trim()) else {
                            continue;
                        };
                        let values =
                            vec![("value".to_string(), Value::String(value.trim().into()))];
                        any |= self.assign_all(std::slice::from_ref(a), &values, &mut patch);
                    }
                }
                (
                    Matcher::HttpJson {
                        path: re,
                        json,
                        each,
                    },
                    Inbound::Http { path, body },
                ) => {
                    let Some(caps) = re.captures(path) else {
                        continue;
                    };
                    let Ok(doc) = serde_json::from_slice::<Value>(body) else {
                        continue;
                    };
                    let items: Vec<&Value> = match each {
                        Some(each) => super::expect::json_path(&doc, each)
                            .and_then(Value::as_array)
                            .map(|a| a.iter().collect())
                            .unwrap_or_default(),
                        None => vec![&doc],
                    };
                    for item in items {
                        let mut values = captures(&caps);
                        for (name, json_path) in json {
                            if let Some(v) = super::expect::json_path(item, json_path) {
                                values.push((name.clone(), v.clone()));
                            }
                        }
                        any |= self.assign_all(&rule.assign, &values, &mut patch);
                    }
                }
                (Matcher::HttpXml { path: re, element }, Inbound::Http { path, body }) => {
                    let Some(caps) = re.captures(path) else {
                        continue;
                    };
                    let Ok(text) = std::str::from_utf8(body) else {
                        continue;
                    };
                    let Ok(doc) = roxmltree::Document::parse(text.trim_start_matches('\u{feff}'))
                    else {
                        continue;
                    };
                    let base = captures(&caps);
                    for node in doc
                        .descendants()
                        .filter(|n| n.has_tag_name(element.as_str()))
                    {
                        let mut values = base.clone();
                        for a in node.attributes() {
                            values.push((a.name().to_string(), Value::String(a.value().into())));
                        }
                        any |= self.assign_all(&rule.assign, &values, &mut patch);
                    }
                }
                (Matcher::Osc(re), Inbound::Osc { address, args }) => {
                    if let Some(caps) = re.captures(address) {
                        let mut values = captures(&caps);
                        for (i, a) in args.iter().enumerate() {
                            values.push((format!("arg{i}"), a.clone()));
                        }
                        any |= self.assign_all(&rule.assign, &values, &mut patch);
                    }
                }
                _ => {}
            }
        }
        any.then_some(patch)
    }

    fn assign_all(
        &self,
        assigns: &[Assign],
        values: &[(String, Value)],
        patch: &mut Value,
    ) -> bool {
        let mut params = Params::new();
        let mut specs = BTreeMap::new();
        // Captures as they arrived, so a string state or a map sees "0800",
        // not the number 800.
        let texts: BTreeMap<&str, &str> = values
            .iter()
            .filter_map(|(n, v)| v.as_str().map(|s| (n.as_str(), s)))
            .collect();
        for (name, v) in values {
            let kind = match v {
                Value::Number(n) if n.is_i64() => ParamType::Int,
                Value::Number(_) => ParamType::Float,
                Value::Bool(_) => ParamType::Bool,
                Value::String(s) if s.parse::<i64>().is_ok() => ParamType::Int,
                _ => ParamType::String,
            };
            let value = match (kind, v) {
                (ParamType::Int, Value::String(s)) => Value::from(s.parse::<i64>().unwrap()),
                _ => v.clone(),
            };
            params.insert(name.clone(), value);
            specs.insert(name.clone(), spec_of(kind));
        }
        let empty = Params::new();
        let empty_specs = BTreeMap::new();
        let ctx = Values {
            params: &params,
            param_specs: &specs,
            settings: &empty,
            setting_specs: &empty_specs,
        };
        let mut any = false;
        for a in assigns {
            let Ok(path) = render(&a.path, &ctx, |s| s.to_string()) else {
                continue;
            };
            let Some(kind) = self.kind_of(&path) else {
                continue;
            };
            // A value that is one capture, as it arrived.
            let text = a
                .value
                .strip_prefix('{')
                .and_then(|v| v.strip_suffix('}'))
                .filter(|n| !n.contains([':', '{', '}']))
                .and_then(|n| texts.get(n).copied());
            let raw = match (text, kind) {
                (Some(t), "string") => Value::String(t.to_string()),
                _ => match sole_value(&a.value, &ctx) {
                    Some((v, _)) => v.clone(),
                    None => match render(&a.value, &ctx, |s| s.to_string()) {
                        Ok(s) => Value::String(s),
                        Err(_) => continue,
                    },
                },
            };
            let value = match &a.map {
                Some(map) => {
                    let key = match &raw {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    match text.and_then(|t| map.get(t)).or_else(|| map.get(&key)) {
                        Some(v) => v.clone(),
                        None => continue,
                    }
                }
                None => match convert(&raw, kind) {
                    Some(v) => v,
                    None => continue,
                },
            };
            set_path(patch, &path, value);
            any = true;
        }
        any
    }
}

fn spec_of(kind: ParamType) -> ParamSpec {
    ParamSpec {
        kind,
        min: None,
        max: None,
        values: None,
        max_length: None,
        pattern: None,
        default: None,
        required: false,
        secret: false,
    }
}

fn captures(caps: &regex::Captures) -> Vec<(String, Value)> {
    caps.iter()
        .enumerate()
        .skip(1)
        .filter_map(|(i, m)| m.map(|m| (i.to_string(), Value::String(m.as_str().to_string()))))
        .collect()
}

/// `outputs.{1:+1}.input` → `outputs.*.input`.
fn placeholder_to_star(path: &str) -> String {
    path.split('.')
        .map(|seg| if seg.contains('{') { "*" } else { seg })
        .collect::<Vec<_>>()
        .join(".")
}

/// A wire value as the declared state type. `None` when it does not convert:
/// nothing is assigned rather than a guess.
fn convert(raw: &Value, kind: &str) -> Option<Value> {
    match (kind, raw) {
        ("string", Value::String(s)) => Some(Value::String(s.clone())),
        ("string", other) => Some(Value::String(other.to_string())),
        ("int", Value::Number(n)) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .map(Value::from),
        ("int", Value::String(s)) => s.trim().parse::<i64>().ok().map(Value::from),
        ("float", Value::Number(n)) => n.as_f64().map(Value::from),
        ("float", Value::String(s)) => s.trim().parse::<f64>().ok().map(Value::from),
        ("bool", Value::Bool(b)) => Some(Value::Bool(*b)),
        ("bool", Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        _ => None,
    }
}

fn set_path(patch: &mut Value, path: &str, value: Value) {
    let mut node = patch;
    let parts: Vec<&str> = path.split('.').collect();
    for (i, part) in parts.iter().enumerate() {
        let map = match node {
            Value::Object(m) => m,
            _ => return,
        };
        if i == parts.len() - 1 {
            map.insert((*part).to_string(), value);
            return;
        }
        node = map
            .entry((*part).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn state(v: Value) -> BTreeMap<String, StateField> {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn lines_under_a_header_with_offsets() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [{
                "header": "^VIDEO OUTPUT ROUTING:$",
                "each_line": "^(\\d+) (\\d+)$",
                "state": {"outputs.{1:+1}.input": "{2:+1}"},
            }]})),
            &state(json!({"outputs.*.input": {"type": "int", "description": "x"}})),
        )
        .unwrap();
        let patch = t
            .apply(&Inbound::Text("VIDEO OUTPUT ROUTING:\n0 5\n1 3\n"))
            .unwrap();
        assert_eq!(
            patch,
            json!({"outputs": {"1": {"input": 6}, "2": {"input": 4}}})
        );
        assert!(t.apply(&Inbound::Text("INPUT LABELS:\n0 Cam\n")).is_none());
    }

    #[test]
    fn fields_with_a_map() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [{
                "header": "^[25]08 transport info:$",
                "fields": {
                    "status": "transport.status",
                    "speed": "transport.speed",
                    "single clip": {"path": "transport.single_clip", "map": {"true": true, "false": false}},
                },
            }]})),
            &state(json!({
                "transport.status": {"type": "string", "description": "x"},
                "transport.speed": {"type": "int", "description": "x"},
                "transport.single_clip": {"type": "bool", "description": "x"},
            })),
        )
        .unwrap();
        let patch = t
            .apply(&Inbound::Text(
                "508 transport info:\nstatus: play\nspeed: 100\nsingle clip: false\nloop: true\n",
            ))
            .unwrap();
        assert_eq!(
            patch,
            json!({"transport": {"status": "play", "speed": 100, "single_clip": false}})
        );
    }

    #[test]
    fn osc_arguments_and_inverted_flags() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"address": "^/ch/(\\d+)/mix/on$", "state": {"channels.{1}.mute": {"value": "{arg0}", "map": {"0": true, "1": false}}}},
                {"address": "^/ch/(\\d+)/mix/fader$", "state": {"channels.{1}.fader": "{arg0}"}},
            ]})),
            &state(json!({
                "channels.*.mute": {"type": "bool", "description": "x"},
                "channels.*.fader": {"type": "float", "description": "x"},
            })),
        )
        .unwrap();
        let p = t
            .apply(&Inbound::Osc {
                address: "/ch/07/mix/on",
                args: &[json!(0)],
            })
            .unwrap();
        assert_eq!(p, json!({"channels": {"7": {"mute": true}}}));
        let p = t
            .apply(&Inbound::Osc {
                address: "/ch/07/mix/fader",
                args: &[json!(0.75)],
            })
            .unwrap();
        assert_eq!(p, json!({"channels": {"7": {"fader": 0.75}}}));
    }

    #[test]
    fn text_captures_keep_their_form_for_strings_and_maps() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"match": "^OSJ:06:([0-9A-F]{4})$", "state": {"shutter": "{1}"}},
                {"match": "^OGU:([0-9A-F]{2})$", "state": {"gain": {"value": "{1}", "map": {"08": "0dB"}}}},
            ]})),
            &state(json!({
                "shutter": {"type": "string", "description": "x"},
                "gain": {"type": "string", "description": "x"},
            })),
        )
        .unwrap();
        assert_eq!(
            t.apply(&Inbound::Text("OSJ:06:0800")).unwrap(),
            json!({"shutter": "0800"})
        );
        assert_eq!(
            t.apply(&Inbound::Text("OGU:08")).unwrap(),
            json!({"gain": "0dB"})
        );
    }

    #[test]
    fn undeclared_paths_are_refused_at_load() {
        let e = Telemetry::parse(
            Some(&json!({"updates": [{"match": "^X (\\d+)$", "state": {"nope.{1}": "{1}"}}]})),
            &state(json!({})),
        )
        .unwrap_err();
        assert!(e.contains("not declared"));
    }

    #[test]
    fn values_that_do_not_convert_are_not_assigned() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [{"match": "^SPEED (.*)$", "state": {"speed": "{1}"}}]})),
            &state(json!({"speed": {"type": "int", "description": "x"}})),
        )
        .unwrap();
        assert!(t.apply(&Inbound::Text("SPEED fast")).is_none());
    }
}
