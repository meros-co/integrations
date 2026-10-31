//! Soundcraft Ui meters and this client's own changes, added to the
//! spec-driven mixer.
//!
//! Commands, the parameter feed and state are the spec's
//! (`specs/soundcraft-ui.yaml`), run by the spec engine over the mixer's
//! WebSocket. Two things the format cannot express are done here:
//!
//! - Meters arrive as `VU2^<base64>`: seven counts (inputs, players,
//!   subgroups, FX, auxes, masters, lines) from byte 0, then from byte 8 a
//!   block per strip of 6, 6, 7, 7, 5, 5 and 6 bytes, each level a byte
//!   times 0.004167508166392142 on a 0 to 1 scale that maps linearly to -80
//!   to 0 dB (the soundcraft-ui library's vu.utils.ts, MIT). With the
//!   `meters` setting on they are kept as `meters.<strips>.<n>.<meter>`; a
//!   message holding meters alone never reaches the rules.
//! - The mixer does not send a client's own SETD and SETS changes back to it
//!   (the library keeps them itself), so each one this client sends is also
//!   offered to the spec's rules, as the mixer would send it.

use crate::json;
use crate::value::{Map, Value};

use crate::catalog::Params;
use crate::engine::SpecEngine;
use crate::module::{
    Action, CommandId, Cx, FileInput, HttpResponse, Key, Module, OpenContext, RequestId, SseInput,
    TcpInput, WsInput,
};

/// The engine's WebSocket to the mixer.
const DEVICE: Key = "device";
/// Every message on the wire carries this prefix.
const PREFIX: &str = "3:::";
/// A meter byte to the 0 to 1 scale (vu.utils.ts).
const METER_FACTOR: f64 = 0.004167508166392142;

/// The kinds of strip a VU2 message holds, in its order: the state key, the
/// bytes each strip's block takes, and the meters at the block's start.
struct Section {
    key: &'static str,
    block: usize,
    meters: &'static [&'static str],
}

const CHANNEL: &[&str] = &["pre", "post", "post_fader"];
const STEREO: &[&str] = &[
    "post_left",
    "post_right",
    "post_fader_left",
    "post_fader_right",
];
const MONO_BUS: &[&str] = &["post", "post_fader"];

const SECTIONS: [Section; 7] = [
    Section {
        key: "inputs",
        block: 6,
        meters: CHANNEL,
    },
    Section {
        key: "players",
        block: 6,
        meters: CHANNEL,
    },
    Section {
        key: "subgroups",
        block: 7,
        meters: STEREO,
    },
    Section {
        key: "fx_returns",
        block: 7,
        meters: STEREO,
    },
    Section {
        key: "auxes",
        block: 5,
        meters: MONO_BUS,
    },
    // The master is two mono blocks, left and right (handled below).
    Section {
        key: "master",
        block: 5,
        meters: MONO_BUS,
    },
    Section {
        key: "lines",
        block: 6,
        meters: CHANNEL,
    },
];

/// The dB value of a meter byte: the library's 0 to 1 scale mapped
/// linearly to -80 to 0 dB, to a tenth of a dB.
fn meter_db(byte: u8) -> f64 {
    let db = f64::from(byte) * METER_FACTOR * 80.0 - 80.0;
    (db * 10.0).round() / 10.0
}

/// One VU2 message's base64 payload as a state patch under `meters`. A
/// payload that does not decode, or is shorter than its counts say, gives
/// what it holds.
fn meters(payload: &str) -> Option<Value> {
    let bytes = crate::catalog::decode_binary(payload.trim_end())?;
    if bytes.len() < 8 {
        return None;
    }
    let mut at = 8;
    let mut patch = Map::new();
    'all: for (i, s) in SECTIONS.iter().enumerate() {
        let count = usize::from(bytes[i]);
        let mut strips = Map::new();
        for n in 1..=count {
            let Some(block) = bytes.get(at..at + s.block) else {
                if !strips.is_empty() {
                    patch.insert(s.key.to_string(), Value::Object(strips));
                }
                break 'all;
            };
            let mut values = Map::new();
            for (j, name) in s.meters.iter().enumerate() {
                values.insert(name.to_string(), json!(meter_db(block[j])));
            }
            strips.insert(n.to_string(), Value::Object(values));
            at += s.block;
        }
        if s.key == "master" {
            // Master 1 is the left side, master 2 the right.
            let side = |n: &str, meter: &str| {
                strips.get(n).and_then(|m| m.get(meter)).cloned().map(|v| {
                    (
                        format!("{meter}_{}", if n == "1" { "left" } else { "right" }),
                        v,
                    )
                })
            };
            let mut master = Map::new();
            for (n, meter) in [
                ("1", "post"),
                ("2", "post"),
                ("1", "post_fader"),
                ("2", "post_fader"),
            ] {
                if let Some((k, v)) = side(n, meter) {
                    master.insert(k, v);
                }
            }
            if !master.is_empty() {
                patch.insert("master".to_string(), Value::Object(master));
            }
        } else if !strips.is_empty() {
            patch.insert(s.key.to_string(), Value::Object(strips));
        }
    }
    (!patch.is_empty()).then(|| json!({ "meters": Value::Object(patch) }))
}

/// The lines of a message as the mixer sends it, without the prefix, or
/// nothing for a message without it.
fn lines(text: &str) -> Option<std::str::Lines<'_>> {
    text.strip_prefix(PREFIX).map(str::lines)
}

/// Whether a message is meters and nothing else.
fn meters_only(text: &str) -> bool {
    lines(text).is_some_and(|mut l| l.all(|line| line.starts_with("VU2^")))
}

/// Whether a message this client sends changes a parameter.
fn is_change(text: &str) -> bool {
    text.strip_prefix(PREFIX)
        .is_some_and(|t| t.starts_with("SETD^") || t.starts_with("SETS^"))
}

pub(crate) struct SoundcraftUi {
    engine: SpecEngine,
    meters: bool,
}

impl SoundcraftUi {
    pub(crate) fn new(engine: SpecEngine, ctx: &OpenContext) -> SoundcraftUi {
        SoundcraftUi {
            engine,
            meters: ctx.settings.get("meters").and_then(Value::as_bool) == Some(true),
        }
    }

    /// Run an engine callback, then offer each change it sent to the mixer
    /// to the rules, as the mixer would have sent it.
    fn relay(&mut self, cx: &mut Cx, f: impl FnOnce(&mut SpecEngine, &mut Cx)) {
        let mut sent = Vec::new();
        let mut inner = Cx::at(cx.now(), cx.unix_millis());
        f(&mut self.engine, &mut inner);
        for action in inner.take() {
            if let Action::WsSend { socket, text } = &action {
                if *socket == DEVICE && is_change(text) {
                    sent.push(text.clone());
                }
            }
            cx.push(action);
        }
        // A change is never answered, and the rules on a change send
        // nothing, so this does not recurse.
        for text in sent {
            self.engine.ws(cx, DEVICE, WsInput::Text(text));
        }
    }
}

impl Module for SoundcraftUi {
    fn start(&mut self, cx: &mut Cx) {
        self.relay(cx, |e, cx| e.start(cx));
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
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
        if socket == DEVICE {
            if let WsInput::Text(text) = &input {
                if self.meters {
                    for line in lines(text).into_iter().flatten() {
                        if let Some(patch) = line.strip_prefix("VU2^").and_then(meters) {
                            cx.state(patch);
                        }
                    }
                }
                if meters_only(text) {
                    cx.alive();
                    return;
                }
            }
        }
        self.relay(cx, |e, cx| e.ws(cx, socket, input));
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        self.relay(cx, |e, cx| e.http_response(cx, id, result));
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        self.relay(cx, |e, cx| e.sse(cx, stream, input));
    }

    fn file(&mut self, cx: &mut Cx, file: Key, input: FileInput) {
        self.relay(cx, |e, cx| e.file(cx, file, input));
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        self.relay(cx, |e, cx| e.timer(cx, key));
    }

    fn stream_watch(&mut self, cx: &mut Cx, stream: &str, watching: bool) {
        self.relay(cx, |e, cx| e.stream_watch(cx, stream, watching));
    }

    fn stop(&mut self, cx: &mut Cx) {
        self.relay(cx, |e, cx| e.stop(cx));
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;

    use super::*;

    fn b64(bytes: &[u8]) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    /// A VU2 payload: one input, no players, one subgroup, no FX, one aux,
    /// two masters, no lines.
    fn payload() -> Vec<u8> {
        let mut p = vec![1, 0, 1, 0, 1, 2, 0, 0];
        p.extend([0, 120, 240, 0, 0, 0]); // input 1: pre, post, post-fader
        p.extend([60, 61, 62, 63, 0, 0, 0]); // subgroup 1
        p.extend([200, 100, 0, 0, 0]); // aux 1
        p.extend([10, 20, 0, 0, 0]); // master left
        p.extend([30, 40, 0, 0, 0]); // master right
        p
    }

    #[test]
    fn a_vu2_message_is_read_into_each_strips_meters() {
        let patch = meters(&b64(&payload())).unwrap();
        let m = &patch["meters"];
        assert_eq!(m["inputs"]["1"]["pre"], json!(-80.0));
        // 120 × 0.0041675 × 80 - 80 = -40.0
        assert_eq!(m["inputs"]["1"]["post"], json!(-40.0));
        assert_eq!(m["inputs"]["1"]["post_fader"], json!(0.0));
        assert_eq!(m["subgroups"]["1"]["post_fader_right"], json!(meter_db(63)));
        assert_eq!(m["auxes"]["1"]["post"], json!(meter_db(200)));
        assert_eq!(
            m["master"],
            json!({
                "post_left": meter_db(10), "post_right": meter_db(30),
                "post_fader_left": meter_db(20), "post_fader_right": meter_db(40)
            })
        );
        assert!(m.get("players").is_none());
        // A payload cut short gives what it holds.
        let short = meters(&b64(&payload()[..8 + 6 + 3])).unwrap();
        assert_eq!(short["meters"].as_object().unwrap().len(), 1);
        assert!(meters("not base64!").is_none());
    }

    #[test]
    fn messages_are_told_apart() {
        assert!(meters_only("3:::VU2^AAAA"));
        assert!(!meters_only("3:::VU2^AAAA\nSETD^i.0.mix^0.5"));
        assert!(!meters_only("VU2^AAAA"));
        assert!(is_change("3:::SETD^i.0.mix^0.5000"));
        assert!(is_change("3:::SETS^i.0.name^KICK"));
        assert!(!is_change("3:::ALIVE"));
        assert!(!is_change("3:::LOADSHOW^Main"));
    }

    fn ui(settings: Value) -> SoundcraftUi {
        let spec = crate::catalog::Catalog::source_tree()
            .device("soundcraft-ui")
            .unwrap()
            .clone();
        let ctx = OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model: "ui24r".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor: true,
        };
        let engine = SpecEngine::new(Arc::new(spec), ctx.clone()).unwrap();
        SoundcraftUi::new(engine, &ctx)
    }

    fn states(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::State(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn meters_are_kept_only_when_turned_on_and_never_reach_the_rules() {
        let message = format!("3:::VU2^{}", b64(&payload()));
        let mut off = ui(json!({}));
        let mut cx = Cx::new(0);
        off.start(&mut cx);
        off.ws(&mut cx, DEVICE, WsInput::Opened);
        let mut cx = Cx::new(10);
        off.ws(&mut cx, DEVICE, WsInput::Text(message.clone()));
        let a = cx.take();
        assert!(states(&a).is_empty());
        assert!(a.iter().any(|a| matches!(a, Action::Alive)));

        let mut on = ui(json!({"meters": true}));
        let mut cx = Cx::new(0);
        on.start(&mut cx);
        on.ws(&mut cx, DEVICE, WsInput::Opened);
        let mut cx = Cx::new(10);
        on.ws(&mut cx, DEVICE, WsInput::Text(message));
        let s = states(&cx.take());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0]["meters"]["inputs"]["1"]["post"], json!(-40.0));
    }

    #[test]
    fn this_clients_own_changes_reach_state() {
        let mut m = ui(json!({}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.ws(&mut cx, DEVICE, WsInput::Opened);
        // The list reads on connecting are answered (empty lists).
        m.ws(&mut cx, DEVICE, WsInput::Text("3:::SHOWLIST".into()));
        m.ws(&mut cx, DEVICE, WsInput::Text("3:::PLISTS".into()));
        let mut cx = Cx::new(10);
        let params: Params = json!({"strip": "input", "channel": 3, "muted": true})
            .as_object()
            .unwrap()
            .clone();
        m.command(&mut cx, 1, "set_mute", &params);
        let a = cx.take();
        assert!(a.iter().any(|a| matches!(a,
            Action::WsSend { socket: DEVICE, text } if text == "3:::SETD^i.2.mute^1")));
        let merged = states(&a).into_iter().fold(json!({}), |mut acc, p| {
            crate::session::merge_patch(&mut acc, &p);
            acc
        });
        assert_eq!(merged["inputs"]["3"]["mute"], json!(true));
        assert_eq!(merged["values"]["i.2.mute"], json!(1.0));
    }
}
