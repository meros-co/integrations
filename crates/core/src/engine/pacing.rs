//! `transport.pacing` (SPEC.md §2, Pacing): the least time a device needs
//! between the messages it is sent. A message that would go too soon is
//! held back, and everything behind it waits with it, so nothing is
//! reordered; the wait is a timer, so nothing else is held up.

use regex::Regex;
use serde_json::{Map, Value};

use crate::module::Millis;

/// The longest gap a spec may ask for: an hour.
const MAX_GAP: u64 = 3_600_000;

/// When a message went, and when its exchange ended: its reply, or its
/// sending when it awaited none.
#[derive(Debug, Clone, Copy)]
struct Marks {
    sent: Millis,
    done: Millis,
}

/// A group of messages told apart by their text (`match`), with the gap
/// between two of them and the wait after one of them before any message.
#[derive(Debug)]
struct Group {
    matches: Regex,
    gap: Millis,
    after: Millis,
    last: Option<Marks>,
}

/// The pacing of one device's messages, resolved for its model.
#[derive(Debug, Default)]
pub(super) struct Pacer {
    /// Between any two messages.
    gap: Millis,
    /// Gaps are counted from the reply to the message before (its sending
    /// when it awaited none) rather than from its sending.
    from_reply: bool,
    groups: Vec<Group>,
    /// The last message sent, and the wait its groups ask for after it.
    last: Option<(Marks, Millis)>,
    /// The groups of the message whose reply is awaited.
    open: Option<Vec<usize>>,
}

fn int(entry: &Map<String, Value>, key: &str, at: &str) -> Result<Option<Millis>, String> {
    match entry.get(key) {
        None => Ok(None),
        Some(v) => match v.as_u64() {
            Some(ms) if ms <= MAX_GAP => Ok(Some(ms)),
            _ => Err(format!("{at}.{key} must be 0 to {MAX_GAP} ms")),
        },
    }
}

/// The `by_model` entry naming `model`, if any; each is checked.
fn for_model<'a>(
    entry: &'a Map<String, Value>,
    model: &str,
    at: &str,
    keys: &[&str],
) -> Result<Option<&'a Map<String, Value>>, String> {
    let Some(list) = entry.get("by_model") else {
        return Ok(None);
    };
    let list = list
        .as_array()
        .filter(|l| !l.is_empty())
        .ok_or(format!("{at}.by_model must be a list"))?;
    let mut found = None;
    for item in list {
        let item = item
            .as_object()
            .ok_or(format!("{at}.by_model: each entry is a mapping"))?;
        for key in item.keys() {
            if key != "models" && !keys.contains(&key.as_str()) {
                return Err(format!("{at}.by_model.{key} is not a pacing key"));
            }
        }
        let models = item
            .get("models")
            .and_then(Value::as_array)
            .filter(|m| !m.is_empty())
            .ok_or(format!("{at}.by_model: each entry lists models"))?;
        if found.is_none() && models.iter().any(|m| m.as_str() == Some(model)) {
            found = Some(item);
        }
    }
    Ok(found)
}

impl Pacer {
    /// The pacing a transport's `pacing` block gives `model`; none without
    /// one.
    pub(super) fn parse(v: Option<&Value>, model: &str) -> Result<Pacer, String> {
        let Some(v) = v else {
            return Ok(Pacer::default());
        };
        let block = v.as_object().ok_or("transport.pacing must be a mapping")?;
        let at = "transport.pacing";
        for key in block.keys() {
            if !["min_interval_ms", "from", "by_model", "groups"].contains(&key.as_str()) {
                return Err(format!("{at}.{key} is not a pacing key"));
            }
        }
        let from_reply = match block.get("from").map(|f| f.as_str()) {
            None | Some(Some("send")) => false,
            Some(Some("reply")) => true,
            _ => return Err(format!("{at}.from must be send or reply")),
        };
        let mut gap = int(block, "min_interval_ms", at)?.unwrap_or(0);
        if let Some(entry) = for_model(block, model, at, &["min_interval_ms"])? {
            gap = int(entry, "min_interval_ms", at)?.unwrap_or(gap);
        }
        let mut groups = Vec::new();
        if let Some(list) = block.get("groups") {
            let list = list
                .as_object()
                .filter(|l| !l.is_empty())
                .ok_or(format!("{at}.groups must map names to groups"))?;
            for (name, group) in list {
                let at = format!("{at}.groups.{name}");
                let group = group.as_object().ok_or(format!("{at} must be a mapping"))?;
                for key in group.keys() {
                    if !["match", "min_interval_ms", "after_ms", "by_model"].contains(&key.as_str())
                    {
                        return Err(format!("{at}.{key} is not a pacing group key"));
                    }
                }
                let pattern = group
                    .get("match")
                    .and_then(Value::as_str)
                    .ok_or(format!("{at}.match must be a regex"))?;
                let matches = Regex::new(pattern).map_err(|e| format!("{at}.match: {e}"))?;
                let mut gap = int(group, "min_interval_ms", &at)?;
                let mut after = int(group, "after_ms", &at)?;
                if gap.is_none() && after.is_none() {
                    return Err(format!("{at} needs min_interval_ms or after_ms"));
                }
                if let Some(entry) = for_model(group, model, &at, &["min_interval_ms", "after_ms"])?
                {
                    gap = int(entry, "min_interval_ms", &at)?.or(gap);
                    after = int(entry, "after_ms", &at)?.or(after);
                }
                groups.push(Group {
                    matches,
                    gap: gap.unwrap_or(0),
                    after: after.unwrap_or(0),
                    last: None,
                });
            }
        }
        Ok(Pacer {
            gap,
            from_reply,
            groups,
            last: None,
            open: None,
        })
    }

    /// No pacing at all: messages go as soon as they may.
    pub(super) fn is_off(&self) -> bool {
        self.gap == 0 && self.groups.iter().all(|g| g.gap == 0 && g.after == 0)
    }

    /// The groups a message is in, by its text: a line message's payload,
    /// an HTTP request's path and query, a websocket message, an OSC
    /// address.
    pub(super) fn groups_of(&self, text: &str) -> Vec<usize> {
        self.groups
            .iter()
            .enumerate()
            .filter(|(_, g)| g.matches.is_match(text))
            .map(|(i, _)| i)
            .collect()
    }

    fn from(&self, marks: Marks) -> Millis {
        if self.from_reply {
            marks.done
        } else {
            marks.sent
        }
    }

    /// How long a message in `groups` must still wait at `now`; 0 when it
    /// may go.
    pub(super) fn wait(&self, groups: &[usize], now: Millis) -> Millis {
        let mut due = 0;
        if let Some((marks, after)) = self.last {
            due = self.from(marks) + self.gap.max(after);
        }
        for &i in groups {
            let group = &self.groups[i];
            if let Some(marks) = group.last {
                due = due.max(self.from(marks) + group.gap);
            }
        }
        due.saturating_sub(now)
    }

    /// A message in `groups` went at `now`; `awaits` when a reply to it is
    /// awaited.
    pub(super) fn sent(&mut self, groups: &[usize], now: Millis, awaits: bool) {
        let marks = Marks {
            sent: now,
            done: now,
        };
        let after = groups.iter().map(|&i| self.groups[i].after).max();
        self.last = Some((marks, after.unwrap_or(0)));
        for &i in groups {
            self.groups[i].last = Some(marks);
        }
        self.open = awaits.then(|| groups.to_vec());
    }

    /// The reply to the message sent last arrived, or will not: its exchange
    /// ended at `now`.
    pub(super) fn answered(&mut self, now: Millis) {
        let Some(groups) = self.open.take() else {
            return;
        };
        if let Some((marks, _)) = &mut self.last {
            marks.done = marks.done.max(now);
        }
        for i in groups {
            if let Some(marks) = &mut self.groups[i].last {
                marks.done = marks.done.max(now);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_gap_counts_from_the_send_or_the_reply() {
        let v = json!({"min_interval_ms": 100});
        let mut p = Pacer::parse(Some(&v), "m").unwrap();
        assert_eq!(p.wait(&[], 0), 0);
        p.sent(&[], 1000, true);
        p.answered(1060);
        assert_eq!(p.wait(&[], 1060), 40);
        assert_eq!(p.wait(&[], 1100), 0);

        let v = json!({"min_interval_ms": 100, "from": "reply"});
        let mut p = Pacer::parse(Some(&v), "m").unwrap();
        p.sent(&[], 1000, true);
        p.answered(1060);
        assert_eq!(p.wait(&[], 1100), 60);
        // Nothing awaited: the exchange ended when it was sent.
        p.sent(&[], 1200, false);
        p.answered(1300);
        assert_eq!(p.wait(&[], 1250), 50);
    }

    #[test]
    fn groups_keep_their_own_gap_and_their_wait_after() {
        let v = json!({
            "groups": {
                "pan_tilt": {"match": "^#", "min_interval_ms": 40,
                             "by_model": [{"models": ["fast"], "min_interval_ms": 16}]},
                "power": {"match": "^POWR", "after_ms": 60000}
            }
        });
        let mut p = Pacer::parse(Some(&v), "m").unwrap();
        let pt = p.groups_of("#PTS5050");
        assert_eq!(pt, vec![0]);
        assert!(p.groups_of("QID").is_empty());
        p.sent(&pt, 0, false);
        // Another kind of message goes at once; another pan-tilt one waits.
        assert_eq!(p.wait(&[], 1), 0);
        p.sent(&[], 1, false);
        assert_eq!(p.wait(&pt, 2), 38);
        let power = p.groups_of("POWR1");
        p.sent(&power, 100, false);
        assert_eq!(p.wait(&[], 100), 60000);
        assert_eq!(p.wait(&pt, 60100), 0);

        let fast = Pacer::parse(Some(&v), "fast").unwrap();
        assert_eq!(fast.groups[0].gap, 16);
    }

    #[test]
    fn a_model_may_have_its_own_gap() {
        let v = json!({"min_interval_ms": 0,
                       "by_model": [{"models": ["old"], "min_interval_ms": 600}]});
        assert!(Pacer::parse(Some(&v), "new").unwrap().is_off());
        assert_eq!(Pacer::parse(Some(&v), "old").unwrap().gap, 600);
    }

    #[test]
    fn a_malformed_block_is_refused() {
        for (v, what) in [
            (json!({"min_interval": 5}), "not a pacing key"),
            (json!({"from": "now"}), "send or reply"),
            (json!({"min_interval_ms": -1}), "0 to"),
            (json!({"groups": {"g": {"min_interval_ms": 5}}}), "match"),
            (json!({"groups": {"g": {"match": "x"}}}), "needs"),
            (
                json!({"by_model": [{"min_interval_ms": 5}]}),
                "lists models",
            ),
        ] {
            let e = Pacer::parse(Some(&v), "m").unwrap_err();
            assert!(e.contains(what), "{e}");
        }
    }
}
