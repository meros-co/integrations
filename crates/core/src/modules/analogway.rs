//! Analog Way presentation switchers over AWJ: LivePremier (Aquilon RS, C,
//! C+, Cmax, Cmini), Midra 4K (Pulse 4K, QuickVu 4K, QuickMatrix 4K, Eikos
//! 4K) and Alta 4K (Zenith 100, Zenith 200).
//!
//! Written from Analog Way's own AWJ programmer's guides (see the specs'
//! sources for versions, dates and pages): "LivePremier AWJ Protocol
//! Programmer's Guide" v4.0 and "Aquilon AWJ Protocol Programmer's Guide"
//! v6.2, "Midra 4K AWJ Protocol Programmer's Guide" v3.2 and "Alta 4K AWJ
//! Protocol Programmer's Guide" v1.2.
//!
//! - TCP port 10606, at most five clients (section 1.1 of every guide).
//! - A message is one JSON object terminated by the byte 0x04. A command has
//!   "op" ("get" or "replace"), "path" and, for replace, "value"; a get is
//!   answered with {"path", "value"}; a replace usually is not answered at
//!   all ("The device will not return a string") (1.2).
//! - A command that cannot be processed is answered with
//!   {"error": {"code": "E1x", "message": ...}}, which carries no path (1.3).
//! - Nothing is pushed until the client replaces "Subscriptions" with a list
//!   of path prefixes; a changed property whose path starts with one of them
//!   is then sent as {"path", "value"} (1.4).
//! - Layer settings are kept in two preset banks, A/B on LivePremier and
//!   DOWN/UP on Midra 4K and Alta 4K, and which one is on program depends on
//!   where the virtual T-bar is: AT_DOWN puts A (DOWN) on program, AT_UP puts
//!   B (UP) there (LivePremier 3.8, Midra 4K 3.8). A command addressed to
//!   program or preview is therefore resolved against the screen's
//!   transition status, which the module reads first when it does not know
//!   it, and the state reports each screen's program and preview content
//!   under those names as well as under the raw bank names.

use std::collections::{BTreeMap, VecDeque};
use std::net::SocketAddr;

use regex::Regex;
use serde_json::{json, Map, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};

pub(crate) const DEFAULT_PORT: u16 = 10606;
const SOCKET: Key = "awj";
const END: u8 = 0x04;

const REPLY_TIMEOUT: Millis = 5_000;
const KEEPALIVE_EVERY: Millis = 10_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;
const MAX_BUFFER: usize = 16 * 1024 * 1024;

const REPLY: Key = "reply";
const KEEPALIVE: Key = "keepalive";
const RETRY: Key = "retry";

/// Tokens in a planned path or value, replaced by the bank that is on
/// program or on preview once the T-bar position is known.
const PGM: &str = "{PGM}";
const PRV: &str = "{PRV}";

/// The two AWJ object models the guides describe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    /// LivePremier / Aquilon: screens "S1", auxiliaries "A1", banks A/B.
    LivePremier,
    /// Midra 4K and Alta 4K: screens and auxiliaries numbered, banks DOWN/UP.
    Midra4k,
}

impl Dialect {
    pub(crate) fn for_spec(id: &str) -> Option<Dialect> {
        match id {
            "analogway-livepremier" => Some(Dialect::LivePremier),
            "analogway-midra4k" | "analogway-alta4k" => Some(Dialect::Midra4k),
            _ => None,
        }
    }

    fn device_type_path(self) -> &'static str {
        match self {
            Dialect::LivePremier => "DeviceObject/system/$device/@items/1/@props/dev",
            Dialect::Midra4k => "DeviceObject/system/@props/dev",
        }
    }

    fn serial_path(self) -> &'static str {
        match self {
            Dialect::LivePremier => {
                "DeviceObject/system/$device/@items/1/serial/@props/serialNumber"
            }
            Dialect::Midra4k => "DeviceObject/system/serial/@props/serialNumber",
        }
    }

    fn firmware_path(self) -> &'static str {
        match self {
            Dialect::LivePremier => "DeviceObject/system/$device/@items/1/version/@props/updater",
            Dialect::Midra4k => "DeviceObject/system/version/@props/updater",
        }
    }

    /// The path prefixes subscribed to on every connection.
    fn subscriptions(self) -> &'static [&'static str] {
        match self {
            Dialect::LivePremier => &[
                "DeviceObject/$screenAuxGroup",
                "DeviceObject/$screen",
                "DeviceObject/$auxiliary",
                "DeviceObject/presetBank/status",
                "DeviceObject/presetBank/$bank",
                "DeviceObject/masterPresetBank/status",
                "DeviceObject/masterPresetBank/$bank",
                "DeviceObject/$input",
                "DeviceObject/$monitoring",
            ],
            Dialect::Midra4k => &[
                "DeviceObject/transition",
                "DeviceObject/$screen",
                "DeviceObject/$auxiliaryScreen",
                "DeviceObject/preset",
                "DeviceObject/$input",
                "DeviceObject/multiviewer",
            ],
        }
    }

    /// The bank holding program (`program` true) or preview content for a
    /// T-bar position, per the guides' table in section 3.8.
    fn bank(self, transition: &str, program: bool) -> Option<&'static str> {
        let (down, up) = match self {
            Dialect::LivePremier => ("A", "B"),
            Dialect::Midra4k => ("DOWN", "UP"),
        };
        match (transition, program) {
            ("AT_DOWN", true) | ("AT_UP", false) => Some(down),
            ("AT_UP", true) | ("AT_DOWN", false) => Some(up),
            _ => None,
        }
    }
}

/// A screen or an auxiliary screen.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Scope {
    aux: bool,
    n: i64,
}

impl Scope {
    fn key(&self) -> (String, String) {
        let kind = if self.aux { "auxiliaries" } else { "screens" };
        (kind.to_string(), self.n.to_string())
    }

    /// The item name in paths: "S1"/"A1" on LivePremier, "1" otherwise.
    fn item(&self, d: Dialect) -> String {
        match (d, self.aux) {
            (Dialect::LivePremier, false) => format!("S{}", self.n),
            (Dialect::LivePremier, true) => format!("A{}", self.n),
            (Dialect::Midra4k, _) => self.n.to_string(),
        }
    }

    /// The list a screen or auxiliary belongs to, for layer paths.
    fn list(&self, d: Dialect) -> &'static str {
        match (d, self.aux) {
            (Dialect::LivePremier, false) => "$screen",
            (Dialect::LivePremier, true) => "$auxiliary",
            (Dialect::Midra4k, false) => "$screen",
            (Dialect::Midra4k, true) => "$auxiliaryScreen",
        }
    }

    /// Where the T-bar position of this screen is read. The auxiliary forms
    /// follow the documented xTake paths; the guides give the read for
    /// screens only (see the specs' quirks).
    fn transition_path(&self, d: Dialect) -> String {
        match d {
            Dialect::LivePremier => format!(
                "DeviceObject/$screenAuxGroup/@items/{}/status/@props/transition",
                self.item(d)
            ),
            Dialect::Midra4k => format!(
                "DeviceObject/transition/{}/@items/{}/status/@props/transition",
                self.list(d),
                self.n
            ),
        }
    }
}

/// One message to send.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Msg {
    pub path: String,
    pub value: Value,
}

fn msg(path: impl Into<String>, value: Value) -> Msg {
    Msg {
        path: path.into(),
        value,
    }
}

/// What a command sends and how it completes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Plan {
    /// A read, completed with the value the device answers.
    Get(String),
    /// Writes, sent at once; AWJ does not acknowledge them.
    Replace(Vec<Msg>),
    /// Writes addressed to program or preview of a screen: the paths and
    /// values hold the PGM/PRV tokens until the T-bar position is known.
    Banked { scope: Scope, msgs: Vec<Msg> },
    /// Add path prefixes to the subscription list.
    Subscribe(Vec<String>),
}

/// A get command as sent: compact JSON in the guides' key order, then 0x04.
pub(crate) fn get_frame(path: &str) -> Vec<u8> {
    frame(&json!({"op": "get", "path": path}))
}

/// A replace command as sent.
pub(crate) fn replace_frame(path: &str, value: &Value) -> Vec<u8> {
    frame(&json!({"op": "replace", "path": path, "value": value}))
}

fn frame(v: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(v).expect("a JSON value serialises");
    bytes.push(END);
    bytes
}

/// Splits the device's stream at 0x04 into JSON messages.
#[derive(Debug, Default)]
pub(crate) struct Deframer {
    buf: Vec<u8>,
}

impl Deframer {
    /// Complete messages, and why any were discarded.
    pub(crate) fn feed(&mut self, data: &[u8]) -> (Vec<Value>, Vec<String>) {
        self.buf.extend_from_slice(data);
        let mut out = Vec::new();
        let mut bad = Vec::new();
        while let Some(end) = self.buf.iter().position(|&b| b == END) {
            let chunk: Vec<u8> = self.buf.drain(..=end).collect();
            let text = String::from_utf8_lossy(&chunk[..chunk.len() - 1]);
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(text) {
                Ok(v) => out.push(v),
                Err(e) => bad.push(format!("{e}: {text}")),
            }
        }
        if self.buf.len() > MAX_BUFFER {
            self.buf.clear();
            bad.push(format!("no 0x04 terminator in {MAX_BUFFER} bytes"));
        }
        (out, bad)
    }
}

// --- Commands --------------------------------------------------------------

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn int(p: &Params, key: &str) -> Result<i64, CommandError> {
    p.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn text<'a>(p: &'a Params, key: &str) -> Result<&'a str, CommandError> {
    p.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn flag(p: &Params, key: &str) -> Result<bool, CommandError> {
    p.get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(format!("'{key}' is required")))
}

fn screen(p: &Params) -> Result<Scope, CommandError> {
    Ok(Scope {
        aux: false,
        n: int(p, "screen")?,
    })
}

fn aux(p: &Params) -> Result<Scope, CommandError> {
    Ok(Scope {
        aux: true,
        n: int(p, "aux")?,
    })
}

/// The bank token for "destination", which defaults to preview.
fn side(p: &Params) -> &'static str {
    match p.get("destination").and_then(Value::as_str) {
        Some("program") => PGM,
        _ => PRV,
    }
}

/// PROGRAM or PREVIEW for preset recalls.
fn recall_target(p: &Params) -> &'static str {
    match p.get("destination").and_then(Value::as_str) {
        Some("program") => "PROGRAM",
        _ => "PREVIEW",
    }
}

/// "NONE" for 0, otherwise the prefix and number.
fn numbered_or_none(prefix: &str, n: i64) -> String {
    if n == 0 {
        "NONE".into()
    } else {
        format!("{prefix}{n}")
    }
}

/// Commands both dialects share.
fn common(name: &str, p: &Params) -> Option<Result<Plan, CommandError>> {
    Some((|| {
        Ok(match name {
            "awj_get" => Plan::Get(text(p, "path")?.trim().to_string()),
            "awj_replace" => {
                let value = p
                    .get("value")
                    .cloned()
                    .ok_or_else(|| invalid("'value' is required"))?;
                Plan::Replace(vec![msg(text(p, "path")?.trim(), value)])
            }
            "subscribe" => {
                let list = p
                    .get("paths")
                    .and_then(Value::as_array)
                    .filter(|l| !l.is_empty())
                    .ok_or_else(|| invalid("'paths' must be a non-empty array of path prefixes"))?;
                let paths = list
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .ok_or_else(|| invalid("'paths' must hold only non-empty strings"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Plan::Subscribe(paths)
            }
            _ => {
                return Err(CommandError::UnknownCommand {
                    command: name.into(),
                })
            }
        })
    })())
    .filter(|r| !matches!(r, Err(CommandError::UnknownCommand { .. })))
}

/// A command to what it sends, for one dialect. Parameters have already been
/// checked against the spec.
pub(crate) fn plan(d: Dialect, name: &str, p: &Params) -> Result<Plan, CommandError> {
    if let Some(plan) = common(name, p) {
        return plan;
    }
    match d {
        Dialect::LivePremier => plan_livepremier(name, p),
        Dialect::Midra4k => plan_midra(name, p),
    }
}

fn banked(scope: Scope, msgs: Vec<Msg>) -> Plan {
    Plan::Banked { scope, msgs }
}

fn plan_livepremier(name: &str, p: &Params) -> Result<Plan, CommandError> {
    let d = Dialect::LivePremier;
    const UPDATE: &str = "DeviceObject/$screenAuxGroup/control/@props/xUpdate";
    let layer_path = |s: &Scope, layer: &str, tail: &str| {
        format!(
            "DeviceObject/{}/@items/{}/$preset/@items/{PRV}/$layer/@items/{layer}/{tail}",
            s.list(d),
            s.item(d)
        )
        .replace(PRV, side(p))
    };
    let audio = "DeviceObject/audio/control/$device/@items/1";
    Ok(match name {
        "take" | "take_aux" => {
            let s = if name == "take" { screen(p)? } else { aux(p)? };
            Plan::Replace(vec![msg(
                format!(
                    "DeviceObject/$screenAuxGroup/@items/{}/control/@props/xTake",
                    s.item(d)
                ),
                json!(true),
            )])
        }
        "recall_screen_preset" | "recall_aux_preset" => {
            let s = if name == "recall_screen_preset" { screen(p)? } else { aux(p)? };
            Plan::Replace(vec![msg(
                format!(
                    "DeviceObject/presetBank/control/load/$slot/@items/{}/{}/@items/{}/$preset/@items/{}/@props/xRequest",
                    int(p, "preset")?,
                    s.list(d),
                    s.item(d),
                    recall_target(p)
                ),
                json!(true),
            )])
        }
        "recall_master_preset" => Plan::Replace(vec![msg(
            format!(
                "DeviceObject/masterPresetBank/control/load/$slot/@items/{}/$preset/@items/{}/@props/xRequest",
                int(p, "preset")?,
                recall_target(p)
            ),
            json!(true),
        )]),
        "recall_multiviewer_preset" => Plan::Replace(vec![msg(
            format!(
                "DeviceObject/monitoringBank/control/load/$slot/@items/{}/$output/@items/{}/@props/xRequest",
                int(p, "preset")?,
                int(p, "multiviewer")?
            ),
            json!(true),
        )]),
        "set_layer_source" | "set_aux_layer_source" => {
            let s = if name == "set_layer_source" { screen(p)? } else { aux(p)? };
            let path = layer_path(&s, &int(p, "layer")?.to_string(), "source/@props/inputNum");
            banked(
                s,
                vec![msg(path, json!(text(p, "source")?)), msg(UPDATE, json!(true))],
            )
        }
        "set_background" | "set_aux_background" => {
            let s = if name == "set_background" { screen(p)? } else { aux(p)? };
            let path = layer_path(&s, "NATIVE", "source/@props/inputNum");
            banked(
                s,
                vec![
                    msg(path, json!(numbered_or_none("NATIVE_", int(p, "set")?))),
                    msg(UPDATE, json!(true)),
                ],
            )
        }
        "set_layer_keying" => {
            let s = screen(p)?;
            let path = layer_path(&s, &int(p, "layer")?.to_string(), "keying/@props/source");
            banked(s, vec![msg(path, json!(numbered_or_none("SLOT_", int(p, "slot")?)))])
        }
        "enable_layer_keying" => {
            let s = screen(p)?;
            let path = layer_path(&s, &int(p, "layer")?.to_string(), "keying/@props/enable");
            banked(s, vec![msg(path, json!(flag(p, "enabled")?))])
        }
        "freeze_layer" => {
            // The value lists the banks frozen (Aquilon guide 3.5).
            let s = screen(p)?;
            let mut banks = Vec::new();
            if flag(p, "program")? {
                banks.push(PGM);
            }
            if flag(p, "preview")? {
                banks.push(PRV);
            }
            let path = format!(
                "DeviceObject/$screen/@items/{}/$layer/@items/{}/control/@props/freeze",
                s.item(d),
                int(p, "layer")?
            );
            banked(s, vec![msg(path, json!(banks))])
        }
        "freeze_input" => Plan::Replace(vec![msg(
            format!(
                "DeviceObject/$input/@items/IN_{}/control/@props/freeze",
                int(p, "input")?
            ),
            json!(flag(p, "frozen")?),
        )]),
        "update" => Plan::Replace(vec![msg(UPDATE, json!(true))]),
        "mute_input_audio" => Plan::Replace(vec![msg(
            format!(
                "{audio}/$rx/@items/INPUT_{}_CHANNEL_{}/control/@props/mute",
                int(p, "input")?,
                int(p, "channel")?
            ),
            json!(flag(p, "muted")?),
        )]),
        "mute_dante_input_audio" => Plan::Replace(vec![msg(
            format!(
                "{audio}/$rx/@items/DANTE_{}_CHANNEL_{}/control/@props/mute",
                int(p, "group")?,
                int(p, "channel")?
            ),
            json!(flag(p, "muted")?),
        )]),
        "mute_output_audio" | "set_output_audio_source" => {
            let base = format!(
                "{audio}/$tx/@items/{}_{}/$channel/@items/{}/control/@props",
                text(p, "output_type")?,
                int(p, "output")?,
                int(p, "channel")?
            );
            if name == "mute_output_audio" {
                Plan::Replace(vec![msg(format!("{base}/mute"), json!(flag(p, "muted")?))])
            } else {
                Plan::Replace(vec![msg(format!("{base}/source"), json!(text(p, "source")?))])
            }
        }
        "set_multiviewer_widget_source" => Plan::Replace(vec![msg(
            // Widgets are numbered from 0 on the wire (guide 4.3).
            format!(
                "DeviceObject/$monitoring/@items/{}/layout/$widget/@items/{}/control/@props/source",
                int(p, "multiviewer")?,
                int(p, "widget")? - 1
            ),
            json!(text(p, "source")?),
        )]),
        "set_multiviewer_vu_meter" => Plan::Replace(vec![msg(
            format!(
                "DeviceObject/$monitoring/@items/{}/layout/control/vuMeters/@props/widget",
                int(p, "multiviewer")?
            ),
            json!(text(p, "widget")?),
        )]),
        "trigger_input_backup" | "set_input_backup_auto" => {
            let base = format!("DeviceObject/$input/@items/IN_{}/backup/control/@props", int(p, "input")?);
            backup(name.starts_with("trigger"), &base, p)?
        }
        "trigger_background_backup" | "set_background_backup_auto" => {
            let base = format!(
                "DeviceObject/preconfig/backgrounds/$screen/@items/S{}/$backgroundSet/@items/{}/backup/control/@props",
                int(p, "screen")?,
                int(p, "set")?
            );
            backup(name.starts_with("trigger"), &base, p)?
        }
        "trigger_group_backup" | "set_group_backup_auto" => {
            let base = format!(
                "DeviceObject/backup/$group/@items/GROUP_{}/control/@props",
                int(p, "group")?
            );
            backup(name.starts_with("trigger"), &base, p)?
        }
        "reboot" | "shutdown" => Plan::Replace(vec![msg(
            "DeviceObject/system/shutdown/cmd/@props/xRequest",
            json!(if name == "reboot" { "REBOOT" } else { "SHUTDOWN" }),
        )]),
        other => {
            return Err(CommandError::UnknownCommand {
                command: other.into(),
            })
        }
    })
}

/// A manual backup selection ("xSelectSlot") or the automatic mode.
fn backup(trigger: bool, base: &str, p: &Params) -> Result<Plan, CommandError> {
    Ok(if trigger {
        Plan::Replace(vec![msg(
            format!("{base}/xSelectSlot"),
            json!(text(p, "slot")?),
        )])
    } else {
        Plan::Replace(vec![msg(
            format!("{base}/enableAutoSelect"),
            json!(flag(p, "enabled")?),
        )])
    })
}

fn plan_midra(name: &str, p: &Params) -> Result<Plan, CommandError> {
    let d = Dialect::Midra4k;
    const UPDATE: &str = "DeviceObject/preset/control/@props/xUpdate";
    let preset_path = |s: &Scope, tail: &str| {
        format!(
            "DeviceObject/{}/@items/{}/$preset/@items/{}/{tail}",
            s.list(d),
            s.n,
            side(p)
        )
    };
    let one = |path: String, value: Value| Ok(Plan::Replace(vec![msg(path, value)]));
    let audio_mode = |base: &str| -> Result<Plan, CommandError> {
        one(
            format!("{base}/control/@props/mode"),
            json!(text(p, "mode")?),
        )
    };
    let audio_source = |base: &str| -> Result<Plan, CommandError> {
        one(
            format!("{base}/control/directRouting/@props/source"),
            json!(text(p, "source")?),
        )
    };
    match name {
        "take" | "take_aux" => {
            let s = if name == "take" { screen(p)? } else { aux(p)? };
            one(
                format!(
                    "DeviceObject/transition/{}/@items/{}/control/@props/xTake",
                    s.list(d),
                    s.n
                ),
                json!(true),
            )
        }
        "recall_screen_preset" => one(
            format!(
                "DeviceObject/preset/bank/control/load/$slot/@items/{}/$screen/@items/{}/$preset/@items/{}/@props/xRequest",
                int(p, "preset")?,
                int(p, "screen")?,
                recall_target(p)
            ),
            json!(true),
        ),
        // "$auxillaryScreen" is spelled as the guides print it (3.4).
        "recall_aux_preset" => one(
            format!(
                "DeviceObject/preset/auxBank/control/load/$slot/@items/{}/$auxillaryScreen/@items/{}/$preset/@items/{}/@props/xRequest",
                int(p, "preset")?,
                int(p, "aux")?,
                recall_target(p)
            ),
            json!(true),
        ),
        "recall_master_preset" => one(
            format!(
                "DeviceObject/preset/masterBank/control/load/$slot/@items/{}/$preset/@items/{}/@props/xRequest",
                int(p, "preset")?,
                recall_target(p)
            ),
            json!(true),
        ),
        "recall_multiviewer_preset" => one(
            format!(
                "DeviceObject/multiviewer/$bank/control/load/$slot/@items/{}/@props/xRequest",
                int(p, "preset")?
            ),
            json!(true),
        ),
        "set_layer_source" => {
            let s = screen(p)?;
            let path = preset_path(&s, &format!("$liveLayer/@items/{}/source/@props/input", int(p, "layer")?));
            Ok(banked(s, vec![msg(path, json!(text(p, "source")?)), msg(UPDATE, json!(true))]))
        }
        "set_background" => {
            let s = screen(p)?;
            let path = preset_path(&s, "background/source/@props/set");
            let set = int(p, "set")?;
            let value = if set == 0 { "NONE".to_string() } else { set.to_string() };
            Ok(banked(s, vec![msg(path, json!(value)), msg(UPDATE, json!(true))]))
        }
        "set_foreground" => {
            let s = screen(p)?;
            let path = preset_path(&s, "top/source/@props/frame");
            let frame = int(p, "frame")?;
            let value = if frame == 0 { "NONE".to_string() } else { frame.to_string() };
            Ok(banked(s, vec![msg(path, json!(value)), msg(UPDATE, json!(true))]))
        }
        "set_aux_source" => {
            let s = aux(p)?;
            let path = preset_path(&s, "background/source/@props/content");
            Ok(banked(s, vec![msg(path, json!(text(p, "source")?))]))
        }
        "set_foreground_image" | "set_background_image" => {
            let list = if name == "set_foreground_image" { "$topFrame" } else { "$backFrame" };
            one(
                format!(
                    "DeviceObject/$screen/@items/{}/{list}/@items/{}/control/@props/librarySlot",
                    int(p, "screen")?,
                    int(p, "image")?
                ),
                json!(int(p, "library_slot")?.to_string()),
            )
        }
        "freeze_screen" => one(
            format!("DeviceObject/$screen/@items/{}/control/@props/freeze", int(p, "screen")?),
            json!(flag(p, "frozen")?),
        ),
        "freeze_layer" => one(
            format!(
                "DeviceObject/$screen/@items/{}/$liveLayer/@items/{}/control/@props/freeze",
                int(p, "screen")?,
                int(p, "layer")?
            ),
            json!(flag(p, "frozen")?),
        ),
        "freeze_input" => one(
            format!("DeviceObject/$input/@items/INPUT_{}/control/@props/freeze", int(p, "input")?),
            json!(flag(p, "frozen")?),
        ),
        "enable_quick_preset" => one(
            "DeviceObject/quickPreset/control/@props/enable".into(),
            json!(flag(p, "enabled")?),
        ),
        "update" => one(UPDATE.into(), json!(true)),
        "set_multiviewer_widget_source" => one(
            format!(
                "DeviceObject/multiviewer/$widget/@items/{}/control/@props/source",
                int(p, "widget")?
            ),
            json!(text(p, "source")?),
        ),
        "set_multiviewer_vu_meter" => one(
            "DeviceObject/multiviewer/audio/control/vuMeters/@props/widget".into(),
            json!(int(p, "widget")?.to_string()),
        ),
        "set_output_audio_mode" => audio_mode(&format!("DeviceObject/$output/@items/{}/audio", int(p, "output")?)),
        "set_output_audio_source" => audio_source(&format!("DeviceObject/$output/@items/{}/audio", int(p, "output")?)),
        "set_screen_audio_mode" => audio_mode(&format!("DeviceObject/$screen/@items/{}/audio", int(p, "screen")?)),
        "set_screen_audio_source" => audio_source(&format!("DeviceObject/$screen/@items/{}/audio", int(p, "screen")?)),
        "set_screen_audio_follow_layer" => one(
            format!(
                "DeviceObject/$screen/@items/{}/audio/control/followLiveLayer/@props/layer",
                int(p, "screen")?
            ),
            json!(int(p, "layer")?.to_string()),
        ),
        "set_screen_audio_layer_source" | "set_aux_audio_layer_source" => {
            let s = if name == "set_screen_audio_layer_source" { screen(p)? } else { aux(p)? };
            let path = preset_path(&s, "audio/control/@props/source");
            Ok(banked(s, vec![msg(path, json!(text(p, "source")?))]))
        }
        "set_aux_audio_mode" => audio_mode(&format!("DeviceObject/$auxiliaryScreen/@items/{}/audio", int(p, "aux")?)),
        "set_aux_audio_source" => audio_source(&format!("DeviceObject/$auxiliaryScreen/@items/{}/audio", int(p, "aux")?)),
        "mute_output_audio" => one(
            format!("DeviceObject/audio/$output/@items/VIDEO_OUT_{}/control/@props/mute", int(p, "output")?),
            json!(flag(p, "muted")?),
        ),
        "mute_screen_audio" => one(
            format!("DeviceObject/audio/$screen/@items/{}/control/@props/mute", int(p, "screen")?),
            json!(flag(p, "muted")?),
        ),
        "mute_aux_audio" => one(
            format!("DeviceObject/audio/$auxiliaryScreen/@items/{}/control/@props/mute", int(p, "aux")?),
            json!(flag(p, "muted")?),
        ),
        "mute_input_audio" => one(
            format!(
                "DeviceObject/audio/$input/@items/{}/$channel/@items/{}/control/@props/mute",
                text(p, "input")?,
                int(p, "channel")?
            ),
            json!(flag(p, "muted")?),
        ),
        "set_dante_audio_mode" => audio_mode(&format!("DeviceObject/audio/dante/$outputGroup/@items/{}", int(p, "group")?)),
        "set_dante_audio_source" => audio_source(&format!("DeviceObject/audio/dante/$outputGroup/@items/{}", int(p, "group")?)),
        "set_dante_follow_screen" => one(
            format!(
                "DeviceObject/audio/dante/$outputGroup/@items/{}/control/followScreen/@props/source",
                int(p, "group")?
            ),
            json!(int(p, "screen")?.to_string()),
        ),
        "set_line_out_mode" => audio_mode(&format!("DeviceObject/audio/$lineOut/@items/{}", int(p, "line_out")?)),
        "set_line_out_source" => audio_source(&format!("DeviceObject/audio/$lineOut/@items/{}", int(p, "line_out")?)),
        "set_line_out_pair" => one(
            format!(
                "DeviceObject/audio/$lineOut/@items/{}/control/@props/selectedAudioPair",
                int(p, "line_out")?
            ),
            json!(text(p, "pair")?),
        ),
        "set_line_out_follow_screen" => one(
            format!(
                "DeviceObject/audio/$lineOut/@items/{}/control/followScreen/@props/source",
                int(p, "line_out")?
            ),
            json!(int(p, "screen")?.to_string()),
        ),
        "set_multiviewer_output_audio_mode" => audio_mode("DeviceObject/$output/@items/MTVW/audio"),
        "set_multiviewer_output_audio_source" => audio_source("DeviceObject/$output/@items/MTVW/audio"),
        "set_multiviewer_audio_mode" => audio_mode("DeviceObject/multiviewer/audio"),
        "set_multiviewer_audio_source" => audio_source("DeviceObject/multiviewer/audio"),
        "set_multiviewer_audio_follow_widget" => one(
            "DeviceObject/multiviewer/audio/control/followWidget/@props/widget".into(),
            json!(int(p, "widget")?.to_string()),
        ),
        "reboot" => one("DeviceObject/system/shutdown/@props/xReboot".into(), json!(true)),
        "standby" | "switch_off" | "wake_up" => one(
            "DeviceObject/system/shutdown/standby/control/@props/xRequest".into(),
            json!(match name {
                "standby" => "STANDBY",
                "switch_off" => "SWITCH_OFF",
                _ => "WAKE_UP",
            }),
        ),
        other => Err(CommandError::UnknownCommand {
            command: other.into(),
        }),
    }
}

/// Replace the bank tokens in a path or (recursively) a value.
fn resolve_text(s: &str, pgm: &str, prv: &str) -> String {
    s.replace(PGM, pgm).replace(PRV, prv)
}

fn resolve_value(v: &Value, pgm: &str, prv: &str) -> Value {
    match v {
        Value::String(s) => Value::String(resolve_text(s, pgm, prv)),
        Value::Array(a) => Value::Array(a.iter().map(|x| resolve_value(x, pgm, prv)).collect()),
        other => other.clone(),
    }
}

/// The messages of a banked plan for a T-bar position, or None when the
/// position is not one the guides map (AT_DOWN or AT_UP).
pub(crate) fn resolve(d: Dialect, transition: &str, msgs: &[Msg]) -> Option<Vec<Msg>> {
    let pgm = d.bank(transition, true)?;
    let prv = d.bank(transition, false)?;
    Some(
        msgs.iter()
            .map(|m| Msg {
                path: resolve_text(&m.path, pgm, prv),
                value: resolve_value(&m.value, pgm, prv),
            })
            .collect(),
    )
}

// --- State -----------------------------------------------------------------

/// Where a reported property lands in the state.
enum Target {
    /// A fixed path; `{n}` is replaced by capture n, `{n+1}` by capture n
    /// plus one.
    Plain(&'static str),
    /// A screen's T-bar position: captures (kind, number).
    Transition,
    /// A value held per bank: captures (kind, number, bank) and the sub-path
    /// template under the bank, using later captures.
    Banked(&'static str),
}

struct Rule {
    re: Regex,
    target: Target,
}

fn rules(d: Dialect) -> Vec<Rule> {
    let table: &[(&str, Target)] = match d {
        Dialect::LivePremier => &[
            (
                r"^DeviceObject/system/\$device/@items/1/@props/dev$",
                Target::Plain("device.type"),
            ),
            (
                r"^DeviceObject/system/\$device/@items/1/serial/@props/serialNumber$",
                Target::Plain("device.serial"),
            ),
            (
                r"^DeviceObject/system/\$device/@items/1/version/@props/updater$",
                Target::Plain("device.firmware"),
            ),
            (
                r"^DeviceObject/\$screenAuxGroup/@items/([SA])(\d+)/status/@props/transition$",
                Target::Transition,
            ),
            (
                r"^DeviceObject/\$(screen|auxiliary)/@items/[SA](\d+)/control/@props/label$",
                Target::Plain("{1}.{2}.label"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliary)/@items/[SA](\d+)/\$preset/@items/(A|B)/\$layer/@items/(\d+)/source/@props/inputNum$",
                Target::Banked("layers.{4}.source"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliary)/@items/[SA](\d+)/\$preset/@items/(A|B)/\$layer/@items/NATIVE/source/@props/inputNum$",
                Target::Banked("background"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliary)/@items/[SA](\d+)/\$preset/@items/(A|B)/\$layer/@items/(\d+)/keying/@props/source$",
                Target::Banked("layers.{4}.keying_source"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliary)/@items/[SA](\d+)/\$preset/@items/(A|B)/\$layer/@items/(\d+)/keying/@props/enable$",
                Target::Banked("layers.{4}.keying_enabled"),
            ),
            (
                r"^DeviceObject/presetBank/status/presetId/\$(?:screen|auxiliary)/@items/([SA])(\d+)/\$preset/@items/(A|B)/@props/id$",
                Target::Banked("preset"),
            ),
            (
                r"^DeviceObject/\$screen/@items/S(\d+)/\$layer/@items/(\d+)/control/@props/freeze$",
                Target::Plain("screens.{1}.layer_freeze.{2}"),
            ),
            (
                r"^DeviceObject/masterPresetBank/status/lastUsed/\$presetMode/@items/PROGRAM/@props/memoryId$",
                Target::Plain("master_preset.program"),
            ),
            (
                r"^DeviceObject/masterPresetBank/status/lastUsed/\$presetMode/@items/PREVIEW/@props/memoryId$",
                Target::Plain("master_preset.preview"),
            ),
            (
                r"^DeviceObject/presetBank/\$bank/@items/(\d+)/control/@props/label$",
                Target::Plain("presets.{1}.label"),
            ),
            (
                r"^DeviceObject/masterPresetBank/\$bank/@items/(\d+)/control/@props/label$",
                Target::Plain("master_presets.{1}.label"),
            ),
            (
                r"^DeviceObject/\$input/@items/IN_(\d+)/control/@props/label$",
                Target::Plain("inputs.{1}.label"),
            ),
            (
                r"^DeviceObject/\$input/@items/IN_(\d+)/control/@props/freeze$",
                Target::Plain("inputs.{1}.frozen"),
            ),
            (
                r"^DeviceObject/\$input/@items/IN_(\d+)/status/@props/isAvailable$",
                Target::Plain("inputs.{1}.available"),
            ),
            (
                r"^DeviceObject/\$input/@items/IN_(\d+)/status/@props/isEnabled$",
                Target::Plain("inputs.{1}.enabled"),
            ),
            (
                r"^DeviceObject/\$input/@items/IN_(\d+)/status/@props/capability$",
                Target::Plain("inputs.{1}.capability"),
            ),
            (
                r"^DeviceObject/\$monitoring/@items/(\d+)/control/@props/label$",
                Target::Plain("multiviewers.{1}.label"),
            ),
            (
                r"^DeviceObject/\$monitoring/@items/(\d+)/status/@props/isEnabled$",
                Target::Plain("multiviewers.{1}.enabled"),
            ),
            (
                r"^DeviceObject/\$monitoring/@items/(\d+)/layout/\$widget/@items/(\d+)/control/@props/source$",
                Target::Plain("multiviewers.{1}.widgets.{2+1}.source"),
            ),
        ],
        Dialect::Midra4k => &[
            (
                r"^DeviceObject/system/@props/dev$",
                Target::Plain("device.type"),
            ),
            (
                r"^DeviceObject/system/serial/@props/serialNumber$",
                Target::Plain("device.serial"),
            ),
            (
                r"^DeviceObject/system/version/@props/updater$",
                Target::Plain("device.firmware"),
            ),
            (
                r"^DeviceObject/transition/\$(screen|auxiliaryScreen)/@items/(\d+)/status/@props/transition$",
                Target::Transition,
            ),
            (
                r"^DeviceObject/\$(screen|auxiliaryScreen)/@items/(\d+)/control/@props/label$",
                Target::Plain("{1}.{2}.label"),
            ),
            (
                r"^DeviceObject/\$screen/@items/(\d+)/control/@props/freeze$",
                Target::Plain("screens.{1}.frozen"),
            ),
            (
                r"^DeviceObject/\$screen/@items/(\d+)/\$liveLayer/@items/(\d+)/control/@props/freeze$",
                Target::Plain("screens.{1}.layer_frozen.{2}"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliaryScreen)/@items/(\d+)/\$preset/@items/(DOWN|UP)/\$liveLayer/@items/(\d+)/source/@props/input$",
                Target::Banked("layers.{4}.source"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliaryScreen)/@items/(\d+)/\$preset/@items/(DOWN|UP)/\$liveLayer/@items/(\d+)/status/@props/state$",
                Target::Banked("layers.{4}.state"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliaryScreen)/@items/(\d+)/\$preset/@items/(DOWN|UP)/background/source/@props/set$",
                Target::Banked("background"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliaryScreen)/@items/(\d+)/\$preset/@items/(DOWN|UP)/background/source/@props/content$",
                Target::Banked("source"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliaryScreen)/@items/(\d+)/\$preset/@items/(DOWN|UP)/background/status/@props/state$",
                Target::Banked("background_state"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliaryScreen)/@items/(\d+)/\$preset/@items/(DOWN|UP)/top/source/@props/frame$",
                Target::Banked("foreground"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliaryScreen)/@items/(\d+)/\$preset/@items/(DOWN|UP)/top/status/@props/state$",
                Target::Banked("foreground_state"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliaryScreen)/@items/(\d+)/\$preset/@items/(DOWN|UP)/status/@props/memoryId$",
                Target::Banked("preset"),
            ),
            (
                r"^DeviceObject/\$(screen|auxiliaryScreen)/@items/(\d+)/\$preset/@items/(DOWN|UP)/audio/status/@props/source$",
                Target::Banked("audio_source"),
            ),
            (
                r"^DeviceObject/preset/masterBank/status/lastUsed/\$presetMode/@items/PROGRAM/@props/memoryId$",
                Target::Plain("master_preset.program"),
            ),
            (
                r"^DeviceObject/preset/masterBank/status/lastUsed/\$presetMode/@items/PREVIEW/@props/memoryId$",
                Target::Plain("master_preset.preview"),
            ),
            (
                r"^DeviceObject/preset/bank/\$slot/@items/(\d+)/control/@props/label$",
                Target::Plain("presets.{1}.label"),
            ),
            (
                r"^DeviceObject/preset/masterBank/\$slot/@items/(\d+)/control/@props/label$",
                Target::Plain("master_presets.{1}.label"),
            ),
            (
                r"^DeviceObject/preset/auxBank/\$slot/@items/(\d+)/control/@props/label$",
                Target::Plain("aux_presets.{1}.label"),
            ),
            (
                r"^DeviceObject/\$input/@items/INPUT_(\d+)/control/@props/freeze$",
                Target::Plain("inputs.{1}.frozen"),
            ),
            (
                r"^DeviceObject/\$input/@items/INPUT_(\d+)/\$plug/@items/(\d+)/control/@props/label$",
                Target::Plain("inputs.{1}.plugs.{2}.label"),
            ),
            (
                r"^DeviceObject/\$input/@items/INPUT_(\d+)/\$plug/@items/(\d+)/status/@props/type$",
                Target::Plain("inputs.{1}.plugs.{2}.type"),
            ),
            (
                r"^DeviceObject/\$input/@items/INPUT_(\d+)/\$plug/@items/(\d+)/status/@props/isAvailable$",
                Target::Plain("inputs.{1}.plugs.{2}.available"),
            ),
            (
                r"^DeviceObject/\$input/@items/INPUT_(\d+)/\$plug/@items/(\d+)/status/signal/@props/isValid$",
                Target::Plain("inputs.{1}.plugs.{2}.valid"),
            ),
            (
                r"^DeviceObject/multiviewer/\$widget/@items/(\d+)/control/@props/source$",
                Target::Plain("multiviewer.widgets.{1}.source"),
            ),
            (
                r"^DeviceObject/multiviewer/\$widget/@items/(\d+)/status/@props/isEnabled$",
                Target::Plain("multiviewer.widgets.{1}.enabled"),
            ),
            (
                r"^DeviceObject/\$output/@items/MTVW/control/@props/label$",
                Target::Plain("multiviewer.label"),
            ),
        ],
    };
    table
        .iter()
        .map(|(re, target)| Rule {
            re: Regex::new(re).expect("rule regex"),
            target: match target {
                Target::Plain(t) => Target::Plain(t),
                Target::Transition => Target::Transition,
                Target::Banked(t) => Target::Banked(t),
            },
        })
        .collect()
}

/// The state list for a path kind: screens or auxiliaries.
fn scope_kind(kind: &str) -> &'static str {
    match kind {
        "S" | "screen" => "screens",
        _ => "auxiliaries",
    }
}

/// Fill `{n}` and `{n+1}` from captures; list kinds become state names.
fn fill(template: &str, caps: &regex::Captures) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let end = start + rest[start..].find('}').expect("closed placeholder");
        let inner = &rest[start + 1..end];
        let (idx, plus) = match inner.split_once('+') {
            Some((i, n)) => (i, n.parse::<i64>().unwrap_or(0)),
            None => (inner, 0),
        };
        let cap = caps
            .get(idx.parse().unwrap_or(0))
            .map(|m| m.as_str())
            .unwrap_or("");
        if plus != 0 {
            out.push_str(&(cap.parse::<i64>().unwrap_or(0) + plus).to_string());
        } else if matches!(cap, "screen" | "auxiliary" | "auxiliaryScreen") {
            out.push_str(scope_kind(cap));
        } else {
            out.push_str(cap);
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Insert `value` at a dotted path in a merge patch.
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
    node.insert(last.to_string(), value);
}

fn patch_at(path: &str, value: Value) -> Value {
    let mut root = Map::new();
    let parts: Vec<&str> = path.split('.').collect();
    insert(&mut root, &parts, value);
    Value::Object(root)
}

/// Every leaf of a value read from `path`: an object answered to a get on an
/// object is taken to mirror the paths beneath it.
fn flatten(path: &str, value: &Value, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(o) if !o.is_empty() => {
            for (k, v) in o {
                flatten(&format!("{path}/{k}"), v, out);
            }
        }
        other => out.push((path.to_string(), other.clone())),
    }
}

/// The friendly projection: rules, T-bar positions and bank contents.
pub(crate) struct Projection {
    dialect: Dialect,
    rules: Vec<Rule>,
    transitions: BTreeMap<(String, String), String>,
    banks: BTreeMap<(String, String), BTreeMap<String, Map<String, Value>>>,
}

impl Projection {
    pub(crate) fn new(dialect: Dialect) -> Projection {
        Projection {
            dialect,
            rules: rules(dialect),
            transitions: BTreeMap::new(),
            banks: BTreeMap::new(),
        }
    }

    fn clear(&mut self) {
        self.transitions.clear();
        self.banks.clear();
    }

    pub(crate) fn transition(&self, key: &(String, String)) -> Option<&str> {
        self.transitions.get(key).map(String::as_str)
    }

    /// The state patches one reported property produces, in order.
    pub(crate) fn apply(&mut self, path: &str, value: &Value) -> Vec<Value> {
        let mut out = Vec::new();
        let Some((rule, caps)) = self
            .rules
            .iter()
            .find_map(|r| r.re.captures(path).map(|c| (r, c)))
        else {
            return out;
        };
        match &rule.target {
            Target::Plain(t) => out.push(patch_at(&fill(t, &caps), value.clone())),
            Target::Transition => {
                let key = (scope_kind(&caps[1]).to_string(), caps[2].to_string());
                let base = format!("{}.{}", key.0, key.1);
                out.push(patch_at(&format!("{base}.transition"), value.clone()));
                if let Some(t) = value.as_str() {
                    let changed = self.transitions.get(&key).map(String::as_str) != Some(t);
                    self.transitions.insert(key.clone(), t.to_string());
                    if changed {
                        out.extend(self.sides(&key));
                    }
                }
            }
            Target::Banked(t) => {
                let key = (scope_kind(&caps[1]).to_string(), caps[2].to_string());
                let bank = caps[3].to_string();
                let sub = fill(t, &caps);
                let parts: Vec<&str> = sub.split('.').collect();
                let held = self
                    .banks
                    .entry(key.clone())
                    .or_default()
                    .entry(bank.clone())
                    .or_default();
                insert(held, &parts, value.clone());
                let base = format!("{}.{}", key.0, key.1);
                out.push(patch_at(
                    &format!("{base}.banks.{bank}.{sub}"),
                    value.clone(),
                ));
                if let Some(t) = self.transitions.get(&key) {
                    for (side, program) in [("program", true), ("preview", false)] {
                        if self.dialect.bank(t, program) == Some(bank.as_str()) {
                            out.push(patch_at(&format!("{base}.{side}.{sub}"), value.clone()));
                        }
                    }
                }
            }
        }
        out
    }

    /// Program and preview rebuilt from the banks after the T-bar moved.
    fn sides(&self, key: &(String, String)) -> Vec<Value> {
        let Some(t) = self.transitions.get(key) else {
            return Vec::new();
        };
        let base = format!("{}.{}", key.0, key.1);
        let mut out = vec![patch_at(&base, json!({"program": null, "preview": null}))];
        let mut sides = Map::new();
        for (side, program) in [("program", true), ("preview", false)] {
            if let Some(bank) = self.dialect.bank(t, program) {
                let held = self
                    .banks
                    .get(key)
                    .and_then(|b| b.get(bank))
                    .cloned()
                    .unwrap_or_default();
                sides.insert(side.into(), Value::Object(held));
            }
        }
        out.push(patch_at(&base, Value::Object(sides)));
        out
    }
}

// --- Session ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Purpose {
    /// The first read: its answer says the device is there.
    Hello,
    Keepalive,
    /// State read on connecting; failures are logged.
    Internal,
    /// A screen's T-bar position, read before a banked command.
    Transition((String, String)),
    Command(CommandId),
}

#[derive(Debug)]
struct PendingGet {
    path: String,
    purpose: Purpose,
    deadline: Millis,
}

#[derive(Debug)]
struct Deferred {
    id: CommandId,
    key: (String, String),
    msgs: Vec<Msg>,
}

pub(crate) struct AnalogWay {
    dialect: Dialect,
    device: SocketAddr,
    read_on_connect: bool,
    subscriptions: Vec<String>,
    deframer: Deframer,
    socket_open: bool,
    ready: bool,
    pending: Vec<PendingGet>,
    deferred: Vec<Deferred>,
    /// Messages sent and not yet known to be answered, oldest first: the
    /// path of each get, None for each replace. The device handles them in
    /// order, so an error, which carries no path, answers the oldest.
    unanswered: VecDeque<Option<String>>,
    projection: Projection,
    last_heard: Millis,
    retry_after: Millis,
}

impl AnalogWay {
    pub(crate) fn new(dialect: Dialect, ctx: OpenContext) -> AnalogWay {
        let mut subscriptions: Vec<String> = dialect
            .subscriptions()
            .iter()
            .map(|s| s.to_string())
            .collect();
        if let Some(extra) = ctx
            .settings
            .get("extra_subscriptions")
            .and_then(Value::as_str)
        {
            for s in extra.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                if !subscriptions.iter().any(|x| x == s) {
                    subscriptions.push(s.to_string());
                }
            }
        }
        AnalogWay {
            dialect,
            device: SocketAddr::new(ctx.host, ctx.port.unwrap_or(DEFAULT_PORT)),
            read_on_connect: ctx
                .settings
                .get("read_on_connect")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            subscriptions,
            deframer: Deframer::default(),
            socket_open: false,
            ready: false,
            pending: Vec::new(),
            deferred: Vec::new(),
            unanswered: VecDeque::new(),
            projection: Projection::new(dialect),
            last_heard: 0,
            retry_after: RETRY_MIN,
        }
    }

    fn open(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.tcp_open(SOCKET, self.device);
    }

    fn get(&mut self, cx: &mut Cx, path: &str, purpose: Purpose) {
        cx.tcp_send(SOCKET, get_frame(path));
        self.note_sent(Some(path.to_string()));
        self.pending.push(PendingGet {
            path: path.to_string(),
            purpose,
            deadline: cx.now() + REPLY_TIMEOUT,
        });
        self.arm_reply_timer(cx);
    }

    fn note_sent(&mut self, get: Option<String>) {
        if self.unanswered.len() >= 1024 {
            self.unanswered.pop_front();
        }
        self.unanswered.push_back(get);
    }

    fn replace(&mut self, cx: &mut Cx, m: &Msg) {
        cx.tcp_send(SOCKET, replace_frame(&m.path, &m.value));
        self.note_sent(None);
    }

    fn subscribe_all(&mut self, cx: &mut Cx) {
        let m = msg("Subscriptions", json!(self.subscriptions));
        self.replace(cx, &m);
    }

    fn arm_reply_timer(&self, cx: &mut Cx) {
        match self.pending.iter().map(|p| p.deadline).min() {
            Some(at) => cx.set_timer(REPLY, at.saturating_sub(cx.now())),
            None => cx.cancel_timer(REPLY),
        }
    }

    fn fail_deferred(&mut self, cx: &mut Cx, key: Option<&(String, String)>, error: CommandError) {
        let (failed, kept): (Vec<Deferred>, Vec<Deferred>) = std::mem::take(&mut self.deferred)
            .into_iter()
            .partition(|d| key.is_none_or(|k| &d.key == k));
        self.deferred = kept;
        for d in failed {
            cx.complete(d.id, Err(error.clone()));
        }
    }

    fn lost(&mut self, cx: &mut Cx, reason: String) {
        let error = CommandError::Transport {
            message: reason.clone(),
        };
        for p in std::mem::take(&mut self.pending) {
            if let Purpose::Command(id) = p.purpose {
                cx.complete(id, Err(error.clone()));
            }
        }
        self.fail_deferred(cx, None, error);
        for key in [REPLY, KEEPALIVE] {
            cx.cancel_timer(key);
        }
        if self.socket_open {
            cx.tcp_close(SOCKET);
        }
        self.socket_open = false;
        self.ready = false;
        self.unanswered.clear();
        self.deframer = Deframer::default();
        // Positions may change while disconnected; read them again.
        self.projection.clear();
        cx.connection(Connection::Disconnected { reason });
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn become_ready(&mut self, cx: &mut Cx) {
        self.ready = true;
        self.retry_after = RETRY_MIN;
        cx.connection(Connection::Connected);
        self.subscribe_all(cx);
        let d = self.dialect;
        for path in [d.serial_path(), d.firmware_path()] {
            self.get(cx, path, Purpose::Internal);
        }
        if self.read_on_connect {
            for path in d.subscriptions() {
                self.get(cx, path, Purpose::Internal);
            }
        }
    }

    /// Apply a reported property to the state.
    fn report(&mut self, cx: &mut Cx, path: &str, value: &Value) {
        let mut leaves = Vec::new();
        flatten(path, value, &mut leaves);
        let mut raw = Map::new();
        let mut moved = Vec::new();
        for (leaf, v) in &leaves {
            raw.insert(leaf.clone(), v.clone());
            for patch in self.projection.apply(leaf, v) {
                cx.state(patch);
            }
            moved.extend(self.positions_known(leaf));
        }
        cx.state(json!({ "awj": raw }));
        for key in moved {
            self.flush(cx, &key);
        }
    }

    /// Scopes whose T-bar position `path` reported.
    fn positions_known(&self, path: &str) -> Vec<(String, String)> {
        self.deferred
            .iter()
            .map(|d| d.key.clone())
            .filter(|k| {
                self.projection.transition(k).is_some()
                    && transition_path_of(self.dialect, k) == path
            })
            .collect()
    }

    /// Send the commands waiting for a screen's T-bar position.
    fn flush(&mut self, cx: &mut Cx, key: &(String, String)) {
        let Some(t) = self.projection.transition(key).map(str::to_string) else {
            return;
        };
        let (ready, kept): (Vec<Deferred>, Vec<Deferred>) = std::mem::take(&mut self.deferred)
            .into_iter()
            .partition(|d| &d.key == key);
        self.deferred = kept;
        for d in ready {
            self.send_banked(cx, d.id, &t, &d.msgs);
        }
    }

    fn send_banked(&mut self, cx: &mut Cx, id: CommandId, transition: &str, msgs: &[Msg]) {
        match resolve(self.dialect, transition, msgs) {
            Some(resolved) => {
                for m in &resolved {
                    self.replace(cx, m);
                }
                cx.complete(id, Ok(Outcome::Unverified));
            }
            None => cx.complete(
                id,
                Err(CommandError::DeviceError {
                    code: None,
                    message: format!(
                        "the T-bar is at '{transition}', not AT_DOWN or AT_UP, so program and preview cannot be told apart"
                    ),
                }),
            ),
        }
    }

    fn message(&mut self, cx: &mut Cx, v: Value) {
        if let Some(error) = v.get("error") {
            let code = error
                .get("code")
                .and_then(Value::as_str)
                .map(str::to_string);
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("error")
                .to_string();
            cx.state(json!({"last_error": {"code": code, "message": message}}));
            // The oldest unanswered message: a get it answers, or a write the
            // device refused (writes it accepts are never answered, so an
            // older accepted write cannot be told from a refused one).
            let owner = match self.unanswered.pop_front() {
                Some(Some(path)) => self.pending.iter().position(|p| p.path == path),
                _ => None,
            };
            match owner {
                Some(i) => {
                    let p = self.pending.remove(i);
                    self.arm_reply_timer(cx);
                    let err = CommandError::DeviceError {
                        code: code.clone(),
                        message: message.clone(),
                    };
                    match p.purpose {
                        Purpose::Command(id) => cx.complete(id, Err(err)),
                        Purpose::Transition(key) => self.fail_deferred(cx, Some(&key), err),
                        // The device answers, if not with its type.
                        Purpose::Hello if !self.ready => self.become_ready(cx),
                        _ => cx.log(
                            Level::Warning,
                            format!("reading {} failed: {message}", p.path),
                        ),
                    }
                }
                None => cx.log(
                    Level::Warning,
                    format!("the device refused a write: {message}"),
                ),
            }
            return;
        }
        let Some(path) = v
            .get("path")
            .and_then(Value::as_str)
            .map(|s| s.trim().to_string())
        else {
            cx.log(
                Level::Debug,
                format!("ignored a message without a path: {v}"),
            );
            return;
        };
        let value = v.get("value").cloned().unwrap_or(Value::Null);
        let answered = self.pending.iter().position(|p| p.path == path);
        if path == "Subscriptions" {
            // The answer to the Subscriptions write (guide 1.4).
            if let Some(i) = self.unanswered.iter().position(Option::is_none) {
                self.unanswered.remove(i);
            }
        } else {
            self.report(cx, &path, &value);
        }
        if let Some(i) = answered {
            let p = self.pending.remove(i);
            // Everything sent before this get has been handled.
            if let Some(at) = self
                .unanswered
                .iter()
                .position(|u| u.as_deref() == Some(path.as_str()))
            {
                self.unanswered.drain(..=at);
            }
            self.arm_reply_timer(cx);
            match p.purpose {
                Purpose::Command(id) => cx.complete(id, Ok(Outcome::Value { value })),
                Purpose::Hello => {
                    if !self.ready {
                        self.become_ready(cx);
                    }
                }
                Purpose::Transition(key) => {
                    if self.projection.transition(&key).is_none() {
                        self.fail_deferred(
                            cx,
                            Some(&key),
                            CommandError::DeviceError {
                                code: None,
                                message: format!("unexpected T-bar position {value}"),
                            },
                        );
                    }
                }
                Purpose::Keepalive | Purpose::Internal => {}
            }
        }
    }

    fn command_plan(&mut self, cx: &mut Cx, id: CommandId, plan: Plan) {
        match plan {
            Plan::Get(path) => self.get(cx, &path, Purpose::Command(id)),
            Plan::Replace(msgs) => {
                for m in &msgs {
                    self.replace(cx, m);
                }
                cx.complete(id, Ok(Outcome::Unverified));
            }
            Plan::Subscribe(paths) => {
                for p in paths {
                    if !self.subscriptions.contains(&p) {
                        self.subscriptions.push(p);
                    }
                }
                self.subscribe_all(cx);
                cx.complete(id, Ok(Outcome::Unverified));
            }
            Plan::Banked { scope, msgs } => {
                let key = scope.key();
                if let Some(t) = self.projection.transition(&key).map(str::to_string) {
                    self.send_banked(cx, id, &t, &msgs);
                    return;
                }
                let path = scope.transition_path(self.dialect);
                let asked = self
                    .pending
                    .iter()
                    .any(|p| p.purpose == Purpose::Transition(key.clone()));
                self.deferred.push(Deferred {
                    id,
                    key: key.clone(),
                    msgs,
                });
                if !asked {
                    self.get(cx, &path, Purpose::Transition(key));
                }
            }
        }
    }
}

/// The transition path for a state scope key.
fn transition_path_of(d: Dialect, key: &(String, String)) -> String {
    Scope {
        aux: key.0 == "auxiliaries",
        n: key.1.parse().unwrap_or(0),
    }
    .transition_path(d)
}

impl Module for AnalogWay {
    fn start(&mut self, cx: &mut Cx) {
        self.open(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if !self.ready {
            cx.complete(id, Err(CommandError::NotConnected));
            return;
        }
        match plan(self.dialect, name, params) {
            Ok(plan) => self.command_plan(cx, id, plan),
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.socket_open = true;
                self.last_heard = cx.now();
                let path = self.dialect.device_type_path();
                self.get(cx, path, Purpose::Hello);
                cx.set_timer(KEEPALIVE, KEEPALIVE_EVERY);
            }
            TcpInput::Data(data) => {
                cx.alive();
                self.last_heard = cx.now();
                let (messages, bad) = self.deframer.feed(&data);
                for reason in bad {
                    cx.log(
                        Level::Warning,
                        format!("discarded a message that is not JSON: {reason}"),
                    );
                }
                for m in messages {
                    if !self.socket_open {
                        return;
                    }
                    self.message(cx, m);
                }
            }
            TcpInput::Closed { reason } => {
                self.socket_open = false;
                self.lost(cx, reason);
            }
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            RETRY => self.open(cx),
            KEEPALIVE => {
                if !self.socket_open {
                    return;
                }
                let asked = self.pending.iter().any(|p| p.purpose == Purpose::Keepalive);
                if !asked && cx.now().saturating_sub(self.last_heard) >= KEEPALIVE_EVERY / 2 {
                    let path = self.dialect.device_type_path();
                    self.get(cx, path, Purpose::Keepalive);
                }
                cx.set_timer(KEEPALIVE, KEEPALIVE_EVERY);
            }
            REPLY => {
                let now = cx.now();
                let (expired, kept): (Vec<PendingGet>, Vec<PendingGet>) =
                    std::mem::take(&mut self.pending)
                        .into_iter()
                        .partition(|p| p.deadline <= now);
                self.pending = kept;
                for p in expired {
                    match p.purpose {
                        Purpose::Command(id) => cx.complete(id, Err(CommandError::Timeout)),
                        Purpose::Transition(key) => {
                            self.fail_deferred(cx, Some(&key), CommandError::Timeout)
                        }
                        Purpose::Hello | Purpose::Keepalive => {
                            self.lost(cx, "the device stopped answering AWJ reads".into());
                            return;
                        }
                        Purpose::Internal => cx.log(
                            Level::Debug,
                            format!("no answer reading {} within {REPLY_TIMEOUT} ms", p.path),
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

    /// A guide example as printed, with the line breaks of the page layout
    /// removed and "\0x04" as the terminator byte.
    fn wire(example: &str) -> Vec<u8> {
        let body = example.replace("\\0x04", "");
        let joined: String = body.lines().map(str::trim).collect();
        let mut out = joined.into_bytes();
        out.push(END);
        out
    }

    fn sent(plan: &Plan) -> Vec<Vec<u8>> {
        match plan {
            Plan::Get(p) => vec![get_frame(p)],
            Plan::Replace(msgs) => msgs
                .iter()
                .map(|m| replace_frame(&m.path, &m.value))
                .collect(),
            other => panic!("not sendable without a position: {other:?}"),
        }
    }

    fn lp(name: &str, p: Value) -> Plan {
        plan(Dialect::LivePremier, name, &params(p)).unwrap()
    }

    fn m4k(name: &str, p: Value) -> Plan {
        plan(Dialect::Midra4k, name, &params(p)).unwrap()
    }

    fn banked_at(plan: Plan, d: Dialect, t: &str) -> Vec<Vec<u8>> {
        match plan {
            Plan::Banked { msgs, .. } => resolve(d, t, &msgs)
                .unwrap()
                .iter()
                .map(|m| replace_frame(&m.path, &m.value))
                .collect(),
            other => panic!("not banked: {other:?}"),
        }
    }

    // Aquilon AWJ guide v6.2, 3.2.
    #[test]
    fn livepremier_take_matches_the_guide() {
        assert_eq!(
            sent(&lp("take", json!({"screen": 1}))),
            vec![wire(
                r#"{"op":"replace","path":"DeviceObject/$screenAuxGroup/@items/S1/control/@props/xTake",
                "value":true}\0x04"#
            )]
        );
    }

    // Aquilon AWJ guide v6.2, 3.3 and 3.4.
    #[test]
    fn livepremier_preset_recalls_match_the_guide() {
        assert_eq!(
            sent(&lp(
                "recall_screen_preset",
                json!({"preset": 33, "screen": 1, "destination": "preview"})
            )),
            vec![wire(
                r#"{"op":"replace","path":"DeviceObject/presetBank/control/load/$slot/@items/33/$screen/
                @items/S1/$preset/@items/PREVIEW/@props/xRequest","value":true}\0x04"#
            )]
        );
        assert_eq!(
            sent(&lp(
                "recall_screen_preset",
                json!({"preset": 13, "screen": 2, "destination": "program"})
            )),
            vec![wire(
                r#"{"op":"replace","path":"DeviceObject/presetBank/control/load/$slot/@items/13/$screen/
                @items/S2/$preset/@items/PROGRAM/@props/xRequest","value":true}\0x04"#
            )]
        );
        assert_eq!(
            sent(&lp(
                "recall_aux_preset",
                json!({"preset": 8, "aux": 1, "destination": "program"})
            )),
            vec![wire(
                r#"{"op":"replace","path":"DeviceObject/presetBank/control/load/$slot/@items/8/$auxiliary/
                @items/A1/$preset/@items/PROGRAM/@props/xRequest","value":true}\0x04"#
            )]
        );
        assert_eq!(
            sent(&lp(
                "recall_master_preset",
                json!({"preset": 15, "destination": "preview"})
            )),
            vec![wire(
                r#"{"op":"replace","path":"DeviceObject/masterPresetBank/control/load/$slot/@items/15/$preset/
                @items/PREVIEW/@props/xRequest","value":true}\0x04"#
            )]
        );
        // 4.2.
        assert_eq!(
            sent(&lp(
                "recall_multiviewer_preset",
                json!({"preset": 15, "multiviewer": 1})
            )),
            vec![wire(
                r#"{"op":"replace","path":"DeviceObject/monitoringBank/control/load/$slot/@items/15/$output/
                @items/1/@props/xRequest","value":true}\0x04"#
            )]
        );
    }

    // Aquilon AWJ guide v6.2, 3.8: program at AT_DOWN is bank A, preview B,
    // followed by the global update.
    #[test]
    fn livepremier_layer_source_follows_the_tbar() {
        let update = wire(
            r#"{"op":"replace","path":"DeviceObject/$screenAuxGroup/control/@props/xUpdate","value":true}\0x04"#,
        );
        assert_eq!(
            banked_at(
                lp(
                    "set_layer_source",
                    json!({"screen": 1, "layer": 2, "source": "LIVE_3", "destination": "program"})
                ),
                Dialect::LivePremier,
                "AT_DOWN"
            ),
            vec![
                wire(
                    r#"{"op":"replace","path":"DeviceObject/$screen/@items/S1/$preset/@items/A/
                    $layer/@items/2/source/@props/inputNum","value":"LIVE_3"}\0x04"#
                ),
                update.clone()
            ]
        );
        assert_eq!(
            banked_at(
                lp(
                    "set_layer_source",
                    json!({"screen": 2, "layer": 1, "source": "LIVE_5", "destination": "preview"})
                ),
                Dialect::LivePremier,
                "AT_DOWN"
            )[0],
            wire(
                r#"{"op":"replace","path":"DeviceObject/$screen/@items/S2/$preset/@items/B/
                $layer/@items/1/source/@props/inputNum","value":"LIVE_5"}\0x04"#
            )
        );
        // AT_UP swaps the banks.
        assert_eq!(
            banked_at(
                lp("set_layer_source", json!({"screen": 1, "layer": 2, "source": "LIVE_3", "destination": "program"})),
                Dialect::LivePremier,
                "AT_UP"
            )[0],
            replace_frame(
                "DeviceObject/$screen/@items/S1/$preset/@items/B/$layer/@items/2/source/@props/inputNum",
                &json!("LIVE_3")
            )
        );
        assert_eq!(
            banked_at(
                lp(
                    "set_aux_layer_source",
                    json!({"aux": 1, "layer": 2, "source": "LIVE_8", "destination": "program"})
                ),
                Dialect::LivePremier,
                "AT_DOWN"
            )[0],
            wire(
                r#"{"op":"replace","path":"DeviceObject/$auxiliary/@items/A1/$preset/@items/A/
                $layer/@items/2/source/@props/inputNum","value":"LIVE_8"}\0x04"#
            )
        );
        // 3.10.
        assert_eq!(
            banked_at(
                lp(
                    "set_background",
                    json!({"screen": 2, "set": 3, "destination": "preview"})
                ),
                Dialect::LivePremier,
                "AT_DOWN"
            )[0],
            wire(
                r#"{"op":"replace","path":"DeviceObject/$screen/@items/S2/$preset/@items/B/
                $layer/@items/NATIVE/source/@props/inputNum","value":"NATIVE_3"}\0x04"#
            )
        );
        // 3.13.
        assert_eq!(
            banked_at(
                lp(
                    "set_layer_keying",
                    json!({"screen": 1, "layer": 3, "slot": 2, "destination": "program"})
                ),
                Dialect::LivePremier,
                "AT_DOWN"
            ),
            vec![wire(
                r#"{"op":"replace","path":"DeviceObject/$screen/@items/S1/$preset/@items/A/$layer/@items/3/
                keying/@props/source","value":"SLOT_2"}\0x04"#
            )]
        );
        // 3.5: the frozen banks, program and preview at AT_DOWN.
        assert_eq!(
            banked_at(
                lp(
                    "freeze_layer",
                    json!({"screen": 2, "layer": 1, "program": true, "preview": true})
                ),
                Dialect::LivePremier,
                "AT_DOWN"
            ),
            vec![replace_frame(
                "DeviceObject/$screen/@items/S2/$layer/@items/1/control/@props/freeze",
                &json!(["A", "B"])
            )]
        );
    }

    // Aquilon AWJ guide v6.2, 4.5, 5.1.2, 5.2.2, 5.2.4, 6.5, 6.6, 6.8, 6.10,
    // 2.4 and 2.5.
    #[test]
    fn livepremier_other_writes_match_the_guide() {
        let cases: Vec<(Plan, &str)> = vec![
            (
                lp(
                    "set_multiviewer_widget_source",
                    json!({"multiviewer": 1, "widget": 13, "source": "IN_3"}),
                ),
                r#"{"op":"replace","path":"DeviceObject/$monitoring/@items/1/layout/$widget/@items/12/control/
                @props/source","value":"IN_3"}\0x04"#,
            ),
            (
                lp(
                    "set_multiviewer_vu_meter",
                    json!({"multiviewer": 1, "widget": "4"}),
                ),
                r#"{"op":"replace","path":"DeviceObject/$monitoring/@items/1/layout/control/vuMeters/@props/
                widget","value":"4"}\0x04"#,
            ),
            (
                lp(
                    "mute_input_audio",
                    json!({"input": 5, "channel": 7, "muted": true}),
                ),
                r#"{"op":"replace","path":"DeviceObject/audio/control/$device/@items/1/$rx/@items/
                INPUT_5_CHANNEL_7/control/@props/mute","value":true}\0x04"#,
            ),
            (
                lp(
                    "mute_output_audio",
                    json!({"output_type": "OUTPUT", "output": 4, "channel": 3, "muted": true}),
                ),
                r#"{"op":"replace","path":"DeviceObject/audio/control/$device/@items/1/$tx/@items/OUTPUT_4/
                $channel/@items/3/control/@props/mute","value":true}\0x04"#,
            ),
            (
                lp(
                    "set_output_audio_source",
                    json!({"output_type": "OUTPUT", "output": 8, "channel": 1, "source": "DANTE_4_CHANNEL_2"}),
                ),
                r#"{"op":"replace","path":"DeviceObject/audio/control/$device/@items/1/$tx/@items/OUTPUT_8/
                $channel/@items/1/control/@props/source","value":"DANTE_4_CHANNEL_2"}\0x04"#,
            ),
            (
                lp("trigger_input_backup", json!({"input": 9, "slot": "1"})),
                r#"{"op":"replace","path":"DeviceObject/$input/@items/IN_9/backup/control/@props/xSelectSlot",
                "value":"1"}\0x04"#,
            ),
            (
                lp(
                    "set_input_backup_auto",
                    json!({"input": 9, "enabled": true}),
                ),
                r#"{"op":"replace","path":"DeviceObject/$input/@items/IN_9/backup/control/@props/
                enableAutoSelect","value":true}\0x04"#,
            ),
            (
                lp(
                    "trigger_background_backup",
                    json!({"screen": 2, "set": 1, "slot": "2"}),
                ),
                r#"{"op":"replace","path":"DeviceObject/preconfig/backgrounds/$screen/@items/S2/$backgroundSet/
                @items/1/backup/control/@props/xSelectSlot","value":"2"}\0x04"#,
            ),
            (
                lp(
                    "set_background_backup_auto",
                    json!({"screen": 2, "set": 3, "enabled": true}),
                ),
                r#"{"op":"replace","path":"DeviceObject/preconfig/backgrounds/$screen/@items/S2/$backgroundSet/
                @items/3/backup/control/@props/enableAutoSelect","value":true}\0x04"#,
            ),
            (
                lp("trigger_group_backup", json!({"group": 4, "slot": "2"})),
                r#"{"op":"replace","path":"DeviceObject/backup/$group/@items/GROUP_4/control/@props/
                xSelectSlot","value":"2"}\0x04"#,
            ),
            (
                lp(
                    "set_group_backup_auto",
                    json!({"group": 2, "enabled": true}),
                ),
                r#"{"op":"replace","path":"DeviceObject/backup/$group/@items/GROUP_2/control/@props/
                enableAutoSelect","value":true}\0x04"#,
            ),
            (
                lp("reboot", json!({})),
                r#"{"op":"replace","path":"DeviceObject/system/shutdown/cmd/@props/xRequest",
                "value":"REBOOT"}\0x04"#,
            ),
            (
                lp("shutdown", json!({})),
                r#"{"op":"replace","path":"DeviceObject/system/shutdown/cmd/@props/xRequest",
                "value":"SHUTDOWN"}\0x04"#,
            ),
            (
                lp(
                    "awj_get",
                    json!({"path": "DeviceObject/$input/@items/IN_8/control/@props/label"}),
                ),
                r#"{"op":"get","path":"DeviceObject/$input/@items/IN_8/control/@props/label"}\0x04"#,
            ),
            (
                lp("awj_get", json!({"path": "Subscriptions"})),
                r#"{"op":"get","path":"Subscriptions"}\0x04"#,
            ),
        ];
        for (plan, example) in cases {
            assert_eq!(sent(&plan), vec![wire(example)], "{example}");
        }
    }

    // Midra 4K AWJ guide v3.2: 3.2 to 3.5, 3.8, 3.10, 3.12, 3.15, 3.18 to
    // 3.20, 4.2, 4.5, 6.7 and 2.4 to 2.7.
    #[test]
    fn midra_writes_match_the_guide() {
        let cases: Vec<(Plan, &str)> = vec![
            (
                m4k("take", json!({"screen": 1})),
                r#"{"op":"replace","path":"DeviceObject/transition/$screen/@items/1/control/@props/xTake",
                "value":true}\0x04"#,
            ),
            (
                m4k("take_aux", json!({"aux": 1})),
                r#"{"op":"replace","path":"DeviceObject/transition/$auxiliaryScreen/@items/1/control/@props/xTake",
                "value":true}\0x04"#,
            ),
            (
                m4k(
                    "recall_screen_preset",
                    json!({"preset": 33, "screen": 1, "destination": "preview"}),
                ),
                r#"{"op":"replace","path":"DeviceObject/preset/bank/control/load/$slot/@items/33/$screen/@items/1
                /$preset/@items/PREVIEW/@props/xRequest","value":true}\0x04"#,
            ),
            (
                m4k(
                    "recall_aux_preset",
                    json!({"preset": 13, "aux": 1, "destination": "program"}),
                ),
                r#"{"op":"replace","path":"DeviceObject/preset/auxBank/control/load/$slot/@items/13/
                $auxillaryScreen/@items/1/$preset/@items/PROGRAM/@props/xRequest","value":true}\0x04"#,
            ),
            (
                m4k(
                    "recall_master_preset",
                    json!({"preset": 3, "destination": "program"}),
                ),
                r#"{"op":"replace","path":"DeviceObject/preset/masterBank/control/load/$slot/@items/3/$preset/
                @items/PROGRAM/@props/xRequest","value":true}\0x04"#,
            ),
            (
                m4k("recall_multiviewer_preset", json!({"preset": 15})),
                r#"{"op":"replace","path":"DeviceObject/multiviewer/$bank/control/load/$slot/@items/15/@props/
                xRequest","value":true}\0x04"#,
            ),
            (
                m4k("enable_quick_preset", json!({"enabled": true})),
                r#"{"op":"replace","path":"DeviceObject/quickPreset/control/@props/enable","value":true}\0x04"#,
            ),
            (
                m4k("freeze_screen", json!({"screen": 1, "frozen": true})),
                r#"{"op":"replace","path":"DeviceObject/$screen/@items/1/control/@props/freeze",
                "value":true}\0x04"#,
            ),
            (
                m4k(
                    "freeze_layer",
                    json!({"screen": 1, "layer": 2, "frozen": true}),
                ),
                r#"{"op":"replace","path":"DeviceObject/$screen/@items/1/$liveLayer/@items/2/control/@props/
                freeze","value":true}\0x04"#,
            ),
            (
                m4k("freeze_input", json!({"input": 3, "frozen": true})),
                r#"{"op":"replace","path":"DeviceObject/$input/@items/INPUT_3/control/@props/freeze",
                "value":true}\0x04"#,
            ),
            (
                m4k(
                    "set_multiviewer_widget_source",
                    json!({"widget": 5, "source": "INPUT_3"}),
                ),
                r#"{"op":"replace","path":"DeviceObject/multiviewer/$widget/@items/5/control/@props/source",
                "value":"INPUT_3"}\0x04"#,
            ),
            (
                m4k(
                    "set_foreground_image",
                    json!({"screen": 1, "image": 4, "library_slot": 5}),
                ),
                r#"{"op":"replace","path":"DeviceObject/$screen/@items/1/$topFrame/@items/4/control/
                @props/librarySlot","value":"5"}\0x04"#,
            ),
            (
                m4k("reboot", json!({})),
                r#"{"op":"replace","path":"DeviceObject/system/shutdown/@props/xReboot","value":true}\0x04"#,
            ),
            (
                m4k("standby", json!({})),
                r#"{"op":"replace","path":"DeviceObject/system/shutdown/standby/control/@props/xRequest",
                "value":"STANDBY"}\0x04"#,
            ),
            (
                m4k("wake_up", json!({})),
                r#"{"op":"replace","path":"DeviceObject/system/shutdown/standby/control/@props/xRequest",
                "value":"WAKE_UP"}\0x04"#,
            ),
            (
                m4k("mute_output_audio", json!({"output": 2, "muted": true})),
                r#"{"op":"replace","path":"DeviceObject/audio/$output/@items/VIDEO_OUT_2/control/@props/mute","value":true}\0x04"#,
            ),
            (
                m4k(
                    "mute_input_audio",
                    json!({"input": "IN9_DP_EMBEDDED", "channel": 3, "muted": true}),
                ),
                r#"{"op":"replace","path":"DeviceObject/audio/$input/@items/IN9_DP_EMBEDDED/$channel/@items/
                3/control/@props/mute","value":true}\0x04"#,
            ),
            (
                m4k(
                    "set_output_audio_source",
                    json!({"output": 2, "source": "IN4"}),
                ),
                r#"{"op":"replace","path":"DeviceObject/$output/@items/2/audio/control/directRouting/@props/
                source","value":"IN4"}\0x04"#,
            ),
            (
                m4k(
                    "set_dante_audio_mode",
                    json!({"group": 1, "mode": "DIRECT_ROUTING"}),
                ),
                r#"{"op":"replace","path":"DeviceObject/audio/dante/$outputGroup/@items/1/control/@props/mode",
                "value":"DIRECT_ROUTING"}\0x04"#,
            ),
            (
                m4k(
                    "set_line_out_pair",
                    json!({"line_out": 1, "pair": "CHANNEL_3_4"}),
                ),
                r#"{"op":"replace","path":"DeviceObject/audio/$lineOut/@items/1/control/@props/selectedAudioPair",
                "value":"CHANNEL_3_4"}\0x04"#,
            ),
            (
                m4k("set_multiviewer_vu_meter", json!({"widget": 7})),
                r#"{"op":"replace","path":"DeviceObject/multiviewer/audio/control/vuMeters/@props/widget",
                "value":"7"}\0x04"#,
            ),
        ];
        for (plan, example) in cases {
            assert_eq!(sent(&plan), vec![wire(example)], "{example}");
        }
    }

    // Midra 4K AWJ guide v3.2, 3.8, 3.10 and 3.15.
    #[test]
    fn midra_banked_writes_follow_the_tbar() {
        let d = Dialect::Midra4k;
        let update = wire(
            r#"{"op":"replace","path":"DeviceObject/preset/control/@props/xUpdate","value":true}\0x04"#,
        );
        assert_eq!(
            banked_at(
                m4k(
                    "set_layer_source",
                    json!({"screen": 1, "layer": 2, "source": "INPUT_3", "destination": "program"})
                ),
                d,
                "AT_DOWN"
            ),
            vec![
                wire(
                    r#"{"op":"replace","path":"DeviceObject/$screen/@items/1/$preset/@items/DOWN/$liveLayer/@items
                    /2/source/@props/input","value":"INPUT_3"}\0x04"#
                ),
                update.clone()
            ]
        );
        assert_eq!(
            banked_at(
                m4k(
                    "set_layer_source",
                    json!({"screen": 2, "layer": 1, "source": "INPUT_5", "destination": "preview"})
                ),
                d,
                "AT_DOWN"
            )[0],
            wire(
                r#"{"op":"replace","path":"DeviceObject/$screen/@items/2/$preset/@items/UP/$liveLayer/@items
                /1/source/@props/input","value":"INPUT_5"}\0x04"#
            )
        );
        assert_eq!(
            banked_at(
                m4k(
                    "set_background",
                    json!({"screen": 1, "set": 2, "destination": "program"})
                ),
                d,
                "AT_DOWN"
            ),
            vec![
                wire(
                    r#"{"op":"replace","path":"DeviceObject/$screen/@items/1/$preset/@items/DOWN/background/
                    source/@props/set","value":"2"}\0x04"#
                ),
                update.clone()
            ]
        );
        assert_eq!(
            banked_at(
                m4k(
                    "set_foreground",
                    json!({"screen": 1, "frame": 3, "destination": "program"})
                ),
                d,
                "AT_DOWN"
            )[0],
            wire(
                r#"{"op":"replace","path":"DeviceObject/$screen/@items/1/$preset/@items/DOWN/top/source/
                @props/frame","value":"3"}\0x04"#
            )
        );
        assert_eq!(
            banked_at(
                m4k(
                    "set_aux_source",
                    json!({"aux": 1, "source": "INPUT_2", "destination": "program"})
                ),
                d,
                "AT_DOWN"
            ),
            vec![wire(
                r#"{"op":"replace","path":"DeviceObject/$auxiliaryScreen/@items/1/$preset/@items/DOWN/
                background/source/@props/content","value":"INPUT_2"}\0x04"#
            )]
        );
        // At AT_UP, program is the UP bank.
        assert_eq!(
            banked_at(m4k("set_aux_source", json!({"aux": 1, "source": "INPUT_2", "destination": "program"})), d, "AT_UP")[0],
            replace_frame(
                "DeviceObject/$auxiliaryScreen/@items/1/$preset/@items/UP/background/source/@props/content",
                &json!("INPUT_2")
            )
        );
        // An unknown position is never guessed.
        assert!(resolve(d, "MOVING", &[msg("x{PGM}", json!(1))]).is_none());
    }

    #[test]
    fn deframer_splits_at_0x04_and_keeps_partial_messages() {
        let mut d = Deframer::default();
        let (m, bad) = d.feed(b"{\"path\":\"a\",\"value\":1}\x04{\"path\":\"b\"");
        assert_eq!(m, vec![json!({"path": "a", "value": 1})]);
        assert!(bad.is_empty());
        let (m, _) = d.feed(b",\"value\":3} \x04");
        // The guide's 3.12 reply has a space before the terminator.
        assert_eq!(m, vec![json!({"path": "b", "value": 3})]);
        let (m, bad) = d.feed(b"not json\x04");
        assert!(m.is_empty());
        assert_eq!(bad.len(), 1);
    }

    fn state_after(d: Dialect, inputs: &[(&str, Value)]) -> Value {
        let mut p = Projection::new(d);
        let mut state = json!({});
        for (path, value) in inputs {
            for patch in p.apply(path, value) {
                merge_patch(&mut state, &patch);
            }
        }
        state
    }

    // The replies of LivePremier guide 3.1, 3.6, 3.8, 3.11, 3.12, 4.3 and 6.1.
    #[test]
    fn livepremier_replies_become_program_and_preview_state() {
        let s = state_after(
            Dialect::LivePremier,
            &[
                ("DeviceObject/$screen/@items/S1/control/@props/label", json!("Sc1")),
                ("DeviceObject/$screen/@items/S2/$preset/@items/A/$layer/@items/1/source/@props/inputNum", json!("LIVE_4")),
                ("DeviceObject/$screen/@items/S2/$preset/@items/B/$layer/@items/1/source/@props/inputNum", json!("LIVE_7")),
                ("DeviceObject/$screenAuxGroup/@items/S2/status/@props/transition", json!("AT_DOWN")),
                ("DeviceObject/presetBank/status/presetId/$screen/@items/S2/$preset/@items/A/@props/id", json!(3)),
                ("DeviceObject/masterPresetBank/status/lastUsed/$presetMode/@items/PROGRAM/@props/memoryId", json!(3)),
                ("DeviceObject/$monitoring/@items/2/layout/$widget/@items/5/control/@props/source", json!("IN_4")),
                ("DeviceObject/$input/@items/IN_8/control/@props/freeze", json!(false)),
                ("DeviceObject/system/$device/@items/1/@props/dev", json!("NLC_RS4")),
            ],
        );
        assert_eq!(s["screens"]["1"]["label"], "Sc1");
        assert_eq!(
            s["screens"]["2"]["program"]["layers"]["1"]["source"],
            "LIVE_4"
        );
        assert_eq!(
            s["screens"]["2"]["preview"]["layers"]["1"]["source"],
            "LIVE_7"
        );
        assert_eq!(s["screens"]["2"]["program"]["preset"], 3);
        assert_eq!(
            s["screens"]["2"]["banks"]["B"]["layers"]["1"]["source"],
            "LIVE_7"
        );
        assert_eq!(s["master_preset"]["program"], 3);
        assert_eq!(s["multiviewers"]["2"]["widgets"]["6"]["source"], "IN_4");
        assert_eq!(s["inputs"]["8"]["frozen"], false);
        assert_eq!(s["device"]["type"], "NLC_RS4");

        // A TAKE moves the T-bar: program and preview swap.
        let mut p = Projection::new(Dialect::LivePremier);
        let mut state = json!({});
        for (path, v) in [
            ("DeviceObject/$screenAuxGroup/@items/S2/status/@props/transition", json!("AT_DOWN")),
            ("DeviceObject/$screen/@items/S2/$preset/@items/A/$layer/@items/1/source/@props/inputNum", json!("LIVE_4")),
            ("DeviceObject/$screen/@items/S2/$preset/@items/B/$layer/@items/1/source/@props/inputNum", json!("LIVE_7")),
            ("DeviceObject/$screenAuxGroup/@items/S2/status/@props/transition", json!("AT_UP")),
        ] {
            for patch in p.apply(path, &v) {
                merge_patch(&mut state, &patch);
            }
        }
        assert_eq!(state["screens"]["2"]["transition"], "AT_UP");
        assert_eq!(
            state["screens"]["2"]["program"]["layers"]["1"]["source"],
            "LIVE_7"
        );
        assert_eq!(
            state["screens"]["2"]["preview"]["layers"]["1"]["source"],
            "LIVE_4"
        );
    }

    // The replies of Midra 4K guide 3.7, 3.8, 3.9, 3.14 and 6.1.
    #[test]
    fn midra_replies_become_state() {
        let s = state_after(
            Dialect::Midra4k,
            &[
                ("DeviceObject/transition/$screen/@items/1/status/@props/transition", json!("AT_UP")),
                ("DeviceObject/$screen/@items/1/$preset/@items/DOWN/$liveLayer/@items/2/source/@props/input", json!("INPUT_9")),
                ("DeviceObject/$screen/@items/1/$preset/@items/DOWN/$liveLayer/@items/2/status/@props/state", json!("OFF")),
                ("DeviceObject/$screen/@items/1/$preset/@items/DOWN/background/source/@props/set", json!("3")),
                ("DeviceObject/$auxiliaryScreen/@items/1/$preset/@items/DOWN/background/source/@props/content", json!("PROGRAM_2")),
                ("DeviceObject/$input/@items/INPUT_2/$plug/@items/1/status/signal/@props/isValid", json!(true)),
                ("DeviceObject/system/@props/dev", json!("PULSE")),
            ],
        );
        // AT_UP: DOWN is preview.
        assert_eq!(
            s["screens"]["1"]["preview"]["layers"]["2"]["source"],
            "INPUT_9"
        );
        assert_eq!(s["screens"]["1"]["preview"]["layers"]["2"]["state"], "OFF");
        assert_eq!(s["screens"]["1"]["preview"]["background"], "3");
        assert_eq!(
            s["auxiliaries"]["1"]["banks"]["DOWN"]["source"],
            "PROGRAM_2"
        );
        assert!(s["auxiliaries"]["1"].get("program").is_none());
        assert_eq!(s["inputs"]["2"]["plugs"]["1"]["valid"], true);
        assert_eq!(s["device"]["type"], "PULSE");
    }

    /// Every command the three specs declare is one the module plans.
    #[test]
    fn every_spec_command_is_planned() {
        use crate::catalog::{Catalog, ParamType};
        let catalog = Catalog::embedded();
        for id in [
            "analogway-livepremier",
            "analogway-midra4k",
            "analogway-alta4k",
        ] {
            let spec = catalog.device(id).unwrap();
            let d = Dialect::for_spec(id).unwrap();
            for (name, command) in &spec.commands {
                let mut p = Params::new();
                for (key, ps) in &command.params {
                    let v = match ps.kind {
                        ParamType::Int => json!(ps.min.unwrap_or(1.0).max(1.0) as i64),
                        ParamType::Bool => json!(true),
                        ParamType::Enum => json!(ps.values.as_ref().unwrap()[0]),
                        ParamType::Json if key == "paths" => json!(["DeviceObject/$input"]),
                        _ => json!("X"),
                    };
                    p.insert(key.clone(), v);
                }
                let planned = plan(d, name, &p);
                assert!(planned.is_ok(), "{id} {name}: {planned:?}");
            }
        }
    }

    fn ctx(d: &str) -> OpenContext {
        OpenContext {
            host: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 20)),
            port: None,
            model: d.into(),
            channels: None,
            settings: params(json!({"read_on_connect": false})),
            monitor: true,
        }
    }

    fn sends(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => {
                    assert_eq!(data.last(), Some(&END));
                    Some(serde_json::from_slice(&data[..data.len() - 1]).unwrap())
                }
                _ => None,
            })
            .collect()
    }

    fn feed(m: &mut AnalogWay, now: Millis, v: Value) -> Vec<Action> {
        let mut cx = Cx::new(now);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(frame(&v)));
        cx.take()
    }

    fn connected(d: Dialect, spec: &str) -> AnalogWay {
        let mut m = AnalogWay::new(d, ctx(spec));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let first = sends(&cx.take());
        assert_eq!(
            first,
            vec![json!({"op": "get", "path": d.device_type_path()})]
        );
        let actions = feed(
            &mut m,
            1,
            json!({"path": d.device_type_path(), "value": "NLC_RS4"}),
        );
        assert!(actions.contains(&Action::Connection(Connection::Connected)));
        let out = sends(&actions);
        assert_eq!(out[0]["path"], "Subscriptions");
        assert_eq!(out[0]["op"], "replace");
        // Only the serial number and firmware are read without read_on_connect.
        assert_eq!(out.len(), 3);
        feed(
            &mut m,
            2,
            json!({"path": "Subscriptions", "value": out[0]["value"]}),
        );
        feed(
            &mut m,
            3,
            json!({"path": d.serial_path(), "value": "XX9999"}),
        );
        feed(
            &mut m,
            4,
            json!({"path": d.firmware_path(), "value": "1.0"}),
        );
        assert!(m.unanswered.is_empty());
        m
    }

    #[test]
    fn a_banked_command_reads_the_tbar_first() {
        let mut m = connected(Dialect::LivePremier, "aquilon-rs4");
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            7,
            "set_layer_source",
            &params(json!({"screen": 1, "layer": 2, "source": "LIVE_3", "destination": "program"})),
        );
        let out = sends(&cx.take());
        assert_eq!(
            out,
            vec![
                json!({"op": "get", "path": "DeviceObject/$screenAuxGroup/@items/S1/status/@props/transition"})
            ]
        );
        let actions = feed(
            &mut m,
            20,
            json!({"path": "DeviceObject/$screenAuxGroup/@items/S1/status/@props/transition", "value": "AT_UP"}),
        );
        let out = sends(&actions);
        assert_eq!(
            out[0]["path"],
            "DeviceObject/$screen/@items/S1/$preset/@items/B/$layer/@items/2/source/@props/inputNum"
        );
        assert_eq!(
            out[1]["path"],
            "DeviceObject/$screenAuxGroup/control/@props/xUpdate"
        );
        assert!(actions.contains(&Action::Complete {
            id: 7,
            result: Ok(Outcome::Unverified)
        }));

        // Known now: the next one goes straight out.
        let mut cx = Cx::new(30);
        m.command(
            &mut cx,
            8,
            "set_layer_source",
            &params(json!({"screen": 1, "layer": 1, "source": "LIVE_1", "destination": "preview"})),
        );
        let out = sends(&cx.take());
        assert_eq!(
            out[0]["path"],
            "DeviceObject/$screen/@items/S1/$preset/@items/A/$layer/@items/1/source/@props/inputNum"
        );
    }

    #[test]
    fn an_error_answers_the_get_just_sent() {
        let mut m = connected(Dialect::Midra4k, "pulse-4k");
        let mut cx = Cx::new(10);
        m.command(
            &mut cx,
            3,
            "awj_get",
            &params(json!({"path": "DeviceObject/system/@props/div"})),
        );
        let actions = feed(
            &mut m,
            11,
            json!({"error": {"code": "E12", "message": "Unexpected path \"DeviceObject/system/@props/div\""}}),
        );
        assert!(actions.iter().any(|a| matches!(a,
            Action::Complete { id: 3, result: Err(CommandError::DeviceError { code: Some(c), .. }) } if c == "E12")));
    }

    #[test]
    fn a_get_completes_with_its_value() {
        let mut m = connected(Dialect::LivePremier, "aquilon-rs4");
        let path = "DeviceObject/$screen/@items/S1/control/@props/label";
        let mut cx = Cx::new(10);
        m.command(&mut cx, 4, "awj_get", &params(json!({"path": path})));
        // The reply path may carry a leading space, as printed in 3.8.
        let actions = feed(
            &mut m,
            11,
            json!({"path": format!(" {path}"), "value": "Sc1"}),
        );
        assert!(actions.contains(&Action::Complete {
            id: 4,
            result: Ok(Outcome::Value {
                value: json!("Sc1")
            })
        }));
    }
}
