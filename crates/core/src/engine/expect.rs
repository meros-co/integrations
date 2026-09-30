//! Reply evaluation: SPEC.md §5, "Responses".

use std::collections::BTreeMap;

use regex::Regex;
use serde_json::{json, Map, Value};

use crate::module::{CommandError, CommandResult, Outcome};

/// A reply, whatever transport carried it.
#[derive(Debug, Clone)]
pub(crate) enum Reply {
    Text(String),
    Http { status: u16, body: Vec<u8> },
    Osc { args: Vec<Value> },
}

impl Reply {
    fn text(&self) -> String {
        match self {
            Reply::Text(t) => t.clone(),
            Reply::Http { body, .. } => String::from_utf8_lossy(body).into_owned(),
            Reply::Osc { args } => Value::Array(args.clone()).to_string(),
        }
    }
}

/// Evaluate `expect` against a reply and produce the command's result.
pub(crate) fn evaluate(
    expect: &Map<String, Value>,
    returns: &str,
    codes: &BTreeMap<String, String>,
    reply: &Reply,
) -> CommandResult {
    let text = reply.text();
    let leading_code = text
        .split_whitespace()
        .next()
        .filter(|t| t.len() == 3 && t.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|t| t.parse::<i64>().ok());

    let fail = |message: String| -> CommandResult {
        let code = leading_code.map(|c| c.to_string());
        let message = code
            .as_ref()
            .and_then(|c| codes.get(c))
            .cloned()
            .unwrap_or(message);
        Err(CommandError::DeviceError { code, message })
    };

    if let Some(range) = expect.get("code_range").and_then(Value::as_array) {
        let (lo, hi) = (
            range[0].as_i64().unwrap_or(0),
            range[1].as_i64().unwrap_or(0),
        );
        match leading_code {
            Some(c) if (lo..=hi).contains(&c) => {}
            _ => return fail(format!("unexpected reply: {}", first_line(&text))),
        }
    }
    if let Some(want) = expect.get("status").and_then(Value::as_u64) {
        let status = match reply {
            Reply::Http { status, .. } => *status as u64,
            _ => 0,
        };
        if status != want {
            return Err(CommandError::DeviceError {
                code: Some(status.to_string()),
                message: format!("HTTP {status}, expected {want}"),
            });
        }
    }
    if let Some(needle) = expect.get("contains").and_then(Value::as_str) {
        if !text.contains(needle) {
            return fail(format!("unexpected reply: {}", first_line(&text)));
        }
    }
    if let Some(needle) = expect.get("not_contains").and_then(Value::as_str) {
        if text.contains(needle) {
            return fail(format!("device reported an error: {}", first_line(&text)));
        }
    }

    let mut value: Option<Value> = None;
    if let Some(pattern) = expect.get("matches").and_then(Value::as_str) {
        let re = Regex::new(pattern).map_err(|e| CommandError::DeviceError {
            code: None,
            message: format!("spec pattern does not compile: {e}"),
        })?;
        match re.captures(&text) {
            Some(caps) => value = caps.get(1).map(|m| json!(m.as_str())),
            None => return fail(format!("unexpected reply: {}", first_line(&text))),
        }
    }
    if let Some(path) = expect.get("json_path").and_then(Value::as_str) {
        let json: Value = serde_json::from_str(&text).map_err(|_| CommandError::DeviceError {
            code: None,
            message: "reply is not JSON".into(),
        })?;
        match json_path(&json, path) {
            Some(v) => value = Some(v.clone()),
            None => return fail(format!("reply has no {path}")),
        }
    }
    if let Reply::Osc { args } = reply {
        let index = expect.get("arg").and_then(Value::as_u64).unwrap_or(0) as usize;
        value = args.get(index).cloned();
    }

    match returns {
        "none" => Ok(Outcome::Unverified),
        "ack" => Ok(Outcome::Ack),
        "value" => Ok(Outcome::Value {
            value: value.unwrap_or(Value::Null),
        }),
        "fields" => Ok(Outcome::Value {
            value: fields(&text),
        }),
        "text" => Ok(Outcome::Value {
            value: json!(body(&text)),
        }),
        other => Err(CommandError::DeviceError {
            code: None,
            message: format!("unknown returns '{other}' in spec"),
        }),
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or("")
}

/// The body of a headed block: the lines after the first. A single-line reply
/// has no body.
fn body(text: &str) -> String {
    text.split_once('\n')
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_default()
}

/// `key: value` lines of the body, split at the first ": ".
fn fields(text: &str) -> Value {
    let mut map = Map::new();
    for line in body(text).lines() {
        if let Some((k, v)) = line.split_once(": ") {
            map.insert(k.trim().to_string(), json!(v.trim()));
        } else if let Some(k) = line.strip_suffix(':') {
            map.insert(k.trim().to_string(), json!(""));
        }
    }
    Value::Object(map)
}

/// `$` or `$.a.b`: the subset of JSONPath the specs use.
fn json_path<'a>(json: &'a Value, path: &str) -> Option<&'a Value> {
    let rest = path.strip_prefix('$')?;
    if rest.is_empty() {
        return Some(json);
    }
    rest.strip_prefix('.')?
        .split('.')
        .try_fold(json, |node, key| node.get(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn codes() -> BTreeMap<String, String> {
        [("111".to_string(), "remote control disabled".to_string())].into()
    }

    #[test]
    fn code_ranges_and_code_messages() {
        let e = expect(json!({"code_range": [200, 299]}));
        assert_eq!(
            evaluate(&e, "ack", &codes(), &Reply::Text("200 ok".into())),
            Ok(Outcome::Ack)
        );
        assert_eq!(
            evaluate(
                &e,
                "ack",
                &codes(),
                &Reply::Text("111 remote control disabled".into())
            ),
            Err(CommandError::DeviceError {
                code: Some("111".into()),
                message: "remote control disabled".into()
            })
        );
    }

    #[test]
    fn fields_from_a_headed_block() {
        let e = expect(json!({"code_range": [200, 299]}));
        let r = Reply::Text("208 transport info:\nstatus: play\nspeed: 100".into());
        assert_eq!(
            evaluate(&e, "fields", &codes(), &r),
            Ok(Outcome::Value {
                value: json!({"status": "play", "speed": "100"})
            })
        );
    }

    #[test]
    fn matches_returns_group_one() {
        let e = expect(json!({"matches": "BATT_BARS (\\d+)"}));
        let r = Reply::Text("< REP 1 BATT_BARS 004 >".into());
        assert_eq!(
            evaluate(&e, "value", &codes(), &r),
            Ok(Outcome::Value {
                value: json!("004")
            })
        );
    }

    #[test]
    fn not_contains_and_json_path() {
        let e = expect(json!({"not_contains": "ERR"}));
        assert!(evaluate(
            &e,
            "ack",
            &codes(),
            &Reply::Text("~01@ROUTE 1,2,3 ERR 003".into())
        )
        .is_err());
        let e = expect(json!({"status": 200, "json_path": "$.value_name"}));
        let r = Reply::Http {
            status: 200,
            body: br#"{"value":"1","value_name":"Playing"}"#.to_vec(),
        };
        assert_eq!(
            evaluate(&e, "value", &codes(), &r),
            Ok(Outcome::Value {
                value: json!("Playing")
            })
        );
    }
}
