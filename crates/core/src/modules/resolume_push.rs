//! Resolume's websocket API, added to the spec-driven Arena and Avenue.
//!
//! Commands are the spec's (`specs/resolume.yaml`), run by the spec engine over
//! the REST API. This keeps the whole composition current by push, which the
//! format cannot express because every subscription names a parameter id
//! learned at run time (Resolume, "Websocket API", support article v7.8,
//! <https://resolume.com/support/en/websocket-api>):
//!
//! - The websocket is `ws://address:port/api/v1`, on the webserver's port. On
//!   connecting Resolume sends the composition state ("similar to GET
//!   /composition"), then `{"type":"sources_update",...}` and
//!   `{"type":"effects_update",...}`. It sends the composition again "when
//!   there are structural changes", such as a layer or column added or
//!   removed.
//! - `{"action":"subscribe","parameter":"/parameter/by-id/<id>"}` subscribes
//!   to a parameter by the id the composition gives it. The answer, and every
//!   later change, is the parameter itself with `type` (`parameter_subscribed`,
//!   `parameter_update`, ...) and `path` added.
//!
//! The composition's layers, columns, clips, decks and layer groups are read
//! into state keyed by unique id (the same paths the spec's rules for a
//! `get_composition` reply write, plus positions and clips), every parameter
//! that state shows is subscribed, and each update is applied. A new
//! composition message is diffed against the last: items that went are
//! removed from state, and subscriptions are added and dropped to match.
//!
//! Three things are not covered by that, and are read again with
//! `GET /api/v1/composition` (Arena & Avenue REST API, list_composition):
//! a deck switch (a deck's `selected` parameter turning true; the deck's
//! layers and clips replace the last one's, and the article does not list it
//! as a structural change), a command of this session that changes the
//! structure without the article saying a new composition follows (decks,
//! open, new, undo, redo), and a deck closed or opened elsewhere: `closed`
//! is a plain boolean in the Deck schema, not a parameter, so it cannot be
//! subscribed. For the last there is a slow safety refresh, the
//! `composition_refresh_s` setting (default 60 s, 0 for none), restarted by
//! every composition that arrives.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::net::IpAddr;

use crate::{
    json,
    value::{Map, Value},
};

use crate::catalog::{DeviceSpec, Params};
use crate::engine::SpecEngine;
use crate::module::{
    Action, CommandId, Cx, FileInput, HttpRequest, HttpResponse, Key, Level, Millis, Module,
    OpenContext, RequestId, SseInput, TcpInput, WsInput, WsRequest,
};

const WS: Key = "resolume-ws";
const REOPEN: Key = "resolume-ws-reopen";
const REFRESH: Key = "resolume-refresh";
/// Request ids at and above this are this module's; the engine's count up
/// from 1 and never reach it.
const OWN_IDS: RequestId = 1 << 48;
const REFRESH_TIMEOUT: Millis = 10_000;
/// A refresh asked for by an event waits this long, so several coalesce.
const SOON: Millis = 250;
/// After the websocket opens, the composition is expected at once; if none
/// has come by then it is read over REST.
const FIRST_COMPOSITION: Millis = 3_000;
const DEFAULT_REFRESH_S: u64 = 60;
const REOPEN_MIN: Millis = 1_000;
const REOPEN_MAX: Millis = 30_000;
/// Commands after which the composition is read again: they change its
/// structure, and the article names only layers and columns added or
/// removed as changes that resend it. Effect chains, reordering, decks and
/// clip content are not named, and a display name is not a parameter.
const STRUCTURAL: &[&str] = &[
    "add_deck",
    "open_deck",
    "close_deck",
    "select_deck",
    "open_composition",
    "new_composition",
    "undo",
    "redo",
    // Clip content: a clip's effects and transport come with what it holds.
    "open_clip",
    "clear_clip",
    "clear_layer_clips",
    // Effect chains, moves and swaps, deck structure and clip content.
    "add_effect_to_composition",
    "add_effect_to_composition_at",
    "move_effect_to_composition",
    "move_effect_to_composition_at",
    "delete_composition_effect",
    "rename_composition_effect",
    "copy_effects_to_composition",
    "add_effect_to_layer",
    "add_effect_to_layer_at",
    "move_effect_to_layer",
    "move_effect_to_layer_at",
    "delete_layer_effect",
    "rename_layer_effect",
    "copy_effects_to_layer",
    "add_effect_to_layer_by_id",
    "add_effect_to_layer_at_by_id",
    "move_effect_to_layer_by_id",
    "move_effect_to_layer_at_by_id",
    "delete_layer_effect_by_id",
    "copy_effects_to_layer_by_id",
    "add_effect_to_selected_layer",
    "add_effect_to_selected_layer_at",
    "delete_selected_layer_effect",
    "copy_effects_to_selected_layer",
    "add_effect_to_group",
    "add_effect_to_group_at",
    "move_effect_to_group",
    "move_effect_to_group_at",
    "delete_group_effect",
    "rename_group_effect",
    "copy_effects_to_group",
    "add_effect_to_group_by_id",
    "add_effect_to_group_at_by_id",
    "move_effect_to_group_by_id",
    "move_effect_to_group_at_by_id",
    "delete_group_effect_by_id",
    "copy_effects_to_group_by_id",
    "add_effect_to_selected_group",
    "add_effect_to_selected_group_at",
    "delete_selected_group_effect",
    "copy_effects_to_selected_group",
    "add_effect_to_clip",
    "add_effect_to_clip_at",
    "move_effect_to_clip",
    "move_effect_to_clip_at",
    "delete_clip_effect",
    "rename_clip_effect",
    "copy_effects_to_clip",
    "add_effect_to_clip_by_id",
    "add_effect_to_clip_at_by_id",
    "move_effect_to_clip_by_id",
    "move_effect_to_clip_at_by_id",
    "delete_clip_effect_by_id",
    "copy_effects_to_clip_by_id",
    "add_effect_to_selected_clip",
    "add_effect_to_selected_clip_at",
    "delete_selected_clip_effect",
    "copy_effects_to_selected_clip",
    "rename_effect",
    "insert_column_move",
    "swap_columns",
    "insert_column_move_by_id",
    "swap_columns_by_id",
    "insert_column_move_at_selected",
    "move_layer",
    "move_layer_by_id",
    "move_layer_to_end",
    "move_layer_to_end_by_id",
    "move_layer_into_group",
    "move_layer_into_group_by_id",
    "remove_layer_from_group",
    "remove_layer_from_group_by_id",
    "clear_layer_clips_by_id",
    "clear_selected_layer_clips",
    "move_group",
    "move_group_before_layer",
    "move_group_to_end",
    "move_group_by_id",
    "move_group_to_end_by_id",
    "delete_deck",
    "delete_deck_by_id",
    "duplicate_deck",
    "duplicate_deck_by_id",
    "add_deck_before",
    "add_deck_before_by_id",
    "select_deck_by_id",
    "open_deck_by_id",
    "close_deck_by_id",
    "open_clip_by_id",
    "open_selected_clip",
    "clear_clip_by_id",
    "clear_selected_clip",
    "remove_clip_audio_track",
    "remove_clip_video_track",
    "insert_into_clip",
    "remove_clip_audio_track_by_id",
    "remove_clip_video_track_by_id",
    "insert_into_clip_by_id",
    "merge_clip",
    "merge_clip_by_id",
    "swap_clips",
    "swap_clips_by_id",
];

/// What a subscribed parameter's value changes.
#[derive(Debug, Clone, PartialEq)]
enum Target {
    /// One state path, set to the value.
    Path(String),
    /// One state path given segment by segment, for an effect parameter's
    /// value: its name is Resolume's and may hold a dot.
    Segments(Vec<String>),
    /// A clip's name: `clips.<id>.name`, and its layer's active clip name.
    ClipName(u64),
    /// A clip's connection state: `clips.<id>.connected`, and its layer's
    /// active clip.
    ClipConnected(u64),
    /// A deck's `selected`: true means the deck's layers replace the last.
    DeckSelected(u64),
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Clip {
    layer: u64,
    name: Option<Value>,
    connected: Option<String>,
}

/// The composition as last read: what is in state and what is subscribed.
#[derive(Debug, Clone, Default, PartialEq)]
struct Model {
    /// Every subscribed parameter id and what it updates.
    params: BTreeMap<u64, Target>,
    layers: BTreeSet<u64>,
    columns: BTreeSet<u64>,
    decks: BTreeSet<u64>,
    groups: BTreeSet<u64>,
    effects: BTreeSet<u64>,
    clips: BTreeMap<u64, Clip>,
    /// Each layer's clips, in column order.
    layer_clips: BTreeMap<u64, Vec<u64>>,
    /// Each layer's playing clip.
    active: BTreeMap<u64, Option<u64>>,
    selected_deck: Option<u64>,
}

/// What is read beyond the composition's structure, from the settings.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Options {
    /// Each clip's transport position, which moves while a clip plays.
    clip_positions: bool,
    /// Each effect's parameters and mixer parameters.
    effect_parameters: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            clip_positions: true,
            effect_parameters: true,
        }
    }
}

/// Put `value` at a dotted path in a nested JSON object.
fn set(patch: &mut Value, path: &str, value: Value) {
    set_segments(patch, path.split('.'), value);
}

/// Put `value` at a path, given segment by segment, in a nested JSON object.
fn set_segments<'s>(patch: &mut Value, path: impl IntoIterator<Item = &'s str>, value: Value) {
    let mut node = patch;
    let mut parts = path.into_iter().peekable();
    while let Some(part) = parts.next() {
        if !node.is_object() {
            *node = Value::Object(Map::new());
        }
        let map = node.as_object_mut().unwrap();
        if parts.peek().is_none() {
            map.insert(part.to_string(), value);
            return;
        }
        node = map
            .entry(part.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
}

fn id_of(v: Option<&Value>) -> Option<u64> {
    match v? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// A clip that is playing: "Connected" or "Connected & previewing", of the
/// five states Resolume's example application lists for `connected`.
fn is_connected(state: &str) -> bool {
    state.starts_with("Connected")
}

/// An array in a JSON object, or none.
fn array<'v>(v: &'v Value, key: &str) -> &'v [Value] {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// Whether a message is a composition: the Composition schema's arrays.
fn is_composition(v: &Value) -> bool {
    ["layers", "columns", "decks", "layergroups"]
        .iter()
        .any(|k| v.get(k).is_some_and(Value::is_array))
}

/// A websocket message, by what it carries.
#[derive(Debug, PartialEq)]
enum Message {
    Composition(Value),
    Parameter(Value),
    Other,
}

fn classify(doc: Value) -> Message {
    match doc.get("type").and_then(Value::as_str) {
        Some("parameter_subscribed" | "parameter_update" | "parameter_get" | "parameter_set") => {
            Message::Parameter(doc)
        }
        // A composition in a {type, value} wrapper, like sources_update. The
        // article shows only the bare composition; this costs nothing.
        Some(_) => match doc.get("value") {
            Some(v) if is_composition(v) => Message::Composition(v.clone()),
            _ => Message::Other,
        },
        None if is_composition(&doc) => Message::Composition(doc),
        None => Message::Other,
    }
}

/// The parameter id a parameter message is about: its `id`, or the id in a
/// `/parameter/by-id/<id>` path.
fn parameter_id(msg: &Value) -> Option<u64> {
    id_of(msg.get("id")).or_else(|| {
        msg.get("path")
            .and_then(Value::as_str)
            .and_then(|p| p.strip_prefix("/parameter/by-id/"))
            .and_then(|id| id.parse().ok())
    })
}

/// The subscription message for a parameter id.
fn subscription(action: &str, id: u64) -> String {
    json!({ "action": action, "parameter": format!("/parameter/by-id/{id}") }).to_string()
}

struct Reader<'a> {
    model: &'a mut Model,
    patch: &'a mut Value,
    options: Options,
}

/// The fields of a parameter object kept in state for an effect parameter,
/// so a consumer can set it by id and offer its range or options.
const PARAMETER_FIELDS: &[&str] = &["id", "valuetype", "value", "min", "max", "options"];

impl Reader<'_> {
    /// An audio track's volume and pan, under `at`.
    fn audio(&mut self, a: Option<&Value>, at: &str) {
        let Some(a) = a.filter(|a| a.is_object()) else {
            return;
        };
        self.plain(a.get("volume"), format!("{at}.volume"));
        self.plain(a.get("pan"), format!("{at}.pan"));
    }

    /// The video and audio effect chains of `owner`, into `effects.<id>`.
    fn effects(&mut self, owner: &Value, kind: &str, owner_id: Option<u64>) {
        for (chain, list) in [
            ("video", owner.pointer("/video/effects")),
            ("audio", owner.pointer("/audio/effects")),
        ] {
            let list = list
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            for (i, effect) in list.iter().enumerate() {
                let Some(id) = id_of(effect.get("id")) else {
                    continue;
                };
                if !self.model.effects.insert(id) {
                    continue;
                }
                let at = format!("effects.{id}");
                set(self.patch, &format!("{at}.chain"), json!(chain));
                // An effect moved keeps its id: what its last owner gave
                // and its new one does not is removed (null).
                set(self.patch, &format!("{at}.owner"), json!(kind));
                set(self.patch, &format!("{at}.owner_id"), json!(owner_id));
                set(self.patch, &format!("{at}.position"), json!(i + 1));
                for field in ["name", "display_name"] {
                    let v = effect.get(field).filter(|v| v.is_string()).cloned();
                    set(
                        self.patch,
                        &format!("{at}.{field}"),
                        v.unwrap_or(Value::Null),
                    );
                }
                if let Some(v) = effect.get("is_user_removable").filter(|v| v.is_boolean()) {
                    set(self.patch, &format!("{at}.removable"), v.clone());
                }
                self.plain(effect.get("bypassed"), format!("{at}.bypassed"));
                if self.options.effect_parameters {
                    for section in ["params", "mixer"] {
                        let Some(params) = effect.get(section).and_then(Value::as_object) else {
                            continue;
                        };
                        for (name, p) in params {
                            self.effect_parameter(id, section, name, p);
                        }
                    }
                }
            }
        }
    }

    /// A clip's transport, audio volume, video opacity and trigger settings,
    /// under `at`.
    fn clip_settings(&mut self, clip: &Value, at: &str) {
        for (key, field) in [
            ("transporttype", "transport_type"),
            ("triggerstyle", "trigger_style"),
            ("target", "target"),
            ("beatsnap", "beat_snap"),
            ("colorid", "color"),
        ] {
            self.plain(clip.get(key), format!("{at}.{field}"));
        }
        self.plain(clip.pointer("/audio/volume"), format!("{at}.volume"));
        self.plain(clip.pointer("/video/opacity"), format!("{at}.opacity"));
        let Some(transport) = clip.get("transport").filter(|t| t.is_object()) else {
            return;
        };
        let position = transport.get("position");
        if self.options.clip_positions {
            self.plain(position, format!("{at}.transport_position"));
        }
        if let Some(max) = position
            .and_then(|p| p.get("max"))
            .filter(|v| v.is_number())
        {
            set(
                self.patch,
                &format!("{at}.transport_position_max"),
                max.clone(),
            );
        }
        let control = |k: &str| transport.get("controls").and_then(|c| c.get(k));
        for (key, field) in [
            ("speed", "speed"),
            ("playdirection", "play_direction"),
            ("playmode", "play_mode"),
            ("duration", "duration"),
        ] {
            self.plain(control(key), format!("{at}.{field}"));
        }
    }

    /// One parameter of an effect: its id, type, value and range or options
    /// at `effects.<id>.<section>.<name>`, its value subscribed.
    fn effect_parameter(&mut self, effect: u64, section: &str, name: &str, p: &Value) {
        let Some(obj) = p.as_object() else { return };
        let effect = effect.to_string();
        let kept: Map<String, Value> = obj
            .iter()
            .filter(|(k, v)| PARAMETER_FIELDS.contains(&k.as_str()) && !v.is_null())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if kept.is_empty() {
            return;
        }
        set_segments(
            self.patch,
            ["effects", effect.as_str(), section, name],
            Value::Object(kept),
        );
        if let Some(id) = id_of(obj.get("id")) {
            let path = ["effects", effect.as_str(), section, name, "value"];
            self.model.params.insert(
                id,
                Target::Segments(path.iter().map(|s| s.to_string()).collect()),
            );
        }
    }

    /// A parameter object: its value into state at `path`, and its id
    /// subscribed with `target`.
    fn param(&mut self, p: Option<&Value>, path: &str, target: Target) {
        let Some(p) = p else { return };
        if let Some(v) = p.get("value").filter(|v| !v.is_null()) {
            set(self.patch, path, v.clone());
        }
        if let Some(id) = id_of(p.get("id")) {
            self.model.params.insert(id, target);
        }
    }

    fn plain(&mut self, p: Option<&Value>, path: String) {
        self.param(p, &path, Target::Path(path.clone()));
    }

    /// An event parameter: only its id, for trigger_parameter.
    fn event_id(&mut self, p: Option<&Value>, path: &str) {
        if let Some(id) = id_of(p.and_then(|p| p.get("id"))) {
            set(self.patch, path, json!(id));
        }
    }

    fn composition(&mut self, c: &Value) {
        self.plain(c.get("name"), "composition.name".into());
        self.plain(c.get("master"), "composition.master".into());
        self.plain(c.pointer("/video/opacity"), "composition.opacity".into());
        self.plain(c.get("speed"), "composition.speed".into());
        self.plain(c.get("bypassed"), "composition.bypassed".into());
        let x = c.get("crossfader");
        let xf = |k: &str| x.and_then(|x| x.get(k));
        self.plain(xf("phase"), "crossfader.phase".into());
        self.plain(xf("behaviour"), "crossfader.behaviour".into());
        self.plain(xf("curve"), "crossfader.curve".into());
        self.event_id(xf("sidea"), "crossfader.side_a_parameter");
        self.event_id(xf("sideb"), "crossfader.side_b_parameter");
        let t = c.get("tempocontroller");
        let tc = |k: &str| t.and_then(|t| t.get(k));
        self.plain(tc("tempo"), "tempo.bpm".into());
        self.plain(tc("play_state"), "tempo.play_state".into());
        self.event_id(tc("tempo_tap"), "tempo.tap_parameter");
        self.event_id(tc("resync"), "tempo.resync_parameter");
        self.event_id(tc("tempo_push"), "tempo.push_parameter");
        self.event_id(tc("tempo_pull"), "tempo.pull_parameter");
        for (key, path) in [
            ("description", "composition.description"),
            ("highdetail", "composition.high_detail"),
            ("framerate", "composition.frame_rate"),
            ("cliptarget", "composition.clip_target"),
            ("cliptriggerstyle", "composition.clip_trigger_style"),
            ("clipbeatsnap", "composition.clip_beat_snap"),
        ] {
            self.plain(c.get(key), path.into());
        }
        self.audio(c.get("audio"), "composition");
        self.effects(c, "composition", None);

        let array = |k: &str| array(c, k);
        let columns: Vec<u64> = array("columns")
            .iter()
            .filter_map(|col| id_of(col.get("id")))
            .collect();
        for (i, col) in array("columns").iter().enumerate() {
            let Some(id) = id_of(col.get("id")) else {
                continue;
            };
            self.model.columns.insert(id);
            let at = format!("columns.{id}");
            set(self.patch, &format!("{at}.position"), json!(i + 1));
            self.plain(col.get("name"), format!("{at}.name"));
            self.plain(col.get("connected"), format!("{at}.connected"));
            self.plain(col.get("selected"), format!("{at}.selected"));
            self.plain(col.get("colorid"), format!("{at}.color"));
            self.plain(col.get("beatsnap"), format!("{at}.beat_snap"));
        }
        for (i, deck) in array("decks").iter().enumerate() {
            let Some(id) = id_of(deck.get("id")) else {
                continue;
            };
            self.model.decks.insert(id);
            let at = format!("decks.{id}");
            set(self.patch, &format!("{at}.position"), json!(i + 1));
            self.plain(deck.get("name"), format!("{at}.name"));
            self.plain(deck.get("colorid"), format!("{at}.color"));
            self.param(
                deck.get("selected"),
                &format!("{at}.selected"),
                Target::DeckSelected(id),
            );
            if deck.pointer("/selected/value") == Some(&Value::Bool(true)) {
                self.model.selected_deck = Some(id);
            }
            if let Some(closed) = deck.get("closed").filter(|v| v.is_boolean()) {
                set(self.patch, &format!("{at}.closed"), closed.clone());
            }
        }
        for (i, layer) in array("layers").iter().enumerate() {
            self.layer(layer, Some(i + 1), None, &columns);
        }
        for (i, group) in array("layergroups").iter().enumerate() {
            let Some(id) = id_of(group.get("id")) else {
                continue;
            };
            self.model.groups.insert(id);
            let at = format!("groups.{id}");
            set(self.patch, &format!("{at}.position"), json!(i + 1));
            self.plain(group.get("name"), format!("{at}.name"));
            self.plain(group.pointer("/video/opacity"), format!("{at}.opacity"));
            self.plain(group.get("master"), format!("{at}.master"));
            self.plain(group.get("bypassed"), format!("{at}.bypassed"));
            self.plain(group.get("solo"), format!("{at}.solo"));
            self.plain(group.get("selected"), format!("{at}.selected"));
            self.plain(group.get("speed"), format!("{at}.speed"));
            self.plain(
                group.get("crossfadergroup"),
                format!("{at}.crossfader_group"),
            );
            self.plain(
                group.get("ignorecolumntrigger"),
                format!("{at}.ignore_column_trigger"),
            );
            self.plain(group.get("colorid"), format!("{at}.color"));
            self.audio(group.get("audio"), &at);
            self.effects(group, "group", Some(id));
            for layer in group
                .get("layers")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                self.layer(layer, None, Some(id), &columns);
            }
        }
    }

    /// A layer, from the composition's layers (with its position) or a
    /// group's (with the group). A layer listed in both is read once and gets
    /// both.
    fn layer(
        &mut self,
        layer: &Value,
        position: Option<usize>,
        group: Option<u64>,
        columns: &[u64],
    ) {
        let Some(id) = id_of(layer.get("id")) else {
            return;
        };
        let at = format!("layers.{id}");
        if let Some(p) = position {
            set(self.patch, &format!("{at}.position"), json!(p));
        }
        if let Some(g) = group {
            set(self.patch, &format!("{at}.group"), json!(g));
        }
        if !self.model.layers.insert(id) {
            return;
        }
        self.plain(layer.get("name"), format!("{at}.name"));
        self.plain(layer.pointer("/video/opacity"), format!("{at}.opacity"));
        self.plain(layer.get("master"), format!("{at}.master"));
        self.plain(layer.get("bypassed"), format!("{at}.bypassed"));
        self.plain(layer.get("solo"), format!("{at}.solo"));
        self.plain(layer.get("selected"), format!("{at}.selected"));
        self.plain(
            layer.get("crossfadergroup"),
            format!("{at}.crossfader_group"),
        );
        for (key, field) in [
            ("maskmode", "mask_mode"),
            ("ignorecolumntrigger", "ignore_column_trigger"),
            ("faderstart", "fader_start"),
            ("colorid", "color"),
        ] {
            self.plain(layer.get(key), format!("{at}.{field}"));
        }
        self.audio(layer.get("audio"), &at);
        let transition = layer.get("transition");
        self.plain(
            transition.and_then(|t| t.get("duration")),
            format!("{at}.transition_duration"),
        );
        self.plain(
            transition.and_then(|t| t.get("blend_mode")),
            format!("{at}.transition_blend_mode"),
        );
        self.effects(layer, "layer", Some(id));

        let mut clips = Vec::new();
        for (i, clip) in layer
            .get("clips")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let Some(cid) = id_of(clip.get("id")) else {
                continue;
            };
            let cat = format!("clips.{cid}");
            set(self.patch, &format!("{cat}.layer"), json!(id));
            if let Some(col) = columns.get(i) {
                set(self.patch, &format!("{cat}.column"), json!(col));
            }
            self.param(
                clip.get("name"),
                &format!("{cat}.name"),
                Target::ClipName(cid),
            );
            self.param(
                clip.get("connected"),
                &format!("{cat}.connected"),
                Target::ClipConnected(cid),
            );
            self.clip_settings(clip, &cat);
            self.effects(clip, "clip", Some(cid));
            let value = |k: &str| clip.get(k).and_then(|p| p.get("value")).cloned();
            self.model.clips.insert(
                cid,
                Clip {
                    layer: id,
                    name: value("name").filter(|v| !v.is_null()),
                    connected: value("connected").and_then(|v| v.as_str().map(str::to_string)),
                },
            );
            clips.push(cid);
        }
        self.model.layer_clips.insert(id, clips);
        // The composition's own answer, where it gives one; otherwise the
        // first clip whose state says it plays.
        let given = layer.get("active_clip").filter(|c| c.is_object());
        let active = given
            .and_then(|c| id_of(c.get("id")))
            .or_else(|| self.model.playing(id));
        self.model.active.insert(id, active);
        self.model.active_state(id, self.patch);
        // A playing clip missing from the layer's list: its name is the
        // composition's.
        if let (Some(c), Some(cid)) = (given, active) {
            if !self.model.clips.contains_key(&cid) {
                if let Some(name) = c.pointer("/name/value").filter(|v| !v.is_null()) {
                    set(self.patch, &format!("{at}.active_clip_name"), name.clone());
                }
            }
        }
    }
}

impl Model {
    /// The state and model a composition gives.
    fn read(composition: &Value, options: Options) -> (Model, Value) {
        let mut model = Model::default();
        let mut patch = json!({});
        Reader {
            model: &mut model,
            patch: &mut patch,
            options,
        }
        .composition(composition);
        (model, patch)
    }

    /// The first clip of a layer that is playing.
    fn playing(&self, layer: u64) -> Option<u64> {
        self.layer_clips.get(&layer)?.iter().copied().find(|c| {
            self.clips
                .get(c)
                .and_then(|c| c.connected.as_deref())
                .is_some_and(is_connected)
        })
    }

    /// A layer's active clip and its name into `patch`; null (removed) when
    /// nothing plays.
    fn active_state(&self, layer: u64, patch: &mut Value) {
        let at = format!("layers.{layer}");
        let active = self.active.get(&layer).copied().flatten();
        let name = active
            .and_then(|c| self.clips.get(&c))
            .and_then(|c| c.name.clone());
        set(patch, &format!("{at}.active_clip"), json!(active));
        set(
            patch,
            &format!("{at}.active_clip_name"),
            name.unwrap_or(Value::Null),
        );
    }

    /// Null every item `self` had that `next` has not.
    fn removed(&self, next: &Model, patch: &mut Value) {
        let gone = |a: &BTreeSet<u64>, b: &BTreeSet<u64>, what: &str, patch: &mut Value| {
            for id in a.difference(b) {
                set(patch, &format!("{what}.{id}"), Value::Null);
            }
        };
        gone(&self.layers, &next.layers, "layers", patch);
        gone(&self.columns, &next.columns, "columns", patch);
        gone(&self.decks, &next.decks, "decks", patch);
        gone(&self.groups, &next.groups, "groups", patch);
        gone(&self.effects, &next.effects, "effects", patch);
        for id in self.clips.keys().filter(|c| !next.clips.contains_key(c)) {
            set(patch, &format!("clips.{id}"), Value::Null);
        }
    }

    /// A parameter message: the state it changes, and whether it switched
    /// the deck.
    fn update(&mut self, msg: &Value) -> Option<(Value, bool)> {
        let id = parameter_id(msg)?;
        let target = self.params.get(&id)?.clone();
        let value = msg.get("value").filter(|v| !v.is_null())?.clone();
        let mut patch = json!({});
        let mut switched = false;
        match target {
            Target::Path(path) => set(&mut patch, &path, value),
            Target::Segments(path) => {
                set_segments(&mut patch, path.iter().map(String::as_str), value)
            }
            Target::ClipName(clip) => {
                set(&mut patch, &format!("clips.{clip}.name"), value.clone());
                let c = self.clips.get_mut(&clip)?;
                c.name = Some(value.clone());
                let layer = c.layer;
                if self.active.get(&layer).copied().flatten() == Some(clip) {
                    set(
                        &mut patch,
                        &format!("layers.{layer}.active_clip_name"),
                        value,
                    );
                }
            }
            Target::ClipConnected(clip) => {
                set(
                    &mut patch,
                    &format!("clips.{clip}.connected"),
                    value.clone(),
                );
                let c = self.clips.get_mut(&clip)?;
                c.connected = value.as_str().map(str::to_string);
                let layer = c.layer;
                let now = self.playing(layer);
                if self.active.get(&layer).copied().flatten() != now {
                    self.active.insert(layer, now);
                    self.active_state(layer, &mut patch);
                }
            }
            Target::DeckSelected(deck) => {
                set(&mut patch, &format!("decks.{deck}.selected"), value.clone());
                if value == Value::Bool(true) && self.selected_deck != Some(deck) {
                    self.selected_deck = Some(deck);
                    switched = true;
                }
            }
        }
        Some((patch, switched))
    }
}

pub(crate) struct ResolumePush {
    engine: SpecEngine,
    ws: WsRequest,
    origin: String,
    model: Option<Model>,
    /// Parameter ids subscribed on the open websocket.
    subscribed: BTreeSet<u64>,
    open: bool,
    stopped: bool,
    backoff: Millis,
    next_id: RequestId,
    refreshing: Option<RequestId>,
    /// The safety refresh period; 0 for none.
    refresh_every: Millis,
    /// Commands in flight that change the structure.
    structural: HashMap<CommandId, String>,
    options: Options,
}

impl ResolumePush {
    pub(crate) fn new(engine: SpecEngine, spec: &DeviceSpec, ctx: &OpenContext) -> ResolumePush {
        let default_port = spec
            .transport
            .as_ref()
            .and_then(|t| t.get("port"))
            .and_then(Value::as_u64)
            .map(|p| p as u16)
            .unwrap_or(8080);
        let host = match ctx.host {
            IpAddr::V6(v6) => format!("[{v6}]"),
            v4 => v4.to_string(),
        };
        let authority = format!("{host}:{}", ctx.port.unwrap_or(default_port));
        let refresh_s = ctx
            .settings
            .get("composition_refresh_s")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_REFRESH_S);
        let flag = |name: &str| {
            ctx.settings
                .get(name)
                .and_then(Value::as_bool)
                .unwrap_or(true)
        };
        let options = Options {
            clip_positions: flag("clip_positions"),
            effect_parameters: flag("effect_parameters"),
        };
        ResolumePush {
            engine,
            ws: WsRequest {
                url: format!("ws://{authority}/api/v1"),
                headers: Vec::new(),
                accept_invalid_certs: false,
            },
            origin: format!("http://{authority}"),
            model: None,
            subscribed: BTreeSet::new(),
            open: false,
            stopped: false,
            backoff: REOPEN_MIN,
            next_id: OWN_IDS,
            refreshing: None,
            refresh_every: refresh_s.saturating_mul(1000),
            structural: HashMap::new(),
            options,
        }
    }

    /// Run an engine callback, watching for structural commands completing.
    fn relay(&mut self, cx: &mut Cx, f: impl FnOnce(&mut SpecEngine, &mut Cx)) {
        let mut inner = Cx::new(cx.now());
        f(&mut self.engine, &mut inner);
        let mut refresh = false;
        for action in inner.take() {
            if let Action::Complete { id, result } = &action {
                if self.structural.remove(id).is_some() && result.is_ok() {
                    refresh = true;
                }
            }
            cx.push(action);
        }
        if refresh {
            self.refresh_soon(cx);
        }
    }

    fn refresh_soon(&mut self, cx: &mut Cx) {
        if self.open {
            cx.set_timer(REFRESH, SOON);
        }
    }

    /// Restart the safety refresh period.
    fn rearm(&mut self, cx: &mut Cx) {
        if self.refresh_every > 0 {
            cx.set_timer(REFRESH, self.refresh_every);
        } else {
            cx.cancel_timer(REFRESH);
        }
    }

    fn refresh(&mut self, cx: &mut Cx) {
        if !self.open || self.refreshing.is_some() {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.refreshing = Some(id);
        cx.http(
            id,
            HttpRequest {
                method: "GET",
                url: format!("{}/api/v1/composition", self.origin),
                headers: Vec::new(),
                body: None,
                body_stream: None,
                timeout: Some(REFRESH_TIMEOUT),
                accept_invalid_certs: false,
                digest: None,
            },
        );
    }

    /// A composition, from the websocket or a refresh: into state, and the
    /// subscriptions brought into line with it.
    fn composition(&mut self, cx: &mut Cx, doc: &Value) {
        let (next, mut patch) = Model::read(doc, self.options);
        if let Some(last) = &self.model {
            last.removed(&next, &mut patch);
        }
        cx.state(patch);
        self.model = Some(next);
        self.sync(cx);
        self.rearm(cx);
    }

    /// Subscribe to what the model shows and is not subscribed; unsubscribe
    /// from what it no longer shows.
    fn sync(&mut self, cx: &mut Cx) {
        if !self.open {
            return;
        }
        let wanted: BTreeSet<u64> = self
            .model
            .as_ref()
            .map(|m| m.params.keys().copied().collect())
            .unwrap_or_default();
        for id in wanted.difference(&self.subscribed) {
            cx.ws_send(WS, subscription("subscribe", *id));
        }
        for id in self.subscribed.difference(&wanted) {
            cx.ws_send(WS, subscription("unsubscribe", *id));
        }
        self.subscribed = wanted;
    }

    fn inbound(&mut self, cx: &mut Cx, text: &str) {
        let Ok(doc) = serde_json::from_str::<Value>(text) else {
            return;
        };
        match classify(doc) {
            Message::Composition(c) => self.composition(cx, &c),
            Message::Parameter(p) => {
                let Some(model) = self.model.as_mut() else {
                    return;
                };
                if let Some((patch, switched)) = model.update(&p) {
                    cx.state(patch);
                    if switched {
                        self.refresh_soon(cx);
                    }
                }
            }
            Message::Other => {}
        }
    }

    fn reopen_later(&mut self, cx: &mut Cx, reason: &str) {
        cx.log(
            Level::Info,
            format!(
                "websocket closed: {reason}; reopening in {} ms",
                self.backoff
            ),
        );
        cx.set_timer(REOPEN, self.backoff);
        self.backoff = (self.backoff * 2).min(REOPEN_MAX);
    }
}

impl Module for ResolumePush {
    fn start(&mut self, cx: &mut Cx) {
        self.engine.start(cx);
        cx.ws_open(WS, self.ws.clone());
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if STRUCTURAL.contains(&name) {
            self.structural.insert(id, name.to_string());
        }
        self.relay(cx, |e, cx| e.command(cx, id, name, params));
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: std::net::SocketAddr, data: &[u8]) {
        self.relay(cx, |e, cx| e.datagram(cx, socket, from, data));
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        self.relay(cx, |e, cx| e.socket_error(cx, socket, message));
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        self.relay(cx, |e, cx| e.tcp(cx, socket, input));
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        if socket != WS {
            return self.relay(cx, |e, cx| e.ws(cx, socket, input));
        }
        if self.stopped {
            return;
        }
        match input {
            WsInput::Opened => {
                self.open = true;
                self.backoff = REOPEN_MIN;
                self.subscribed.clear();
                cx.alive();
                // Resolume sends the composition at once; read it over REST
                // if it does not.
                cx.set_timer(REFRESH, FIRST_COMPOSITION);
            }
            WsInput::Text(text) => {
                cx.alive();
                self.inbound(cx, &text);
            }
            WsInput::Binary(_) | WsInput::Activity | WsInput::Pong(_) => cx.alive(),
            WsInput::Subprotocol(_) => {}
            WsInput::Closed { reason, .. } => {
                self.open = false;
                self.subscribed.clear();
                self.refreshing = None;
                cx.cancel_timer(REFRESH);
                self.reopen_later(cx, &reason);
            }
        }
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        if id < OWN_IDS {
            return self.relay(cx, |e, cx| e.http_response(cx, id, result));
        }
        if self.refreshing != Some(id) {
            return;
        }
        self.refreshing = None;
        let body = match result {
            Ok(r) if (200..300).contains(&r.status) => r.body,
            Ok(r) => {
                cx.log(
                    Level::Debug,
                    format!("composition refresh refused (HTTP {})", r.status),
                );
                return self.rearm(cx);
            }
            Err(e) => {
                cx.log(Level::Debug, format!("composition refresh: {e}"));
                return self.rearm(cx);
            }
        };
        match serde_json::from_slice::<Value>(&body) {
            Ok(doc) if is_composition(&doc) => {
                cx.alive();
                self.composition(cx, &doc);
            }
            _ => {
                cx.log(Level::Debug, "composition refresh: not a composition");
                self.rearm(cx);
            }
        }
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        self.relay(cx, |e, cx| e.sse(cx, stream, input));
    }

    fn file(&mut self, cx: &mut Cx, file: Key, input: FileInput) {
        self.relay(cx, |e, cx| e.file(cx, file, input));
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            REOPEN if !self.stopped => cx.ws_open(WS, self.ws.clone()),
            REFRESH => self.refresh(cx),
            REOPEN => {}
            _ => self.relay(cx, |e, cx| e.timer(cx, key)),
        }
    }

    fn stream_watch(&mut self, cx: &mut Cx, stream: &str, watching: bool) {
        self.relay(cx, |e, cx| e.stream_watch(cx, stream, watching));
    }

    fn stop(&mut self, cx: &mut Cx) {
        self.stopped = true;
        self.open = false;
        cx.cancel_timer(REFRESH);
        cx.cancel_timer(REOPEN);
        cx.ws_close(WS);
        self.relay(cx, |e, cx| e.stop(cx));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::catalog::Catalog;
    use crate::session::merge_patch;

    const LAYER_1: u64 = 1001;
    const LAYER_2: u64 = 1002;
    const COLUMN_1: u64 = 2001;
    const COLUMN_2: u64 = 2002;
    const DECK_1: u64 = 3001;
    const DECK_2: u64 = 3002;
    const CLIP_11: u64 = 4011;
    const CLIP_12: u64 = 4012;
    const CLIP_21: u64 = 4021;
    const CLIP_22: u64 = 4022;
    const GROUP: u64 = 5001;

    /// A parameter object, with its id derived from `id` and a field number.
    fn p(id: u64, value: Value) -> Value {
        json!({ "id": id, "value": value })
    }

    fn clip(id: u64, name: &str, connected: &str) -> Value {
        json!({
            "id": id,
            "name": p(id * 10 + 1, json!(name)),
            "connected": { "id": id * 10 + 2, "valuetype": "ParamState", "value": connected, "index": 0 },
            "selected": p(id * 10 + 3, json!(false)),
        })
    }

    fn layer(id: u64, name: &str, clips: Vec<Value>, active: Value) -> Value {
        json!({
            "id": id,
            "name": p(id * 10 + 1, json!(name)),
            "selected": p(id * 10 + 2, json!(false)),
            "bypassed": p(id * 10 + 3, json!(false)),
            "solo": p(id * 10 + 4, json!(false)),
            "master": p(id * 10 + 5, json!(1.0)),
            "crossfadergroup": p(id * 10 + 6, json!("None")),
            "video": { "opacity": p(id * 10 + 7, json!(1.0)) },
            "clips": clips,
            "active_clip": active,
        })
    }

    /// Two layers of two clips, two columns, two decks and a group; clip
    /// 1/1 plays.
    fn composition(extra_layer: bool) -> Value {
        let mut layers = vec![
            layer(
                LAYER_1,
                "Background",
                vec![
                    clip(CLIP_11, "Clouds", "Connected"),
                    clip(CLIP_12, "Fire", "Disconnected"),
                ],
                clip(CLIP_11, "Clouds", "Connected"),
            ),
            layer(
                LAYER_2,
                "Overlay",
                vec![
                    clip(CLIP_21, "", "Empty"),
                    clip(CLIP_22, "Logo", "Disconnected"),
                ],
                Value::Null,
            ),
        ];
        if extra_layer {
            layers.push(layer(
                1003,
                "Text",
                vec![clip(4031, "", "Empty"), clip(4032, "", "Empty")],
                Value::Null,
            ));
        }
        json!({
            "name": p(1, json!("Show")),
            "master": p(2, json!(1.0)),
            "speed": p(3, json!(1.0)),
            "bypassed": p(4, json!(false)),
            "video": { "opacity": p(5, json!(0.5)) },
            "crossfader": {
                "id": 6, "phase": p(7, json!(0.0)), "behaviour": p(8, json!("Fade")),
                "curve": p(9, json!("Linear")),
                "sidea": { "id": 10, "valuetype": "ParamEvent" },
                "sideb": { "id": 11, "valuetype": "ParamEvent" },
            },
            "tempocontroller": {
                "tempo": p(12, json!(120.0)), "play_state": p(13, json!("Playing")),
                "tempo_tap": { "id": 14 }, "resync": { "id": 15 },
                "tempo_push": { "id": 16 }, "tempo_pull": { "id": 17 },
            },
            "decks": [
                { "id": DECK_1, "closed": false, "name": p(DECK_1 * 10 + 1, json!("Set 1")),
                  "selected": p(DECK_1 * 10 + 2, json!(true)) },
                { "id": DECK_2, "closed": false, "name": p(DECK_2 * 10 + 1, json!("Set 2")),
                  "selected": p(DECK_2 * 10 + 2, json!(false)) },
            ],
            "layers": layers,
            "columns": [
                { "id": COLUMN_1, "name": p(COLUMN_1 * 10 + 1, json!("Intro")),
                  "connected": p(COLUMN_1 * 10 + 2, json!("Disconnected")),
                  "selected": p(COLUMN_1 * 10 + 3, json!(true)) },
                { "id": COLUMN_2, "name": p(COLUMN_2 * 10 + 1, json!("Drop")),
                  "connected": p(COLUMN_2 * 10 + 2, json!("Disconnected")),
                  "selected": p(COLUMN_2 * 10 + 3, json!(false)) },
            ],
            "layergroups": [
                { "id": GROUP, "name": p(GROUP * 10 + 1, json!("Front")),
                  "selected": p(GROUP * 10 + 2, json!(false)), "bypassed": p(GROUP * 10 + 3, json!(false)),
                  "solo": p(GROUP * 10 + 4, json!(false)), "master": p(GROUP * 10 + 5, json!(1.0)),
                  "video": { "opacity": p(GROUP * 10 + 6, json!(1.0)) },
                  "layers": [ { "id": LAYER_2 } ] },
            ],
        })
    }

    fn module() -> ResolumePush {
        let catalog = Catalog::embedded();
        let spec = catalog.device("resolume").unwrap().clone();
        let ctx = OpenContext {
            host: "192.0.2.10".parse().unwrap(),
            host_name: None,
            port: None,
            model: "arena".into(),
            channels: None,
            settings: Params::new(),
            monitor: true,
        };
        let engine = SpecEngine::new(Arc::new(spec.clone()), ctx.clone()).unwrap();
        ResolumePush::new(engine, &spec, &ctx)
    }

    fn state_of(actions: &[Action], state: &mut Value) {
        for a in actions {
            if let Action::State(p) = a {
                merge_patch(state, p);
            }
        }
    }

    fn sent(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::WsSend { socket: WS, text } => serde_json::from_str(text).ok(),
                _ => None,
            })
            .collect()
    }

    fn refresh_requests(actions: &[Action]) -> Vec<String> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::Http { id, request } if *id >= OWN_IDS => Some(request.url.clone()),
                _ => None,
            })
            .collect()
    }

    fn timer_after(actions: &[Action], key: Key) -> Option<Millis> {
        actions.iter().rev().find_map(|a| match a {
            Action::SetTimer { key: k, after } if *k == key => Some(*after),
            _ => None,
        })
    }

    /// Started, websocket open, composition received.
    fn connected(state: &mut Value) -> (ResolumePush, Vec<Action>) {
        let mut m = module();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let started = cx.take();
        assert!(started.iter().any(|a| matches!(
            a,
            Action::WsOpen { socket: WS, request } if request.url == "ws://192.0.2.10:8080/api/v1"
        )));
        let mut cx = Cx::new(1);
        m.ws(&mut cx, WS, WsInput::Opened);
        m.ws(&mut cx, WS, WsInput::Text(composition(false).to_string()));
        m.ws(
            &mut cx,
            WS,
            WsInput::Text(json!({"type": "sources_update", "value": {}}).to_string()),
        );
        let actions = cx.take();
        state_of(&actions, state);
        (m, actions)
    }

    #[test]
    fn messages_are_told_apart() {
        assert!(matches!(
            classify(composition(false)),
            Message::Composition(_)
        ));
        let wrapped = json!({"type": "composition_update", "value": composition(false)});
        assert!(matches!(classify(wrapped), Message::Composition(_)));
        assert_eq!(
            classify(json!({"type": "sources_update", "value": {"video": []}})),
            Message::Other
        );
        assert_eq!(
            classify(json!({"type": "effects_update", "value": {}})),
            Message::Other
        );
        assert!(matches!(
            classify(json!({"type": "parameter_update", "path": "/parameter/by-id/5", "value": 1})),
            Message::Parameter(_)
        ));
        assert_eq!(
            classify(json!({"type": "parameter_unsubscribed", "path": "/parameter/by-id/5"})),
            Message::Other
        );
        assert_eq!(
            classify(json!({"id": "my_new_layer", "error": null})),
            Message::Other
        );
        assert_eq!(
            parameter_id(&json!({"path": "/parameter/by-id/1699880877077", "value": 1})),
            Some(1699880877077)
        );
    }

    #[test]
    fn the_composition_is_read_into_state() {
        let (model, state) = Model::read(&composition(false), Options::default());
        assert_eq!(
            state["composition"],
            json!({"name": "Show", "master": 1.0, "speed": 1.0, "bypassed": false, "opacity": 0.5})
        );
        assert_eq!(
            state["crossfader"],
            json!({"phase": 0.0, "behaviour": "Fade", "curve": "Linear",
                   "side_a_parameter": 10, "side_b_parameter": 11})
        );
        assert_eq!(
            state["tempo"],
            json!({"bpm": 120.0, "play_state": "Playing", "tap_parameter": 14,
                   "resync_parameter": 15, "push_parameter": 16, "pull_parameter": 17})
        );
        assert_eq!(
            state["layers"]["1001"],
            json!({"position": 1, "name": "Background", "opacity": 1.0, "master": 1.0,
                   "bypassed": false, "solo": false, "selected": false, "crossfader_group": "None",
                   "active_clip": CLIP_11, "active_clip_name": "Clouds"})
        );
        // Nothing plays in layer 2, which is in the group.
        assert_eq!(state["layers"]["1002"]["active_clip"], Value::Null);
        assert_eq!(state["layers"]["1002"]["group"], json!(GROUP));
        assert_eq!(state["layers"]["1002"]["position"], json!(2));
        assert_eq!(
            state["clips"]["4012"],
            json!({"layer": LAYER_1, "column": COLUMN_2, "name": "Fire", "connected": "Disconnected"})
        );
        assert_eq!(
            state["columns"]["2001"],
            json!({"position": 1, "name": "Intro", "connected": "Disconnected", "selected": true})
        );
        assert_eq!(
            state["decks"]["3002"],
            json!({"position": 2, "name": "Set 2", "selected": false, "closed": false})
        );
        assert_eq!(
            state["groups"]["5001"],
            json!({"position": 1, "name": "Front", "opacity": 1.0, "master": 1.0,
                   "bypassed": false, "solo": false, "selected": false})
        );
        assert_eq!(model.selected_deck, Some(DECK_1));
        // Every parameter shown, and no event parameter.
        assert_eq!(
            model.params.get(&5),
            Some(&Target::Path("composition.opacity".into()))
        );
        assert_eq!(
            model.params.get(&(CLIP_12 * 10 + 2)),
            Some(&Target::ClipConnected(CLIP_12))
        );
        assert_eq!(
            model.params.get(&(DECK_2 * 10 + 2)),
            Some(&Target::DeckSelected(DECK_2))
        );
        assert!(!model.params.contains_key(&10) && !model.params.contains_key(&14));
        // A clip's selected parameter is not part of the state.
        assert!(!model.params.contains_key(&(CLIP_12 * 10 + 3)));
        // 10 composition + 2x3 columns + 2x2 decks + 2x7 layers + 4x2 clips + 6 group.
        assert_eq!(model.params.len(), 10 + 6 + 4 + 14 + 8 + 6);
    }

    #[test]
    fn every_state_path_written_is_declared_in_the_spec() {
        let catalog = Catalog::embedded();
        let spec = catalog.device("resolume").unwrap();
        let declared: Vec<Vec<String>> = spec
            .state
            .keys()
            .map(|k| k.split('.').map(str::to_string).collect())
            .collect();
        // An object-typed path holds its fields: they are its, not declared.
        let objects: Vec<Vec<String>> = spec
            .state
            .iter()
            .filter(|(_, d)| d.kind == "object")
            .map(|(k, _)| k.split('.').map(str::to_string).collect())
            .collect();
        let mut state = Model::read(&composition(true), Options::default()).1;
        merge_patch(
            &mut state,
            &Model::read(&rich_composition(), Options::default()).1,
        );
        fn leaves(v: &Value, at: Vec<String>, out: &mut Vec<Vec<String>>) {
            match v {
                Value::Object(m) => {
                    for (k, v) in m {
                        let mut next = at.clone();
                        next.push(k.clone());
                        leaves(v, next, out);
                    }
                }
                _ => out.push(at),
            }
        }
        let mut paths = Vec::new();
        leaves(&state, Vec::new(), &mut paths);
        let matches = |d: &Vec<String>, path: &[String]| {
            d.len() == path.len() && d.iter().zip(path).all(|(a, b)| a == "*" || a == b)
        };
        for path in paths {
            let in_object = objects
                .iter()
                .any(|o| path.len() > o.len() && matches(o, &path[..o.len()]));
            assert!(
                in_object || declared.iter().any(|d| matches(d, &path)),
                "{} is not declared",
                path.join(".")
            );
        }
    }

    /// A composition with what the full-control state reads: effect chains
    /// on the composition, a layer and a clip, audio tracks, a layer
    /// transition and a clip's transport.
    fn rich_composition() -> Value {
        let effect = |id: u64, name: &str| {
            json!({
                "id": id, "name": name, "display_name": name, "is_user_removable": true,
                "bypassed": p(id * 10 + 1, json!(false)),
                "params": {
                    "Amount": { "id": id * 10 + 2, "valuetype": "ParamRange", "min": 0.0, "max": 1.0,
                                "value": 0.5, "view": { "suffix": "" } },
                    "Mode.Fast": { "id": id * 10 + 3, "valuetype": "ParamChoice", "value": "On",
                                   "index": 1, "options": ["Off", "On"] },
                },
                "mixer": { "Blend Mode": { "id": id * 10 + 4, "valuetype": "ParamChoice", "value": "Add",
                                           "options": ["Add", "Alpha"] } },
            })
        };
        json!({
            "description": p(21, json!("Opening")),
            "highdetail": p(22, json!(false)),
            "framerate": p(23, json!(60.0)),
            "cliptarget": p(24, json!("Layer")),
            "cliptriggerstyle": p(25, json!("Normal")),
            "clipbeatsnap": p(26, json!("None")),
            "audio": { "volume": p(27, json!(0.0)), "pan": p(28, json!(0.0)),
                       "effects": [ effect(7001, "EQ-3") ] },
            "video": { "opacity": p(5, json!(1.0)), "effects": [ effect(7002, "Blow") ] },
            "layers": [ {
                "id": 1101,
                "name": p(11011, json!("Main")),
                "maskmode": p(11012, json!("No Mask")),
                "ignorecolumntrigger": p(11013, json!(false)),
                "faderstart": p(11014, json!(true)),
                "colorid": p(11015, json!("Red")),
                "audio": { "volume": p(11016, json!(-6.0)), "pan": p(11017, json!(0.25)) },
                "video": { "opacity": p(11018, json!(1.0)), "effects": [ effect(7003, "Transform") ] },
                "transition": { "duration": p(11019, json!(1.5)), "blend_mode": p(11020, json!("Alpha")) },
                "clips": [ {
                    "id": 4101,
                    "name": p(41011, json!("Clouds")),
                    "connected": p(41012, json!("Connected")),
                    "transporttype": p(41013, json!("Timeline")),
                    "triggerstyle": p(41014, json!("Toggle")),
                    "target": p(41015, json!("Own Layer")),
                    "beatsnap": p(41016, json!("None")),
                    "colorid": p(41017, json!("Blue")),
                    "audio": { "volume": p(41018, json!(0.0)) },
                    "video": { "opacity": p(41019, json!(1.0)), "effects": [ effect(7004, "Hue Rotate") ] },
                    "transport": {
                        "position": { "id": 41020, "valuetype": "ParamRange", "min": 0.0, "max": 19269.25,
                                      "value": 1250.0 },
                        "controls": { "speed": p(41021, json!(1.0)), "playdirection": p(41022, json!(">")),
                                      "playmode": p(41023, json!("Loop")),
                                      "duration": p(41024, json!(19.269)) },
                    },
                } ],
            } ],
            "columns": [ { "id": 2101, "name": p(21011, json!("A")), "colorid": p(21012, json!("Green")),
                           "beatsnap": p(21013, json!("1 Bar")) } ],
            "decks": [ { "id": 3101, "name": p(31011, json!("D")), "colorid": p(31012, json!("Grey")),
                         "selected": p(31013, json!(true)) } ],
            "layergroups": [ { "id": 5101, "name": p(51011, json!("G")), "speed": p(51012, json!(1.0)),
                               "crossfadergroup": p(51013, json!("A")),
                               "ignorecolumntrigger": p(51014, json!(false)),
                               "colorid": p(51015, json!("Red")),
                               "audio": { "volume": p(51016, json!(0.0)), "pan": p(51017, json!(0.0)) },
                               "video": { "effects": [ effect(7005, "Mirror") ] },
                               "layers": [] } ],
        })
    }

    #[test]
    fn effects_transport_and_settings_are_read_and_subscribed() {
        let (model, state) = Model::read(&rich_composition(), Options::default());
        assert_eq!(state["composition"]["description"], "Opening");
        assert_eq!(state["composition"]["frame_rate"], 60.0);
        assert_eq!(state["composition"]["volume"], 0.0);
        let layer = &state["layers"]["1101"];
        assert_eq!(layer["volume"], -6.0);
        assert_eq!(layer["pan"], 0.25);
        assert_eq!(layer["transition_duration"], 1.5);
        assert_eq!(layer["transition_blend_mode"], "Alpha");
        assert_eq!(layer["mask_mode"], "No Mask");
        assert_eq!(layer["color"], "Red");
        let clip = &state["clips"]["4101"];
        assert_eq!(clip["transport_position"], 1250.0);
        assert_eq!(clip["transport_position_max"], 19269.25);
        assert_eq!(clip["speed"], 1.0);
        assert_eq!(clip["play_direction"], ">");
        assert_eq!(clip["play_mode"], "Loop");
        assert_eq!(clip["duration"], 19.269);
        assert_eq!(clip["transport_type"], "Timeline");
        assert_eq!(state["groups"]["5101"]["crossfader_group"], "A");
        assert_eq!(state["columns"]["2101"]["beat_snap"], "1 Bar");
        assert_eq!(state["decks"]["3101"]["color"], "Grey");

        // Effects: each chain's effects, keyed by id, with owner and place.
        let eq = &state["effects"]["7001"];
        assert_eq!(eq["chain"], "audio");
        assert_eq!(eq["owner"], "composition");
        assert_eq!(eq["owner_id"], Value::Null);
        assert_eq!(eq["position"], 1);
        let hue = &state["effects"]["7004"];
        assert_eq!(hue["owner"], "clip");
        assert_eq!(hue["owner_id"], 4101);
        assert_eq!(hue["chain"], "video");
        assert_eq!(hue["display_name"], "Hue Rotate");
        assert_eq!(hue["bypassed"], false);
        assert_eq!(hue["removable"], true);
        assert_eq!(
            hue["params"]["Amount"],
            json!({"id": 70042, "valuetype": "ParamRange", "min": 0.0, "max": 1.0, "value": 0.5})
        );
        // A parameter name holding a dot stays one key.
        assert_eq!(hue["params"]["Mode.Fast"]["options"], json!(["Off", "On"]));
        assert_eq!(hue["mixer"]["Blend Mode"]["value"], "Add");
        assert_eq!(state["effects"]["7005"]["owner"], "group");
        assert_eq!(state["effects"]["7003"]["owner_id"], 1101);
        assert_eq!(
            model.params.get(&70043),
            Some(&Target::Segments(
                ["effects", "7004", "params", "Mode.Fast", "value"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            ))
        );
        assert_eq!(
            model.params.get(&41020),
            Some(&Target::Path("clips.4101.transport_position".into()))
        );
        assert_eq!(
            model.params.get(&11016),
            Some(&Target::Path("layers.1101.volume".into()))
        );

        // An update to an effect parameter changes only its value.
        let mut model = model;
        let mut state = state;
        let (patch, _) = model
            .update(&json!({"type": "parameter_update", "path": "/parameter/by-id/70043", "value": "Off"}))
            .unwrap();
        merge_patch(&mut state, &patch);
        assert_eq!(
            state["effects"]["7004"]["params"]["Mode.Fast"]["value"],
            "Off"
        );

        // The effect leaves when a composition no longer holds it.
        let mut next = rich_composition();
        next["layers"][0]["clips"][0]["video"]["effects"] = json!([]);
        let (after, _) = Model::read(&next, Options::default());
        let mut patch = json!({});
        model.removed(&after, &mut patch);
        assert_eq!(patch["effects"]["7004"], Value::Null);
        assert!(patch["effects"].get("7003").is_none());
    }

    #[test]
    fn positions_and_effect_parameters_can_be_left_out() {
        let options = Options {
            clip_positions: false,
            effect_parameters: false,
        };
        let (model, state) = Model::read(&rich_composition(), options);
        assert!(state["clips"]["4101"].get("transport_position").is_none());
        assert_eq!(state["clips"]["4101"]["speed"], 1.0);
        assert!(state["effects"]["7004"].get("params").is_none());
        assert_eq!(state["effects"]["7004"]["bypassed"], false);
        assert!(!model.params.contains_key(&41020));
        assert!(!model.params.contains_key(&70042));
        assert!(model.params.contains_key(&70041));
    }

    #[test]
    fn structural_commands_are_spec_commands() {
        let catalog = Catalog::embedded();
        let spec = catalog.device("resolume").unwrap();
        for name in STRUCTURAL {
            assert!(spec.commands.contains_key(*name), "{name} is not a command");
        }
    }

    #[test]
    fn subscriptions_follow_the_composition() {
        let mut state = json!({});
        let (mut m, actions) = connected(&mut state);
        let subscribed: BTreeSet<String> = sent(&actions)
            .iter()
            .map(|s| {
                assert_eq!(s["action"], "subscribe");
                s["parameter"].as_str().unwrap().to_string()
            })
            .collect();
        assert_eq!(subscribed.len(), m.model.as_ref().unwrap().params.len());
        assert!(subscribed.contains(&format!("/parameter/by-id/{}", CLIP_11 * 10 + 2)));
        assert_eq!(state["layers"]["1001"]["name"], "Background");
        // The safety refresh is armed for the default period.
        assert_eq!(timer_after(&actions, REFRESH), Some(60_000));

        // A layer is added: Resolume sends the composition again. Only the
        // new layer's parameters are subscribed.
        let mut cx = Cx::new(2);
        m.ws(&mut cx, WS, WsInput::Text(composition(true).to_string()));
        let actions = cx.take();
        state_of(&actions, &mut state);
        let added = sent(&actions);
        assert_eq!(added.len(), 7 + 2 * 2);
        assert!(added.iter().all(|s| s["action"] == "subscribe"));
        assert!(added
            .iter()
            .any(|s| s["parameter"] == format!("/parameter/by-id/{}", 1003 * 10 + 1)));
        assert_eq!(state["layers"]["1003"]["name"], "Text");
        assert_eq!(state["layers"]["1003"]["position"], 3);
        assert_eq!(state["clips"]["4031"]["connected"], "Empty");

        // And removed again: its state goes and its parameters are dropped.
        let mut cx = Cx::new(3);
        m.ws(&mut cx, WS, WsInput::Text(composition(false).to_string()));
        let actions = cx.take();
        state_of(&actions, &mut state);
        let dropped = sent(&actions);
        assert_eq!(dropped.len(), 7 + 2 * 2);
        assert!(dropped.iter().all(|s| s["action"] == "unsubscribe"));
        assert!(state["layers"].get("1003").is_none());
        assert!(state["clips"].get("4031").is_none());
        assert!(state["layers"].get("1001").is_some());

        // A reopened websocket subscribes to everything again.
        let mut cx = Cx::new(4);
        m.ws(
            &mut cx,
            WS,
            WsInput::Closed {
                code: None,
                reason: "gone".into(),
            },
        );
        assert_eq!(timer_after(&cx.take(), REOPEN), Some(REOPEN_MIN));
        let mut cx = Cx::new(1_004);
        m.timer(&mut cx, REOPEN);
        m.ws(&mut cx, WS, WsInput::Opened);
        m.ws(&mut cx, WS, WsInput::Text(composition(false).to_string()));
        assert_eq!(sent(&cx.take()).len(), subscribed.len());
    }

    #[test]
    fn parameter_updates_change_state() {
        let mut state = json!({});
        let (mut m, _) = connected(&mut state);
        let update = |id: u64, value: Value| {
            WsInput::Text(
                json!({"type": "parameter_update", "path": format!("/parameter/by-id/{id}"),
                       "id": id, "valuetype": "ParamRange", "value": value})
                .to_string(),
            )
        };
        let mut cx = Cx::new(5);
        m.ws(&mut cx, WS, update(LAYER_1 * 10 + 7, json!(0.25)));
        m.ws(&mut cx, WS, update(12, json!(128.0)));
        // A subscription's answer carries the value too, and an id only in
        // the path is enough.
        m.ws(
            &mut cx,
            WS,
            WsInput::Text(
                json!({"type": "parameter_subscribed", "path": format!("/parameter/by-id/{}", COLUMN_2 * 10 + 1),
                       "value": "Chorus"})
                .to_string(),
            ),
        );
        // An id nothing shows changes nothing.
        m.ws(&mut cx, WS, update(999_999, json!(1)));
        state_of(&cx.take(), &mut state);
        assert_eq!(state["layers"]["1001"]["opacity"], 0.25);
        assert_eq!(state["tempo"]["bpm"], 128.0);
        assert_eq!(state["columns"]["2002"]["name"], "Chorus");

        // Clip 1/2 starts: it becomes the layer's active clip; clip 1/1 stops.
        let mut cx = Cx::new(6);
        m.ws(&mut cx, WS, update(CLIP_11 * 10 + 2, json!("Disconnected")));
        m.ws(&mut cx, WS, update(CLIP_12 * 10 + 2, json!("Connected")));
        state_of(&cx.take(), &mut state);
        assert_eq!(state["clips"]["4012"]["connected"], "Connected");
        assert_eq!(state["layers"]["1001"]["active_clip"], CLIP_12);
        assert_eq!(state["layers"]["1001"]["active_clip_name"], "Fire");

        // Renaming the playing clip renames the layer's active clip.
        let mut cx = Cx::new(7);
        m.ws(&mut cx, WS, update(CLIP_12 * 10 + 1, json!("Fire 2")));
        state_of(&cx.take(), &mut state);
        assert_eq!(state["clips"]["4012"]["name"], "Fire 2");
        assert_eq!(state["layers"]["1001"]["active_clip_name"], "Fire 2");

        // Layer cleared: nothing plays, so the active clip is removed.
        let mut cx = Cx::new(8);
        m.ws(&mut cx, WS, update(CLIP_12 * 10 + 2, json!("Disconnected")));
        state_of(&cx.take(), &mut state);
        assert!(state["layers"]["1001"].get("active_clip").is_none());
        assert!(state["layers"]["1001"].get("active_clip_name").is_none());
    }

    #[test]
    fn a_deck_switch_reads_the_composition_again() {
        let mut state = json!({});
        let (mut m, _) = connected(&mut state);
        let mut cx = Cx::new(10);
        m.ws(
            &mut cx,
            WS,
            WsInput::Text(
                json!({"type": "parameter_update", "path": format!("/parameter/by-id/{}", DECK_2 * 10 + 2),
                       "value": true})
                .to_string(),
            ),
        );
        let actions = cx.take();
        state_of(&actions, &mut state);
        assert_eq!(state["decks"]["3002"]["selected"], true);
        assert_eq!(timer_after(&actions, REFRESH), Some(SOON));

        let mut cx = Cx::new(10 + SOON);
        m.timer(&mut cx, REFRESH);
        let actions = cx.take();
        assert_eq!(
            refresh_requests(&actions),
            ["http://192.0.2.10:8080/api/v1/composition"]
        );
        let id = actions
            .iter()
            .find_map(|a| match a {
                Action::Http { id, .. } if *id >= OWN_IDS => Some(*id),
                _ => None,
            })
            .unwrap();
        // The new deck: layer 2 is gone, a layer 3 is there.
        let mut next = composition(true);
        next["layers"].as_array_mut().unwrap().remove(1);
        next["layergroups"] = json!([]);
        let mut cx = Cx::new(20 + SOON);
        m.http_response(
            &mut cx,
            id,
            Ok(HttpResponse {
                headers: Vec::new(),
                status: 200,
                body: next.to_string().into_bytes(),
            }),
        );
        let actions = cx.take();
        state_of(&actions, &mut state);
        assert!(state["layers"].get("1002").is_none());
        assert!(state["groups"].get("5001").is_none());
        assert_eq!(state["layers"]["1003"]["position"], 2);
        assert!(sent(&actions).iter().any(|s| s["action"] == "unsubscribe"));
        assert_eq!(timer_after(&actions, REFRESH), Some(60_000));
    }

    #[test]
    fn structural_commands_read_the_composition_again_when_they_succeed() {
        let mut state = json!({});
        let (mut m, _) = connected(&mut state);
        let engine_request = |actions: &[Action]| {
            actions
                .iter()
                .find_map(|a| match a {
                    Action::Http { id, .. } if *id < OWN_IDS => Some(*id),
                    _ => None,
                })
                .unwrap()
        };
        let mut cx = Cx::new(10);
        // The engine's probe goes out first; answer it so the command runs.
        m.http_response(
            &mut cx,
            1,
            Ok(HttpResponse {
                headers: Vec::new(),
                status: 200,
                body: br#"{"name":"Arena","major":7,"minor":23,"micro":0,"revision":1}"#.to_vec(),
            }),
        );
        m.command(
            &mut cx,
            7,
            "close_deck",
            &crate::value::from_value(json!({"deck": 2})).unwrap(),
        );
        let request = engine_request(&cx.take());
        let mut cx = Cx::new(11);
        m.http_response(
            &mut cx,
            request,
            Ok(HttpResponse {
                headers: Vec::new(),
                status: 204,
                body: Vec::new(),
            }),
        );
        let actions = cx.take();
        assert!(actions.iter().any(|a| matches!(
            a,
            Action::Complete {
                id: 7,
                result: Ok(_)
            }
        )));
        assert_eq!(timer_after(&actions, REFRESH), Some(SOON));

        // A failed one, or one the article covers, does not.
        let mut cx = Cx::new(12);
        m.command(
            &mut cx,
            8,
            "open_deck",
            &crate::value::from_value(json!({"deck": 9})).unwrap(),
        );
        let request = engine_request(&cx.take());
        let mut cx = Cx::new(13);
        m.http_response(
            &mut cx,
            request,
            Ok(HttpResponse {
                headers: Vec::new(),
                status: 404,
                body: Vec::new(),
            }),
        );
        assert_eq!(timer_after(&cx.take(), REFRESH), None);
        let mut cx = Cx::new(14);
        m.command(&mut cx, 9, "add_layer", &Params::new());
        let request = engine_request(&cx.take());
        let mut cx = Cx::new(15);
        m.http_response(
            &mut cx,
            request,
            Ok(HttpResponse {
                headers: Vec::new(),
                status: 204,
                body: Vec::new(),
            }),
        );
        assert_eq!(timer_after(&cx.take(), REFRESH), None);
        assert!(m.structural.is_empty());
    }

    #[test]
    fn no_composition_on_opening_is_read_over_rest() {
        let mut m = module();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.ws(&mut cx, WS, WsInput::Opened);
        assert_eq!(timer_after(&cx.take(), REFRESH), Some(FIRST_COMPOSITION));
        let mut cx = Cx::new(FIRST_COMPOSITION);
        m.timer(&mut cx, REFRESH);
        assert_eq!(refresh_requests(&cx.take()).len(), 1);
        // Not twice while one is in flight.
        let mut cx = Cx::new(FIRST_COMPOSITION + 1);
        m.timer(&mut cx, REFRESH);
        assert!(refresh_requests(&cx.take()).is_empty());
    }
}
