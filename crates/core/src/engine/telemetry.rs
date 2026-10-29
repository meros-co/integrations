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

/// `blob_each`: numbers packed back to back in an OSC blob argument.
#[derive(Debug)]
struct BlobEach {
    /// The argument holding the blob.
    arg: usize,
    /// Bytes skipped at the blob's start (a count the device puts first).
    offset: usize,
    /// At most this many numbers; as many as the blob holds when absent.
    count: Option<usize>,
    /// Each number's width in bytes, and how it is read.
    format: BlobFormat,
}

#[derive(Debug, Clone, Copy)]
enum BlobFormat {
    I8,
    U8,
    I16(bool),
    U16(bool),
    I32(bool),
    U32(bool),
    F32(bool),
}

impl BlobFormat {
    /// `i16le`, `f32be`, ...: the type, then the byte order (`le` or `be`)
    /// for anything wider than a byte.
    fn parse(s: &str) -> Option<BlobFormat> {
        Some(match s {
            "i8" => BlobFormat::I8,
            "u8" => BlobFormat::U8,
            "i16le" => BlobFormat::I16(true),
            "i16be" => BlobFormat::I16(false),
            "u16le" => BlobFormat::U16(true),
            "u16be" => BlobFormat::U16(false),
            "i32le" => BlobFormat::I32(true),
            "i32be" => BlobFormat::I32(false),
            "u32le" => BlobFormat::U32(true),
            "u32be" => BlobFormat::U32(false),
            "f32le" => BlobFormat::F32(true),
            "f32be" => BlobFormat::F32(false),
            _ => return None,
        })
    }

    fn width(self) -> usize {
        match self {
            BlobFormat::I8 | BlobFormat::U8 => 1,
            BlobFormat::I16(_) | BlobFormat::U16(_) => 2,
            BlobFormat::I32(_) | BlobFormat::U32(_) | BlobFormat::F32(_) => 4,
        }
    }

    /// The number in `b`, exactly `width()` bytes: an integer as a JSON
    /// integer, a float as a JSON number (none for NaN or infinity).
    fn read(self, b: &[u8]) -> Option<Value> {
        fn two(b: &[u8], le: bool) -> [u8; 2] {
            let a = [b[0], b[1]];
            if le {
                a
            } else {
                [a[1], a[0]]
            }
        }
        fn four(b: &[u8], le: bool) -> [u8; 4] {
            let a = [b[0], b[1], b[2], b[3]];
            if le {
                a
            } else {
                [a[3], a[2], a[1], a[0]]
            }
        }
        Some(match self {
            BlobFormat::I8 => Value::from(b[0] as i8),
            BlobFormat::U8 => Value::from(b[0]),
            BlobFormat::I16(le) => Value::from(i16::from_le_bytes(two(b, le))),
            BlobFormat::U16(le) => Value::from(u16::from_le_bytes(two(b, le))),
            BlobFormat::I32(le) => Value::from(i32::from_le_bytes(four(b, le))),
            BlobFormat::U32(le) => Value::from(u32::from_le_bytes(four(b, le))),
            BlobFormat::F32(le) => {
                let f = f32::from_le_bytes(four(b, le));
                Value::from(serde_json::Number::from_f64(f64::from(f))?)
            }
        })
    }
}

impl BlobEach {
    fn parse(v: &Value) -> Result<BlobEach, String> {
        let m = v
            .as_object()
            .ok_or("telemetry: blob_each is a mapping of arg, offset, count and format")?;
        for k in m.keys() {
            if !["arg", "offset", "count", "format"].contains(&k.as_str()) {
                return Err(format!("telemetry: blob_each.{k} is not a blob_each key"));
            }
        }
        let num = |k: &str| -> Result<Option<usize>, String> {
            match m.get(k) {
                None => Ok(None),
                Some(n) => n
                    .as_u64()
                    .map(|n| Some(n as usize))
                    .ok_or(format!("telemetry: blob_each.{k} is a whole number")),
            }
        };
        let format = m
            .get("format")
            .and_then(Value::as_str)
            .and_then(BlobFormat::parse)
            .ok_or("telemetry: blob_each.format is one of i8, u8, i16le, i16be, u16le, u16be, i32le, i32be, u32le, u32be, f32le, f32be")?;
        let count = num("count")?;
        if count == Some(0) {
            return Err("telemetry: blob_each.count is at least 1".into());
        }
        Ok(BlobEach {
            arg: num("arg")?.unwrap_or(0),
            offset: num("offset")?.unwrap_or(0),
            count,
            format,
        })
    }

    /// The numbers in the blob at `arg` (its bytes as a JSON array), from
    /// `offset`, as many as fit whole, up to `count`. None when that
    /// argument is not a blob.
    fn numbers(&self, args: &[Value]) -> Option<Vec<Value>> {
        let bytes = match args.get(self.arg)? {
            Value::Array(a) => a
                .iter()
                .map(|b| b.as_u64().and_then(|b| u8::try_from(b).ok()))
                .collect::<Option<Vec<u8>>>()?,
            _ => return None,
        };
        let rest = bytes.get(self.offset..).unwrap_or(&[]);
        let width = self.format.width();
        let n = rest.len() / width;
        let n = self.count.map_or(n, |c| c.min(n));
        Some(
            rest.chunks_exact(width)
                .take(n)
                .filter_map(|c| self.format.read(c))
                .collect(),
        )
    }
}

#[derive(Debug)]
enum Matcher {
    /// The whole message matches; captures are `{1}`, `{2}`, ... With
    /// `request` (`request_match`), only a reply whose request's text
    /// matches it too, its captures numbered on after the message's. With
    /// `each` (`match_each`), the rule matches once for each match of that
    /// regex along the rest of the message, after what `re` matched: a
    /// list on one line (Spyder's `0 2 1 Preset%201 4 Wide`), each item's
    /// captures numbered on after the others and `{index}` its place, from 0.
    Message {
        re: Regex,
        request: Option<Regex>,
        each: Option<Regex>,
        /// `match_each_in`: `each` goes along this capture group of `re`
        /// instead of the rest of the message (a list between markers).
        each_in: Option<usize>,
    },
    /// The first line matches `header`; `line` is applied to every other line.
    Lines { header: Regex, line: Regex },
    /// The first line matches `header`; the other lines are `name: value`.
    Fields {
        header: Regex,
        fields: BTreeMap<String, Assign>,
    },
    /// An OSC message whose address matches; arguments are `{arg0}`, `{arg1}`.
    /// With `json`, `json_match` or `json_each`, the string argument at
    /// `json_arg` is parsed as JSON (QLab's replies) and read as a JSON
    /// message is: `select` must match it, and `json` names values, once per
    /// element of `each` when it is set.
    Osc {
        address: Regex,
        json: BTreeMap<String, String>,
        json_arg: usize,
        /// The message's OSC type tags must be exactly these (`sis`), for a
        /// device that sends different shapes on one address (grandMA3).
        arg_types: Option<String>,
        /// `arg_match`: arguments, by index, whose text must match these
        /// (Millumin's `/millumin/info` names what changed in a string);
        /// their captures follow the address's.
        arg_match: Vec<(usize, Regex)>,
        /// `match_each` with `match_each_in`: the rule matches once for each
        /// match of this regex along the text of that capture (an address's
        /// or `arg_match`'s): a list inside a string argument (WING's node
        /// query), each item's captures numbered on and `{index}` its place.
        text_each: Option<(Regex, usize)>,
        /// `blob_each`: the rule matches once for each number packed in a
        /// blob argument (a console's meter block), `{value}` the number and
        /// `{index}` its place.
        blob_each: Option<BlobEach>,
        select: Vec<(String, Regex)>,
        each: Option<String>,
        each_select: Vec<(String, Regex)>,
    },
    /// A JSON message on a websocket. Every `select` JSON path must hold a
    /// value matching its regex; their captures are `{1}`, `{2}`, ... in
    /// order. `json` and `each` work as for an HTTP reply.
    Json {
        select: Vec<(String, Regex)>,
        json: BTreeMap<String, String>,
        each: Option<String>,
        /// `each_match`: selectors each element must match.
        each_select: Vec<(String, Regex)>,
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
        /// `each_match`: selectors on each element (`$^.` for the one it
        /// lies in); an element that does not match is skipped.
        each_select: Vec<(String, Regex)>,
        /// `headers`: capture names to response header names. A rule with
        /// only these reads no body, so the body need not be JSON.
        headers: BTreeMap<String, String>,
    },
    /// The XML reply to an HTTP request whose path matches; every element
    /// named `element` is one match, its attributes the captures.
    HttpXml { path: Regex, element: String },
    /// The text reply to an HTTP request whose path matches, matched by
    /// `match`: the path's captures, then the text's, numbered on after
    /// them. For CGI devices that answer `key=value` lines whose keys other
    /// replies share (Panasonic's `status=1` from both RTMP and SRT).
    HttpText { path: Regex, re: Regex },
    /// `quiet_ms`: no message. The rule's values are assigned once per
    /// connection, when the stream has been silent this long after the
    /// device last sent anything: the end of a status dump the device does
    /// not mark (older Videohub firmware sends no END PRELUDE).
    Quiet { ms: u64 },
}

/// One element `json_each` reaches: the element, its index in its array
/// (or its place among its object's members), the member's name when it is
/// an object's member, and the elements it lies in, outermost first (the
/// document, then each `[*]` or `.*` level's element), each with its own
/// place (none for the document).
struct EachItem<'a> {
    item: &'a Value,
    index: usize,
    key: Option<&'a str>,
    outer: Vec<(&'a Value, Option<Place<'a>>)>,
}

/// Where an element `json_each` went through lies: its index in its array,
/// or its place among its object's members and the member's name.
#[derive(Clone, Copy)]
struct Place<'a> {
    index: usize,
    key: Option<&'a str>,
}

/// An element reached so far, with its place and the elements it lies in.
type Reached<'a> = (
    &'a Value,
    Option<Place<'a>>,
    Vec<(&'a Value, Option<Place<'a>>)>,
);

/// The steps of a `json_each` path: each a JSON path, from the element the
/// step before reached, to an array whose elements it goes through, or with
/// `members` to an object whose members it goes through. `[*]` and `.*` end
/// a step; what follows the last of them is an array, unless the path ends
/// with `.*`.
fn each_steps(each: &str) -> Vec<(String, bool)> {
    let mut steps = Vec::new();
    let mut rest = each;
    let mut start = true;
    loop {
        let array = rest.find("[*]");
        // `.*` as a whole step: followed by the end, a `.` or a `[`.
        let members = rest
            .match_indices(".*")
            .map(|(i, _)| i)
            .find(|&i| matches!(rest.as_bytes().get(i + 2), None | Some(b'.') | Some(b'[')));
        let (at, len, is_members) = match (array, members) {
            (Some(a), Some(m)) if m < a => (m, 2, true),
            (Some(a), _) => (a, 3, false),
            (None, Some(m)) => (m, 2, true),
            (None, None) => break,
        };
        let part = &rest[..at];
        steps.push((path_of(part, start), is_members));
        start = false;
        rest = &rest[at + len..];
        if rest.is_empty() && is_members {
            return steps;
        }
    }
    steps.push((path_of(rest, start), false));
    steps
}

/// A step's JSON path: the first is the document's, every other is the
/// element's, from `$`.
fn path_of(part: &str, start: bool) -> String {
    match start {
        true => part.to_string(),
        false => format!("${part}"),
    }
}

/// The elements `each` reaches, through every `[*]` (an array's elements)
/// and `.*` (an object's members) in it: `$.data[*].ingest_streams` is each
/// ingest stream of each live video, `$.sources.*` each member of the
/// object at `$.sources`.
fn each_items<'a>(doc: &'a Value, each: &str) -> Vec<EachItem<'a>> {
    let steps = each_steps(each);
    // Elements reached so far, each with the elements it lies in.
    let mut level: Vec<Reached> = vec![(doc, None, Vec::new())];
    let last = steps.len() - 1;
    let mut out = Vec::new();
    for (i, (path, members)) in steps.iter().enumerate() {
        let mut next = Vec::new();
        for (node, place, outer) in level {
            let Some(at) = super::expect::json_path(node, path) else {
                continue;
            };
            let found: Vec<(Option<&str>, &Value)> = match (members, at) {
                (false, Value::Array(a)) => a.iter().map(|v| (None, v)).collect(),
                (true, Value::Object(o)) => o.iter().map(|(k, v)| (Some(k.as_str()), v)).collect(),
                _ => continue,
            };
            let mut within = outer.clone();
            within.push((node, place));
            for (index, (key, item)) in found.into_iter().enumerate() {
                if i == last {
                    out.push(EachItem {
                        item,
                        index,
                        key,
                        outer: within.clone(),
                    });
                } else {
                    next.push((item, Some(Place { index, key }), within.clone()));
                }
            }
        }
        level = next;
    }
    out
}

/// A `json_each` path with its `{settings.name}` placeholders filled in from
/// the settings current now: a member name (H2R's project id), so a rule
/// reads only the configured one. `None`, and no element, when a setting is
/// empty or unset, or its value holds `.`, `[`, `]` or `*`, which would make
/// it another path.
fn each_path(each: &str, settings: &Params) -> Option<String> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\{settings\.([A-Za-z0-9_]+)\}").unwrap());
    if !re.is_match(each) {
        return Some(each.to_string());
    }
    let mut out = String::new();
    let mut last = 0;
    for caps in re.captures_iter(each) {
        let whole = caps.get(0).unwrap();
        let text = match settings.get(&caps[1])? {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => return None,
        };
        if text.is_empty() || text.contains(['.', '[', ']', '*']) {
            return None;
        }
        out.push_str(&each[last..whole.start()]);
        out.push_str(&text);
        last = whole.end();
    }
    out.push_str(&each[last..]);
    Some(out)
}

/// An XML element's own text (its text and CDATA, not its children's),
/// trimmed.
/// Whether an element is the one `xml_each` names (SPEC.md §8, "XML
/// elements"): a local name, or a path of them separated by `/` that ends
/// with the element and names its enclosing elements outward, so
/// `Position/PanTilt` is a `PanTilt` directly inside a `Position`. A name is
/// compared without its namespace prefix, or, written `{uri}name`, with its
/// namespace too, whatever prefix the document gives it.
fn xml_element_is(node: &roxmltree::Node, path: &str) -> bool {
    if !node.is_element() {
        return false;
    }
    let mut names = xml_path_names(path).into_iter().rev();
    if !names.next().is_some_and(|n| xml_name_is(node, n)) {
        return false;
    }
    let mut at = *node;
    for name in names {
        match at.parent_element() {
            Some(parent) if xml_name_is(&parent, name) => at = parent,
            _ => return false,
        }
    }
    true
}

/// An `xml_each` path's names: split at the `/` outside a `{uri}`.
fn xml_path_names(path: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut start, mut in_uri) = (0, false);
    for (i, c) in path.char_indices() {
        match c {
            '{' => in_uri = true,
            '}' => in_uri = false,
            '/' if !in_uri => {
                out.push(&path[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&path[start..]);
    out
}

/// One element name: `name`, any namespace, or `{uri}name`.
fn xml_name_is(node: &roxmltree::Node, name: &str) -> bool {
    match name.strip_prefix('{').and_then(|rest| rest.split_once('}')) {
        Some((uri, local)) => {
            node.tag_name().namespace() == Some(uri) && node.tag_name().name() == local
        }
        None => node.has_tag_name(name),
    }
}

fn xml_text(node: roxmltree::Node) -> Value {
    let text: String = node
        .children()
        .filter(|c| c.is_text())
        .filter_map(|c| c.text())
        .collect();
    Value::String(text.trim().to_string())
}

/// The value at a JSON path from an element `json_each` reached: `$.` is the
/// element's, `$^.` the element one level out's (`$^^.` two).
fn element_path<'a>(e: &EachItem<'a>, path: &str) -> Option<&'a Value> {
    let up = path
        .strip_prefix('$')
        .map_or(0, |rest| rest.len() - rest.trim_start_matches('^').len());
    match up {
        0 => super::expect::json_path(e.item, path),
        n => {
            let node = e.outer.len().checked_sub(n).map(|i| e.outer[i].0)?;
            super::expect::json_path(node, &format!("${}", &path[1 + n..]))
        }
    }
}

/// The captures of each match of a JSON rule: `base` (the path's and the
/// selectors'), then its `json` names. With `json_each`, once per element
/// that `each_match` selects (its captures numbered on after `base`'s):
/// `json` paths are the element's, `$^.` names the element one level out
/// (`$^^.` two), and `{index}` is the element's index in its array. Of the
/// elements it lies in, `{^index}` and `{^key}` are the place and member name
/// of the one a level out, `{^^index}` and `{^^key}` two, and so on.
fn each_values(
    doc: &Value,
    each: Option<&str>,
    settings: &Params,
    each_select: &[(String, Regex)],
    json: &BTreeMap<String, String>,
    base: &[(String, Value)],
) -> Vec<Vec<(String, Value)>> {
    let items = match each {
        Some(each) => match each_path(each, settings) {
            Some(path) => each_items(doc, &path),
            None => Vec::new(),
        },
        None => vec![EachItem {
            item: doc,
            index: 0,
            key: None,
            outer: Vec::new(),
        }],
    };
    items
        .into_iter()
        .filter_map(|e| {
            let mut values = base.to_vec();
            let mut offset = base
                .iter()
                .filter(|(k, _)| k.bytes().all(|b| b.is_ascii_digit()))
                .count();
            for (path, re) in each_select {
                let text = match element_path(&e, path)? {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                let caps = re.captures(&text)?;
                for (i, v) in captures(&caps) {
                    let i: usize = i.parse().unwrap_or(0);
                    values.push(((i + offset).to_string(), v));
                }
                offset += caps.len() - 1;
            }
            if each.is_some() {
                values.push(("index".to_string(), Value::from(e.index)));
            }
            // `.*`: the member's name.
            if let Some(key) = e.key {
                values.push(("key".to_string(), Value::String(key.to_string())));
            }
            // The elements it lies in, a level out at a time.
            for (n, (_, place)) in e.outer.iter().rev().enumerate() {
                let Some(place) = place else {
                    continue;
                };
                let up = "^".repeat(n + 1);
                values.push((format!("{up}index"), Value::from(place.index)));
                if let Some(key) = place.key {
                    values.push((format!("{up}key"), Value::String(key.to_string())));
                }
            }
            for (name, path) in json {
                if let Some(v) = present(element_path(&e, path)) {
                    values.push((name.clone(), v.clone()));
                }
            }
            Some(values)
        })
        .collect()
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

/// `arg_match`: an OSC argument index to a regex over its text.
fn arg_selectors(v: Option<&Value>) -> Result<Vec<(usize, Regex)>, String> {
    let Some(v) = v else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for (index, re) in v
        .as_object()
        .ok_or("telemetry: arg_match maps argument indexes to regexes")?
    {
        let index: usize = index
            .parse()
            .map_err(|_| format!("telemetry: arg_match key '{index}' is not an argument index"))?;
        out.push((index, regex(re, "arg_match")?));
    }
    out.sort_by_key(|(i, _)| *i);
    Ok(out)
}

/// OSC arguments with each blob (`b`) that holds UTF-8 text replaced by that
/// text, so rules read it as they read a string; a blob that is not UTF-8 is
/// kept as its bytes.
fn blobs_as_text(types: &str, args: &[Value]) -> Vec<Value> {
    types
        .chars()
        .zip(args)
        .map(|(tag, arg)| {
            let text = match (tag, arg) {
                ('b', Value::Array(bytes)) => bytes
                    .iter()
                    .map(|b| b.as_u64().and_then(|b| u8::try_from(b).ok()))
                    .collect::<Option<Vec<u8>>>()
                    .and_then(|b| String::from_utf8(b).ok()),
                _ => None,
            };
            text.map_or_else(|| arg.clone(), Value::String)
        })
        .chain(args.iter().skip(types.chars().count()).cloned())
        .collect()
}

/// Whether each `arg_match` regex matches its argument's text (a string as
/// it is, a number as its decimal text); their captures are appended to
/// `base`, numbered on from those already there.
fn select_args(
    args: &[Value],
    selectors: &[(usize, Regex)],
    base: &mut Vec<(String, Value)>,
) -> bool {
    for (index, re) in selectors {
        let text = match args.get(*index) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) | None => return false,
            Some(other) => other.to_string(),
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
    /// `replace`: state subtrees the rule's values replace: removed whenever
    /// the rule's message arrives, before its values are applied. Templates
    /// over the captures before `json_each`, and with it also rendered for
    /// each element from that element's captures.
    replace: Vec<String>,
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

/// One group of `poll` queries and its interval: sent on connecting, and
/// again every `every` ms when set.
#[derive(Debug, Clone)]
pub(crate) struct Poll {
    pub(crate) send: Vec<Value>,
    pub(crate) every: Option<u64>,
    /// Its key under `polled` in state: its `name`, or its place from 1.
    pub(crate) name: String,
}

/// The most poll groups a spec may have: each has a timer of its own.
pub(crate) const MAX_POLLS: usize = 4;

#[derive(Debug, Default)]
pub(crate) struct Telemetry {
    /// Sent after connecting, and again every `renew_every` ms if set.
    pub(crate) subscribe: Vec<Value>,
    pub(crate) renew_every: Option<u64>,
    /// Queries sent on a schedule, in groups each with its own interval
    /// (`poll` as a list); their replies go through the rules.
    pub(crate) polls: Vec<Poll>,
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
            polls: match spec.get("poll") {
                // A list: groups, each at its own interval.
                Some(Value::Array(groups)) => groups
                    .iter()
                    .map(|g| Poll {
                        send: items(g.get("send")),
                        every: every(Some(g)),
                        name: String::new(),
                    })
                    .collect(),
                Some(one) => vec![Poll {
                    send: items(one.get("send")),
                    every: every(Some(one)),
                    name: String::new(),
                }],
                None => Vec::new(),
            },
            rules: Vec::new(),
            types,
            conversions,
        };
        for (poll, name) in t
            .polls
            .iter_mut()
            .zip(crate::catalog::poll_group_names(Some(spec)))
        {
            poll.name = name;
        }
        if t.polls.len() > MAX_POLLS {
            return Err(format!("telemetry: at most {MAX_POLLS} poll groups"));
        }
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
            let matcher = if let Some(q) = rule.get("quiet_ms") {
                let ms = q
                    .as_u64()
                    .filter(|ms| *ms >= 1)
                    .ok_or("telemetry: quiet_ms is a positive number of milliseconds")?;
                let alone = [
                    "match",
                    "address",
                    "header",
                    "path",
                    "json_match",
                    "then_send",
                    "replace",
                ]
                .iter()
                .all(|k| rule.get(k).is_none());
                if !alone || assigns.is_empty() {
                    return Err(
                        "telemetry: a quiet_ms rule has state and no message selector, then_send or replace"
                            .into(),
                    );
                }
                if t.rules
                    .iter()
                    .any(|r| matches!(r.matcher, Matcher::Quiet { .. }))
                {
                    return Err("telemetry: at most one quiet_ms rule".into());
                }
                Matcher::Quiet { ms }
            } else if let (Some(p), Some(m)) = (rule.get("path"), rule.get("match")) {
                Matcher::HttpText {
                    path: regex(p, "path")?,
                    re: regex(m, "match")?,
                }
            } else if let Some(m) = rule.get("match") {
                Matcher::Message {
                    re: regex(m, "match")?,
                    request: match rule.get("request_match") {
                        Some(r @ Value::String(_)) => Some(regex(r, "request_match")?),
                        _ => None,
                    },
                    each: match rule.get("match_each") {
                        Some(r) => Some(regex(r, "match_each")?),
                        None => None,
                    },
                    each_in: match rule.get("match_each_in") {
                        None => None,
                        Some(n) => {
                            Some(n.as_u64().filter(|n| *n >= 1).ok_or(
                                "telemetry: match_each_in is a capture group of match, from 1",
                            )? as usize)
                        }
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
                    arg_match: arg_selectors(rule.get("arg_match"))?,
                    text_each: match rule.get("match_each") {
                        None => None,
                        Some(r) => Some((
                            regex(r, "match_each")?,
                            rule.get("match_each_in")
                                .and_then(Value::as_u64)
                                .filter(|n| *n >= 1)
                                .ok_or(
                                    "telemetry: match_each on an address rule needs match_each_in, a capture from 1",
                                )? as usize,
                        )),
                    },
                    blob_each: match rule.get("blob_each") {
                        None => None,
                        Some(b) => {
                            if ["match_each", "json", "json_each", "json_match"]
                                .iter()
                                .any(|k| rule.get(k).is_some())
                            {
                                return Err(
                                    "telemetry: blob_each goes without match_each or json".into()
                                );
                            }
                            Some(BlobEach::parse(b)?)
                        }
                    },
                    select: selectors(rule.get("json_match"), "json_match")?,
                    each: rule
                        .get("json_each")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    each_select: selectors(rule.get("each_match"), "each_match")?,
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
                        each_select: selectors(rule.get("each_match"), "each_match")?,
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
                    each_select: selectors(rule.get("each_match"), "each_match")?,
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
                replace: items(rule.get("replace"))
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .map(str::to_string)
                            .ok_or("telemetry: replace lists state paths".to_string())
                    })
                    .collect::<Result<_, _>>()?,
            });
        }
        // Every path a rule writes must be declared, so its type is known.
        for rule in &t.rules {
            for path in &rule.replace {
                if !t.declares(&placeholder_to_star(path)) {
                    return Err(format!(
                        "telemetry: replace '{path}' is not declared in the spec's state"
                    ));
                }
            }
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

    /// Whether any rule replaces a list whole (`replace`).
    pub(crate) fn replaces(&self) -> bool {
        self.rules.iter().any(|r| !r.replace.is_empty())
    }

    /// `quiet_ms`: how long the stream must be silent before the quiet
    /// rule's values are assigned, when the spec has one.
    pub(crate) fn quiet_ms(&self) -> Option<u64> {
        self.rules.iter().find_map(|r| match r.matcher {
            Matcher::Quiet { ms } => Some(ms),
            _ => None,
        })
    }

    /// The state patch of the quiet rule, its values rendered with no
    /// captures.
    pub(crate) fn quiet_patch(&self) -> Option<Value> {
        let mut patch = Value::Object(Map::new());
        let mut any = false;
        for rule in &self.rules {
            if matches!(rule.matcher, Matcher::Quiet { .. }) {
                any |= self.assign_all(&rule.assign, &[], &mut patch);
            }
        }
        any.then_some(patch)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rules.is_empty()
            && self.subscribe.is_empty()
            && self.polls.iter().all(|p| p.send.is_empty())
    }

    /// Whether a concrete or starred path is declared, or is a prefix of
    /// declared paths: what a deletion may remove.
    fn declares<S: AsRef<str>>(&self, parts: &[S]) -> bool {
        self.types.iter().any(|(pattern, _)| {
            pattern.len() >= parts.len()
                && pattern.iter().zip(parts).all(|(p, s)| {
                    let s = s.as_ref();
                    p == "*" || p == s || s == "*"
                })
        })
    }

    /// The declared type of a concrete or starred path.
    fn kind_of<S: AsRef<str>>(&self, parts: &[S]) -> Option<&str> {
        self.types
            .iter()
            .find(|(pattern, _)| {
                pattern.len() == parts.len()
                    && pattern.iter().zip(parts).all(|(p, s)| {
                        let s = s.as_ref();
                        p == "*" || p == s || s == "*"
                    })
            })
            .map(|(_, k)| k.as_str())
    }

    /// The state patch this message produces, if any rule matches.
    #[cfg(test)]
    pub(crate) fn apply(&self, message: &Inbound) -> Option<Value> {
        let mut cleared = Value::Object(Map::new());
        let patch = self.apply_into(message, &mut Vec::new(), &mut cleared);
        match (cleared.as_object().is_some_and(|c| !c.is_empty()), patch) {
            (false, patch) => patch,
            // The two patches in order, as one, for a test to compare.
            (true, patch) => {
                let mut both = cleared;
                if let Some(p) = patch {
                    crate::session::merge_patch(&mut both, &p);
                }
                Some(both)
            }
        }
    }

    /// As `apply`, adding to `triggers` the `then_send` items of every rule
    /// that matched, once per match, with its captures, and to `cleared` the
    /// `replace` subtrees of the rules whose message this is: a patch of
    /// removals to apply before the one returned. Without settings.
    #[cfg(test)]
    pub(crate) fn apply_into(
        &self,
        message: &Inbound,
        triggers: &mut Vec<Triggered>,
        cleared: &mut Value,
    ) -> Option<Value> {
        self.apply_with(message, &Params::new(), triggers, cleared)
    }

    /// As `apply_into`, with the device's settings, which a `json_each`
    /// path may name (`{settings.project}`).
    pub(crate) fn apply_with(
        &self,
        message: &Inbound,
        settings: &Params,
        triggers: &mut Vec<Triggered>,
        cleared: &mut Value,
    ) -> Option<Value> {
        // A reply is text to the text rules; its request is for request_match.
        let (message, request) = match message {
            Inbound::Answer { text, request } => (&Inbound::Text(text), Some(*request)),
            other => (other, None),
        };
        // `blob_each` reads a blob's bytes as they came, even ones that
        // happen to be UTF-8 (a block of silent meters is all zeros).
        let raw_args: &[Value] = match message {
            Inbound::Osc { args, .. } => args,
            _ => &[],
        };
        // An OSC blob holding UTF-8 text (OSCPoint's names, notes and JSON)
        // is read as that text; one that is not UTF-8 stays its bytes.
        let texts;
        let with_text;
        let message = match message {
            Inbound::Osc {
                address,
                types,
                args,
            } if types.contains('b') => {
                texts = blobs_as_text(types, args);
                with_text = Inbound::Osc {
                    address,
                    types,
                    args: &texts,
                };
                &with_text
            }
            other => other,
        };
        let mut patch = Value::Object(Map::new());
        let mut any = false;
        for rule in &self.rules {
            match (&rule.matcher, message) {
                (
                    Matcher::Message {
                        re,
                        request: asked,
                        each,
                        each_in,
                    },
                    Inbound::Text(text),
                ) => {
                    let text = text.trim_end();
                    let Some(caps) = re.captures(text) else {
                        continue;
                    };
                    let mut values = captures(&caps);
                    let mut numbered = caps.len() - 1;
                    if let Some(asked) = asked {
                        let Some(request_caps) = request.and_then(|r| asked.captures(r)) else {
                            continue;
                        };
                        for (i, v) in captures(&request_caps) {
                            let n: usize = i.parse().unwrap_or(0);
                            values.push(((n + numbered).to_string(), v));
                        }
                        numbered += request_caps.len() - 1;
                    }
                    self.clear(rule, &values, cleared);
                    match each {
                        None => any |= self.matched(rule, &values, &mut patch, triggers),
                        // Each item along the rest of the message, after
                        // what `match` matched.
                        Some(each) => {
                            // `match_each_in`: within that capture group; a
                            // group that took part in no match has no items.
                            let rest = match each_in {
                                Some(n) => caps.get(*n).map_or("", |m| m.as_str()),
                                None => &text[caps.get(0).map_or(0, |m| m.end())..],
                            };
                            for (index, item) in each.captures_iter(rest).enumerate() {
                                let mut values = values.clone();
                                for (i, v) in captures(&item) {
                                    let n: usize = i.parse().unwrap_or(0);
                                    values.push(((n + numbered).to_string(), v));
                                }
                                values.push(("index".to_string(), Value::from(index)));
                                any |= self.matched(rule, &values, &mut patch, triggers);
                            }
                        }
                    }
                }
                (Matcher::Lines { header, line }, Inbound::Text(text)) => {
                    let mut lines = text.lines();
                    if !lines.next().is_some_and(|h| header.is_match(h.trim_end())) {
                        continue;
                    }
                    self.clear(rule, &[], cleared);
                    // `{index}`: the line's place among those matching, from 0.
                    let mut index = 0;
                    for l in lines {
                        if let Some(caps) = line.captures(l.trim_end()) {
                            let mut values = captures(&caps);
                            values.push(("index".to_string(), Value::from(index)));
                            index += 1;
                            any |= self.matched(rule, &values, &mut patch, triggers);
                        }
                    }
                }
                (Matcher::Fields { header, fields }, Inbound::Text(text)) => {
                    let mut lines = text.lines();
                    if !lines.next().is_some_and(|h| header.is_match(h.trim_end())) {
                        continue;
                    }
                    self.trigger(rule, &[], triggers);
                    self.clear(rule, &[], cleared);
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
                        each_select,
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
                    self.clear(rule, &base, cleared);
                    for values in
                        each_values(&doc, each.as_deref(), settings, each_select, json, &base)
                    {
                        if each.is_some() {
                            self.clear(rule, &values, cleared);
                        }
                        any |= self.matched(rule, &values, &mut patch, triggers);
                    }
                }
                (
                    Matcher::HttpText {
                        path: re,
                        re: text_re,
                    },
                    Inbound::Http { path, body, .. },
                ) => {
                    let Some(caps) = re.captures(path) else {
                        continue;
                    };
                    let Ok(text) = std::str::from_utf8(body) else {
                        continue;
                    };
                    let Some(text_caps) = text_re.captures(text.trim_end()) else {
                        continue;
                    };
                    let mut values = captures(&caps);
                    let offset = caps.len() - 1;
                    for (i, v) in captures(&text_caps) {
                        let n: usize = i.parse().unwrap_or(0);
                        values.push(((n + offset).to_string(), v));
                    }
                    self.clear(rule, &values, cleared);
                    any |= self.matched(rule, &values, &mut patch, triggers);
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
                    self.clear(rule, &base, cleared);
                    for node in doc.descendants().filter(|n| xml_element_is(n, element)) {
                        let mut values = base.clone();
                        for a in node.attributes() {
                            values.push((a.name().to_string(), Value::String(a.value().into())));
                        }
                        // Its text, each child element's text by the child's
                        // name, and the attributes of the elements it lies
                        // in (`{^name}` one out, `{^^name}` two), none
                        // replacing a capture already named.
                        let mut more = vec![("text".to_string(), xml_text(node))];
                        for child in node.children().filter(|c| c.is_element()) {
                            if child.children().all(|c| !c.is_element()) {
                                more.push((child.tag_name().name().to_string(), xml_text(child)));
                            }
                        }
                        for (n, outer) in node
                            .ancestors()
                            .skip(1)
                            .filter(|a| a.is_element())
                            .enumerate()
                        {
                            let up = "^".repeat(n + 1);
                            for a in outer.attributes() {
                                more.push((
                                    format!("{up}{}", a.name()),
                                    Value::String(a.value().into()),
                                ));
                            }
                        }
                        for (name, v) in more {
                            if !values.iter().any(|(k, _)| *k == name) {
                                values.push((name, v));
                            }
                        }
                        any |= self.matched(rule, &values, &mut patch, triggers);
                    }
                }
                (
                    Matcher::Json {
                        select: selectors,
                        json,
                        each,
                        each_select,
                    },
                    Inbound::Json(doc),
                ) => {
                    let mut base = Vec::new();
                    if !select(doc, selectors, &mut base) {
                        continue;
                    }
                    self.clear(rule, &base, cleared);
                    for values in
                        each_values(doc, each.as_deref(), settings, each_select, json, &base)
                    {
                        if each.is_some() {
                            self.clear(rule, &values, cleared);
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
                        arg_match,
                        text_each,
                        blob_each,
                        select: selectors,
                        each,
                        each_select,
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
                    let Some(caps) = re.captures(address) else {
                        continue;
                    };
                    let mut base = captures(&caps);
                    if !select_args(args, arg_match, &mut base) {
                        continue;
                    }
                    let reads_json = !json.is_empty() || !selectors.is_empty() || each.is_some();
                    let doc = match reads_json {
                        false => Value::Null,
                        true => match args
                            .get(*json_arg)
                            .and_then(Value::as_str)
                            .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        {
                            Some(doc) => doc,
                            None => continue,
                        },
                    };
                    // `json_match` captures are numbered on after the address's.
                    if !select(&doc, selectors, &mut base) {
                        continue;
                    }
                    let numbered = base.len();
                    for (i, a) in args.iter().enumerate() {
                        base.push((format!("arg{i}"), a.clone()));
                    }
                    // Each number of a blob; an argument that is not a blob
                    // gives nothing.
                    if let Some(blob) = blob_each {
                        let Some(numbers) = blob.numbers(raw_args) else {
                            continue;
                        };
                        self.clear(rule, &base, cleared);
                        for (index, n) in numbers.into_iter().enumerate() {
                            let mut values = base.clone();
                            values.push(("value".to_string(), n));
                            values.push(("index".to_string(), Value::from(index)));
                            any |= self.matched(rule, &values, &mut patch, triggers);
                        }
                        continue;
                    }
                    self.clear(rule, &base, cleared);
                    // Each item of a list inside a capture's text; a capture
                    // that took part in no match has no items.
                    if let Some((each, group)) = text_each {
                        let name = group.to_string();
                        let text = match base.iter().find(|(k, _)| *k == name) {
                            Some((_, Value::String(s))) => s.clone(),
                            _ => String::new(),
                        };
                        for (index, item) in each.captures_iter(&text).enumerate() {
                            let mut values = base.clone();
                            for (i, v) in captures(&item) {
                                let n: usize = i.parse().unwrap_or(0);
                                values.push(((n + numbered).to_string(), v));
                            }
                            values.push(("index".to_string(), Value::from(index)));
                            any |= self.matched(rule, &values, &mut patch, triggers);
                        }
                        continue;
                    }
                    if !reads_json {
                        any |= self.matched(rule, &base, &mut patch, triggers);
                        continue;
                    }
                    for values in
                        each_values(&doc, each.as_deref(), settings, each_select, json, &base)
                    {
                        if each.is_some() {
                            self.clear(rule, &values, cleared);
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

    /// `replace`: the subtrees the rule's values replace, removed first.
    fn clear(&self, rule: &Rule, values: &[(String, Value)], cleared: &mut Value) {
        if rule.replace.is_empty() {
            return;
        }
        let (params, specs) = capture_params(values);
        let ctx = Values {
            params: &params,
            param_specs: &specs,
            settings: &Params::new(),
            setting_specs: &BTreeMap::new(),
            conversions: &self.conversions,
            maps: &Default::default(),
            session: None,
        };
        for template in &rule.replace {
            let Some(path) = render_path(template, &ctx) else {
                continue;
            };
            if path.iter().any(String::is_empty)
                || path.len() != segments(template).len()
                || !self.declares(&path)
            {
                continue;
            }
            set_path(cleared, &path, Value::Null);
        }
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
            session: None,
        };
        let mut any = false;
        for a in assigns {
            let Some(path) = render_path(&a.path, &ctx) else {
                continue;
            };
            if a.delete {
                // A segment rendered empty, or holding a dot (other than a
                // `:key` one), would delete something other than what the
                // rule names.
                if path.iter().any(String::is_empty)
                    || path.len() != segments(&a.path).len()
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
        multiline: false,
        negative_infinity: false,
        default_by_model: None,
        select: None,
    }
}

fn captures(caps: &regex::Captures) -> Vec<(String, Value)> {
    caps.iter()
        .enumerate()
        .skip(1)
        .filter_map(|(i, m)| m.map(|m| (i.to_string(), Value::String(m.as_str().to_string()))))
        .collect()
}

/// `outputs.{1:+1}.input` → `outputs.*.input`, as segments.
fn placeholder_to_star(path: &str) -> Vec<&str> {
    segments(path)
        .into_iter()
        .map(|seg| if seg.contains('{') { "*" } else { seg })
        .collect()
}

/// A state path template's segments: split at each dot outside a
/// placeholder, so `{2:.1f}` stays whole.
fn segments(template: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (i, c) in template.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '.' if depth == 0 => {
                out.push(&template[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&template[start..]);
    out
}

/// A segment whose placeholder carries `:key`, without that directive:
/// `{1:url_decode:key}` becomes `{1:url_decode}`. `None` for any other
/// segment.
fn without_key(segment: &str) -> Option<String> {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\{([^{}:]+)((?::[^{}:]+)*)\}").unwrap());
    let mut keyed = false;
    let out = re.replace_all(segment, |caps: &regex::Captures| {
        let mut s = format!("{{{}", &caps[1]);
        for d in caps[2].split(':').filter(|d| !d.is_empty()) {
            if d == "key" {
                keyed = true;
            } else {
                s.push(':');
                s.push_str(d);
            }
        }
        s.push('}');
        s
    });
    keyed.then(|| out.into_owned())
}

/// A state path template rendered to its segments (SPEC.md §8, "Keys
/// holding dots"). A segment whose placeholder carries `:key` stays one
/// segment whatever its value holds, so mimoLive's
/// `com.boinx.mimoLive.layers.text` is one key. Any other segment is split
/// at the dots its value holds, as it always was, so such a value reaches no
/// declared path and a deletion's length check refuses it. `None` when a
/// value is missing or a `:key` segment renders empty.
fn render_path(template: &str, ctx: &Values) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for segment in segments(template) {
        match without_key(segment) {
            Some(stripped) => {
                let text = render(&stripped, ctx, |s| s.to_string()).ok()?;
                if text.is_empty() {
                    return None;
                }
                out.push(text);
            }
            None => {
                let text = render(segment, ctx, |s| s.to_string()).ok()?;
                out.extend(text.split('.').map(str::to_string));
            }
        }
    }
    Some(out)
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

fn set_path(patch: &mut Value, parts: &[String], value: Value) {
    let mut node = patch;
    for (i, part) in parts.iter().enumerate() {
        let map = match node {
            Value::Object(m) => m,
            _ => return,
        };
        if i == parts.len() - 1 {
            map.insert(part.clone(), value);
            return;
        }
        node = map
            .entry(part.clone())
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
    fn hexadecimal_captures_become_numbers() {
        // LG's acknowledgement: data in hex digits, `64` for 100.
        let t = Telemetry::parse(
            Some(&json!({"updates": [{
                "match": "^f (\\w+) OK([0-9a-fA-F]+)x$",
                "state": {"volume": "{2:hex}", "set_id": "{1:hex}"},
            }]})),
            &state(json!({
                "volume": {"type": "int", "description": "x"},
                "set_id": {"type": "int", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        assert_eq!(
            t.apply(&Inbound::Text("f 01 OK64x")),
            Some(json!({"volume": 100, "set_id": 1}))
        );
        assert_eq!(
            t.apply(&Inbound::Text("f 3e8 OK0ax")),
            Some(json!({"volume": 10, "set_id": 1000}))
        );
    }

    #[test]
    fn bits_of_a_flags_word_become_booleans() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [{
                "match": r"^TRACK (\d+) (-?\w*)$",
                "state": {
                    "tracks.{1}.flags": "{2}",
                    "tracks.{1}.muted": "{2:bit.3}",
                    "tracks.{1}.soloed": "{2:bit.4}",
                    "tracks.{1}.armed": {"value": "{2:bit.6}", "map": {"true": "armed", "false": "safe"}},
                },
            }]})),
            &state(json!({
                "tracks.*.flags": {"type": "int", "description": "x"},
                "tracks.*.muted": {"type": "bool", "description": "x"},
                "tracks.*.soloed": {"type": "bool", "description": "x"},
                "tracks.*.armed": {"type": "string", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        assert_eq!(
            t.apply(&Inbound::Text("TRACK 2 72")).unwrap(),
            json!({"tracks": {"2": {"flags": 72, "muted": true, "soloed": false, "armed": "armed"}}})
        );
        // A word that is not a decimal integer assigns no bit.
        assert_eq!(t.apply(&Inbound::Text("TRACK 2 x8")), None);
    }

    #[test]
    fn a_quiet_rule_stands_alone_and_matches_no_message() {
        let declared = || state(json!({"complete": {"type": "bool", "description": "x"}}));
        for (rule, ok) in [
            (
                json!({"quiet_ms": 500, "state": {"complete": "true"}}),
                true,
            ),
            (json!({"quiet_ms": 0, "state": {"complete": "true"}}), false),
            (json!({"quiet_ms": 500}), false),
            (
                json!({"quiet_ms": 500, "match": "^X$", "state": {"complete": "true"}}),
                false,
            ),
            (
                json!({"quiet_ms": 500, "then_send": ["X"], "state": {"complete": "true"}}),
                false,
            ),
        ] {
            let parsed = Telemetry::parse(
                Some(&json!({ "updates": [rule] })),
                &declared(),
                Conversions::new(),
            );
            assert_eq!(parsed.is_ok(), ok, "{rule}");
        }
        let two = json!({"updates": [
            {"quiet_ms": 500, "state": {"complete": "true"}},
            {"quiet_ms": 900, "state": {"complete": "true"}}
        ]});
        assert!(Telemetry::parse(Some(&two), &declared(), Conversions::new()).is_err());
        let t = Telemetry::parse(
            Some(&json!({"updates": [{"quiet_ms": 500, "state": {"complete": "true"}}]})),
            &declared(),
            Conversions::new(),
        )
        .unwrap();
        assert_eq!(t.quiet_ms(), Some(500));
        assert_eq!(t.quiet_patch(), Some(json!({"complete": true})));
        assert!(t.apply(&Inbound::Text("anything")).is_none());
        assert!(!t.is_empty());
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
    fn a_list_inside_an_osc_string_is_read_item_by_item() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"address": "^/cards/([12])/stat$",
                 "arg_match": {"0": "(?:^|\\s)names\\s+list \\[([^\\]]*)\\]"},
                 "match_each": "\\s*([^,]+?)\\s*(?:,|$)", "match_each_in": 2,
                 "replace": "cards.{1}.names",
                 "state": {"cards.{1}.names.{index:+1}": "{3}"}},
            ]})),
            &state(json!({"cards.*.names.*": {"type": "string", "description": "x"}})),
            Conversions::new(),
        )
        .unwrap();
        let reply = |text: &str| {
            let args = [json!(text)];
            let mut cleared = json!({});
            let patch = t.apply_into(
                &Inbound::Osc {
                    address: "/cards/2/stat",
                    types: "s",
                    args: &args,
                },
                &mut Vec::new(),
                &mut cleared,
            );
            (cleared, patch)
        };
        let (cleared, patch) =
            reply(" count int [0 .. 9]\n names   list [Take 1, Second take,Last]\n");
        assert_eq!(cleared, json!({"cards": {"2": {"names": null}}}));
        assert_eq!(
            patch,
            Some(json!({"cards": {"2": {"names": {
                "1": "Take 1", "2": "Second take", "3": "Last"}}}}))
        );
        // An empty list clears and writes nothing; another text is no match.
        let (cleared, patch) = reply(" names list []");
        assert_eq!(cleared, json!({"cards": {"2": {"names": null}}}));
        assert_eq!(patch, None);
        assert_eq!(reply(" other list [a]"), (json!({}), None));
        // An address rule's match_each needs the capture it goes along.
        assert!(Telemetry::parse(
            Some(&json!({"updates": [{"address": "^/x$", "match_each": "y", "state": {}}]})),
            &state(json!({})),
            Conversions::new(),
        )
        .is_err());
    }

    #[test]
    fn a_list_in_an_osc_argument_replaces_the_last_one() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"address": "^/reply/(?:workspace/[^/]+/)?cueLists$",
                 "json_match": {"$.workspace_id": "^([A-Z0-9-]+)$", "$.status": "^ok$"},
                 "json_each": "$.data", "json": {"id": "$.uniqueID", "name": "$.name"},
                 "replace": "ws.{1}.cues",
                 "state": {"ws.{1}.cues.{id}.name": "{name}", "ws.{1}.cues.{id}.index": "{index}"},
                 "then_send": [{"address": "/workspace/{1}/cue_id/{id}/valuesForKeys"}]},
                {"address": "^/reply/(?:workspace/[^/]+/)?cueLists$",
                 "json_match": {"$.workspace_id": "^([A-Z0-9-]+)$"},
                 "json_each": "$.data[*].cues", "each_match": {"$.type": "^(Audio)$"},
                 "json": {"id": "$.uniqueID", "parent": "$^.uniqueID"},
                 "state": {"ws.{1}.cues.{id}.parent": "{parent}", "ws.{1}.cues.{id}.type": "{2}"}},
            ]})),
            &state(json!({
                "ws.*.cues.*.name": {"type": "string", "description": "x"},
                "ws.*.cues.*.index": {"type": "int", "description": "x"},
                "ws.*.cues.*.parent": {"type": "string", "description": "x"},
                "ws.*.cues.*.type": {"type": "string", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let reply = json!({"workspace_id": "W1", "status": "ok", "data": [
            {"uniqueID": "L1", "name": "Main", "cues": [
                {"uniqueID": "C1", "type": "Audio"}, {"uniqueID": "C2", "type": "Wait"}]},
        ]})
        .to_string();
        let mut cleared = json!({});
        let mut triggers = Vec::new();
        let patch = t.apply_into(
            &Inbound::Osc {
                address: "/reply/cueLists",
                types: "s",
                args: &[json!(reply)],
            },
            &mut triggers,
            &mut cleared,
        );
        assert_eq!(cleared, json!({"ws": {"W1": {"cues": null}}}));
        assert_eq!(
            patch,
            Some(json!({"ws": {"W1": {"cues": {
                "L1": {"name": "Main", "index": 0},
                "C1": {"parent": "L1", "type": "Audio"},
            }}}}))
        );
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].params.get("id"), Some(&json!("L1")));
        // A reply whose status does not match is not read, and replaces nothing.
        let denied = json!({"workspace_id": "W1", "status": "denied", "data": []}).to_string();
        let mut cleared = json!({});
        let patch = t.apply_into(
            &Inbound::Osc {
                address: "/reply/cueLists",
                types: "s",
                args: &[json!(denied)],
            },
            &mut Vec::new(),
            &mut cleared,
        );
        assert_eq!((cleared, patch), (json!({}), None));
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
    fn osc_string_arguments_can_be_matched_with_captures() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"address": "^/(\\w+)/info$", "arg_match": {"0": "^layer:([^/]+)/(opacity|scale)$"},
                 "state": {"layers.{2}.{3}": "{arg1}", "apps.{1}": "{arg0}"}},
                {"address": "^/(\\w+)/info$", "arg_match": {"1": "^(\\d+)$", "0": "^light:([^/]+)/intensity$"},
                 "state": {"lights.{2}.intensity": "{3}"}},
            ]})),
            &state(json!({
                "layers.*.opacity": {"type": "float", "description": "x"},
                "layers.*.scale": {"type": "float", "description": "x"},
                "lights.*.intensity": {"type": "int", "description": "x"},
                "apps.*": {"type": "string", "description": "x"},
            })),
            Default::default(),
        )
        .unwrap();
        let osc = |args: &[Value]| {
            t.apply(&Inbound::Osc {
                address: "/millumin/info",
                types: "sf",
                args,
            })
        };
        // The address's captures come first, then the argument's.
        assert_eq!(
            osc(&[json!("layer:Screen 1/opacity"), json!(0.5)]).unwrap(),
            json!({"layers": {"Screen 1": {"opacity": 0.5}}, "apps": {"millumin": "layer:Screen 1/opacity"}})
        );
        // Every argument named must match, a number as its decimal text, in
        // index order whatever the order written.
        assert_eq!(
            osc(&[json!("light:Spot/intensity"), json!(200)]).unwrap(),
            json!({"lights": {"Spot": {"intensity": 200}}})
        );
        assert!(osc(&[json!("light:Spot/intensity"), json!(0.5)]).is_none());
        assert!(osc(&[json!("layer:Screen 1/rotation"), json!(0.5)]).is_none());
        assert!(osc(&[]).is_none());
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
    fn osc_blobs_holding_utf8_are_read_as_text() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"address": "^/name$", "state": {"name": "{arg0}"}},
                {"address": "^/event$", "arg_match": {"1": "^Caf(é)"},
                 "state": {"accent": "{1}"}},
                {"address": "^/doc$", "json_arg": 1, "json": {"n": "$.name"},
                 "state": {"doc": "{n}"}},
            ]})),
            &state(json!({
                "name": {"type": "string", "description": "x"},
                "accent": {"type": "string", "description": "x"},
                "doc": {"type": "string", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let bytes = |s: &str| json!(s.as_bytes());
        let osc = |address, types, args: &[Value]| {
            t.apply(&Inbound::Osc {
                address,
                types,
                args,
            })
        };
        // A blob's UTF-8 text, as a value, for arg_match and as JSON.
        assert_eq!(
            osc("/name", "b", &[bytes("Grüße")]).unwrap(),
            json!({"name": "Grüße"})
        );
        assert_eq!(
            osc("/event", "sb", &[json!("open"), bytes("Café.pptx")]).unwrap(),
            json!({"accent": "é"})
        );
        assert_eq!(
            osc(
                "/doc",
                "sb",
                &[json!("{}"), bytes(r#"{"name":"Ünï.pptx"}"#)]
            )
            .unwrap(),
            json!({"doc": "Ünï.pptx"})
        );
        // A blob that is not UTF-8 stays bytes, which a JSON rule cannot read.
        assert!(osc("/doc", "sb", &[json!("{}"), json!([0xff, 0xfe])]).is_none());
    }

    #[test]
    fn numbers_packed_in_a_blob_are_read_one_by_one() {
        let conversions = super::super::template::conversions(Some(&json!({
            "q8": {"points": [[-32768, -128.0], [32767, 127.99609375]]},
            "amp": {"law": "db20", "floor": -90.0},
        })))
        .unwrap();
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                // A count first, then i16 levels in 1/256 dB.
                {"address": "^/meters/1$", "blob_each": {"offset": 4, "count": 2, "format": "i16le"},
                 "state": {"ch.{index:+1}.db": "{value:from.q8}"}},
                {"address": "^/meters/1$", "blob_each": {"offset": 8, "format": "i16le"},
                 "state": {"bus.{index:+1}.db": "{value:from.q8}"}},
                // Linear floats, as dB.
                {"address": "^/meters/2$", "blob_each": {"arg": 1, "offset": 4, "format": "f32le"},
                 "state": {"main.{index:+1}.db": "{value:from.amp}"}},
                {"address": "^/raw$", "blob_each": {"format": "u16be"}, "replace": "raw",
                 "state": {"raw.{index}": "{value}"}},
            ]})),
            &state(json!({
                "ch.*.db": {"type": "float", "description": "x"},
                "bus.*.db": {"type": "float", "description": "x"},
                "main.*.db": {"type": "float", "description": "x"},
                "raw.*": {"type": "int", "description": "x"},
            })),
            conversions,
        )
        .unwrap();
        let osc = |address, types, args: &[Value]| {
            t.apply(&Inbound::Osc {
                address,
                types,
                args,
            })
        };
        let mut blob = vec![3u8, 0, 0, 0];
        for v in [-2560i16, 0, 256] {
            blob.extend_from_slice(&v.to_le_bytes());
        }
        // Count 2 takes the first two numbers; the bus rule starts 4 bytes on
        // and takes what is left, the odd byte past the end dropped.
        blob.push(9);
        assert_eq!(
            osc("/meters/1", "b", &[json!(blob)]).unwrap(),
            json!({"ch": {"1": {"db": -10.0}, "2": {"db": 0.0}}, "bus": {"1": {"db": 1.0}}})
        );
        // All zeros is valid UTF-8, yet still read as numbers: silence.
        let mut floats = vec![2u8, 0, 0, 0];
        floats.extend_from_slice(&1.0f32.to_le_bytes());
        floats.extend_from_slice(&0.0f32.to_le_bytes());
        assert_eq!(
            osc("/meters/2", "ib", &[json!(7), json!(floats)]).unwrap(),
            json!({"main": {"1": {"db": 0.0}, "2": {"db": -90.0}}})
        );
        assert_eq!(
            osc("/raw", "b", &[json!([0, 0, 0, 0])]).unwrap(),
            json!({"raw": {"0": 0, "1": 0}})
        );
        assert_eq!(
            osc("/raw", "b", &[json!([1, 2])]).unwrap(),
            json!({"raw": {"0": 258}})
        );
        // An argument that is not a blob gives nothing.
        assert!(osc("/raw", "s", &[json!("x")]).is_none());
        for bad in [
            json!({"format": "f64le"}),
            json!({"format": "i16le", "count": 0}),
            json!({"format": "i16le", "size": 2}),
        ] {
            let e = Telemetry::parse(
                Some(&json!({"updates": [{"address": "^/x$", "blob_each": bad, "state": {}}]})),
                &BTreeMap::new(),
                Conversions::new(),
            );
            assert!(e.is_err(), "{bad}");
        }
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
    fn a_key_segment_keeps_its_dots() {
        let state_decl = state(json!({
            "types.*.name": {"type": "string", "description": "x"},
            "types.*.live": {"type": "bool", "description": "x"},
            "plain.*.name": {"type": "string", "description": "x"},
        }));
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"json_match": {"$.event": "^added$"},
                 "json": {"id": "$.id", "name": "$.name"},
                 "state": {"types.{id:key}.name": "{name}", "plain.{id}.name": "{name}"}},
                {"json_match": {"$.event": "^removed$", "$.id": "^(.*)$"},
                 "state": {"types.{1:key}": {"delete": true}}},
                {"json_match": {"$.event": "^list$"},
                 "json_each": "$.ids",
                 "json": {"id": "$"},
                 "replace": "types.{id:url_decode:key}",
                 "state": {"types.{id:url_decode:key}.live": "true"}},
            ]})),
            &state_decl,
            Conversions::new(),
        )
        .unwrap();
        // The id is one key, dots and all; without `:key` it reaches nothing.
        let added = json!({"event": "added", "id": "com.boinx.text", "name": "Text"});
        assert_eq!(
            t.apply(&Inbound::Json(&added)),
            Some(json!({"types": {"com.boinx.text": {"name": "Text"}}}))
        );
        let plain = json!({"event": "added", "id": "A1", "name": "Bg"});
        assert_eq!(
            t.apply(&Inbound::Json(&plain)),
            Some(json!({"types": {"A1": {"name": "Bg"}}, "plain": {"A1": {"name": "Bg"}}}))
        );
        // A deletion and a replace remove that one key.
        let removed = json!({"event": "removed", "id": "com.boinx.text"});
        assert_eq!(
            t.apply(&Inbound::Json(&removed)),
            Some(json!({"types": {"com.boinx.text": null}}))
        );
        let list = json!({"event": "list", "ids": ["a.b", "c%2Ed"]});
        let mut s = json!({"types": {"a.b": {"name": "AB", "live": false}}});
        let mut cleared = json!({});
        let patch = t.apply_into(&Inbound::Json(&list), &mut Vec::new(), &mut cleared);
        assert_eq!(cleared, json!({"types": {"a.b": null, "c.d": null}}));
        crate::session::merge_patch(&mut s, &cleared);
        crate::session::merge_patch(&mut s, &patch.unwrap());
        assert_eq!(
            s,
            json!({"types": {"a.b": {"live": true}, "c.d": {"live": true}}})
        );
        // An empty key is no key.
        let empty = json!({"event": "removed", "id": ""});
        assert_eq!(t.apply(&Inbound::Json(&empty)), None);
    }

    #[test]
    fn an_objects_members_are_gone_through_by_name() {
        assert_eq!(
            each_steps("$.data[*].ingest_streams"),
            vec![
                ("$.data".to_string(), false),
                ("$.ingest_streams".to_string(), false)
            ]
        );
        assert_eq!(
            each_steps("$.sources.*"),
            vec![("$.sources".to_string(), true)]
        );
        assert_eq!(
            each_steps("$.devices.*.inputs"),
            vec![
                ("$.devices".to_string(), true),
                ("$.inputs".to_string(), false)
            ]
        );
        // `.*x` is a member's name, not a step.
        assert_eq!(each_steps("$.a.*x"), vec![("$.a.*x".to_string(), false)]);
        let state_decl = state(json!({
            "sources.*.address": {"type": "string", "description": "x"},
            "sources.*.index": {"type": "int", "description": "x"},
            "devices.*.inputs.*.name": {"type": "string", "description": "x"},
        }));
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"path": "^/List$", "json_each": "$.*", "json": {"address": "$"},
                 "replace": "sources",
                 "state": {"sources.{key:key}.address": "{address}", "sources.{key:key}.index": "{index}"}},
                {"path": "^/devices$", "json_each": "$.devices.*.inputs",
                 "json": {"device": "$^.id", "name": "$.name"},
                 "state": {"devices.{device}.inputs.{index}.name": "{name}"}},
            ]})),
            &state_decl,
            Conversions::new(),
        )
        .unwrap();
        let body = br#"{"CAM 1 (BirdDog)": "10.0.0.5:5961", "studio.cam2": "10.0.0.6:5961"}"#;
        let reply = Inbound::Http {
            path: "/List",
            headers: &[],
            body,
            request: None,
        };
        assert_eq!(
            t.apply(&reply),
            Some(json!({"sources": {
                "CAM 1 (BirdDog)": {"address": "10.0.0.5:5961", "index": 0},
                "studio.cam2": {"address": "10.0.0.6:5961", "index": 1}}}))
        );
        // Through members, then an array in each: `$^.` is the member.
        let body = br#"{"devices": {"a": {"id": "A", "inputs": [{"name": "In 1"}]}, "b": {"id": "B", "inputs": []}}}"#;
        let reply = Inbound::Http {
            path: "/devices",
            headers: &[],
            body,
            request: None,
        };
        assert_eq!(
            t.apply(&reply),
            Some(json!({"devices": {"A": {"inputs": {"0": {"name": "In 1"}}}}}))
        );
        // An array where members are expected is no match.
        let body = br#"["10.0.0.5"]"#;
        let reply = Inbound::Http {
            path: "/List",
            headers: &[],
            body,
            request: None,
        };
        assert_eq!(t.apply(&reply), Some(json!({"sources": null})));
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
            let patch = t.apply_into(
                &Inbound::Answer { text, request },
                &mut triggers,
                &mut json!({}),
            );
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
    fn a_list_can_be_read_within_one_capture_group() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"match": "^< SAMPLE((?: \\d{3})+) >$", "match_each": "(\\d{3})", "match_each_in": 1,
                 "state": {"meters.{index:+1}": "{2:-60}"}},
            ]})),
            &state(json!({"meters.*": {"type": "int", "description": "x"}})),
            Default::default(),
        )
        .unwrap();
        assert_eq!(
            t.apply(&Inbound::Text("< SAMPLE 060 045 000 >")).unwrap(),
            json!({"meters": {"1": 0, "2": -15, "3": -60}})
        );
        // A message sharing the opening words, but no list of levels, does
        // not match.
        assert!(t
            .apply(&Inbound::Text(
                "< SAMPLE TALKER_POSITIONS 1 2 -1524 321 914 0 >"
            ))
            .is_none());
        assert!(Telemetry::parse(
            Some(&json!({"updates": [{"match": "^x$", "match_each": "y", "match_each_in": 0, "state": {}}]})),
            &state(json!({})),
            Default::default(),
        )
        .is_err());
    }

    #[test]
    fn a_list_on_one_line_is_read_item_by_item() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"match": "^0 (\\d+)(?: |$)", "request_match": "^RRL (\\d+) -1$",
                 "match_each": "(\\d+) (\\S+)",
                 "replace": "registers.{2}",
                 "state": {"registers.{2}.{3}.name": "{4:url_decode}",
                           "registers.{2}.{3}.place": "{index}"},
                 "then_send": ["SCR {3} R"]},
            ]})),
            &state(json!({
                "registers.*.*.name": {"type": "string", "description": "x"},
                "registers.*.*.place": {"type": "int", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let mut triggers = Vec::new();
        let mut cleared = json!({});
        let patch = t.apply_into(
            &Inbound::Answer {
                text: "0 2 1 Look%201 7 Wide",
                request: "RRL 4 -1",
            },
            &mut triggers,
            &mut cleared,
        );
        assert_eq!(
            patch,
            Some(json!({"registers": {"4": {
                "1": {"name": "Look 1", "place": 0},
                "7": {"name": "Wide", "place": 1}}}}))
        );
        // The list is replaced whole, and each item asks for its own re-read.
        assert_eq!(cleared, json!({"registers": {"4": null}}));
        assert_eq!(triggers.len(), 2);
        assert_eq!(triggers[1].params["3"], json!(7));
        // An empty list clears and assigns nothing.
        let mut cleared = json!({});
        let patch = t.apply_into(
            &Inbound::Answer {
                text: "0 0",
                request: "RRL 4 -1",
            },
            &mut Vec::new(),
            &mut cleared,
        );
        assert_eq!(patch, None);
        assert_eq!(cleared, json!({"registers": {"4": null}}));
    }

    #[test]
    fn a_list_replaces_the_last_one() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"header": "^FILE LIST:$", "each_line": "^([^:]+)$", "replace": "files",
                 "state": {"files.{index}": "{1}"}},
                {"path": "^/service/items$", "json_each": "$", "json": {"id": "$.id", "title": "$.title"},
                 "replace": ["service.items"], "state": {"service.items.{id}.title": "{title}"}},
            ]})),
            &state(json!({
                "files.*": {"type": "string", "description": "x"},
                "service.items.*.title": {"type": "string", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let apply = |m: &Inbound| {
            let mut cleared = json!({});
            let patch = t.apply_into(m, &mut Vec::new(), &mut cleared);
            (cleared, patch)
        };
        let (cleared, patch) = apply(&Inbound::Text("FILE LIST:\nCount: 2\nA.ult\nB.ult\n"));
        assert_eq!(cleared, json!({"files": null}));
        assert_eq!(patch, Some(json!({"files": {"0": "A.ult", "1": "B.ult"}})));
        // An empty list clears what was there.
        let (cleared, patch) = apply(&Inbound::Http {
            path: "/service/items",
            headers: &[],
            body: b"[]",
            request: None,
        });
        assert_eq!(cleared, json!({"service": {"items": null}}));
        assert_eq!(patch, None);
        // As the session applies them: the removal, then the new list.
        let mut s = json!({"files": {"0": "Old.ult", "1": "B.ult", "2": "C.ult"}});
        let (cleared, patch) = apply(&Inbound::Text("FILE LIST:\nB.ult\n"));
        crate::session::merge_patch(&mut s, &cleared);
        crate::session::merge_patch(&mut s, &patch.unwrap());
        assert_eq!(s, json!({"files": {"0": "B.ult"}}));
    }

    #[test]
    fn each_element_selected_and_its_own_list_replaced() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                // Each set that carries a list (empty or not) has it replaced.
                {"path": "^/sets$", "json_each": "$.data", "json": {"id": "$.id"},
                 "each_match": {"$.type": "^sets$", "$.list": "^\\["},
                 "replace": "sets.{id}.list"},
                // Each entry of each set's list, sets only.
                {"path": "^/sets$", "json_each": "$.data[*].list",
                 "each_match": {"$^.type": "^(sets)$"}, "json": {"id": "$^.id", "v": "$.v"},
                 "state": {"{1}.{id}.list.{index}": "{v}"}},
            ]})),
            &state(json!({"sets.*.list.*": {"type": "string", "description": "x"}})),
            Conversions::new(),
        )
        .unwrap();
        let body = br#"{"data": [
            {"type": "sets", "id": "a", "list": [{"v": "x"}, {"v": "y"}]},
            {"type": "sets", "id": "b", "list": []},
            {"type": "sets", "id": "c"},
            {"type": "other", "id": "d", "list": [{"v": "z"}]}]}"#;
        let mut cleared = json!({});
        let patch = t.apply_into(
            &Inbound::Http {
                path: "/sets",
                headers: &[],
                body,
                request: None,
            },
            &mut Vec::new(),
            &mut cleared,
        );
        // c has no list: kept. d is not a set: neither cleared nor written.
        assert_eq!(
            cleared,
            json!({"sets": {"a": {"list": null}, "b": {"list": null}}})
        );
        assert_eq!(
            patch,
            Some(json!({"sets": {"a": {"list": {"0": "x", "1": "y"}}}}))
        );
    }

    #[test]
    fn nested_arrays_keyed_by_a_field_or_an_index() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"path": "^/live_videos$", "json_each": "$.data[*].ingest_streams",
                 "json": {"video": "$^.id", "stream": "$.id", "kbps": "$.stream_health.video_bitrate",
                          "page": "$^^.page"},
                 "state": {"videos.{video}.ingest.{stream}.bitrate": "{kbps}",
                           "videos.{video}.ingest.{stream}.position": "{index}",
                           "videos.{video}.page": "{page}"}},
                {"json_match": {"$.type": "^tags$"}, "json_each": "$.tags",
                 "json": {"tag": "$"}, "state": {"tags.{index}": "{tag}"}},
            ]})),
            &state(json!({
                "videos.*.ingest.*.bitrate": {"type": "int", "description": "x"},
                "videos.*.ingest.*.position": {"type": "int", "description": "x"},
                "videos.*.page": {"type": "string", "description": "x"},
                "tags.*": {"type": "string", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let body = json!({"page": "P1", "data": [
            {"id": "V1", "ingest_streams": [
                {"id": "S1", "stream_health": {"video_bitrate": 4000}},
                {"id": "S2", "stream_health": {"video_bitrate": 3900}}]},
            {"id": "V2", "ingest_streams": []},
            {"id": "V3"}]})
        .to_string();
        assert_eq!(
            t.apply(&Inbound::Http {
                path: "/live_videos",
                headers: &[],
                body: body.as_bytes(),
                request: None
            }),
            Some(json!({"videos": {"V1": {"page": "P1", "ingest": {
                "S1": {"bitrate": 4000, "position": 0},
                "S2": {"bitrate": 3900, "position": 1}}}}}))
        );
        let tags = json!({"type": "tags", "tags": ["English", "Music"]});
        assert_eq!(
            t.apply(&Inbound::Json(&tags)),
            Some(json!({"tags": {"0": "English", "1": "Music"}}))
        );
    }

    #[test]
    fn an_outer_elements_place_keys_the_inner_ones() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                // Each slide of each group, keyed by both positions.
                {"path": "^/active$", "json_each": "$.*.groups[*].slides",
                 "json": {"text": "$.text"},
                 "state": {"p.groups.{^index}.slides.{index:+1}.text": "{text}"}},
                // Members, then arrays in arrays: each level's place and name.
                {"json_match": {"$.event": "^update$"}, "json_each": "$.data.projects.*.cues[*].scores[*]",
                 "json": {"id": "$^^.id", "score": "$.score"},
                 "state": {"scores.{^^^key}.{id}.{^index:+1}.{index:+1}": "{score}",
                           "cue_place.{id}": "{^^index}"}},
            ]})),
            &state(json!({
                "p.groups.*.slides.*.text": {"type": "string", "description": "x"},
                "scores.*.*.*.*": {"type": "int", "description": "x"},
                "cue_place.*": {"type": "int", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let body = br#"{"presentation": {"groups": [
            {"slides": [{"text": "a"}, {"text": "b"}]},
            {"slides": []},
            {"slides": [{"text": "c"}]}]}}"#;
        assert_eq!(
            t.apply(&Inbound::Http {
                path: "/active",
                headers: &[],
                body,
                request: None
            }),
            Some(json!({"p": {"groups": {
                "0": {"slides": {"1": {"text": "a"}, "2": {"text": "b"}}},
                "2": {"slides": {"1": {"text": "c"}}}}}}))
        );
        let event = json!({"event": "update", "data": {"projects": {
            "ABCD": {"cues": [{"id": "lt"}, {"id": "sc", "scores": [
                [{"score": 3}, {"score": 1}], [{"score": 7}]]}]},
            "EFGH": {"cues": [{"id": "s2", "scores": [[{"score": 9}]]}]}}}});
        assert_eq!(
            t.apply(&Inbound::Json(&event)),
            Some(json!({
                "scores": {
                    "ABCD": {"sc": {"1": {"1": 3, "2": 1}, "2": {"1": 7}}},
                    "EFGH": {"s2": {"1": {"1": 9}}}},
                "cue_place": {"sc": 1, "s2": 0}}))
        );
    }

    #[test]
    fn xml_elements_give_their_text_children_and_ancestors() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"path": "^/version$", "xml_each": "product_information",
                 "state": {"product.model": "{product_model}", "product.width": "{session_x_resolution}"}},
                {"path": "^/buffers$", "xml_each": "buffer",
                 "state": {"me.{^^index}.buffer": "{selection}"}},
                {"path": "^/names$", "xml_each": "name",
                 "state": {"names.{id}": "{text}"}},
            ]})),
            &state(json!({
                "product.model": {"type": "string", "description": "x"},
                "product.width": {"type": "int", "description": "x"},
                "me.*.buffer": {"type": "string", "description": "x"},
                "names.*": {"type": "string", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let reply = |path: &'static str, body: &'static str| {
            t.apply(&Inbound::Http {
                path,
                headers: &[],
                body: body.as_bytes(),
                request: None,
            })
        };
        assert_eq!(
            reply(
                "/version",
                "<product_information><product_model> TC1 </product_model>\
                   <session_x_resolution>1920</session_x_resolution></product_information>"
            ),
            Some(json!({"product": {"model": "TC1", "width": 1920}}))
        );
        assert_eq!(
            reply(
                "/buffers",
                r#"<buffers><main><buffer selection="BFR4"/></main><me index="0"><row><buffer selection="BFR1"/></row></me><me index="1"><row><buffer selection="BFR2"/></row></me></buffers>"#
            ),
            Some(json!({"me": {"0": {"buffer": "BFR1"}, "1": {"buffer": "BFR2"}}}))
        );
        // An attribute keeps its name over the element's text.
        assert_eq!(
            reply(
                "/names",
                r#"<n><name id="a">Cam &amp; 1</name><name id="b" text="x">y</name></n>"#
            ),
            Some(json!({"names": {"a": "Cam & 1", "b": "x"}}))
        );
    }

    #[test]
    fn an_xml_each_path_names_the_enclosing_elements() {
        // ONVIF's PTZ status and its presets both hold tt:PanTilt elements:
        // the path tells the status's position from a preset's.
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"path": "^/onvif/ptz$", "xml_each": "PTZStatus/Position/PanTilt",
                 "state": {"position.pan": "{x}"}},
                {"path": "^/onvif/ptz$", "xml_each": "Preset/PTZPosition/PanTilt",
                 "state": {"presets.{^^token}.pan": "{x}"}},
            ]})),
            &state(json!({
                "position.pan": {"type": "float", "description": "x"},
                "presets.*.pan": {"type": "float", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let reply = |body: &'static str| {
            t.apply(&Inbound::Http {
                path: "/onvif/ptz",
                headers: &[],
                body: body.as_bytes(),
                request: None,
            })
        };
        assert_eq!(
            reply(
                r#"<s:Envelope xmlns:s="s" xmlns:tt="t" xmlns:tptz="p"><s:Body><tptz:GetStatusResponse><tptz:PTZStatus><tt:Position><tt:PanTilt x="0.5" y="0"/></tt:Position><tt:MoveStatus><tt:PanTilt>IDLE</tt:PanTilt></tt:MoveStatus></tptz:PTZStatus></tptz:GetStatusResponse></s:Body></s:Envelope>"#
            ),
            Some(json!({"position": {"pan": 0.5}}))
        );
        assert_eq!(
            reply(
                r#"<s:Envelope xmlns:s="s" xmlns:tt="t" xmlns:tptz="p"><s:Body><tptz:GetPresetsResponse><tptz:Preset token="1"><tt:Name>A</tt:Name><tt:PTZPosition><tt:PanTilt x="-0.25" y="0"/></tt:PTZPosition></tptz:Preset></tptz:GetPresetsResponse></s:Body></s:Envelope>"#
            ),
            Some(json!({"presets": {"1": {"pan": -0.25}}}))
        );
        // A namespace tells apart elements of one name from two services.
        let t2 = Telemetry::parse(
            Some(&json!({"updates": [
                {"path": "^/x$", "xml_each": "{urn:img}GetPresetsResponse/Preset",
                 "state": {"imaging.{token}": "{Name}"}},
            ]})),
            &state(json!({"imaging.*": {"type": "string", "description": "x"}})),
            Conversions::new(),
        )
        .unwrap();
        let body = |b: &'static str| Inbound::Http {
            path: "/x",
            headers: &[],
            body: b.as_bytes(),
            request: None,
        };
        assert_eq!(
            t2.apply(&body(
                r#"<a:GetPresetsResponse xmlns:a="urn:img" xmlns:t="t"><a:Preset token="p1"><t:Name>Day</t:Name></a:Preset></a:GetPresetsResponse>"#
            )),
            Some(json!({"imaging": {"p1": "Day"}}))
        );
        assert_eq!(
            t2.apply(&body(
                r#"<a:GetPresetsResponse xmlns:a="urn:ptz"><a:Preset token="p1"><Name>Home</Name></a:Preset></a:GetPresetsResponse>"#
            )),
            None
        );
        // A path that does not fit the element's enclosing elements matches nothing.
        assert_eq!(
            reply("<Position><Other><PanTilt x=\"1\"/></Other></Position>"),
            None
        );
    }

    #[test]
    fn a_json_each_path_can_name_a_setting() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"json_match": {"$.event": "^update$"}, "json_each": "$.projects.{settings.project}.cues",
                 "json": {"id": "$.id"}, "replace": "cues", "state": {"cues.{id}.index": "{index}"}},
            ]})),
            &state(json!({"cues.*.index": {"type": "int", "description": "x"}})),
            Conversions::new(),
        )
        .unwrap();
        let event = json!({"event": "update", "projects": {
            "ABCD": {"cues": [{"id": "a"}]}, "EFGH": {"cues": [{"id": "e"}, {"id": "f"}]}}});
        let apply = |project: Value| {
            let mut settings = Params::new();
            settings.insert("project".into(), project);
            let mut cleared = json!({});
            let patch = t.apply_with(
                &Inbound::Json(&event),
                &settings,
                &mut Vec::new(),
                &mut cleared,
            );
            (cleared, patch)
        };
        assert_eq!(
            apply(json!("EFGH")).1,
            Some(json!({"cues": {"e": {"index": 0}, "f": {"index": 1}}}))
        );
        assert_eq!(
            apply(json!("ABCD")).1,
            Some(json!({"cues": {"a": {"index": 0}}}))
        );
        // A project the message does not hold, or a value that would be
        // another path, gives no element; the list is still replaced.
        for project in [json!("WXYZ"), json!(""), json!("AB.CD"), json!("*")] {
            assert_eq!(apply(project), (json!({"cues": null}), None));
        }
        assert_eq!(each_path("$.a.*", &Params::new()).as_deref(), Some("$.a.*"));
        assert_eq!(each_path("$.a.{settings.x}", &Params::new()), None);
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
    fn a_text_reply_is_matched_with_its_path() {
        let t = Telemetry::parse(
            Some(&json!({"updates": [
                {"path": "^/cgi-bin/get_(rtmp|srt)_status$", "match": "(?m)^status\\s*=\\s*([01])$",
                 "state": {"{1}.streaming": {"value": "{2}", "map": {"0": false, "1": true}}}},
            ]})),
            &state(json!({
                "rtmp.streaming": {"type": "bool", "description": "x"},
                "srt.streaming": {"type": "bool", "description": "x"},
            })),
            Conversions::new(),
        )
        .unwrap();
        let reply = |path: &'static str, body: &'static [u8]| {
            t.apply(&Inbound::Http {
                path,
                headers: &[],
                body,
                request: None,
            })
        };
        assert_eq!(
            reply("/cgi-bin/get_srt_status", b"status=1\r\n"),
            Some(json!({"srt": {"streaming": true}}))
        );
        assert_eq!(
            reply("/cgi-bin/get_rtmp_status", b"status = 0\r\n"),
            Some(json!({"rtmp": {"streaming": false}}))
        );
        // The same text from another path is not this rule's, and a plain
        // text message never reaches a path rule.
        assert_eq!(reply("/cgi-bin/get_ts_status", b"status=1\r\n"), None);
        assert_eq!(t.apply(&Inbound::Text("status=1")), None);
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
