//! MA Lighting's MIDI Show Control via Ethernet, added to a spec-driven
//! console.
//!
//! Commands are the spec's (`specs/grandma2.yaml`), run by the spec engine
//! over telnet. This adds what the format cannot express: the console's MSC
//! output, which it sends as UDP datagrams when its MSC Out mode is Ethernet
//! (grandMA2 User Manual, "MIDI Show Control (MSC)"):
//!
//! - Each datagram holds an 8-byte identifier `GMA\0MSC\0`, a 4-byte
//!   little-endian length counting the whole message with those 12 bytes, and
//!   the MSC message itself, `F0 7F <device> 02 <format> <command> <data> F7`.
//! - The console sends GO (01), STOP (02), RESUME (03), TIMED_GO (04), SET
//!   (06), FIRE (07) and GO_OFF (0B). Cue data is ASCII digits and a dot,
//!   then, after a 00, the executor and page: `exec.page` in one field with
//!   the Exec.Page setting, or two fields separated by 00 with Exec Page.
//!   SET carries the fader (00 for fader 1), the page, and the position as a
//!   14-bit value, fine byte first. Times are hours, minutes, seconds, frames
//!   (24 per second when the console sends) and fractions.
//!
//! Each message is decoded to one line of text and offered to the spec's
//! telemetry rules, as a reply line would be:
//! `MSC GO device=127 format=7F cue=37.200 list=5.1`, with `list` and `path`
//! the second and third cue fields, `fade=<seconds>` for a time,
//! `fader=<n> page=<n> value=<0-16383>` for SET, `macro=<n>` for FIRE, and
//! `data=<hex>` for a command not listed.

use std::net::SocketAddr;

use serde_json::Value;

use crate::catalog::Params;
use crate::engine::SpecEngine;
use crate::module::{
    Bind, CommandId, Cx, HttpResponse, Key, Level, Module, OpenContext, RequestId, SettingsUpdate,
    SseInput, TcpInput, WsInput,
};

const MSC: Key = "ma-msc";
const IDENTIFIER: &[u8; 8] = b"GMA\0MSC\0";
const HEADER: usize = 12;

pub(crate) struct MaMsc {
    engine: SpecEngine,
    port: Option<u16>,
}

/// The MSC port setting: a port, or none (nothing listened on).
fn port_of(settings: &Params) -> Option<u16> {
    settings
        .get("msc_port")
        .and_then(Value::as_u64)
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| *p != 0)
}

/// The MSC messages of one datagram, each decoded to a line of text.
/// Anything that is not MA's framing of an MSC message is skipped.
pub(crate) fn messages(datagram: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = datagram;
    while rest.len() > HEADER && rest.starts_with(IDENTIFIER) {
        let length = u32::from_le_bytes([rest[8], rest[9], rest[10], rest[11]]) as usize;
        // A length that does not fit what arrived: the rest is the message.
        let end = if length > HEADER && length <= rest.len() {
            length
        } else {
            rest.len()
        };
        if let Some(text) = decode(&rest[HEADER..end]) {
            out.push(text);
        }
        rest = &rest[end..];
    }
    out
}

fn command_name(command: u8) -> Option<&'static str> {
    Some(match command {
        0x01 => "GO",
        0x02 => "STOP",
        0x03 => "RESUME",
        0x04 => "TIMED_GO",
        0x05 => "LOAD",
        0x06 => "SET",
        0x07 => "FIRE",
        0x08 => "ALL_OFF",
        0x09 => "RESTORE",
        0x0a => "RESET",
        0x0b => "GO_OFF",
        _ => return None,
    })
}

/// One MSC message, `F0 7F <device> 02 <format> <command> <data> [F7]`.
fn decode(message: &[u8]) -> Option<String> {
    let message = message.strip_suffix(&[0xf7]).unwrap_or(message);
    let [0xf0, 0x7f, device, 0x02, format, command, data @ ..] = message else {
        return None;
    };
    let mut text = match command_name(*command) {
        Some(name) => format!("MSC {name} device={device} format={format:02X}"),
        None => format!("MSC CMD_{command:02X} device={device} format={format:02X}"),
    };
    match command {
        0x01 | 0x02 | 0x03 | 0x05 | 0x0b => text.push_str(&cue_fields(data)?),
        0x04 => {
            let (time, cue) = data.split_at_checked(5)?;
            text.push_str(&format!(" fade={}", seconds(time)));
            text.push_str(&cue_fields(cue)?);
        }
        0x06 => {
            let [fader, page, fine, coarse, time @ ..] = data else {
                return None;
            };
            let value = u16::from(*fine & 0x7f) | (u16::from(*coarse & 0x7f) << 7);
            text.push_str(&format!(
                " fader={} page={} value={value}",
                u16::from(*fader) + 1,
                page
            ));
            if time.len() >= 5 {
                text.push_str(&format!(" fade={}", seconds(&time[..5])));
            }
        }
        0x07 => {
            let [number, ..] = data else {
                return None;
            };
            text.push_str(&format!(" macro={number}"));
        }
        _ if data.is_empty() => {}
        _ => {
            let hex: String = data.iter().map(|b| format!("{b:02x}")).collect();
            text.push_str(&format!(" data={hex}"));
        }
    }
    Some(text)
}

/// The cue, list and path fields, separated by 00, each ASCII digits and a
/// dot; an empty field is left out. Anything else is not cue data.
fn cue_fields(data: &[u8]) -> Option<String> {
    let mut text = String::new();
    for (name, field) in ["cue", "list", "path"].iter().zip(data.split(|b| *b == 0)) {
        if field.is_empty() {
            continue;
        }
        if !field.iter().all(|b| b.is_ascii_digit() || *b == b'.') {
            return None;
        }
        text.push_str(&format!(" {name}={}", String::from_utf8_lossy(field)));
    }
    Some(text)
}

/// A time (hours, minutes, seconds, frames, fractions) as seconds, two
/// decimals: frames at 24 per second, as the console sends them, and the
/// hours byte's time-type bits dropped.
fn seconds(time: &[u8]) -> String {
    let whole = u32::from(time[0] & 0x1f) * 3600 + u32::from(time[1]) * 60 + u32::from(time[2]);
    let frames = f64::from(time[3] & 0x1f) + f64::from(time[4].min(99)) / 100.0;
    format!("{:.2}", f64::from(whole) + frames / 24.0)
}

impl MaMsc {
    pub(crate) fn new(engine: SpecEngine, ctx: &OpenContext) -> MaMsc {
        MaMsc {
            engine,
            port: port_of(&ctx.settings),
        }
    }
}

impl Module for MaMsc {
    fn start(&mut self, cx: &mut Cx) {
        self.engine.start(cx);
        if let Some(port) = self.port {
            // Shared: each datagram goes to the console it came from.
            cx.udp_open(MSC, Bind::Shared(port));
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        self.engine.command(cx, id, name, params);
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        if socket != MSC {
            return self.engine.datagram(cx, socket, from, data);
        }
        let texts = messages(data);
        if texts.is_empty() {
            return;
        }
        cx.alive();
        for text in texts {
            self.engine.apply_text(cx, &text);
        }
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        if socket == MSC {
            cx.log(
                Level::Warning,
                format!("MSC via Ethernet unavailable: {message}"),
            );
            return;
        }
        self.engine.socket_error(cx, socket, message);
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        self.engine.tcp(cx, socket, input);
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        self.engine.http_response(cx, id, result);
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        self.engine.sse(cx, stream, input);
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        self.engine.ws(cx, socket, input);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        self.engine.timer(cx, key);
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.port.is_some() {
            cx.udp_close(MSC);
        }
        self.engine.stop(cx);
    }

    fn update_settings(&mut self, cx: &mut Cx, settings: &Params) -> SettingsUpdate {
        // A new MSC port is listened on by a module built afresh.
        if port_of(settings) != self.port {
            return SettingsUpdate::Restart;
        }
        self.engine.update_settings(cx, settings)
    }

    fn reconnect(&mut self, cx: &mut Cx) -> bool {
        self.engine.reconnect(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::catalog::Catalog;
    use crate::module::Action;

    fn framed(msc: &[u8]) -> Vec<u8> {
        let mut out = IDENTIFIER.to_vec();
        out.extend_from_slice(&((msc.len() + HEADER) as u32).to_le_bytes());
        out.extend_from_slice(msc);
        out
    }

    /// The manual's examples, decoded.
    #[test]
    fn the_manuals_examples() {
        // "Go to cue 35", given whole with its header.
        let go = [
            0x47, 0x4d, 0x41, 0x00, 0x4d, 0x53, 0x43, 0x00, 0x18, 0x00, 0x00, 0x00, 0xf0, 0x7f,
            0x7f, 0x02, 0x7f, 0x01, 0x33, 0x35, 0x2e, 0x30, 0x30, 0x30, 0xf7,
        ];
        assert_eq!(messages(&go), ["MSC GO device=127 format=7F cue=35.000"]);
        // "Fire macro 1".
        let fire = [
            0x47, 0x4d, 0x41, 0x00, 0x4d, 0x53, 0x43, 0x00, 0x13, 0x00, 0x00, 0x00, 0xf0, 0x7f,
            0x7f, 0x02, 0x7f, 0x07, 0x01, 0xf7,
        ];
        assert_eq!(messages(&fire), ["MSC FIRE device=127 format=7F macro=1"]);
        // Cue 37.2 on executor 5, page 1, with Exec.Page.
        let exec_page = framed(&[
            0xf0, 0x7f, 0x01, 0x02, 0x01, 0x01, 0x33, 0x37, 0x2e, 0x32, 0x30, 0x30, 0x00, 0x35,
            0x2e, 0x31, 0xf7,
        ]);
        assert_eq!(
            messages(&exec_page),
            ["MSC GO device=1 format=01 cue=37.200 list=5.1"]
        );
        // The same with Exec Page: executor and page in fields of their own.
        let exec_space_page = framed(&[
            0xf0, 0x7f, 0x01, 0x02, 0x02, 0x01, 0x33, 0x37, 0x2e, 0x32, 0x30, 0x30, 0x00, 0x35,
            0x00, 0x31, 0xf7,
        ]);
        assert_eq!(
            messages(&exec_space_page),
            ["MSC GO device=1 format=02 cue=37.200 list=5 path=1"]
        );
        // Stop (pause) executor 5 on page 1: cue 0.000.
        let stop = framed(&[
            0xf0, 0x7f, 0x7f, 0x02, 0x7f, 0x02, 0x30, 0x2e, 0x30, 0x30, 0x30, 0x00, 0x35, 0x2e,
            0x31, 0xf7,
        ]);
        assert_eq!(
            messages(&stop),
            ["MSC STOP device=127 format=7F cue=0.000 list=5.1"]
        );
        // Resume with Default Only: no data.
        let resume = framed(&[0xf0, 0x7f, 0x7f, 0x02, 0x7f, 0x03, 0xf7]);
        assert_eq!(messages(&resume), ["MSC RESUME device=127 format=7F"]);
        // Cue 75 in 20 s.
        let timed = framed(&[
            0xf0, 0x7f, 0x7f, 0x02, 0x7f, 0x04, 0x00, 0x00, 0x14, 0x00, 0x00, 0x37, 0x35, 0x2e,
            0x30, 0x30, 0x30, 0xf7,
        ]);
        assert_eq!(
            messages(&timed),
            ["MSC TIMED_GO device=127 format=7F fade=20.00 cue=75.000"]
        );
        // Fader 3 on page 2 to 45 %.
        let set = framed(&[
            0xf0, 0x7f, 0x7f, 0x02, 0x7f, 0x06, 0x02, 0x02, 0x4c, 0x39, 0xf7,
        ]);
        assert_eq!(
            messages(&set),
            ["MSC SET device=127 format=7F fader=3 page=2 value=7372"]
        );
        // Fader 15 on page 1 to 100 % in 5 s.
        let set_timed = framed(&[
            0xf0, 0x7f, 0x7f, 0x02, 0x7f, 0x06, 0x0e, 0x01, 0x7f, 0x7f, 0x00, 0x00, 0x05, 0x00,
            0x00, 0xf7,
        ]);
        assert_eq!(
            messages(&set_timed),
            ["MSC SET device=127 format=7F fader=15 page=1 value=16383 fade=5.00"]
        );
        // Off: executor 9 on page 5.
        let off = framed(&[
            0xf0, 0x7f, 0x7f, 0x02, 0x7f, 0x0b, 0x30, 0x2e, 0x30, 0x30, 0x30, 0x00, 0x39, 0x2e,
            0x35, 0xf7,
        ]);
        assert_eq!(
            messages(&off),
            ["MSC GO_OFF device=127 format=7F cue=0.000 list=9.5"]
        );
    }

    #[test]
    fn other_datagrams_are_skipped() {
        assert!(messages(b"").is_empty());
        assert!(messages(b"GMA\0MSC\0").is_empty());
        assert!(messages(b"Art-Net\0\0\0\0\0\0").is_empty());
        // Not an MSC message inside the framing.
        assert!(messages(&framed(&[0xf0, 0x7e, 0x7f, 0x06, 0x01, 0xf7])).is_empty());
        // Cue data that is not digits.
        assert!(messages(&framed(&[0xf0, 0x7f, 0x7f, 0x02, 0x7f, 0x01, 0x41, 0xf7])).is_empty());
        // A command MA does not list, kept as hex; two messages in one datagram.
        let mut two = framed(&[0xf0, 0x7f, 0x7f, 0x02, 0x7f, 0x15, 0x31, 0xf7]);
        two.extend(framed(&[0xf0, 0x7f, 0x7f, 0x02, 0x7f, 0x08, 0xf7]));
        assert_eq!(
            messages(&two),
            [
                "MSC CMD_15 device=127 format=7F data=31",
                "MSC ALL_OFF device=127 format=7F"
            ]
        );
    }

    fn module(settings: Value) -> MaMsc {
        let catalog = Catalog::embedded();
        let spec = catalog.device("grandma2").unwrap().clone();
        let ctx = OpenContext {
            host: "192.0.2.20".parse().unwrap(),
            host_name: None,
            port: None,
            model: "grandma2".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor: true,
        };
        let engine = SpecEngine::new(Arc::new(spec), ctx.clone()).unwrap();
        MaMsc::new(engine, &ctx)
    }

    fn patches(actions: &[Action]) -> Vec<Value> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::State(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    /// The port is listened on, shared, and a GO from the console becomes
    /// the executor's state through the spec's rules.
    #[test]
    fn a_go_reaches_state() {
        let mut m = module(json!({"msc_port": 6004}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        let started = cx.take();
        assert!(started.iter().any(|a| matches!(
            a,
            Action::UdpOpen { socket, bind: Bind::Shared(6004) } if *socket == MSC
        )));

        let mut cx = Cx::new(10);
        let go = framed(&[
            0xf0, 0x7f, 0x01, 0x02, 0x01, 0x01, 0x33, 0x37, 0x2e, 0x32, 0x30, 0x30, 0x00, 0x35,
            0x2e, 0x31, 0xf7,
        ]);
        m.datagram(&mut cx, MSC, "192.0.2.20:6004".parse().unwrap(), &go);
        let mut state = json!({});
        for p in patches(&cx.take()) {
            crate::session::merge_patch(&mut state, &p);
        }
        assert_eq!(
            state["pages"]["1"]["executors"]["5"],
            json!({"cue": "37.200", "status": "running"})
        );

        // Settings with another port rebuild the module.
        let mut cx = Cx::new(20);
        let other = json!({"msc_port": 6010});
        assert_eq!(
            m.update_settings(&mut cx, other.as_object().unwrap()),
            SettingsUpdate::Restart
        );
    }

    #[test]
    fn no_port_no_listener() {
        let mut m = module(json!({"msc_port": null}));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(!cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::UdpOpen { socket, .. } if *socket == MSC)));
    }
}
