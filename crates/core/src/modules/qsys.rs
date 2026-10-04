//! QSC Q-SYS Cores and Q-SYS Designer in Emulate mode, over the Q-SYS Remote
//! Control protocol (QRC).
//!
//! Written from QSC's own Q-SYS Help, version 10.5.0 (see the spec's sources
//! for the pages): "Q-SYS Remote Control Protocol (QRC)", "QRC Commands",
//! "Q-SYS PA Remote API (PARAPI)", "Change Groups for ECP and QRC", and the
//! knowledge-base article "How To | Formatting a QRC command".
//!
//! - TCP port 1710 to a Core running a design, or to Q-SYS Designer with the
//!   design open in Emulate mode (QRC Overview, "Connections").
//! - Every message is a JSON-RPC 2.0 object terminated by a NUL byte; `id`
//!   must be a number and is copied into the reply (QRC Commands, intro and
//!   "Formatting Requirements"; KB article).
//! - The Core closes a connection it has not heard from for 60 seconds; NoOp
//!   exists to keep it open (QRC Overview note; NoOp).
//! - Where Access Control is on, the client logs on with `Logon` {User,
//!   Password}, the user and PIN created in Q-SYS Administrator; a command
//!   without it fails with error 10, "Logon required" (Error Code Reference).
//! - The Core pushes `EngineStatus` on connecting and whenever the status
//!   changes; `StatusGet` asks for it, with the Core model as `Platform`.
//! - Change groups (at most four per connection) collect controls; a poll
//!   returns what changed since the last one, everything on the first poll,
//!   and `ChangeGroup.AutoPoll` makes the Core poll on its own. The Core
//!   deletes them when the connection goes, so they are created again after
//!   every reconnection ("Change Groups for ECP and QRC").
//!
//! State is the engine status, the values of every control a change group,
//! `Control.Get`, `Component.Get`, `Component.GetControls` or a
//! `Component.Set` with `ResponseValues` reports, PA zone and page status
//! notifications, and the last Loop Player error.
//!
//! Opened for commands only (`monitor` false), the module creates, polls and
//! recreates no change group of its own: change-group commands the consumer
//! sends go to the Core as sent, without the default AutoPoll and without
//! being rebuilt after a reconnection. StatusGet on connecting and the
//! keepalive stay, as they are how the module knows the Core is there.

use std::collections::BTreeMap;
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};

pub(crate) const DEFAULT_PORT: u16 = 1710;
const SOCKET: Key = "qrc";

/// A reply within this, or the request has failed.
const REPLY_TIMEOUT: Millis = 10_000;
/// The Core drops a client silent for 60 s; something is sent at least this
/// often, and a Core quiet this long is asked for its status.
const KEEPALIVE_EVERY: Millis = 15_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
/// A message without its end in this many bytes is not QRC.
const MAX_BUFFER: usize = 16 * 1024 * 1024;
/// The change group used when a command names none.
pub(crate) const DEFAULT_GROUP: &str = "meros";
/// AutoPoll rate, in seconds, given to a change group the first time controls
/// are added to it, unless the operator sets another (0: never).
const DEFAULT_AUTO_POLL: f64 = 0.25;

const REPLY: Key = "reply";
const KEEPALIVE: Key = "keepalive";
const RETRY: Key = "retry";

/// The Error Code Reference of "QRC Commands", and PARAPI's codes 1-4 where
/// they differ.
fn error_meaning(code: i64) -> Option<&'static str> {
    Some(match code {
        -32700 => "Parse error. Invalid JSON was received by the server",
        -32600 => "Invalid request",
        -32601 => "Method not found",
        -32602 => "Invalid params",
        -32603 => "Server error",
        -32604 => "Core is on Standby (not the active Core of a redundant pair)",
        1 => "Bad Parameters",
        2 => "Invalid Page Request ID",
        3 => "Bad Page Request - could not create the requested Page Request",
        4 => "Missing file",
        5 => "Change Groups exhausted",
        6 => "Unknown change group",
        7 => "Unknown component name",
        8 => "Unknown control",
        9 => "Illegal mixer channel index",
        10 => "Logon required",
        _ => return None,
    })
}

const LOGON_REQUIRED: i64 = 10;

/// A JSON-RPC request, keys in the order of QSC's examples.
pub(crate) fn request(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

/// A message as sent: compact JSON and the NUL terminator.
pub(crate) fn frame(message: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(message).expect("a JSON value serialises");
    bytes.push(0);
    bytes
}

/// Splits the Core's byte stream into JSON messages. A NUL ends a message;
/// one that arrives without it is taken when its JSON is complete, since the
/// documents say commands, not replies, are NUL-terminated.
#[derive(Debug, Default)]
pub(crate) struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    /// The complete messages, and why any bytes were discarded.
    pub(crate) fn feed(&mut self, data: &[u8]) -> (Vec<Value>, Vec<String>) {
        self.buf.extend_from_slice(data);
        let mut messages = Vec::new();
        let mut discarded = Vec::new();
        let mut at = 0;
        loop {
            while at < self.buf.len() && (self.buf[at] == 0 || self.buf[at].is_ascii_whitespace()) {
                at += 1;
            }
            if at >= self.buf.len() {
                break;
            }
            let mut stream = serde_json::Deserializer::from_slice(&self.buf[at..]).into_iter();
            match stream.next() {
                Some(Ok(value)) => {
                    at += stream.byte_offset();
                    messages.push(value);
                }
                Some(Err(e)) if e.is_eof() => break,
                Some(Err(e)) => {
                    discarded.push(e.to_string());
                    at = match self.buf[at..].iter().position(|&b| b == 0) {
                        Some(nul) => at + nul + 1,
                        None => self.buf.len(),
                    };
                }
                None => break,
            }
        }
        self.buf.drain(..at);
        if self.buf.len() > MAX_BUFFER {
            self.buf.clear();
            discarded.push(format!("no complete message in {MAX_BUFFER} bytes"));
        }
        (messages, discarded)
    }
}

// --- Commands --------------------------------------------------------------

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn text<'a>(p: &'a Params, key: &str) -> Result<&'a str, CommandError> {
    p.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn group(p: &Params) -> String {
    p.get("group")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_GROUP)
        .to_string()
}

/// A non-empty JSON array of strings.
fn names(p: &Params, key: &str) -> Result<Vec<String>, CommandError> {
    let list = p
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("'{key}' must be an array of names")))?;
    if list.is_empty() {
        return Err(invalid(format!("'{key}' is empty")));
    }
    list.iter()
        .map(|v| {
            v.as_str()
                .map(str::to_string)
                .ok_or_else(|| invalid(format!("'{key}' must contain only strings")))
        })
        .collect()
}

/// A non-empty JSON array of integers.
fn integers(p: &Params, key: &str) -> Result<Value, CommandError> {
    match p.get(key).and_then(Value::as_array) {
        Some(list) if !list.is_empty() && list.iter().all(|v| v.as_i64().is_some()) => {
            Ok(Value::Array(list.clone()))
        }
        _ => Err(invalid(format!(
            "'{key}' must be a non-empty array of integers"
        ))),
    }
}

/// A control's new value: a number, a string or a boolean ("Value : The value
/// of the control. This can be a number, string, or boolean.").
fn control_value(p: &Params) -> Result<Value, CommandError> {
    let given: Vec<&Value> = ["value", "value_string", "value_bool"]
        .iter()
        .filter_map(|k| p.get(*k))
        .collect();
    match given.as_slice() {
        [v] => Ok((*v).clone()),
        [] => Err(invalid(
            "give one of 'value', 'value_string' or 'value_bool'",
        )),
        _ => Err(invalid(
            "give only one of 'value', 'value_string' or 'value_bool'",
        )),
    }
}

/// Copy `from` into `to` as `key` when the caller gave it.
fn put(to: &mut Map<String, Value>, key: &str, p: &Params, from: &str) {
    if let Some(v) = p.get(from) {
        to.insert(key.into(), v.clone());
    }
}

/// How a change group is edited once the Core accepts a command.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Edit {
    Add {
        group: String,
        controls: Vec<String>,
    },
    AddComponent {
        group: String,
        component: String,
        controls: Vec<String>,
    },
    Remove {
        group: String,
        controls: Vec<String>,
    },
    AutoPoll {
        group: String,
        rate: Value,
    },
    Clear {
        group: String,
    },
    Destroy {
        group: String,
    },
}

/// What a command sends and how it completes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Plan {
    pub method: String,
    pub params: Value,
    /// Complete with the reply's result rather than a plain acknowledgement.
    pub value: bool,
    pub edit: Option<Edit>,
}

impl Plan {
    fn new(method: &str, params: Value, value: bool) -> Plan {
        Plan {
            method: method.into(),
            params,
            value,
            edit: None,
        }
    }

    fn ack(method: &str, params: Value) -> Plan {
        Plan::new(method, params, false)
    }

    fn value(method: &str, params: Value) -> Plan {
        Plan::new(method, params, true)
    }

    fn edit(mut self, edit: Edit) -> Plan {
        self.edit = Some(edit);
        self
    }
}

/// The mixer methods: QRC method, the string-specification parameters it
/// takes in the documented order, whether Value is a boolean, and whether it
/// takes a Ramp.
const MIXER: &[(&str, &str, &[&str], bool, bool)] = &[
    (
        "mixer_set_crosspoint_gain",
        "Mixer.SetCrossPointGain",
        &["inputs", "outputs"],
        false,
        true,
    ),
    (
        "mixer_set_crosspoint_delay",
        "Mixer.SetCrossPointDelay",
        &["inputs", "outputs"],
        false,
        true,
    ),
    (
        "mixer_set_crosspoint_mute",
        "Mixer.SetCrossPointMute",
        &["inputs", "outputs"],
        true,
        false,
    ),
    (
        "mixer_set_crosspoint_solo",
        "Mixer.SetCrossPointSolo",
        &["inputs", "outputs"],
        true,
        false,
    ),
    (
        "mixer_set_input_gain",
        "Mixer.SetInputGain",
        &["inputs"],
        false,
        true,
    ),
    (
        "mixer_set_input_mute",
        "Mixer.SetInputMute",
        &["inputs"],
        true,
        false,
    ),
    (
        "mixer_set_input_solo",
        "Mixer.SetInputSolo",
        &["inputs"],
        true,
        false,
    ),
    (
        "mixer_set_output_gain",
        "Mixer.SetOutputGain",
        &["outputs"],
        false,
        true,
    ),
    (
        "mixer_set_output_mute",
        "Mixer.SetOutputMute",
        &["outputs"],
        true,
        false,
    ),
    (
        "mixer_set_cue_mute",
        "Mixer.SetCueMute",
        &["cues"],
        true,
        false,
    ),
    (
        "mixer_set_cue_gain",
        "Mixer.SetCueGain",
        &["cues"],
        false,
        true,
    ),
    (
        "mixer_set_input_cue_enable",
        "Mixer.SetInputCueEnable",
        &["cues", "inputs"],
        true,
        false,
    ),
    (
        "mixer_set_input_cue_afl",
        "Mixer.SetInputCueAfl",
        &["cues", "inputs"],
        true,
        false,
    ),
];

fn capitalised(key: &str) -> String {
    let mut c = key.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

fn mixer(name: &str, p: &Params) -> Option<Result<Plan, CommandError>> {
    let &(_, method, selectors, boolean, ramp) = MIXER.iter().find(|m| m.0 == name)?;
    Some((|| {
        let mut params = Map::new();
        params.insert("Name".into(), json!(text(p, "mixer")?));
        for key in selectors {
            params.insert(capitalised(key), json!(text(p, key)?));
        }
        let value = p
            .get("value")
            .cloned()
            .ok_or_else(|| invalid("'value' is required"))?;
        if boolean != value.is_boolean() {
            return Err(invalid("'value' has the wrong type for this method"));
        }
        params.insert("Value".into(), value);
        if ramp {
            put(&mut params, "Ramp", p, "ramp");
        }
        Ok(Plan::ack(method, Value::Object(params)))
    })())
}

/// The PA.PageSubmit parameters, in the order of PARAPI's table.
const PAGE_SUBMIT: &[(&str, &str)] = &[
    ("Mode", "mode"),
    ("Description", "description"),
    ("Zones", "zones"),
    ("ZoneTags", "zone_tags"),
    ("Priority", "priority"),
    ("Preamble", "preamble"),
    ("Message", "message"),
    ("MessageDelete", "message_delete"),
    ("Station", "station"),
    ("Start", "start"),
    ("QueueTimeout", "queue_timeout"),
    ("Archive", "archive"),
    ("Split", "split"),
    ("RetryCount", "retry_count"),
    ("MaxPageTime", "max_page_time"),
    ("CancelDelay", "cancel_delay"),
    ("Originator", "originator"),
    ("SplitOnInterrupt", "split_on_interrupt"),
];

fn page_submit(p: &Params) -> Result<Plan, CommandError> {
    let mode = text(p, "mode")?;
    if p.get("zones").is_none() && p.get("zone_tags").is_none() {
        return Err(invalid("give 'zones', 'zone_tags' or both"));
    }
    if let Some(zones) = p.get("zones") {
        let ok = zones.as_array().is_some_and(|z| {
            !z.is_empty() && z.iter().all(|v| v.is_string() || v.as_i64().is_some())
        });
        if !ok {
            return Err(invalid(
                "'zones' must be a non-empty array of zone numbers and names",
            ));
        }
    }
    if p.get("zone_tags").is_some() {
        names(p, "zone_tags")?;
    }
    if mode == "message" && p.get("message").is_none() {
        return Err(invalid("a message page needs 'message'"));
    }
    if mode != "message" && p.get("station").is_none() {
        return Err(invalid("a voice page needs 'station'"));
    }
    let mut params = Map::new();
    for (key, from) in PAGE_SUBMIT {
        put(&mut params, key, p, from);
    }
    Ok(Plan::value("PA.PageSubmit", Value::Object(params)))
}

fn loop_player(method: &str, p: &Params) -> Result<Plan, CommandError> {
    let mut params = Map::new();
    if method == "LoopPlayer.Start" {
        let files = match (p.get("files"), p.get("file"), p.get("output")) {
            (Some(files), None, None) => {
                let ok = files.as_array().is_some_and(|f| {
                    !f.is_empty()
                        && f.iter().all(|x| {
                            x.get("Name").is_some_and(Value::is_string)
                                && x.get("Output").is_some_and(|o| o.as_i64().is_some())
                        })
                });
                if !ok {
                    return Err(invalid(
                        "'files' must be a non-empty array of {\"Name\": path, \"Output\": number}",
                    ));
                }
                files.clone()
            }
            (None, Some(file), Some(output)) => json!([{"Name": file, "Output": output}]),
            _ => return Err(invalid("give 'files', or 'file' with 'output'")),
        };
        params.insert("Files".into(), files);
        params.insert("Name".into(), json!(text(p, "name")?));
        put(&mut params, "StartTime", p, "start_time");
        put(&mut params, "Loop", p, "loop");
        put(&mut params, "Log", p, "log");
        put(&mut params, "RefID", p, "ref_id");
        put(&mut params, "Seek", p, "seek");
    } else {
        params.insert("Name".into(), json!(text(p, "name")?));
        params.insert("Outputs".into(), integers(p, "outputs")?);
        put(&mut params, "Log", p, "log");
        put(&mut params, "RefID", p, "ref_id");
    }
    Ok(Plan::ack(method, Value::Object(params)))
}

/// A command to the request that carries it. Parameters have already been
/// checked against the spec; what the spec cannot express is checked here.
pub(crate) fn plan(name: &str, p: &Params) -> Result<Plan, CommandError> {
    if let Some(plan) = mixer(name, p) {
        return plan;
    }
    let page = |method: &str| -> Result<Plan, CommandError> {
        let id = p
            .get("page_id")
            .cloned()
            .ok_or_else(|| invalid("'page_id' is required"))?;
        Ok(Plan::ack(method, json!({"PageID": id})))
    };
    Ok(match name {
        "status_get" => Plan::value("StatusGet", json!(0)),
        "no_op" => Plan::ack("NoOp", json!({})),
        "control_get" => Plan::value("Control.Get", json!(names(p, "controls")?)),
        "control_set" => {
            let mut params = Map::new();
            params.insert("Name".into(), json!(text(p, "name")?));
            params.insert("Value".into(), control_value(p)?);
            put(&mut params, "Ramp", p, "ramp");
            Plan::ack("Control.Set", Value::Object(params))
        }
        "component_get" => {
            let controls: Vec<Value> = names(p, "controls")?
                .into_iter()
                .map(|n| json!({"Name": n}))
                .collect();
            Plan::value(
                "Component.Get",
                json!({"Name": text(p, "component")?, "Controls": controls}),
            )
        }
        "component_get_controls" => Plan::value(
            "Component.GetControls",
            json!({"Name": text(p, "component")?}),
        ),
        "component_get_components" => Plan::value("Component.GetComponents", json!({})),
        "component_set" | "component_set_controls" => {
            let controls = if name == "component_set" {
                let mut control = Map::new();
                control.insert("Name".into(), json!(text(p, "control")?));
                control.insert("Value".into(), control_value(p)?);
                put(&mut control, "Ramp", p, "ramp");
                json!([control])
            } else {
                let given = p.get("controls").cloned().unwrap_or(Value::Null);
                let ok = given.as_array().is_some_and(|c| {
                    !c.is_empty()
                        && c.iter().all(|x| {
                            x.get("Name").is_some_and(Value::is_string) && x.get("Value").is_some()
                        })
                });
                if !ok {
                    return Err(invalid(
                        "'controls' must be a non-empty array of {\"Name\", \"Value\", \"Ramp\"?}",
                    ));
                }
                given
            };
            let mut params = Map::new();
            params.insert("Name".into(), json!(text(p, "component")?));
            if p.get("response_values").and_then(Value::as_bool) == Some(true) {
                params.insert("ResponseValues".into(), json!(true));
            }
            params.insert("Controls".into(), controls);
            Plan::value("Component.Set", Value::Object(params))
        }
        "change_group_add_controls" => {
            let (group, controls) = (group(p), names(p, "controls")?);
            Plan::ack(
                "ChangeGroup.AddControl",
                json!({"Id": group, "Controls": controls}),
            )
            .edit(Edit::Add { group, controls })
        }
        "change_group_add_component_controls" => {
            let (group, component) = (group(p), text(p, "component")?.to_string());
            let controls = names(p, "controls")?;
            let list: Vec<Value> = controls.iter().map(|n| json!({"Name": n})).collect();
            Plan::ack(
                "ChangeGroup.AddComponentControl",
                json!({"Id": group, "Component": {"Name": component, "Controls": list}}),
            )
            .edit(Edit::AddComponent {
                group,
                component,
                controls,
            })
        }
        "change_group_remove" => {
            let (group, controls) = (group(p), names(p, "controls")?);
            Plan::value(
                "ChangeGroup.Remove",
                json!({"Id": group, "Controls": controls}),
            )
            .edit(Edit::Remove { group, controls })
        }
        "change_group_poll" => Plan::value("ChangeGroup.Poll", json!({"Id": group(p)})),
        "change_group_auto_poll" => {
            let rate = p
                .get("rate")
                .cloned()
                .ok_or_else(|| invalid("'rate' is required"))?;
            if rate.as_f64().is_none_or(|r| r <= 0.0) {
                return Err(invalid("'rate' must be above 0 seconds"));
            }
            let group = group(p);
            Plan::ack("ChangeGroup.AutoPoll", json!({"Id": group, "Rate": rate}))
                .edit(Edit::AutoPoll { group, rate })
        }
        "change_group_clear" => Plan::ack("ChangeGroup.Clear", json!({"Id": group(p)}))
            .edit(Edit::Clear { group: group(p) }),
        "change_group_destroy" => Plan::ack("ChangeGroup.Destroy", json!({"Id": group(p)}))
            .edit(Edit::Destroy { group: group(p) }),
        "change_group_invalidate" => Plan::ack("ChangeGroup.Invalidate", json!({"Id": group(p)})),
        "loop_player_start" => loop_player("LoopPlayer.Start", p)?,
        "loop_player_stop" => loop_player("LoopPlayer.Stop", p)?,
        "loop_player_cancel" => loop_player("LoopPlayer.Cancel", p)?,
        "snapshot_load" | "snapshot_save" => {
            // The bank's name is "Name"; the snapshot's number is "Bank".
            let mut params = Map::new();
            params.insert("Name".into(), json!(text(p, "bank")?));
            put(&mut params, "Bank", p, "snapshot");
            if name == "snapshot_load" {
                put(&mut params, "Ramp", p, "ramp");
                Plan::ack("Snapshot.Load", Value::Object(params))
            } else {
                Plan::ack("Snapshot.Save", Value::Object(params))
            }
        }
        "pa_page_submit" => page_submit(p)?,
        "pa_page_start" => page("PA.PageStart")?,
        "pa_page_stop" => page("PA.PageStop")?,
        "pa_page_cancel" => page("PA.PageCancel")?,
        "pa_page_proceed" => page("PA.PageProceed")?,
        "pa_zone_status_configure" => Plan::ack(
            "PA.ZoneStatusConfigure",
            json!({"Enabled": p.get("enabled").cloned().unwrap_or(json!(true))}),
        ),
        "pa_get_config" => Plan::value("PA.GetConfig", json!({})),
        "pa_source_watchdog" => Plan::ack(
            "PA.SourceWatchdog",
            json!({"RemoteSystem": text(p, "remote_system")?}),
        ),
        "raw_rpc" => {
            let method = text(p, "method")?;
            if method.is_empty() {
                return Err(invalid("'method' is empty"));
            }
            Plan::value(method, p.get("params").cloned().unwrap_or(json!({})))
        }
        other => {
            return Err(CommandError::UnknownCommand {
                command: other.into(),
            })
        }
    })
}

// --- State -----------------------------------------------------------------

/// Insert `value` at `path` in a merge patch, creating objects on the way.
fn insert(root: &mut Map<String, Value>, path: &[&str], value: Value) {
    let (last, parents) = path.split_last().expect("a path has a key");
    let mut node = root;
    for key in parents {
        let child = node
            .entry(key.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !child.is_object() {
            *child = Value::Object(Map::new());
        }
        node = child.as_object_mut().expect("just made an object");
    }
    match (node.get_mut(*last), value) {
        (Some(Value::Object(existing)), Value::Object(more)) => existing.extend(more),
        (_, value) => {
            node.insert(last.to_string(), value);
        }
    }
}

/// One control's reported fields, under the state's names.
const CONTROL_FIELDS: &[(&str, &str)] = &[
    ("Value", "value"),
    ("String", "string"),
    ("Position", "position"),
    ("Type", "type"),
    ("Direction", "direction"),
    ("ValueMin", "value_min"),
    ("ValueMax", "value_max"),
    ("StringMin", "string_min"),
    ("StringMax", "string_max"),
];

fn control_record(o: &Map<String, Value>) -> Value {
    let mut record = Map::new();
    for (from, to) in CONTROL_FIELDS {
        if let Some(v) = o.get(*from) {
            record.insert(to.to_string(), v.clone());
        }
    }
    Value::Object(record)
}

/// Controls as QRC reports them, each `{Name, Value, String, Position…}`,
/// with `Component` set when it belongs to a named component (or `component`
/// given for all of them), to a state patch.
pub(crate) fn controls_patch(entries: &[Value], component: Option<&str>) -> Option<Value> {
    let mut root = Map::new();
    for entry in entries {
        let Some(o) = entry.as_object() else { continue };
        let Some(name) = o.get("Name").and_then(Value::as_str) else {
            continue;
        };
        let record = control_record(o);
        match o.get("Component").and_then(Value::as_str).or(component) {
            Some(c) => insert(&mut root, &["components", c, name], record),
            None => insert(&mut root, &["controls", name], record),
        }
    }
    (!root.is_empty()).then_some(Value::Object(root))
}

/// EngineStatus params or the StatusGet result to a state patch.
pub(crate) fn engine_patch(o: &Map<String, Value>) -> Value {
    let mut engine = Map::new();
    for (from, to) in [
        ("Platform", "platform"),
        ("State", "state"),
        ("DesignName", "design_name"),
        ("DesignCode", "design_code"),
        ("IsRedundant", "is_redundant"),
        ("IsEmulator", "is_emulator"),
    ] {
        if let Some(v) = o.get(from) {
            engine.insert(to.into(), v.clone());
        }
    }
    if let Some(status) = o.get("Status").and_then(Value::as_object) {
        let mut s = Map::new();
        if let Some(code) = status.get("Code") {
            s.insert("code".into(), code.clone());
        }
        if let Some(text) = status.get("String") {
            s.insert("string".into(), text.clone());
        }
        engine.insert("status".into(), Value::Object(s));
    }
    json!({ "engine": engine })
}

/// A result's controls, by the method that asked for them.
fn result_patch(method: &str, result: &Value) -> Option<Value> {
    if let Some(changes) = result.get("Changes").and_then(Value::as_array) {
        return controls_patch(changes, None);
    }
    match method {
        "Control.Get" => controls_patch(result.as_array()?, None),
        "Component.Set" => controls_patch(result.as_array()?, None),
        "Component.Get" | "Component.GetControls" => controls_patch(
            result.get("Controls")?.as_array()?,
            Some(result.get("Name")?.as_str()?),
        ),
        _ => None,
    }
}

/// A key for a number or string identifier, as a state path segment.
fn key(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// PA.PageStatus params to a state patch.
fn page_patch(o: &Map<String, Value>) -> Option<Value> {
    let page = key(o.get("PageID")?)?;
    let mut record = Map::new();
    for (from, to) in [
        ("State", "state"),
        ("SubState", "sub_state"),
        ("Message", "message"),
        ("Count", "count"),
    ] {
        record.insert(to.into(), o.get(from).cloned().unwrap_or(Value::Null));
    }
    Some(json!({"pa": {"pages": {page: record}}}))
}

/// PA.ZoneStatus params to a state patch. Fields the notification leaves out
/// are cleared: an inactive zone is reported without its page.
fn zone_patch(o: &Map<String, Value>) -> Option<Value> {
    let zone = key(o.get("Zone")?)?;
    let mut record = Map::new();
    for (from, to) in [
        ("Active", "active"),
        ("Time", "time"),
        ("PageID", "page_id"),
        ("Priority", "priority"),
        ("PriorityName", "priority_name"),
        ("Station", "station"),
    ] {
        record.insert(to.into(), o.get(from).cloned().unwrap_or(Value::Null));
    }
    Some(json!({"pa": {"zones": {zone: record}}}))
}

/// The request id a reply carries: a number, or a string holding one (the
/// PARAPI examples use strings).
fn reply_id(v: &Value) -> Option<u64> {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

/// A JSON-RPC error object to its code and a message.
fn error_parts(e: &Value) -> (Option<i64>, String) {
    let code = e.get("code").and_then(Value::as_i64);
    let message = e
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| e.as_str().map(str::to_string))
        .or_else(|| code.and_then(error_meaning).map(str::to_string))
        .unwrap_or_else(|| e.to_string());
    (code, message)
}

// --- Session ---------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Purpose {
    Logon,
    /// StatusGet: on connecting, and when the Core has been quiet.
    Status,
    /// Change group upkeep and keepalives; failures are logged.
    Internal,
    Command {
        id: CommandId,
        value: bool,
    },
}

#[derive(Debug)]
struct Pending {
    purpose: Purpose,
    method: String,
    sent_at: Millis,
    deadline: Millis,
    edit: Option<Edit>,
}

/// What a change group holds, so it can be made again after reconnecting.
#[derive(Debug, Default, Clone, PartialEq)]
struct Group {
    controls: Vec<String>,
    components: BTreeMap<String, Vec<String>>,
    rate: Option<Value>,
}

pub(crate) struct Qsys {
    device: SocketAddr,
    username: String,
    pin: String,
    auto_poll: f64,
    deframer: Deframer,
    socket_open: bool,
    ready: bool,
    refused: Option<String>,
    next_id: u64,
    pending: BTreeMap<u64, Pending>,
    groups: BTreeMap<String, Group>,
    design_code: Option<String>,
    last_sent: Millis,
    last_heard: Millis,
    retry_after: Millis,
    /// False: commands only, no change groups of the module's own.
    monitor: bool,
}

fn setting<'a>(settings: &'a Params, key: &str) -> &'a str {
    settings.get(key).and_then(Value::as_str).unwrap_or("")
}

impl Qsys {
    pub(crate) fn new(ctx: OpenContext) -> Qsys {
        Qsys {
            device: SocketAddr::new(ctx.host, ctx.port.unwrap_or(DEFAULT_PORT)),
            username: setting(&ctx.settings, "username").to_string(),
            pin: setting(&ctx.settings, "pin").to_string(),
            auto_poll: ctx
                .settings
                .get("auto_poll_rate")
                .and_then(Value::as_f64)
                .unwrap_or(DEFAULT_AUTO_POLL),
            deframer: Deframer::default(),
            socket_open: false,
            ready: false,
            refused: None,
            next_id: 1,
            pending: BTreeMap::new(),
            groups: BTreeMap::new(),
            design_code: None,
            last_sent: 0,
            last_heard: 0,
            retry_after: RETRY_MIN,
            monitor: ctx.monitor,
        }
    }

    fn open(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.device);
    }

    fn send(
        &mut self,
        cx: &mut Cx,
        method: &str,
        params: Value,
        purpose: Purpose,
        edit: Option<Edit>,
    ) {
        let id = self.next_id;
        self.next_id += 1;
        cx.tcp_send(SOCKET, frame(&request(id, method, params)));
        self.last_sent = cx.now();
        self.pending.insert(
            id,
            Pending {
                purpose,
                method: method.into(),
                sent_at: cx.now(),
                deadline: cx.now() + REPLY_TIMEOUT,
                edit,
            },
        );
        self.arm_reply_timer(cx);
    }

    fn arm_reply_timer(&self, cx: &mut Cx) {
        match self.pending.values().map(|p| p.deadline).min() {
            Some(at) => cx.set_timer(REPLY, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REPLY),
        }
    }

    /// End the connection: fail commands still waiting with `error` and
    /// forget what the Core held for it.
    fn drop_connection(&mut self, cx: &mut Cx, error: CommandError) {
        for (_, p) in std::mem::take(&mut self.pending) {
            if let Purpose::Command { id, .. } = p.purpose {
                cx.complete(id, Err(error.clone()));
            }
        }
        for key in [REPLY, KEEPALIVE] {
            cx.cancel_timer(key);
        }
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.ready = false;
        self.deframer = Deframer::default();
    }

    /// The connection failed or ended: try again, backing off to 30 s.
    fn lost(&mut self, cx: &mut Cx, reason: String) {
        self.drop_connection(
            cx,
            CommandError::Transport {
                message: reason.clone(),
            },
        );
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    /// The Core refused the logon, or needs one and none is configured. Not
    /// retried until the device is opened again with corrected settings.
    fn refuse(&mut self, cx: &mut Cx, reason: String) {
        self.drop_connection(
            cx,
            CommandError::Auth {
                message: reason.clone(),
            },
        );
        cx.cancel_timer(RETRY);
        cx.log(
            Level::Warning,
            format!("{reason}; no further attempts until the device is opened again with corrected settings"),
        );
        cx.connection(Connection::Unauthorized {
            reason: reason.clone(),
        });
        self.refused = Some(reason);
    }

    /// The Core has answered: report it connected and rebuild the change
    /// groups the Core deleted when the previous connection ended.
    fn become_ready(&mut self, cx: &mut Cx) {
        self.ready = true;
        self.retry_after = RETRY_MIN;
        self.pending.retain(|_, p| p.purpose != Purpose::Logon);
        self.arm_reply_timer(cx);
        cx.connection(Connection::Connected);
        // Commands only: `groups` is never filled, so nothing is rebuilt.
        let groups: Vec<(String, Group)> = self
            .groups
            .iter()
            .map(|(k, g)| (k.clone(), g.clone()))
            .collect();
        for (id, g) in groups {
            if !g.controls.is_empty() {
                self.send(
                    cx,
                    "ChangeGroup.AddControl",
                    json!({"Id": id, "Controls": g.controls}),
                    Purpose::Internal,
                    None,
                );
            }
            for (component, controls) in &g.components {
                let list: Vec<Value> = controls.iter().map(|n| json!({"Name": n})).collect();
                self.send(
                    cx,
                    "ChangeGroup.AddComponentControl",
                    json!({"Id": id, "Component": {"Name": component, "Controls": list}}),
                    Purpose::Internal,
                    None,
                );
            }
            if let Some(rate) = &g.rate {
                self.send(
                    cx,
                    "ChangeGroup.AutoPoll",
                    json!({"Id": id, "Rate": rate}),
                    Purpose::Internal,
                    None,
                );
            }
        }
    }

    fn apply_edit(&mut self, cx: &mut Cx, edit: Edit) {
        if !self.monitor {
            // Commands only: the consumer's change groups are theirs to keep
            // and poll; none is remembered, auto-polled or rebuilt.
            return;
        }
        let added = match edit {
            Edit::Add { group, controls } => {
                let g = self.groups.entry(group.clone()).or_default();
                for c in controls {
                    if !g.controls.contains(&c) {
                        g.controls.push(c);
                    }
                }
                Some(group)
            }
            Edit::AddComponent {
                group,
                component,
                controls,
            } => {
                let g = self.groups.entry(group.clone()).or_default();
                let list = g.components.entry(component).or_default();
                for c in controls {
                    if !list.contains(&c) {
                        list.push(c);
                    }
                }
                Some(group)
            }
            Edit::Remove { group, controls } => {
                if let Some(g) = self.groups.get_mut(&group) {
                    g.controls.retain(|c| !controls.contains(c));
                }
                None
            }
            Edit::AutoPoll { group, rate } => {
                self.groups.entry(group).or_default().rate = Some(rate);
                None
            }
            Edit::Clear { group } => {
                if let Some(g) = self.groups.get_mut(&group) {
                    g.controls.clear();
                    g.components.clear();
                }
                None
            }
            Edit::Destroy { group } => {
                self.groups.remove(&group);
                None
            }
        };
        // A group's first controls: have the Core push its changes.
        if let Some(group) = added {
            let g = self.groups.get_mut(&group).expect("just added");
            if g.rate.is_none() && self.auto_poll > 0.0 {
                let rate = json!(self.auto_poll);
                g.rate = Some(rate.clone());
                self.send(
                    cx,
                    "ChangeGroup.AutoPoll",
                    json!({"Id": group, "Rate": rate}),
                    Purpose::Internal,
                    None,
                );
            }
        }
    }

    /// Engine status from a push or a StatusGet reply. A different design
    /// makes every control value stale: they are cleared and the change
    /// groups asked to send everything again.
    fn engine(&mut self, cx: &mut Cx, o: &Map<String, Value>) {
        cx.state(engine_patch(o));
        let Some(code) = o.get("DesignCode").and_then(Value::as_str) else {
            return;
        };
        let changed = self.design_code.as_deref().is_some_and(|old| old != code);
        self.design_code = Some(code.to_string());
        if changed {
            cx.state(json!({"controls": null, "components": null}));
            if self.ready {
                let ids: Vec<String> = self.groups.keys().cloned().collect();
                for id in ids {
                    self.send(
                        cx,
                        "ChangeGroup.Invalidate",
                        json!({ "Id": id }),
                        Purpose::Internal,
                        None,
                    );
                }
            }
        }
    }

    fn message(&mut self, cx: &mut Cx, msg: Value) {
        let Some(o) = msg.as_object() else {
            cx.log(
                Level::Debug,
                format!("ignored a message that is not an object: {msg}"),
            );
            return;
        };
        if let Some(method) = o.get("method").and_then(Value::as_str) {
            let params = o.get("params").cloned().unwrap_or(Value::Null);
            self.notification(cx, method, &params);
        } else if o.contains_key("result") || o.contains_key("error") || o.contains_key("response")
        {
            self.response(cx, o);
        } else {
            cx.log(
                Level::Debug,
                format!("ignored an unrecognised message: {msg}"),
            );
        }
    }

    fn notification(&mut self, cx: &mut Cx, method: &str, params: &Value) {
        let empty = Map::new();
        let o = params.as_object().unwrap_or(&empty);
        match method {
            "EngineStatus" => self.engine(cx, o),
            "PA.PageStatus" => {
                if let Some(patch) = page_patch(o) {
                    cx.state(patch);
                }
            }
            "PA.ZoneStatus" => {
                if let Some(patch) = zone_patch(o) {
                    cx.state(patch);
                }
            }
            "LoopPlayer.Error" => {
                let error = o.get("Error").cloned().unwrap_or(Value::Null);
                let reference = o
                    .get("RefId")
                    .or_else(|| o.get("RefID"))
                    .cloned()
                    .unwrap_or(Value::Null);
                cx.log(
                    Level::Warning,
                    format!("Loop Player error {error} (RefID {reference})"),
                );
                cx.state(
                    json!({"loop_player": {"last_error": {"error": error, "ref_id": reference}}}),
                );
            }
            _ => match o.get("Changes").and_then(Value::as_array) {
                // Change group results pushed by AutoPoll.
                Some(changes) => {
                    if let Some(patch) = controls_patch(changes, None) {
                        cx.state(patch);
                    }
                }
                None => cx.log(Level::Debug, format!("ignored notification {method}")),
            },
        }
    }

    fn response(&mut self, cx: &mut Cx, o: &Map<String, Value>) {
        let pending = o
            .get("id")
            .and_then(reply_id)
            .and_then(|id| self.pending.remove(&id));
        if let Some(p) = &pending {
            self.arm_reply_timer(cx);
            cx.round_trip(cx.now().saturating_sub(p.sent_at));
        }
        let error = o.get("error").filter(|e| !e.is_null());
        if let Some(error) = error {
            let (code, message) = error_parts(error);
            let purpose = pending.as_ref().map(|p| p.purpose);
            if code == Some(LOGON_REQUIRED) || purpose == Some(Purpose::Logon) {
                let reason = if self.username.is_empty() {
                    format!("the Core requires a logon ({message}); set username and pin")
                } else {
                    format!(
                        "the Core refused the logon for user '{}' ({message})",
                        self.username
                    )
                };
                self.refuse(cx, reason);
                return;
            }
            match pending {
                Some(Pending {
                    purpose: Purpose::Command { id, .. },
                    ..
                }) => {
                    let message = match code.and_then(error_meaning) {
                        Some(meaning) if meaning != message => format!("{message} ({meaning})"),
                        _ => message,
                    };
                    cx.complete(
                        id,
                        Err(CommandError::DeviceError {
                            code: code.map(|c| c.to_string()),
                            message,
                        }),
                    );
                }
                Some(p) => {
                    cx.log(Level::Warning, format!("{} failed: {message}", p.method));
                    if p.purpose == Purpose::Status && !self.ready {
                        // The Core answers, if not with its status.
                        self.become_ready(cx);
                    }
                }
                None => cx.log(
                    Level::Warning,
                    format!("the Core reported an error: {message}"),
                ),
            }
            return;
        }

        let result = o
            .get("result")
            .or_else(|| o.get("response"))
            .cloned()
            .unwrap_or(Value::Null);
        let Some(p) = pending else {
            // AutoPoll's later results, or a reply without its id.
            if let Some(patch) = result_patch("", &result) {
                cx.state(patch);
            }
            return;
        };
        if p.method == "StatusGet" {
            if let Some(status) = result.as_object() {
                self.engine(cx, status);
            }
        } else if let Some(patch) = result_patch(&p.method, &result) {
            cx.state(patch);
        }
        match p.purpose {
            Purpose::Logon | Purpose::Status => {
                if !self.ready {
                    self.become_ready(cx);
                }
            }
            Purpose::Internal => {}
            Purpose::Command { id, value } => {
                let outcome = if value {
                    Outcome::Value { value: result }
                } else {
                    Outcome::Ack
                };
                cx.complete(id, Ok(outcome));
            }
        }
        if let Some(edit) = p.edit {
            self.apply_edit(cx, edit);
        }
    }

    fn data(&mut self, cx: &mut Cx, data: &[u8]) {
        cx.alive();
        self.last_heard = cx.now();
        let (messages, discarded) = self.deframer.feed(data);
        for reason in discarded {
            cx.log(
                Level::Warning,
                format!("discarded bytes that are not JSON: {reason}"),
            );
        }
        for msg in messages {
            if self.refused.is_some() || !self.socket_open {
                return;
            }
            self.message(cx, msg);
        }
    }
}

impl Module for Qsys {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if let Some(reason) = &self.refused {
            cx.complete(
                id,
                Err(CommandError::Auth {
                    message: reason.clone(),
                }),
            );
            return;
        }
        if !self.ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match plan(name, params) {
            Ok(plan) => self.send(
                cx,
                &plan.method,
                plan.params,
                Purpose::Command {
                    id,
                    value: plan.value,
                },
                plan.edit,
            ),
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        if self.refused.is_some() {
            return;
        }
        match input {
            TcpInput::Connected => {
                self.socket_open = true;
                self.last_heard = cx.now();
                if !self.username.is_empty() {
                    let logon = json!({"User": self.username, "Password": self.pin});
                    self.send(cx, "Logon", logon, Purpose::Logon, None);
                }
                // Its answer says the Core is there and, after a Logon,
                // whether the Logon was accepted.
                self.send(cx, "StatusGet", json!(0), Purpose::Status, None);
                cx.set_timer(KEEPALIVE, KEEPALIVE_EVERY);
            }
            TcpInput::Data(data) => self.data(cx, &data),
            TcpInput::Closed { reason } => {
                self.socket_open = false;
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => {
                if self.refused.is_none() {
                    self.open(cx);
                }
            }
            KEEPALIVE => {
                if !self.socket_open {
                    return;
                }
                let now = cx.now();
                if now.saturating_sub(self.last_heard) >= KEEPALIVE_EVERY {
                    // Quiet: ask for something the Core always answers.
                    let asked = self.pending.values().any(|p| p.purpose == Purpose::Status);
                    if !asked {
                        self.send(cx, "StatusGet", json!(0), Purpose::Status, None);
                    }
                } else if now.saturating_sub(self.last_sent) >= KEEPALIVE_EVERY {
                    self.send(cx, "NoOp", json!({}), Purpose::Internal, None);
                }
                cx.set_timer(KEEPALIVE, KEEPALIVE_EVERY);
            }
            REPLY => {
                let now = cx.now();
                let expired: Vec<u64> = self
                    .pending
                    .iter()
                    .filter(|(_, p)| p.deadline <= now)
                    .map(|(id, _)| *id)
                    .collect();
                for rpc in expired {
                    let p = self.pending.remove(&rpc).expect("listed");
                    match p.purpose {
                        Purpose::Command { id, .. } => cx.complete(id, Err(CommandError::Timeout)),
                        Purpose::Status => {
                            let reason = if self.ready {
                                "the Core stopped answering"
                            } else {
                                "the Core accepted the connection but never answered; check that a design is running (or emulating) and External Control is enabled"
                            };
                            self.lost(cx, reason.to_string());
                            return;
                        }
                        // A Logon may go unanswered; the StatusGet after it decides.
                        Purpose::Logon => {}
                        Purpose::Internal => cx.log(
                            Level::Debug,
                            format!("no reply to {} within {REPLY_TIMEOUT} ms", p.method),
                        ),
                    }
                }
                self.arm_reply_timer(cx);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.socket_open {
            cx.tcp_close(SOCKET);
            self.socket_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use crate::session::merge_patch;
    use std::net::{IpAddr, Ipv4Addr};

    fn params(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    /// QSC's example as printed, without the whitespace between tokens, and
    /// NUL-terminated: the bytes the Core is documented to accept. The help
    /// pages print a minus sign as U+2010; the examples below use ASCII '-'.
    fn wire(example: &str) -> Vec<u8> {
        let mut out = Vec::new();
        let mut in_string = false;
        let mut escaped = false;
        for &b in example.as_bytes() {
            if in_string {
                out.push(b);
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_string = false;
                }
            } else if !b.is_ascii_whitespace() {
                if b == b'"' {
                    in_string = true;
                }
                out.push(b);
            }
        }
        out.push(0);
        out
    }

    /// The bytes a command sends, as request number `id`.
    fn sent_for(name: &str, p: Value, id: u64) -> Vec<u8> {
        let plan = plan(name, &params(p)).unwrap();
        frame(&request(id, &plan.method, plan.params))
    }

    fn parsed(bytes: &[u8]) -> Value {
        assert_eq!(bytes.last(), Some(&0), "NUL-terminated");
        assert_eq!(bytes.iter().filter(|&&b| b == 0).count(), 1);
        serde_json::from_slice(&bytes[..bytes.len() - 1]).unwrap()
    }

    // QRC Commands, "Control Methods", Control.Get example 1.
    #[test]
    fn control_get_matches_qsc_example_byte_for_byte() {
        let example = r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "Control.Get",
  "params": ["MainGain"]
}"#;
        assert_eq!(
            sent_for("control_get", json!({"controls": ["MainGain"]}), 1234),
            wire(example)
        );
        let example = r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method":  "Control.Get",
  "params": ["MainGain", "MainMute"]
}"#;
        assert_eq!(
            sent_for(
                "control_get",
                json!({"controls": ["MainGain", "MainMute"]}),
                1234
            ),
            wire(example)
        );
    }

    // QRC Commands, Control.Set example.
    #[test]
    fn control_set_matches_qsc_example() {
        let example = r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "Control.Set",
  "params": {
    "Name": "MainGain",
    "Value": -12
  }
}"#;
        assert_eq!(
            sent_for(
                "control_set",
                json!({"name": "MainGain", "value": -12}),
                1234
            ),
            wire(example)
        );
        // Ramp is the documented optional third parameter.
        assert_eq!(
            parsed(&sent_for(
                "control_set",
                json!({"name": "MainGain", "value": -12, "ramp": 2.0}),
                1
            ))["params"],
            json!({"Name": "MainGain", "Value": -12, "Ramp": 2.0})
        );
        // A string or boolean Value.
        assert_eq!(
            parsed(&sent_for(
                "control_set",
                json!({"name": "MainMute", "value_bool": true}),
                1
            ))["params"]["Value"],
            json!(true)
        );
        assert!(plan(
            "control_set",
            &params(json!({"name": "x", "value": 1, "value_string": "a"}))
        )
        .is_err());
        assert!(plan("control_set", &params(json!({"name": "x"}))).is_err());
    }

    // KB "How To | Formatting a QRC command": the StatusGet and Control.Get
    // examples, as given with their terminator.
    #[test]
    fn status_get_matches_the_knowledge_base_example() {
        let kb = br#"{"jsonrpc": "2.0", "method": "StatusGet", "id": 1234,"params": 0}"#;
        let ours = sent_for("status_get", json!({}), 1234);
        assert_eq!(parsed(&ours), serde_json::from_slice::<Value>(kb).unwrap());
        let kb =
            r#"{"jsonrpc": "2.0","id": 1234,"method": "Control.Get","params": ["Gain","Mute"]}"#;
        assert_eq!(
            sent_for("control_get", json!({"controls": ["Gain", "Mute"]}), 1234),
            wire(kb)
        );
    }

    // QRC Commands, "Component Control Methods".
    #[test]
    fn component_methods_match_qsc_examples() {
        let example = r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "Component.Get",
  "params": {
    "Name": "My APM",
    "Controls": [
      { "Name": "ent.xfade.gain" }
    ]
  }
}"#;
        assert_eq!(
            sent_for(
                "component_get",
                json!({"component": "My APM", "controls": ["ent.xfade.gain"]}),
                1234
            ),
            wire(example)
        );
        let example = r#"{
    "jsonrpc": "2.0",
    "id": 1234,
    "method": "Component.GetControls",
    "params": {
        "Name": "MyGain"
    }
}"#;
        assert_eq!(
            sent_for(
                "component_get_controls",
                json!({"component": "MyGain"}),
                1234
            ),
            wire(example)
        );
        // Component.Set example 1.
        let example = r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "Component.Set",
  "params": {
    "Name": "My APM",
    "Controls": [
      {
        "Name": "ent.xfade.gain",
        "Value": -100.0,
        "Ramp": 2.0
      }
    ]
  }
}"#;
        assert_eq!(
            sent_for(
                "component_set",
                json!({"component": "My APM", "control": "ent.xfade.gain", "value": -100.0, "ramp": 2.0}),
                1234
            ),
            wire(example)
        );
        // Example 2: several controls.
        let example = r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "Component.Set",
  "params": {
    "Name": "My APM",
    "Controls": [
      {
        "Name": "ent.xfade.gain",
        "Value": -100.0,
        "Ramp": 2.0
      },
      {
        "Name": "bgm.xfade.gain",
        "Value": 0.0,
        "Ramp": 1.0
      }
    ]
  }
}"#;
        assert_eq!(
            sent_for(
                "component_set_controls",
                json!({"component": "My APM", "controls": [
                    {"Name": "ent.xfade.gain", "Value": -100.0, "Ramp": 2.0},
                    {"Name": "bgm.xfade.gain", "Value": 0.0, "Ramp": 1.0}
                ]}),
                1234
            ),
            wire(example)
        );
        // Example 3, ResponseValues (the example puts id last).
        let example = r#"{
  "jsonrpc": "2.0",
  "method": "Component.Set",
  "params": {
    "Name": "Gain",
    "ResponseValues": true,
    "Controls": [
      {
        "Name": "gain",
        "Value": -10,
        "Ramp": 0
      }
    ]
  },
  "id": 101
}"#;
        let ours = sent_for(
            "component_set",
            json!({"component": "Gain", "control": "gain", "value": -10, "ramp": 0, "response_values": true}),
            101,
        );
        assert_eq!(
            parsed(&ours),
            serde_json::from_str::<Value>(example).unwrap()
        );
        assert_eq!(
            parsed(&ours)["params"].to_string(),
            r#"{"Name":"Gain","ResponseValues":true,"Controls":[{"Name":"gain","Value":-10,"Ramp":0}]}"#
        );
    }

    // QRC Commands, "Change Group Methods".
    #[test]
    fn change_group_methods_match_qsc_examples() {
        let cases = [
            (
                "change_group_add_controls",
                json!({"group": "my change group", "controls": ["some control", "another control"]}),
                r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "ChangeGroup.AddControl",
  "params": {
    "Id": "my change group",
    "Controls" : [
      "some control", "another control"
    ]
  }
}"#,
            ),
            (
                "change_group_add_component_controls",
                json!({"group": "my change group", "component": "My Component", "controls": ["gain", "mute"]}),
                r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "ChangeGroup.AddComponentControl",
  "params": {
    "Id": "my change group",
    "Component" : {
      "Name": "My Component",
      "Controls": [
        { "Name": "gain" },
        { "Name": "mute" }
      ]
    }
  }
}"#,
            ),
            // The help's example lacks the comma after "Id"; added here.
            (
                "change_group_remove",
                json!({"group": "my change group", "controls": ["some control"]}),
                r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "ChangeGroup.Remove",
  "params": {
    "Id": "my change group",
    "Controls" : [
      "some control"
    ]
  }
}"#,
            ),
            (
                "change_group_poll",
                json!({"group": "my change group"}),
                r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "ChangeGroup.Poll",
  "params": {
    "Id": "my change group"
  }
}"#,
            ),
            (
                "change_group_destroy",
                json!({"group": "my change group"}),
                r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "ChangeGroup.Destroy",
  "params": {
    "Id": "my change group"
  }
}"#,
            ),
            (
                "change_group_invalidate",
                json!({"group": "my change group"}),
                r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "ChangeGroup.Invalidate",
  "params": {
    "Id": "my change group"
  }
}"#,
            ),
            (
                "change_group_clear",
                json!({"group": "my change group"}),
                r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "ChangeGroup.Clear",
  "params": {
    "Id": "my change group"
  }
}"#,
            ),
            (
                "change_group_auto_poll",
                json!({"group": "my change group", "rate": 5}),
                r#"{
  "jsonrpc": "2.0",
  "id": 1234,
  "method": "ChangeGroup.AutoPoll",
  "params": {
    "Id": "my change group",
    "Rate": 5
  }
}"#,
            ),
        ];
        for (name, p, example) in cases {
            assert_eq!(sent_for(name, p, 1234), wire(example), "{name}");
        }
        assert!(plan(
            "change_group_auto_poll",
            &params(json!({"group": "g", "rate": 0}))
        )
        .is_err());
    }

    // QRC Commands: Mixer, Snapshot and Loop Player examples. Their keys come
    // in another order, so they are compared as JSON.
    #[test]
    fn mixer_snapshot_and_loop_player_match_qsc_examples() {
        let cases = [
            (
                "mixer_set_crosspoint_gain",
                json!({"mixer": "Parade", "inputs": "*", "outputs": "*", "value": -100.0, "ramp": 5.0}),
                r#"{
  "jsonrpc": "2.0",
  "method": "Mixer.SetCrossPointGain",
  "id": 1234,
  "params": {
    "Name": "Parade",
    "Inputs": "*",
    "Outputs": "*",
    "Value": -100.0,
    "Ramp": 5.0
  }
}"#,
            ),
            (
                "snapshot_load",
                json!({"bank": "MySpecialBank", "snapshot": 7, "ramp": 8.5}),
                r#"{
  "jsonrpc": "2.0",
  "method": "Snapshot.Load",
  "params": {
    "Name": "MySpecialBank",
    "Bank": 7,
    "Ramp": 8.5
  },
  "id": 1234
}"#,
            ),
            // The help's example ends "Bank": 4 with a comma; removed here.
            (
                "snapshot_save",
                json!({"bank": "MyVerySpecialBank", "snapshot": 4}),
                r#"{
  "jsonrpc": "2.0",
  "method": "Snapshot.Save",
  "params": {
    "Name": "MyVerySpecialBank",
    "Bank": 4
  },
  "id": 1234
}"#,
            ),
            (
                "loop_player_start",
                json!({"name": "test", "file": "Audio/mainloop.wav", "output": 1,
                       "start_time": 62600, "loop": false, "log": true}),
                r#"{
  "jsonrpc": "2.0",
  "method": "LoopPlayer.Start",
  "params": {
    "Files": [
      {
        "Name": "Audio/mainloop.wav",
        "Output": 1
      }
    ],
    "Name": "test",
    "StartTime": 62600,
    "Loop": false,
    "Log": true
  },
  "id": 1234
}"#,
            ),
            (
                "loop_player_stop",
                json!({"name": "test", "outputs": [1, 3, 4], "log": true}),
                r#"{
  "jsonrpc": "2.0",
  "method": "LoopPlayer.Stop",
  "params": {
    "Name": "test",
    "Outputs": [ 1, 3, 4 ],
    "Log": true
  },
  "id": 1234
}"#,
            ),
            (
                "loop_player_cancel",
                json!({"name": "test", "outputs": [1, 3, 4], "log": true}),
                r#"{
  "jsonrpc": "2.0",
  "method": "LoopPlayer.Cancel",
  "params": {
    "Name": "test",
    "Outputs": [ 1, 3, 4 ],
    "Log": true
  },
  "id": 1234
}"#,
            ),
        ];
        for (name, p, example) in cases {
            let ours = parsed(&sent_for(name, p, 1234));
            assert_eq!(
                ours,
                serde_json::from_str::<Value>(example).unwrap(),
                "{name}"
            );
        }
        // Mute takes a boolean; gain a number.
        assert!(plan(
            "mixer_set_input_mute",
            &params(json!({"mixer": "Parade", "inputs": "4-6", "value": 1.0}))
        )
        .is_err());
        assert_eq!(
            parsed(&sent_for(
                "mixer_set_input_cue_enable",
                json!({"mixer": "Parade", "cues": "1", "inputs": "1-8 !3", "value": true}),
                1
            ))["params"]
                .to_string(),
            r#"{"Name":"Parade","Cues":"1","Inputs":"1-8 !3","Value":true}"#
        );
    }

    // PARAPI examples: the live page submission and zone status configuration.
    #[test]
    fn pa_methods_match_parapi_examples() {
        let example = r#"{"id":"8561","method":"PA.PageSubmit","jsonrpc":"2.0","params":{"Zones":[2],"Preamble":"Chime long ascending triple.wav","Description":"Remote Client XYZ","ZoneTags":["Terminal B Retail"], "MaxPageTime":45,"Mode":"live","Station":4,"Priority":3}}"#;
        let ours = parsed(&sent_for(
            "pa_page_submit",
            json!({"zones": [2], "preamble": "Chime long ascending triple.wav",
                   "description": "Remote Client XYZ", "zone_tags": ["Terminal B Retail"],
                   "max_page_time": 45, "mode": "live", "station": 4, "priority": 3}),
            8561,
        ));
        let example: Value = serde_json::from_str(example).unwrap();
        assert_eq!(ours["method"], example["method"]);
        assert_eq!(ours["params"], example["params"]);

        let ours = parsed(&sent_for("pa_page_start", json!({"page_id": 895}), 4549));
        assert_eq!(ours["params"], json!({"PageID": 895}));
        let ours = parsed(&sent_for(
            "pa_zone_status_configure",
            json!({"enabled": true}),
            8185,
        ));
        assert_eq!(ours["method"], "PA.ZoneStatusConfigure");
        assert_eq!(ours["params"], json!({"Enabled": true}));
        // A voice page needs a station; a message page a message.
        assert!(plan(
            "pa_page_submit",
            &params(json!({"mode": "live", "zones": [1], "priority": 1}))
        )
        .is_err());
        assert!(plan(
            "pa_page_submit",
            &params(json!({"mode": "message", "zone_tags": ["a"], "priority": 1}))
        )
        .is_err());
    }

    #[test]
    fn deframer_splits_on_nul_and_tolerates_its_absence() {
        let mut d = Deframer::default();
        let (m, _) = d.feed(b"{\"a\":1}\0{\"b\"");
        assert_eq!(m, vec![json!({"a": 1})]);
        let (m, _) = d.feed(b":2}\0\r\n{\"c\":3}");
        assert_eq!(m, vec![json!({"b": 2}), json!({"c": 3})]);
        let (m, bad) = d.feed(b"garbage\0{\"d\":4}\0");
        assert_eq!(m, vec![json!({"d": 4})]);
        assert_eq!(bad.len(), 1);
        assert!(d.buf.is_empty());
    }

    // Inbound examples from "QRC Commands", with the commas the help leaves
    // out restored and U+2010 read as '-'.
    #[test]
    fn replies_and_pushes_become_state() {
        // EngineStatus.
        let push: Value = serde_json::from_str(
            r#"{"jsonrpc":"2.0","method":"EngineStatus","params":{"State":"Active","DesignName":"MyDesign","DesignCode":"qALFilm6IcCo","IsRedundant":false,"IsEmulator":true}}"#,
        )
        .unwrap();
        assert_eq!(
            engine_patch(push["params"].as_object().unwrap()),
            json!({"engine": {"state": "Active", "design_name": "MyDesign",
                "design_code": "qALFilm6IcCo", "is_redundant": false, "is_emulator": true}})
        );
        // StatusGet's result.
        let reply: Value = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":1234,"result":{"Platform":"Core 510i","State":"Active","DesignName":"SAF-MainPA","DesignCode":"qALFilm6IcAz","IsRedundant":false,"IsEmulator":true,"Status":{"Code":0,"String":"OK"}}}"#,
        )
        .unwrap();
        let patch = engine_patch(reply["result"].as_object().unwrap());
        assert_eq!(patch["engine"]["platform"], "Core 510i");
        assert_eq!(
            patch["engine"]["status"],
            json!({"code": 0, "string": "OK"})
        );
        // Control.Get example 2.
        let reply = json!([
            {"Name": "MainGain", "Value": -12, "String": "-12.0dB"},
            {"Name": "MainMute", "Value": false, "String": "Unmuted"}
        ]);
        assert_eq!(
            result_patch("Control.Get", &reply).unwrap(),
            json!({"controls": {
                "MainGain": {"value": -12, "string": "-12.0dB"},
                "MainMute": {"value": false, "string": "Unmuted"}
            }})
        );
        // Component.Get's response.
        let reply = json!({"Name": "My APM", "Controls": [
            {"Name": "ent.xfade.gain", "Value": -100.0, "String": "-100.0dB", "Position": 0}
        ]});
        assert_eq!(
            result_patch("Component.Get", &reply).unwrap(),
            json!({"components": {"My APM": {"ent.xfade.gain":
                {"value": -100.0, "string": "-100.0dB", "position": 0}}}})
        );
        // Component.GetControls carries the control's range and type.
        let reply = json!({"Name": "MyGain", "Controls": [
            {"Name": "gain", "Type": "Float", "Value": 0.0, "ValueMin": -100.0, "ValueMax": 20.0,
             "StringMin": "-100dB", "StringMax": "20.0dB", "String": "0dB",
             "Position": 0.83333331, "Direction": "Read/Write"}
        ]});
        let patch = result_patch("Component.GetControls", &reply).unwrap();
        assert_eq!(patch["components"]["MyGain"]["gain"]["value_max"], 20.0);
        assert_eq!(
            patch["components"]["MyGain"]["gain"]["direction"],
            "Read/Write"
        );
        // Component.Set example 3's response.
        let reply = json!([{"Component": "Gain", "Name": "gain", "String": "-10.0dB",
            "Value": -10, "Position": 0.75}]);
        assert_eq!(
            result_patch("Component.Set", &reply).unwrap(),
            json!({"components": {"Gain": {"gain":
                {"value": -10, "string": "-10.0dB", "position": 0.75}}}})
        );
        // ChangeGroup.Poll's response: a named control and a component's.
        let reply = json!({"Id": "my change group", "Changes": [
            {"Name": "some control", "Value": -12, "String": "-12dB"},
            {"Component": "My Component", "Name": "gain", "Value": -12, "String": "-12dB"}
        ]});
        assert_eq!(
            result_patch("", &reply).unwrap(),
            json!({
                "controls": {"some control": {"value": -12, "string": "-12dB"}},
                "components": {"My Component": {"gain": {"value": -12, "string": "-12dB"}}}
            })
        );
        // PARAPI notifications.
        let page: Value = serde_json::from_str(r#"{"method":"PA.PageStatus","jsonrpc":"2.0","params":{"State":"done","Message":"Page complete","SubState":"success","PageID":895}}"#).unwrap();
        assert_eq!(
            page_patch(page["params"].as_object().unwrap()).unwrap(),
            json!({"pa": {"pages": {"895": {"state": "done", "sub_state": "success",
                "message": "Page complete", "count": null}}}})
        );
        let zone: Value = serde_json::from_str(r#"{"method":"PA.ZoneStatus","jsonrpc":"2.0","params":{"PriorityName":"Urgent","Time":"2012-05-14T16:57:24Z","Zone":1,"Priority":2,"Station":1,"Active":true}}"#).unwrap();
        assert_eq!(
            zone_patch(zone["params"].as_object().unwrap()).unwrap()["pa"]["zones"]["1"],
            json!({"active": true, "time": "2012-05-14T16:57:24Z", "page_id": null,
                "priority": 2, "priority_name": "Urgent", "station": 1})
        );
    }

    // --- The session -------------------------------------------------------

    fn module(settings: Value) -> Qsys {
        Qsys::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 30)),
            host_name: None,
            port: None,
            model: "core".into(),
            channels: None,
            settings: params(settings),
            monitor: true,
        })
    }

    fn sent(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(parsed(data)),
                _ => None,
            })
            .collect()
    }

    fn state(actions: &[Action]) -> Value {
        let mut merged = json!({});
        for a in actions {
            if let Action::State(p) = a {
                merge_patch(&mut merged, p);
            }
        }
        merged
    }

    fn feed(m: &mut Qsys, now: Millis, msg: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(frame(&msg)));
        cx.take()
    }

    fn run(m: &mut Qsys, now: Millis, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.command(&mut cx, id, name, &params(p));
        cx.take()
    }

    fn connect(m: &mut Qsys) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        cx.take()
    }

    /// Connected, with the StatusGet answered (no logon configured).
    fn ready(settings: Value) -> Qsys {
        let mut m = module(settings);
        let a = connect(&mut m);
        let status = sent(&a).pop().unwrap();
        assert_eq!(status["method"], "StatusGet");
        let a = feed(
            &mut m,
            10,
            json!({"jsonrpc": "2.0", "id": status["id"], "result": {"Platform": "Core 110f",
                "State": "Active", "DesignName": "D", "DesignCode": "abc",
                "IsRedundant": false, "IsEmulator": false}}),
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert_eq!(state(&a)["engine"]["platform"], "Core 110f");
        m
    }

    #[test]
    fn logon_then_status_and_connected() {
        let mut m = module(json!({"username": "control", "pin": "1234"}));
        let a = connect(&mut m);
        assert!(a.contains(&Action::TcpOpen {
            socket: SOCKET,
            to: "10.0.0.30:1710".parse().unwrap()
        }));
        let out = sent(&a);
        assert_eq!(
            out[0],
            json!({"jsonrpc": "2.0", "id": 1, "method": "Logon",
                "params": {"User": "control", "Password": "1234"}})
        );
        assert_eq!(out[1]["method"], "StatusGet");
        // Commands wait for the Core.
        let a = run(&mut m, 5, 7, "status_get", json!({}));
        assert!(a.contains(&Action::Complete {
            id: 7,
            result: Err(CommandError::NotConnected)
        }));
        let a = feed(
            &mut m,
            10,
            json!({"jsonrpc": "2.0", "id": 1, "result": true}),
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));
    }

    #[test]
    fn a_refused_logon_is_terminal() {
        let mut m = module(json!({"username": "control", "pin": "0000"}));
        connect(&mut m);
        let a = feed(
            &mut m,
            10,
            json!({"jsonrpc": "2.0", "id": 1, "error": {"code": 10, "message": "Logon required"}}),
        );
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::Connection(Connection::Unauthorized { .. }))));
        assert!(a.contains(&Action::TcpClose { socket: SOCKET }));
        assert!(!a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { key: RETRY, .. })));
        let a = run(&mut m, 20, 3, "no_op", json!({}));
        assert!(matches!(
            &a[..],
            [Action::Complete {
                id: 3,
                result: Err(CommandError::Auth { .. })
            }]
        ));
        let mut cx = Cx::new(30);
        m.timer(&mut cx, RETRY);
        assert!(cx.take().is_empty());
    }

    #[test]
    fn logon_required_without_credentials_is_terminal() {
        let mut m = module(json!({}));
        let a = connect(&mut m);
        assert_eq!(sent(&a).len(), 1, "no Logon without a username");
        let a = feed(
            &mut m,
            10,
            json!({"jsonrpc": "2.0", "id": 1, "error": {"code": 10, "message": "Logon required"}}),
        );
        assert!(a.iter().any(|x| matches!(
            x,
            Action::Connection(Connection::Unauthorized { reason }) if reason.contains("set username")
        )));
    }

    #[test]
    fn commands_complete_from_their_replies() {
        let mut m = ready(json!({}));
        let a = run(
            &mut m,
            20,
            5,
            "control_set",
            json!({"name": "MainGain", "value": -12}),
        );
        let req = sent(&a).pop().unwrap();
        let a = feed(
            &mut m,
            30,
            json!({"jsonrpc": "2.0", "id": req["id"], "result": true}),
        );
        assert!(a.contains(&Action::Complete {
            id: 5,
            result: Ok(Outcome::Ack)
        }));

        let a = run(
            &mut m,
            40,
            6,
            "control_get",
            json!({"controls": ["MainGain"]}),
        );
        let req = sent(&a).pop().unwrap();
        let a = feed(
            &mut m,
            50,
            json!({"jsonrpc": "2.0", "id": req["id"], "result": [{"Name": "MainGain", "Value": -12, "String": "-12.0dB"}]}),
        );
        assert!(a.contains(&Action::Complete {
            id: 6,
            result: Ok(Outcome::Value {
                value: json!([{"Name": "MainGain", "Value": -12, "String": "-12.0dB"}])
            })
        }));
        assert_eq!(state(&a)["controls"]["MainGain"]["string"], "-12.0dB");

        // An error reply, with the code's documented meaning.
        let a = run(
            &mut m,
            60,
            7,
            "control_set",
            json!({"name": "Nope", "value": 1}),
        );
        let req = sent(&a).pop().unwrap();
        let a = feed(
            &mut m,
            70,
            json!({"jsonrpc": "2.0", "id": req["id"], "error": {"code": 8, "message": "Unknown control"}}),
        );
        assert!(a.contains(&Action::Complete {
            id: 7,
            result: Err(CommandError::DeviceError {
                code: Some("8".into()),
                message: "Unknown control".into()
            })
        }));

        // No reply.
        run(&mut m, 80, 8, "no_op", json!({}));
        let mut cx = Cx::new(80 + REPLY_TIMEOUT);
        m.timer(&mut cx, REPLY);
        assert!(cx.take().contains(&Action::Complete {
            id: 8,
            result: Err(CommandError::Timeout)
        }));
    }

    #[test]
    fn change_groups_auto_poll_and_survive_reconnection() {
        let mut m = ready(json!({"auto_poll_rate": 0.5}));
        let a = run(
            &mut m,
            20,
            1,
            "change_group_add_controls",
            json!({"controls": ["MainGain"]}),
        );
        let req = sent(&a).pop().unwrap();
        assert_eq!(req["params"]["Id"], DEFAULT_GROUP);
        let a = feed(
            &mut m,
            30,
            json!({"jsonrpc": "2.0", "id": req["id"], "result": true}),
        );
        // The first controls in a group start its AutoPoll.
        let auto = sent(&a).pop().unwrap();
        assert_eq!(auto["method"], "ChangeGroup.AutoPoll");
        assert_eq!(auto["params"], json!({"Id": DEFAULT_GROUP, "Rate": 0.5}));
        // Its results, the first with the request's id, later ones pushed.
        let a = feed(
            &mut m,
            40,
            json!({"jsonrpc": "2.0", "id": auto["id"], "result": {"Id": DEFAULT_GROUP,
                "Changes": [{"Name": "MainGain", "Value": -12, "String": "-12dB", "Position": 0.5}]}}),
        );
        assert_eq!(state(&a)["controls"]["MainGain"]["value"], -12);
        let a = feed(
            &mut m,
            540,
            json!({"jsonrpc": "2.0", "method": "ChangeGroup.Poll", "params": {"Id": DEFAULT_GROUP,
                "Changes": [{"Name": "MainGain", "Value": -6, "String": "-6dB", "Position": 0.7}]}}),
        );
        assert_eq!(state(&a)["controls"]["MainGain"]["position"], 0.7);

        let a = run(
            &mut m,
            600,
            2,
            "change_group_add_component_controls",
            json!({"component": "Mixer", "controls": ["input.1.mute"]}),
        );
        let req = sent(&a).pop().unwrap();
        let a = feed(
            &mut m,
            610,
            json!({"jsonrpc": "2.0", "id": req["id"], "result": true}),
        );
        assert!(sent(&a).is_empty(), "AutoPoll already running");

        // The Core deletes change groups with the connection; they are made
        // again once it answers.
        let mut cx = Cx::new(700);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        let a = cx.take();
        assert!(a.contains(&Action::SetTimer {
            key: RETRY,
            after: RETRY_MIN
        }));
        let mut cx = Cx::new(1_700);
        m.timer(&mut cx, RETRY);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let status = sent(&cx.take()).pop().unwrap();
        let a = feed(
            &mut m,
            1_710,
            json!({"jsonrpc": "2.0", "id": status["id"], "result": {"State": "Active", "DesignCode": "abc"}}),
        );
        let methods: Vec<Value> = sent(&a).iter().map(|r| r["method"].clone()).collect();
        assert_eq!(
            methods,
            [
                "ChangeGroup.AddControl",
                "ChangeGroup.AddComponentControl",
                "ChangeGroup.AutoPoll"
            ]
        );
    }

    #[test]
    fn a_new_design_clears_control_state() {
        let mut m = ready(json!({}));
        let a = feed(
            &mut m,
            20,
            json!({"jsonrpc": "2.0", "method": "EngineStatus", "params": {"State": "Active",
                "DesignName": "Other", "DesignCode": "xyz", "IsRedundant": false, "IsEmulator": false}}),
        );
        assert!(a.contains(&Action::State(
            json!({"controls": null, "components": null})
        )));
    }

    #[test]
    fn keepalive_and_a_quiet_core() {
        let mut m = ready(json!({}));
        // Heard recently but nothing sent: NoOp.
        let mut cx = Cx::new(16_000);
        m.last_heard = 15_000;
        m.timer(&mut cx, KEEPALIVE);
        let out = sent(&cx.take());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["method"], "NoOp");
        assert_eq!(out[0]["params"], json!({}));
        // Quiet: StatusGet, and no answer means the Core is gone.
        let mut cx = Cx::new(40_000);
        m.timer(&mut cx, KEEPALIVE);
        let out = sent(&cx.take());
        assert_eq!(out[0]["method"], "StatusGet");
        let mut cx = Cx::new(40_000 + REPLY_TIMEOUT);
        m.timer(&mut cx, REPLY);
        assert!(cx.take().iter().any(|x| matches!(
            x,
            Action::Connection(Connection::Disconnected { reason }) if reason.contains("stopped")
        )));
    }

    #[test]
    fn opened_for_commands_only_it_keeps_no_change_group_of_its_own() {
        let mut m = module(json!({"auto_poll_rate": 0.5}));
        m.monitor = false;
        // On connecting: only StatusGet, which says the Core is there.
        let a = connect(&mut m);
        let methods: Vec<Value> = sent(&a).iter().map(|r| r["method"].clone()).collect();
        assert_eq!(methods, ["StatusGet"]);
        let status = sent(&a).pop().unwrap();
        let a = feed(
            &mut m,
            10,
            json!({"jsonrpc": "2.0", "id": status["id"], "result": {"State": "Active", "DesignCode": "abc"}}),
        );
        assert!(a.contains(&Action::Connection(Connection::Connected)));

        // The consumer's change-group command goes as sent, with no AutoPoll
        // added, and its reply completes it.
        let a = run(
            &mut m,
            20,
            1,
            "change_group_add_controls",
            json!({"controls": ["MainGain"]}),
        );
        let req = sent(&a).pop().unwrap();
        assert_eq!(req["method"], "ChangeGroup.AddControl");
        let a = feed(
            &mut m,
            30,
            json!({"jsonrpc": "2.0", "id": req["id"], "result": true}),
        );
        assert!(sent(&a).is_empty());
        assert!(a.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Ack)
        }));

        // After a reconnection nothing is rebuilt.
        let mut cx = Cx::new(700);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Closed {
                reason: "reset".into(),
            },
        );
        cx.take();
        let mut cx = Cx::new(1_700);
        m.timer(&mut cx, RETRY);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let status = sent(&cx.take()).pop().unwrap();
        let a = feed(
            &mut m,
            1_710,
            json!({"jsonrpc": "2.0", "id": status["id"], "result": {"State": "Active", "DesignCode": "abc"}}),
        );
        assert!(sent(&a).is_empty());
    }

    #[test]
    fn the_time_to_each_reply_is_reported() {
        let mut m = ready(json!({}));
        let a = run(&mut m, 100, 4, "no_op", json!({}));
        let req = sent(&a).pop().unwrap();
        let a = feed(
            &mut m,
            163,
            json!({"jsonrpc": "2.0", "id": req["id"], "result": true}),
        );
        assert!(a.contains(&Action::RoundTrip(63)));
        // A pushed notification is not a reply.
        let a = feed(
            &mut m,
            170,
            json!({"jsonrpc": "2.0", "method": "EngineStatus", "params": {"State": "Active",
                "DesignCode": "abc"}}),
        );
        assert!(!a.iter().any(|x| matches!(x, Action::RoundTrip(_))));
    }
}
