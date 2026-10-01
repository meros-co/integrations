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
            // A body ends where its text ends: a trailing line break is not part of
            // the reply ("p1\r\n" from a Panasonic camera).
            Reply::Http { body, .. } => String::from_utf8_lossy(body)
                .trim_end_matches(['\r', '\n'])
                .to_string(),
            Reply::Osc { args } => Value::Array(args.clone()).to_string(),
        }
    }
}

/// Evaluate `expect` against a reply and produce the command's result.
/// `headed` is true for `headed-block` replies, whose first line is a header
/// rather than part of the body.
pub(crate) fn evaluate(
    expect: &Map<String, Value>,
    returns: &str,
    codes: &BTreeMap<String, String>,
    reply: &Reply,
    headed: bool,
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

    // An OSC reply's JSON is the string argument at `arg` (QLab); any other
    // reply's is its body.
    let arg = expect.get("arg").and_then(Value::as_u64).unwrap_or(0) as usize;
    let wants_json = expect.contains_key("json_path") || expect.contains_key("json_equals");
    let json: Option<Value> = if wants_json {
        let source = match reply {
            Reply::Osc { args } => args.get(arg).and_then(Value::as_str).map(str::to_string),
            _ => Some(text.clone()),
        };
        Some(
            source
                .and_then(|s| serde_json::from_str(&s).ok())
                .ok_or_else(|| CommandError::DeviceError {
                    code: None,
                    message: "reply is not JSON".into(),
                })?,
        )
    } else {
        None
    };
    if let (Some(doc), Some(wanted)) = (&json, expect.get("json_equals").and_then(Value::as_object))
    {
        for (path, want) in wanted {
            let got = json_path(doc, path);
            if got != Some(want) {
                let got = got.map_or("nothing".to_string(), Value::to_string);
                return Err(CommandError::DeviceError {
                    code: None,
                    message: format!("reply has {path} = {got}, expected {want}"),
                });
            }
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
    if let (Some(doc), Some(path)) = (&json, expect.get("json_path").and_then(Value::as_str)) {
        match json_path(doc, path) {
            Some(v) => value = Some(v.clone()),
            None => return fail(format!("reply has no {path}")),
        }
    } else if let Reply::Osc { args } = reply {
        value = args.get(arg).cloned();
    }

    match returns {
        "none" => Ok(Outcome::Unverified),
        "ack" => Ok(Outcome::Ack),
        "value" => Ok(Outcome::Value {
            value: value.unwrap_or(Value::Null),
        }),
        "fields" => Ok(Outcome::Value {
            value: fields(&body(&text, headed)),
        }),
        "text" => Ok(Outcome::Value {
            value: json!(body(&text, headed)),
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

/// The reply body (SPEC.md §5): for a headed block, the lines after the
/// first, so a single-line reply has no body; otherwise the whole reply.
fn body(text: &str, headed: bool) -> String {
    if !headed {
        return text.to_string();
    }
    text.split_once('\n')
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_default()
}

/// `key: value` lines of the body, split at the first ": ".
fn fields(body: &str) -> Value {
    let mut map = Map::new();
    for line in body.lines() {
        if let Some((k, v)) = line.split_once(": ") {
            map.insert(k.trim().to_string(), json!(v.trim()));
        } else if let Some(k) = line.strip_suffix(':') {
            map.insert(k.trim().to_string(), json!(""));
        }
    }
    Value::Object(map)
}

/// `$` or `$.a.b`: the subset of JSONPath the specs use.
pub(crate) fn json_path<'a>(json: &'a Value, path: &str) -> Option<&'a Value> {
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
            evaluate(&e, "ack", &codes(), &Reply::Text("200 ok".into()), false),
            Ok(Outcome::Ack)
        );
        assert_eq!(
            evaluate(
                &e,
                "ack",
                &codes(),
                &Reply::Text("111 remote control disabled".into()),
                false
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
            evaluate(&e, "fields", &codes(), &r, true),
            Ok(Outcome::Value {
                value: json!({"status": "play", "speed": "100"})
            })
        );
    }

    #[test]
    fn only_a_headed_block_drops_its_first_line() {
        let r = Reply::Http {
            status: 200,
            body: b"focus_mode=\"1\"\nfocus_zone=\"1\"\n".to_vec(),
        };
        assert_eq!(
            evaluate(&Map::new(), "text", &codes(), &r, false),
            Ok(Outcome::Value {
                value: json!("focus_mode=\"1\"\nfocus_zone=\"1\"")
            })
        );
        let r = Reply::Text("error: none\nstatus: done".into());
        assert_eq!(
            evaluate(&Map::new(), "fields", &codes(), &r, false),
            Ok(Outcome::Value {
                value: json!({"error": "none", "status": "done"})
            })
        );
    }

    #[test]
    fn json_inside_an_osc_argument() {
        let e = expect(
            json!({"address": "/reply/cue/1/name", "json_path": "$.data",
                              "json_equals": {"$.status": "ok"}}),
        );
        let reply = |status: &str| Reply::Osc {
            args: vec![json!(format!(
                r#"{{"workspace_id":"w","address":"/cue/1/name","status":"{status}","data":"Intro"}}"#
            ))],
        };
        assert_eq!(
            evaluate(&e, "value", &codes(), &reply("ok"), false),
            Ok(Outcome::Value {
                value: json!("Intro")
            })
        );
        assert_eq!(
            evaluate(&e, "value", &codes(), &reply("denied"), false),
            Err(CommandError::DeviceError {
                code: None,
                message: r#"reply has $.status = "denied", expected "ok""#.into()
            })
        );
        // Without json keys, an OSC reply still returns its argument.
        let plain = expect(json!({"address": "/x", "arg": 1}));
        let r = Reply::Osc {
            args: vec![json!("a"), json!(2)],
        };
        assert_eq!(
            evaluate(&plain, "value", &codes(), &r, false),
            Ok(Outcome::Value { value: json!(2) })
        );
    }

    #[test]
    fn matches_returns_group_one() {
        let e = expect(json!({"matches": "BATT_BARS (\\d+)"}));
        let r = Reply::Text("< REP 1 BATT_BARS 004 >".into());
        assert_eq!(
            evaluate(&e, "value", &codes(), &r, false),
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
            &Reply::Text("~01@ROUTE 1,2,3 ERR 003".into()),
            false
        )
        .is_err());
        let e = expect(json!({"status": 200, "json_path": "$.value_name"}));
        let r = Reply::Http {
            status: 200,
            body: br#"{"value":"1","value_name":"Playing"}"#.to_vec(),
        };
        assert_eq!(
            evaluate(&e, "value", &codes(), &r, false),
            Ok(Outcome::Value {
                value: json!("Playing")
            })
        );
    }
}
