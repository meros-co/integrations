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

use super::template::{render, sole_converted, sole_value, Conversions, Values};
use crate::catalog::{ParamSpec, ParamType, Params, StateField};

/// One assignment: a state path and the value to put there.
#[derive(Debug, Clone)]
struct Assign {
    path: String,
    value: String,
    /// Wire text to state value, for enumerations and booleans spelled in the
    /// device's own words. A wire value missing from the map is not assigned.
    map: Option<Map<String, Value>>,
    /// `{delete: true}`: the path, a value or a whole subtree, is removed
    /// from state (a JSON merge patch null).
    delete: bool,
}

#[derive(Debug)]
enum Matcher {
    /// The whole message matches; captures are `{1}`, `{2}`, ... With
    /// `request` (`request_match`), only a reply whose request's text
    /// matches it too, its captures numbered on after the message's.
    Message { re: Regex, request: Option<Regex> },
    /// The first line matches `header`; `line` is applied to every other line.
    Lines { header: Regex, line: Regex },
    /// The first line matches `header`; the other lines are `name: value`.
    Fields {
        header: Regex,
        fields: BTreeMap<String, Assign>,
    },
    /// An OSC message whose address matches; arguments are `{arg0}`, `{arg1}`.
    /// With `json`, the string argument at `json_arg` is parsed as JSON and
    /// the named paths become captures too.
    Osc {
        address: Regex,
        json: BTreeMap<String, String>,
        json_arg: usize,
        /// The message's OSC type tags must be exactly these (`sis`), for a
        /// device that sends different shapes on one address (grandMA3).
        arg_types: Option<String>,
    },
    /// A JSON message on a websocket. Every `select` JSON path must hold a
    /// value matching its regex; their captures are `{1}`, `{2}`, ... in
    /// order. `json` and `each` work as for an HTTP reply.
    Json {
        select: Vec<(String, Regex)>,
        json: BTreeMap<String, String>,
        each: Option<String>,
    },
    /// The JSON reply to an HTTP request whose path matches; `json` names the
    /// values to take, by JSON path. `select` (`json_match`) must match the
    /// reply and `request` (`request_match`) the JSON body of the request it
    /// answers, so replies that share a path, as JSON-RPC's do, can be told
    /// apart. Their captures follow the path's, in order.
    HttpJson {
        path: Regex,
        select: Vec<(String, Regex)>,
        request: Vec<(String, Regex)>,
        json: BTreeMap<String, String>,
        /// A JSON path to an array: the rule matches once per element, and
        /// `json` paths are taken from the element.
        each: Option<String>,
        /// `headers`: capture names to response header names. A rule with
        /// only these reads no body, so the body need not be JSON.
        headers: BTreeMap<String, String>,
    },
    /// The XML reply to an HTTP request whose path matches; every element
    /// named `element` is one match, its attributes the captures.
    HttpXml { path: Regex, element: String },
}

/// A rule's `json:` names and JSON paths; empty when it has none.
/// `json_match`-style selectors: JSON paths to regexes.
fn selectors(v: Option<&Value>, what: &str) -> Result<Vec<(String, Regex)>, String> {
    let Some(v) = v else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (path, re) in v
        .as_object()
        .ok_or(format!("telemetry: {what} maps JSON paths to regexes"))?
    {
        out.push((path.clone(), regex(re, what)?));
    }
    Ok(out)
}

/// Whether every selector matches `doc`; their captures are appended to
/// `base`, numbered on from those already there.
fn select(doc: &Value, selectors: &[(String, Regex)], base: &mut Vec<(String, Value)>) -> bool {
    for (path, re) in selectors {
        let text = match super::expect::json_path(doc, path) {
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => return false,
        };
        let Some(caps) = re.captures(&text) else {
            return false;
        };
        let offset = base.len();
        for (i, v) in captures(&caps) {
            let n: usize = i.parse().unwrap_or(0);
            base.push(((n + offset).to_string(), v));
        }
    }
    true
}

/// A JSON value a rule names, unless it is null: a device that reports
/// nothing there (OpenLP's theme, mimoLive's live variant of a layer that is
/// off) has given no value, and the state keeps what it had rather than the
/// text "null".
fn present(v: Option<&Value>) -> Option<&Value> {
    v.filter(|v| !v.is_null())
}

fn json_paths(rule: &Value) -> Result<BTreeMap<String, String>, String> {
    rule.get("json")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(k, v)| {
            v.as_str()
                .map(|s| (k.clone(), s.to_string()))
                .ok_or(format!("telemetry: json '{k}' must be a JSON path"))
        })
        .collect()
}

#[derive(Debug)]
struct Rule {
    matcher: Matcher,
    assign: Vec<Assign>,
    /// `then_send`: requests queued when the rule matches, templates over
    /// its captures and the settings (a re-read the push only announces).
    then_send: Vec<Value>,
}

/// A `then_send` item a matching rule queued, with the captures it is
/// rendered from.
#[derive(Debug, Clone)]
pub(crate) struct Triggered {
    pub(crate) item: Value,
    pub(crate) params: Params,
    pub(crate) specs: BTreeMap<String, ParamSpec>,
}

/// Captures as template parameters: a number or a numeric string is an
/// integer, a JSON number a float, a JSON boolean a bool, anything else a
/// string.
pub(crate) fn capture_params(values: &[(String, Value)]) -> (Params, BTreeMap<String, ParamSpec>) {
    let mut params = Params::new();
    let mut specs = BTreeMap::new();
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
            // An object or array: its JSON text, as a string.
            (ParamType::String, Value::Object(_) | Value::Array(_)) => Value::String(v.to_string()),
            _ => v.clone(),
        };
        params.insert(name.clone(), value);
        specs.insert(name.clone(), spec_of(kind));
    }
    (params, specs)
}

/// An inbound message, as the rules see it.
pub(crate) enum Inbound<'a> {
    Text(&'a str),
    /// A line transport's reply, with the text of the message it answers:
    /// text to every text rule, and to a `match` rule's `request_match`.
    Answer {
        text: &'a str,
        request: &'a str,
    },
    Osc {
        address: &'a str,
        /// The type tags, without the leading comma.
        types: &'a str,
        args: &'a [Value],
    },
    /// A JSON message received on a websocket.
    Json(&'a Value),
    /// An HTTP reply, with the path and query it answered and the request's
    /// body where that was JSON.
    Http {
        path: &'a str,
        /// The response's headers, names lowercased.
        headers: &'a [(String, String)],
        body: &'a [u8],
        request: Option<&'a Value>,
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
    conversions: Conversions,
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
            delete: false,
        },
        Value::Object(o) if o.get("delete") == Some(&Value::Bool(true)) => Assign {
            path: path.into(),
            value: String::new(),
            map: None,
            delete: true,
        },
        Value::Object(o) => Assign {
            path: path.into(),
            value: o
                .get("value")
                .and_then(Value::as_str)
                .ok_or(format!("telemetry: '{path}' needs a value"))?
                .into(),
            map: o.get("map").and_then(Value::as_object).cloned(),
            delete: false,
        },
        _ => {
            return Err(format!(
                "telemetry: '{path}' must be a template, {{value, map}} or {{delete: true}}"
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
    /// `parse`, shared: rules compile to many regexes (a Yamaha console spec
    /// has about 250), so every device opened with the same telemetry and
    /// state declarations uses one compiled copy.
    pub(crate) fn shared(
        spec: Option<&Value>,
        state: &BTreeMap<String, StateField>,
        conversions: Option<&Value>,
    ) -> Result<std::sync::Arc<Telemetry>, String> {
        use std::collections::HashMap;
        use std::sync::{Arc, Mutex};
        type Cache = Mutex<Option<HashMap<String, Arc<Telemetry>>>>;
        static CACHE: Cache = Mutex::new(None);
        let key = format!(
            "{}\u{0}{}\u{0}{}",
            spec.map(Value::to_string).unwrap_or_default(),
            conversions.map(Value::to_string).unwrap_or_default(),
            state
                .iter()
                .map(|(path, field)| format!("{path}={}", field.kind))
                .collect::<Vec<_>>()
                .join("\u{0}")
        );
        if let Some(hit) = CACHE
            .lock()
            .unwrap()
            .get_or_insert_with(HashMap::new)
            .get(&key)
        {
            return Ok(hit.clone());
        }
        let conversions = super::template::conversions(conversions)?;
        let parsed = Arc::new(Telemetry::parse(spec, state, conversions)?);
        CACHE
            .lock()
            .unwrap()
            .get_or_insert_with(HashMap::new)
            .insert(key, parsed.clone());
        Ok(parsed)
    }

    pub(crate) fn parse(
        spec: Option<&Value>,
        state: &BTreeMap<String, StateField>,
        conversions: Conversions,
    ) -> Result<Telemetry, String> {
        let types: Vec<(Vec<String>, String)> = state
            .iter()
            .map(|(k, f)| (k.split('.').map(str::to_string).collect(), f.kind.clone()))
            .collect();
        let Some(spec) = spec else {
            return Ok(Telemetry {
                types,
                conversions,
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
            conversions,
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
                Matcher::Message {
                    re: regex(m, "match")?,
                    request: match rule.get("request_match") {
                        Some(r @ Value::String(_)) => Some(regex(r, "request_match")?),
                        _ => None,
                    },
                }
            } else if let Some(a) = rule.get("address") {
                Matcher::Osc {
                    address: regex(a, "address")?,
                    json: json_paths(rule)?,
                    json_arg: rule.get("json_arg").and_then(Value::as_u64).unwrap_or(0) as usize,
                    arg_types: rule
                        .get("arg_types")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                }
            } else if let Some(p) = rule.get("path") {
                let path = regex(p, "path")?;
                if rule.get("json").is_some() || rule.get("headers").is_some() {
                    let mut headers = BTreeMap::new();
                    for (name, header) in rule
                        .get("headers")
                        .and_then(Value::as_object)
                        .into_iter()
                        .flatten()
                    {
                        let header = header
                            .as_str()
                            .ok_or(format!("telemetry: headers '{name}' must be a header name"))?;
                        headers.insert(name.clone(), header.to_ascii_lowercase());
                    }
                    let json = json_paths(rule)?;
                    let each = rule
                        .get("json_each")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    Matcher::HttpJson {
                        path,
                        select: selectors(rule.get("json_match"), "json_match")?,
                        request: selectors(rule.get("request_match"), "request_match")?,
                        json,
                        each,
                        headers,
                    }
                } else if let Some(e) = rule.get("xml_each").and_then(Value::as_str) {
                    Matcher::HttpXml {
                        path,
                        element: e.to_string(),
                    }
                } else {
                    return Err("telemetry: a path rule needs json, headers or xml_each".into());
                }
            } else if rule.get("json_match").is_some() {
                Matcher::Json {
                    select: selectors(rule.get("json_match"), "json_match")?,
                    json: json_paths(rule)?,
                    each: rule
                        .get("json_each")
                        .and_then(Value::as_str)
                        .map(str::to_string),
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
                                delete: false,
                            },
                            Value::Object(o) => Assign {
                                path: o
                                    .get("path")
                                    .and_then(Value::as_str)
                                    .ok_or(format!("telemetry: field '{name}' needs a path"))?
                                    .into(),
                                value: "{value}".into(),
                                map: o.get("map").and_then(Value::as_object).cloned(),
                                delete: false,
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
                return Err(
                    "telemetry: a rule needs match, address, header, path or json_match".into(),
                );
            };
            t.rules.push(Rule {
                matcher,
                assign: assigns,
                then_send: items(rule.get("then_send")),
            });
        }
        // Every path a rule writes must be declared, so its type is known.
        for rule in &t.rules {
            let paths: Vec<&Assign> = match &rule.matcher {
                Matcher::Fields { fields, .. } => fields.values().collect(),
                _ => rule.assign.iter().collect(),
            };
            for Assign { path, delete, .. } in paths {
                let shape = placeholder_to_star(path);
                // A deletion may remove a declared value or a subtree of them.
                let declared = match delete {
                    true => t.declares(&shape),
                    false => t.kind_of(&shape).is_some(),
                };
                if !declared {
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

    /// Whether a concrete or starred path is declared, or is a prefix of
    /// declared paths: what a deletion may remove.
    fn declares(&self, path: &str) -> bool {
        let parts: Vec<&str> = path.split('.').collect();
        self.types.iter().any(|(pattern, _)| {
            pattern.len() >= parts.len()
                && pattern
                    .iter()
                    .zip(&parts)
                    .all(|(p, s)| p == "*" || p == s || *s == "*")
        })
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
    #[cfg(test)]
    pub(crate) fn apply(&self, message: &Inbound) -> Option<Value> {
        self.apply_into(message, &mut Vec::new())
    }

    /// As `apply`, adding to `triggers` the `then_send` items of every rule
    /// that matched, once per match, with its captures.
    pub(crate) fn apply_into(
        &self,
        message: &Inbound,
        triggers: &mut Vec<Triggered>,
    ) -> Option<Value> {
        // A reply is text to the text rules; its request is for request_match.
        let (message, request) = match message {
            Inbound::Answer { text, request } => (&Inbound::Text(text), Some(*request)),
            other => (other, None),
        };
        let mut patch = Value::Object(Map::new());
        let mut any = false;
        for rule in &self.rules {
            match (&rule.matcher, message) {
                (Matcher::Message { re, request: asked }, Inbound::Text(text)) => {
                    let Some(caps) = re.captures(text.trim_end()) else {
                        continue;
                    };
                    let mut values = captures(&caps);
                    if let Some(asked) = asked {
                        let Some(request_caps) = request.and_then(|r| asked.captures(r)) else {
                            continue;
                        };
                        let offset = caps.len() - 1;
                        for (i, v) in captures(&request_caps) {
                            let n: usize = i.parse().unwrap_or(0);
                            values.push(((n + offset).to_string(), v));
                        }
                    }
                    any |= self.matched(rule, &values, &mut patch, triggers);
                }
                (Matcher::Lines { header, line }, Inbound::Text(text)) => {
                    let mut lines = text.lines();
                    if !lines.next().is_some_and(|h| header.is_match(h.trim_end())) {
                        continue;
                    }
                    for l in lines {
                        if let Some(caps) = line.captures(l.trim_end()) {
                            any |= self.matched(rule, &captures(&caps), &mut patch, triggers);
                        }
                    }
                }
                (Matcher::Fields { header, fields }, Inbound::Text(text)) => {
                    let mut lines = text.lines();
                    if !lines.next().is_some_and(|h| header.is_match(h.trim_end())) {
                        continue;
                    }
                    self.trigger(rule, &[], triggers);
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
                        select: reply_select,
                        request: request_select,
                        json,
                        each,
                        headers: header_names,
                    },
                    Inbound::Http {
                        path,
                        headers,
                        body,
                        request,
                    },
                ) => {
                    let Some(caps) = re.captures(path) else {
                        continue;
                    };
                    let reads_body = !json.is_empty() || !reply_select.is_empty() || each.is_some();
                    let doc = match serde_json::from_slice::<Value>(body) {
                        Ok(doc) => doc,
                        Err(_) if !reads_body => Value::Null,
                        Err(_) => continue,
                    };
                    let mut base = captures(&caps);
                    for (name, header) in header_names {
                        if let Some((_, v)) = headers.iter().find(|(k, _)| k == header) {
                            base.push((name.clone(), Value::String(v.clone())));
                        }
                    }
                    if !request_select.is_empty()
                        && !request.is_some_and(|r| select(r, request_select, &mut base))
                    {
                        continue;
                    }
                    if !select(&doc, reply_select, &mut base) {
                        continue;
                    }
                    let items: Vec<&Value> = match each {
                        Some(each) => super::expect::json_path(&doc, each)
                            .and_then(Value::as_array)
                            .map(|a| a.iter().collect())
                            .unwrap_or_default(),
                        None => vec![&doc],
                    };
                    for item in items {
                        let mut values = base.clone();
                        for (name, json_path) in json {
                            if let Some(v) = present(super::expect::json_path(item, json_path)) {
                                values.push((name.clone(), v.clone()));
                            }
                        }
                        any |= self.matched(rule, &values, &mut patch, triggers);
                    }
                }
                (Matcher::HttpXml { path: re, element }, Inbound::Http { path, body, .. }) => {
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
                        any |= self.matched(rule, &values, &mut patch, triggers);
                    }
                }
                (
                    Matcher::Json {
                        select: selectors,
                        json,
                        each,
                    },
                    Inbound::Json(doc),
                ) => {
                    let mut base = Vec::new();
                    if !select(doc, selectors, &mut base) {
                        continue;
                    }
                    let items: Vec<&Value> = match each {
                        Some(each) => super::expect::json_path(doc, each)
                            .and_then(Value::as_array)
                            .map(|a| a.iter().collect())
                            .unwrap_or_default(),
                        None => vec![*doc],
                    };
                    for item in items {
                        let mut values = base.clone();
                        for (name, json_path) in json {
                            if let Some(v) = present(super::expect::json_path(item, json_path)) {
                                values.push((name.clone(), v.clone()));
                            }
                        }
                        any |= self.matched(rule, &values, &mut patch, triggers);
                    }
                }
                (
                    Matcher::Osc {
                        address: re,
                        json,
                        json_arg,
                        arg_types,
                    },
                    Inbound::Osc {
                        address,
                        types,
                        args,
                    },
                ) => {
                    if arg_types.as_deref().is_some_and(|t| t != *types) {
                        continue;
                    }
                    if let Some(caps) = re.captures(address) {
                        let mut values = captures(&caps);
                        for (i, a) in args.iter().enumerate() {
                            values.push((format!("arg{i}"), a.clone()));
                        }
                        if !json.is_empty() {
                            let Some(doc) = args
                                .get(*json_arg)
                                .and_then(Value::as_str)
                                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                            else {
                                continue;
                            };
                            for (name, path) in json {
                                if let Some(v) = present(super::expect::json_path(&doc, path)) {
                                    values.push((name.clone(), v.clone()));
                                }
                            }
                        }
                        any |= self.matched(rule, &values, &mut patch, triggers);
                    }
                }
                _ => {}
            }
        }
        any.then_some(patch)
    }

    /// A rule matched with these captures: its assignments, and its
    /// `then_send` items queued.
    fn matched(
        &self,
        rule: &Rule,
        values: &[(String, Value)],
        patch: &mut Value,
        triggers: &mut Vec<Triggered>,
    ) -> bool {
        self.trigger(rule, values, triggers);
        self.assign_all(&rule.assign, values, patch)
    }

    fn trigger(&self, rule: &Rule, values: &[(String, Value)], triggers: &mut Vec<Triggered>) {
        if rule.then_send.is_empty() {
            return;
        }
        let (params, specs) = capture_params(values);
        for item in &rule.then_send {
            let t = Triggered {
                item: item.clone(),
                params: params.clone(),
                specs: specs.clone(),
            };
            // Once per message: a rule matching every element of a list
            // asks for the same re-read once.
            if !triggers
                .iter()
                .any(|x| x.item == t.item && x.params == t.params)
            {
                triggers.push(t);
            }
        }
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
            conversions: &self.conversions,
            maps: &Default::default(),
        };
        let mut any = false;
        for a in assigns {
            let Ok(path) = render(&a.path, &ctx, |s| s.to_string()) else {
                continue;
            };
            if a.delete {
                // A segment rendered empty, or holding a dot, would delete
                // something other than what the rule names.
                if path.split('.').any(str::is_empty)
                    || path.split('.').count() != a.path.split('.').count()
                    || !self.declares(&path)
                {
                    continue;
                }
                set_path(patch, &path, Value::Null);
                any = true;
                continue;
            }
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
                // A conversion of one capture (a fader position to dB) is a
                // number; one outside the conversion is not assigned.
                _ if sole_converted(&a.value, &ctx).is_some() => {
                    match sole_converted(&a.value, &ctx) {
                        Some(Ok(x)) => Value::from(x),
                        _ => continue,
                    }
                }
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
        label: None,
        description: None,
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
            Conversions::new(),
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
            Conversions::new(),
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
            Conversions::new(),
        )
        .unwrap();
        let p = t
            .apply(&Inbound::Osc {
                address: "/ch/07/mix/on",
                types: "i",
                args: &[json!(0)],
            })
            .unwrap();
        assert_eq!(p, json!({"channels": {"7": {"mute": true}}}));
        let p = t
            .apply(&Inbound::Osc {
                address: "/ch/07/mix/fader",
                types: "f",
                args: &[json!(0.75)],
            })
            .unwrap();
        assert_eq!(p, json!({"channels": {"7": {"fader": 0.75}}}));
    }

    #[test]
    fn json_in_an_osc_argument() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [{
                "address": "^/reply/cue_id/([^/]+)/name$",
                "json": {"status": "$.status", "name": "$.data"},
                "state": {"cues.{1}.name": "{name}"},
            }]})),
            &state(json!({"cues.*.name": {"type": "string", "description": "x"}})),
            Conversions::new(),
        )
        .unwrap();
        let reply =
            r#"{"workspace_id":"w","address":"/cue_id/A1/name","status":"ok","data":"Intro"}"#;
        let p = t
            .apply(&Inbound::Osc {
                address: "/reply/cue_id/A1/name",
                types: "s",
                args: &[json!(reply)],
            })
            .unwrap();
        assert_eq!(p, json!({"cues": {"A1": {"name": "Intro"}}}));
        // An argument that is not JSON matches nothing.
        assert_eq!(
            t.apply(&Inbound::Osc {
                address: "/reply/cue_id/A1/name",
                types: "s",
                args: &[json!("Intro")],
            }),
            None
        );
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
            Conversions::new(),
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
    fn json_messages_selected_by_path_with_each() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"json_match": {"$.type": "^parameter_(update|subscribed)$", "$.path": "^/composition/layers/(\\d+)/video/opacity$"},
                 "json": {"value": "$.value"},
                 "state": {"positions.{2}.opacity": "{value}"}},
                {"json_match": {"$.layers": "^\\["}, "json_each": "$.layers",
                 "json": {"id": "$.id", "name": "$.name.value"},
                 "state": {"layers.{id}.name": "{name}"}},
            ]})),
            &state(json!({
                "positions.*.opacity": {"type": "float", "description": "x"},
                "layers.*.name": {"type": "string", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let update = json!({"type": "parameter_update", "path": "/composition/layers/2/video/opacity",
                            "id": 17, "valuetype": "ParamRange", "value": 0.25});
        assert_eq!(
            t.apply(&Inbound::Json(&update)).unwrap(),
            json!({"positions": {"2": {"opacity": 0.25}}})
        );
        let composition = json!({"layers": [{"id": 5, "name": {"value": "Bg"}}, {"id": 6, "name": {"value": "Fg"}}]});
        assert_eq!(
            t.apply(&Inbound::Json(&composition)).unwrap(),
            json!({"layers": {"5": {"name": "Bg"}, "6": {"name": "Fg"}}})
        );
        assert!(t
            .apply(&Inbound::Json(&json!({"type": "sources_update"})))
            .is_none());
    }

    #[test]
    fn osc_type_tags_and_conversions() {
        let conversions = super::super::template::conversions(Some(&json!({
            "fader": {"points": [[0.0, -90.0], [0.0625, -60.0], [0.25, -30.0], [0.5, -10.0], [1.0, 10.0]]},
        })))
        .unwrap();
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"address": "^/seq/(\\d+)$", "arg_types": "sis",
                 "state": {"seq.{1}.cue": "{arg2}"}},
                {"address": "^/seq/(\\d+)$", "arg_types": "sif",
                 "state": {"seq.{1}.fader": "{arg2}"}},
                {"address": "^/ch/(\\d+)/fader$", "state": {"ch.{1}.db": "{arg0:from.fader:.1f}", "ch.{1}.raw_db": "{arg0:from.fader}"}},
            ]})),
            &state(json!({
                "seq.*.cue": {"type": "string", "description": "x"},
                "seq.*.fader": {"type": "float", "description": "x"},
                "ch.*.db": {"type": "float", "description": "x"},
                "ch.*.raw_db": {"type": "float", "description": "x"},
            })),
            conversions,
        )
        .unwrap();
        let osc = |types, args: &[Value]| {
            t.apply(&Inbound::Osc {
                address: "/seq/1",
                types,
                args,
            })
        };
        assert_eq!(
            osc("sis", &[json!("Flash"), json!(1), json!("Strobe 1 Cue 1")]).unwrap(),
            json!({"seq": {"1": {"cue": "Strobe 1 Cue 1"}}})
        );
        assert_eq!(
            osc("sif", &[json!("Master"), json!(3), json!(75.0)]).unwrap(),
            json!({"seq": {"1": {"fader": 75.0}}})
        );
        assert!(osc("sii", &[json!("Master"), json!(3), json!(5)]).is_none());
        let fader = |x: f64| {
            t.apply(&Inbound::Osc {
                address: "/ch/3/fader",
                types: "f",
                args: &[json!(x)],
            })
        };
        assert_eq!(
            fader(0.375).unwrap(),
            json!({"ch": {"3": {"db": -20.0, "raw_db": -20.0}}})
        );
        // Outside the conversion's points: nothing is assigned.
        assert!(fader(1.5).is_none());
    }

    #[test]
    fn undeclared_paths_are_refused_at_load() {
        let e = Telemetry::parse(
            Some(&json!({"updates": [{"match": "^X (\\d+)$", "state": {"nope.{1}": "{1}"}}]})),
            &state(json!({})),
            Conversions::new(),
        )
        .unwrap_err();
        assert!(e.contains("not declared"));
    }

    #[test]
    fn values_that_do_not_convert_are_not_assigned() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [{"match": "^SPEED (.*)$", "state": {"speed": "{1}"}}]})),
            &state(json!({"speed": {"type": "int", "description": "x"}})),
            Conversions::new(),
        )
        .unwrap();
        assert!(t.apply(&Inbound::Text("SPEED fast")).is_none());
    }

    #[test]
    fn a_json_null_is_no_value() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [{"json_match": {"$.type": "^layers$"},
                "json": {"name": "$.name", "variant": "$.live"},
                "state": {"layer.name": "{name}", "layer.live_variant": "{variant}"}}]})),
            &state(json!({
                "layer.name": {"type": "string", "description": "x"},
                "layer.live_variant": {"type": "string", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let layer = json!({"type": "layers", "name": "Lower Third", "live": null});
        assert_eq!(
            t.apply(&Inbound::Json(&layer)).unwrap(),
            json!({"layer": {"name": "Lower Third"}})
        );
    }

    #[test]
    fn a_rule_can_delete_a_value_or_a_subtree() {
        let state_decl = state(json!({
            "layers.*.name": {"type": "string", "description": "x"},
            "layers.*.live": {"type": "bool", "description": "x"},
            "sources.*.name": {"type": "string", "description": "x"},
            "clip": {"type": "string", "description": "x"},
        }));
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"json_match": {"$.event": "^removed$", "$.type": "^(layers|sources)$", "$.id": "^(.+)$"},
                 "state": {"{1}.{2}": {"delete": true}}},
                {"match": "^CLEAR$", "state": {"clip": {"delete": true}}},
                {"json_match": {"$.event": "^renamed$", "$.id": "^(.+)$"},
                 "json": {"name": "$.name"},
                 "state": {"layers.{1}.name": "{name}", "layers.{1}.live": {"delete": true}}},
            ]})),
            &state_decl,
            Conversions::new(),
        )
        .unwrap();
        let removed = json!({"event": "removed", "type": "layers", "id": "A1"});
        assert_eq!(
            t.apply(&Inbound::Json(&removed)),
            Some(json!({"layers": {"A1": null}}))
        );
        assert_eq!(
            t.apply(&Inbound::Text("CLEAR")),
            Some(json!({"clip": null}))
        );
        let renamed = json!({"event": "renamed", "id": "A2", "name": "Lower third"});
        assert_eq!(
            t.apply(&Inbound::Json(&renamed)),
            Some(json!({"layers": {"A2": {"name": "Lower third", "live": null}}}))
        );
        // Applied as a merge patch, the deletion removes the whole subtree.
        let mut s = json!({"layers": {"A1": {"name": "Bg", "live": true}, "A2": {"name": "x", "live": false}}});
        crate::session::merge_patch(&mut s, &t.apply(&Inbound::Json(&removed)).unwrap());
        crate::session::merge_patch(&mut s, &t.apply(&Inbound::Json(&renamed)).unwrap());
        assert_eq!(s, json!({"layers": {"A2": {"name": "Lower third"}}}));
        // A path that does not lead to declared state is refused at load.
        let e = Telemetry::parse(
            Some(&json!({"updates": [{"match": "^X$", "state": {"nope.{1}": {"delete": true}}}]})),
            &state_decl,
            Conversions::new(),
        )
        .unwrap_err();
        assert!(e.contains("not declared"), "{e}");
        // An id that would reach elsewhere ("a.b") deletes nothing.
        let dotted = json!({"event": "removed", "type": "layers", "id": "A1.name"});
        assert_eq!(t.apply(&Inbound::Json(&dotted)), None);
    }

    #[test]
    fn a_line_reply_is_read_with_the_message_it_answers() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"match": "^([01])$", "request_match": "^([A-Z0-9]+)\\.Mute([A-H]) \\?$",
                 "state": {"amps.{2}.mute.{3}": {"value": "{1}", "map": {"0": false, "1": true}}}},
                {"match": "^([01])$", "request_match": "^([A-Z0-9]+)\\.Power \\?$",
                 "state": {"amps.{2}.power": {"value": "{1}", "map": {"0": false, "1": true}}},
                 "then_send": ["{2}.Status ?"]},
            ]})),
            &state(json!({
                "amps.*.mute.*": {"type": "bool", "description": "x"},
                "amps.*.power": {"type": "bool", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let answer = |text, request| {
            let mut triggers = Vec::new();
            let patch = t.apply_into(&Inbound::Answer { text, request }, &mut triggers);
            (patch, triggers)
        };
        let (patch, triggers) = answer("1", "AMP2.MuteC ?");
        assert_eq!(
            patch,
            Some(json!({"amps": {"AMP2": {"mute": {"C": true}}}}))
        );
        assert!(triggers.is_empty());
        let (patch, triggers) = answer("0", "AMP2.Power ?");
        assert_eq!(patch, Some(json!({"amps": {"AMP2": {"power": false}}})));
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].params["2"], json!("AMP2"));
        // Without its request, a bare value is nothing.
        assert_eq!(t.apply(&Inbound::Text("1")), None);
        assert_eq!(answer("1", "Subnet.Status ?").0, None);
    }

    #[test]
    fn response_headers_and_array_elements() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"path": "^/clip$", "headers": {"etag": "ETag"}, "state": {"clip.etag": "{etag}"}},
                {"path": "^/summary$", "json": {"name": "$.streams[0].name"},
                 "headers": {"rate": "X-Rate-Remaining"},
                 "state": {"stream.name": "{name}", "rate": "{rate}"}},
            ]})),
            &state(json!({
                "clip.etag": {"type": "string", "description": "x"},
                "stream.name": {"type": "string", "description": "x"},
                "rate": {"type": "int", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let headers = vec![
            ("etag".to_string(), "\"abc\"".to_string()),
            ("x-rate-remaining".to_string(), "42".to_string()),
        ];
        // A headers-only rule reads no body, so the body need not be JSON.
        assert_eq!(
            t.apply(&Inbound::Http {
                path: "/clip",
                headers: &headers,
                body: b"\xff\xd8 not json",
                request: None
            }),
            Some(json!({"clip": {"etag": "\"abc\""}}))
        );
        assert_eq!(
            t.apply(&Inbound::Http {
                path: "/summary",
                headers: &headers,
                body: br#"{"streams":[{"name":"CAM (1)"},{"name":"CAM (2)"}]}"#,
                request: None
            }),
            Some(json!({"stream": {"name": "CAM (1)"}, "rate": 42}))
        );
    }

    #[test]
    fn replies_sharing_a_path_are_told_apart_by_their_request() {
        // JSON-RPC: every request goes to one path, and the reply names
        // neither the method nor what it answers.
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {
                    "path": "^/$",
                    "request_match": {"$.method": "^Pixera\\.Timelines\\.Timeline\\.getCurrentTime$",
                                      "$.params.handle": "^(\\d+)$"},
                    "json_match": {"$.result": "^\\d+$"},
                    "json": {"frame": "$.result"},
                    "state": {"timelines.{1}.frame": "{frame}"},
                },
                {
                    "path": "^/$",
                    "request_match": {"$.method": "^Pixera\\.Utility\\.getApiRevision$"},
                    "json": {"revision": "$.result"},
                    "state": {"device.api_revision": "{revision}"},
                },
            ]})),
            &state(json!({
                "timelines.*.frame": {"type": "int", "description": "x"},
                "device.api_revision": {"type": "int", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let time = json!({"jsonrpc": "2.0", "id": 4,
            "method": "Pixera.Timelines.Timeline.getCurrentTime", "params": {"handle": 7}});
        let revision =
            json!({"jsonrpc": "2.0", "id": 5, "method": "Pixera.Utility.getApiRevision"});
        let reply = br#"{"jsonrpc":"2.0","id":4,"result":1250}"#;
        assert_eq!(
            t.apply(&Inbound::Http {
                path: "/",
                headers: &[],
                body: reply,
                request: Some(&time)
            }),
            Some(json!({"timelines": {"7": {"frame": 1250}}}))
        );
        assert_eq!(
            t.apply(&Inbound::Http {
                path: "/",
                headers: &[],
                body: reply,
                request: Some(&revision)
            }),
            Some(json!({"device": {"api_revision": 1250}}))
        );
        // Without the request, neither rule can claim the reply.
        assert!(t
            .apply(&Inbound::Http {
                path: "/",
                headers: &[],
                body: reply,
                request: None
            })
            .is_none());
        // json_match on the reply: an error reply is not a time.
        let error = br#"{"jsonrpc":"2.0","id":4,"error":{"code":-32601}}"#;
        assert!(t
            .apply(&Inbound::Http {
                path: "/",
                headers: &[],
                body: error,
                request: Some(&time)
            })
            .is_none());
    }
}
