//! `transport.acknowledge` (SPEC.md §2, Acknowledgements): what the device
//! sends by itself and wants answered, and the answer the host sends.
//!
//! A Denon Professional or TASCAM player sends each status it notifies by
//! itself until the host acknowledges it with the ACK byte; a RED camera
//! keeps its RCP2 connection only while its `rcp_session` messages are
//! echoed back. The answer belongs to the connection, not to state, so it
//! goes whether or not the device is monitored.

use std::collections::{BTreeMap, VecDeque};

use regex::Regex;
use serde_json::Value;

use super::expect;
use super::Millis;

/// One kind of message the device wants answered.
#[derive(Debug)]
pub(super) struct Rule {
    /// `match`: a regex over the message's text, its captures `{1}`, `{2}`...
    text: Option<Regex>,
    /// `json_match`: JSON paths to regexes the message's values must match,
    /// their captures numbered on in order.
    select: Vec<(String, Regex)>,
    /// `json`: capture names to JSON paths.
    json: BTreeMap<String, String>,
    /// The answer: a template over the captures and the settings.
    pub(super) send: String,
    /// `framed: false`: the answer goes as it is, without the transport's
    /// framing (Denon's ACK is the byte 06H alone, no CR after it).
    pub(super) framed: bool,
    /// `answers`: messages the host sends that the device answers with a
    /// message of this kind (a status request, answered ACK and then the
    /// status): the next such message after one is that answer, not a
    /// notification, and is not acknowledged.
    answers: Option<Regex>,
}

/// The rules, and the answers the device owes to messages sent.
#[derive(Debug, Default)]
pub(super) struct Acknowledge {
    rules: Vec<Rule>,
    /// For each message sent that the device answers (`answers`), the time
    /// by which its answer is due; an answer not come by then is not coming.
    pending: VecDeque<Millis>,
}

fn regex(v: &Value, what: &str) -> Result<Regex, String> {
    let s = v
        .as_str()
        .ok_or(format!("transport.acknowledge: {what} must be a regex"))?;
    Regex::new(s).map_err(|e| format!("transport.acknowledge: {what} '{s}': {e}"))
}

impl Acknowledge {
    /// `transport.acknowledge`: one rule or a list of them.
    pub(super) fn parse(v: Option<&Value>) -> Result<Acknowledge, String> {
        let items = match v {
            None => return Ok(Acknowledge::default()),
            Some(Value::Array(items)) => items.clone(),
            Some(one) => vec![one.clone()],
        };
        let mut rules = Vec::new();
        for item in &items {
            let o = item
                .as_object()
                .ok_or("transport.acknowledge: each entry is a mapping")?;
            let text = match o.get("match") {
                Some(v) => Some(regex(v, "match")?),
                None => None,
            };
            let mut select = Vec::new();
            for (path, re) in o
                .get("json_match")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                select.push((path.clone(), regex(re, "json_match")?));
            }
            if text.is_some() == !select.is_empty() {
                return Err(
                    "transport.acknowledge: each entry has match or json_match, not both".into(),
                );
            }
            let mut json = BTreeMap::new();
            for (name, path) in o
                .get("json")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                let path = path.as_str().ok_or(format!(
                    "transport.acknowledge: json '{name}' is a JSON path"
                ))?;
                json.insert(name.clone(), path.to_string());
            }
            let send = o
                .get("send")
                .and_then(Value::as_str)
                .ok_or("transport.acknowledge: each entry has a send template")?
                .to_string();
            rules.push(Rule {
                text,
                select,
                json,
                send,
                framed: o.get("framed").and_then(Value::as_bool).unwrap_or(true),
                answers: match o.get("answers") {
                    Some(v) => Some(regex(v, "answers")?),
                    None => None,
                },
            });
        }
        Ok(Acknowledge {
            rules,
            pending: VecDeque::new(),
        })
    }

    pub(super) fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// A message went out: one the device answers with a message of its
    /// own is owed that answer until `due`.
    pub(super) fn sent(&mut self, text: &str, due: Millis) {
        if self
            .rules
            .iter()
            .any(|r| r.answers.as_ref().is_some_and(|re| re.is_match(text)))
        {
            self.pending.push_back(due);
        }
    }

    /// A connection lost or opened afresh owes nothing.
    pub(super) fn reset(&mut self) {
        self.pending.clear();
    }

    /// A message arrived; `error` says it is an error answer (the
    /// transport's `error_match`), after which a request's own answer is not
    /// coming. The rule to answer it with and its captures, when it is to be
    /// answered: every message a rule matches is, except the answer owed to
    /// a message sent (`answers`), whether or not it was taken as a reply.
    pub(super) fn heard(
        &mut self,
        message: &str,
        error: bool,
        now: Millis,
    ) -> Option<(&Rule, Vec<(String, Value)>)> {
        while self.pending.front().is_some_and(|due| *due < now) {
            self.pending.pop_front();
        }
        let doc = || serde_json::from_str::<Value>(message).ok();
        let mut found = None;
        for rule in &self.rules {
            if let Some(captures) = rule.captures(message, &doc) {
                found = Some((rule, captures));
                break;
            }
        }
        let Some((rule, captures)) = found else {
            if error {
                self.pending.pop_front();
            }
            return None;
        };
        // The answer to a message sent: not a notification.
        if self.pending.pop_front().is_some() {
            return None;
        }
        Some((rule, captures))
    }
}

impl Rule {
    /// The rule's captures when the message is one it answers.
    fn captures(
        &self,
        message: &str,
        doc: &dyn Fn() -> Option<Value>,
    ) -> Option<Vec<(String, Value)>> {
        let mut out = Vec::new();
        if let Some(re) = &self.text {
            let c = re.captures(message.trim_end_matches(['\r', '\n']))?;
            for i in 1..c.len() {
                let v = c.get(i).map_or("", |m| m.as_str());
                out.push((i.to_string(), Value::String(v.to_string())));
            }
            return Some(out);
        }
        let doc = doc()?;
        let mut n = 0;
        for (path, re) in &self.select {
            let text = match expect::json_path(&doc, path)? {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let c = re.captures(&text)?;
            for i in 1..c.len() {
                n += 1;
                let v = c.get(i).map_or("", |m| m.as_str());
                out.push((n.to_string(), Value::String(v.to_string())));
            }
        }
        // A `json` name is the value as JSON text, so an echo sends back
        // exactly what the device sent, whatever its type.
        for (name, path) in &self.json {
            if let Some(v) = expect::json_path(&doc, path) {
                out.push((name.clone(), Value::String(v.to_string())));
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn denon() -> Acknowledge {
        Acknowledge::parse(Some(&json!({
            "match": "^@0", "send": "\u{6}", "framed": false, "answers": "^@0\\?",
        })))
        .unwrap()
    }

    #[test]
    fn a_notification_is_answered_and_a_requested_status_is_not() {
        let mut a = denon();
        // Nothing asked: a status is a notification.
        assert!(a.heard("@0STPL", false, 0).is_some());
        // The ACK byte is no status.
        assert!(a.heard("\u{6}", false, 0).is_none());
        // Asked for: the next status is the answer; the one after is a
        // notification again.
        a.sent("@0?ST", 1000);
        assert!(a.heard("\u{6}", false, 10).is_none());
        assert!(a.heard("@0STPP", false, 20).is_none());
        assert!(a.heard("@0Tr0003", false, 30).is_some());
        // A control command is not answered with a status.
        a.sent("@02353", 1100);
        assert!(a.heard("@0STPL", false, 110).is_some());
        // An answer that never came is not waited for past its time.
        a.sent("@0?Tr", 1200);
        assert!(a.heard("@0STPL", false, 1300).is_some());
        // An error answer (NACK) means the request's answer is not coming.
        a.sent("@0?Tr", 2000);
        assert!(a.heard("\u{15}", true, 1500).is_none());
        assert!(a.heard("@0STPL", false, 1510).is_some());
    }

    #[test]
    fn a_json_message_is_answered_from_its_captures() {
        let mut a = Acknowledge::parse(Some(&json!([{
            "json_match": {"$.type": "^rcp_session$", "$.status": "^(open)$"},
            "json": {"data": "$.data"},
            "send": "{\"type\":\"rcp_session\",\"data\":{data}}",
        }])))
        .unwrap();
        let (rule, captures) = a
            .heard(
                r#"{"type":"rcp_session","status":"open","data":7}"#,
                false,
                0,
            )
            .unwrap();
        assert!(rule.framed);
        assert_eq!(
            captures,
            vec![("1".into(), json!("open")), ("data".into(), json!("7"))]
        );
        assert!(a.heard(r#"{"type":"rcp_cur_int"}"#, false, 0).is_none());
        assert!(a.heard("not json", false, 0).is_none());
    }

    #[test]
    fn a_malformed_block_is_refused() {
        assert!(Acknowledge::parse(Some(&json!({"send": "x"}))).is_err());
        assert!(Acknowledge::parse(Some(
            &json!({"match": "a", "json_match": {"$.a": "b"}, "send": "x"})
        ))
        .is_err());
        assert!(Acknowledge::parse(Some(&json!({"match": "("}))).is_err());
        assert!(Acknowledge::parse(Some(&json!({"match": "a"}))).is_err());
    }
}
