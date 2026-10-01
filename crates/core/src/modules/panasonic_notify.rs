//! Panasonic AW-series update notifications, added to the spec-driven camera.
//!
//! Commands and queries are the spec's (`specs/panasonic-ptz.yaml`), run by the
//! spec engine. This adds what the format cannot express: the camera's own
//! notification channel (Panasonic HD/4K Integrated Camera Interface
//! Specifications, chapter 5).
//!
//! - The client registers a local TCP port with
//!   `GET /cgi-bin/event?connect=start&my_port=<port>&uid=0` (204 No Content)
//!   and unregisters with `connect=stop` on closing.
//! - The camera connects to that port and sends each changed setting, wrapped
//!   in a binary envelope, as `CR LF <response text> CR LF`: the same text a
//!   query returns, such as `p1` or `DCB:1`. Each one goes through the spec's
//!   telemetry rules, exactly as a query reply does.
//! - Version information arrives every 60 seconds, so a channel silent for
//!   longer than that is registered again (the document asks for this after
//!   a loss of communication).

use std::net::IpAddr;

use crate::catalog::Params;
use crate::engine::SpecEngine;
use crate::module::{
    Cx, HttpRequest, HttpResponse, Key, Level, Millis, Module, OpenContext, RequestId, SseInput,
    TcpInput, WsInput,
};

const NOTIFY: Key = "notify";
const REREGISTER: Key = "notify-reregister";
/// Version information comes every 60 s; allow for one missed.
const SILENT_AFTER: Millis = 150_000;
const REGISTER_RETRY: Millis = 10_000;
/// Request ids at and above this are this module's; the engine's count up
/// from 1 and never reach it.
const OWN_IDS: RequestId = 1 << 48;
const DEFAULT_NOTIFY_PORT: u16 = 31004;

pub(crate) struct PanasonicNotify {
    engine: SpecEngine,
    camera: String,
    port: u16,
    next_id: RequestId,
    buffer: Vec<u8>,
}

/// `CR LF text CR LF` runs of printable ASCII in the stream; the envelope's
/// binary fields around them are skipped.
fn notifications(buffer: &mut Vec<u8>) -> Vec<String> {
    let mut out = Vec::new();
    // Everything before `keep` has been read and can never start a match.
    let mut keep = 0;
    let mut i = 0;
    loop {
        let Some(at) = buffer[i..].windows(2).position(|w| w == b"\r\n") else {
            // A trailing CR may be the start of the next CR LF.
            keep = buffer.len().saturating_sub(1).max(keep);
            break;
        };
        let start = i + at + 2;
        let end = buffer[start..]
            .iter()
            .position(|b| !(0x20..=0x7e).contains(b))
            .map(|n| start + n);
        match end {
            // Text running to the end of what has arrived: wait for more.
            None => {
                keep = i + at;
                break;
            }
            Some(end) if end + 1 >= buffer.len() => {
                keep = i + at;
                break;
            }
            Some(end) if end > start && buffer[end] == b'\r' && buffer[end + 1] == b'\n' => {
                out.push(String::from_utf8_lossy(&buffer[start..end]).into_owned());
                i = end + 2;
                keep = i;
            }
            // Not a notification: carry on from the next byte.
            Some(_) => {
                i = i + at + 1;
                keep = i;
            }
        }
    }
    buffer.drain(..keep.min(buffer.len()));
    // Never keep an unbounded tail of envelope bytes.
    if buffer.len() > 2048 {
        let keep = buffer.split_off(buffer.len() - 512);
        *buffer = keep;
    }
    out
}

impl PanasonicNotify {
    pub(crate) fn new(engine: SpecEngine, ctx: &OpenContext) -> PanasonicNotify {
        let host = match ctx.host {
            IpAddr::V6(v6) => format!("[{v6}]"),
            v4 => v4.to_string(),
        };
        let port = ctx
            .settings
            .get("notify_port")
            .and_then(serde_json::Value::as_u64)
            .map(|p| p as u16)
            .unwrap_or(DEFAULT_NOTIFY_PORT);
        PanasonicNotify {
            engine,
            camera: format!("http://{host}:{}", ctx.port.unwrap_or(80)),
            port,
            next_id: OWN_IDS,
            buffer: Vec::new(),
        }
    }

    fn event(&mut self, cx: &mut Cx, connect: &str) {
        let id = self.next_id;
        self.next_id += 1;
        cx.http(
            id,
            HttpRequest {
                method: "GET",
                url: format!(
                    "{}/cgi-bin/event?connect={connect}&my_port={}&uid=0",
                    self.camera, self.port
                ),
                headers: Vec::new(),
                body: None,
                timeout: Some(4_000),
                accept_invalid_certs: false,
                digest: None,
            },
        );
    }

    fn register(&mut self, cx: &mut Cx) {
        self.event(cx, "start");
        cx.set_timer(REREGISTER, SILENT_AFTER);
    }
}

impl Module for PanasonicNotify {
    fn start(&mut self, cx: &mut Cx) {
        self.engine.start(cx);
        cx.tcp_listen(NOTIFY, self.port);
        self.register(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: crate::module::CommandId, name: &str, params: &Params) {
        self.engine.command(cx, id, name, params);
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: std::net::SocketAddr, data: &[u8]) {
        self.engine.datagram(cx, socket, from, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        if socket == NOTIFY {
            cx.log(
                Level::Warning,
                format!("update notifications unavailable: {message}"),
            );
            return;
        }
        self.engine.socket_error(cx, socket, message);
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        if socket != NOTIFY {
            return self.engine.tcp(cx, socket, input);
        }
        match input {
            TcpInput::Connected => self.buffer.clear(),
            TcpInput::Data(data) => {
                cx.alive();
                cx.set_timer(REREGISTER, SILENT_AFTER);
                self.buffer.extend_from_slice(&data);
                for text in notifications(&mut self.buffer) {
                    self.engine.apply_text(cx, &text);
                }
            }
            // The camera opens a connection when it has something to send.
            TcpInput::Closed { .. } => {}
        }
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        if id < OWN_IDS {
            return self.engine.http_response(cx, id, result);
        }
        match result {
            Ok(r) if (200..300).contains(&r.status) => {}
            Ok(r) => {
                cx.log(
                    Level::Warning,
                    format!(
                        "the camera refused update notifications (HTTP {})",
                        r.status
                    ),
                );
                cx.set_timer(REREGISTER, REGISTER_RETRY);
            }
            Err(e) => {
                cx.log(
                    Level::Debug,
                    format!("update notification registration: {e}"),
                );
                cx.set_timer(REREGISTER, REGISTER_RETRY);
            }
        }
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        self.engine.sse(cx, stream, input);
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        self.engine.ws(cx, socket, input);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if key == REREGISTER {
            self.register(cx);
        } else {
            self.engine.timer(cx, key);
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        // "The update notification receive end step must be taken without fail."
        self.event(cx, "stop");
        self.engine.stop(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifications_are_found_inside_the_envelope_across_reads() {
        let mut envelope = vec![0u8; 22];
        envelope.extend_from_slice(&[0x00, 0x0e]);
        envelope.extend_from_slice(&[0u8; 4]);
        envelope.extend_from_slice(b"\r\np1\r\n");
        envelope.extend_from_slice(&[0u8; 24]);
        let mut second = vec![0u8; 28];
        second.extend_from_slice(b"\r\nDCB:1\r\n");
        second.extend_from_slice(&[0u8; 24]);

        let stream = [envelope, second].concat();
        let mut buffer = Vec::new();
        let mut got = Vec::new();
        for chunk in stream.chunks(5) {
            buffer.extend_from_slice(chunk);
            got.extend(notifications(&mut buffer));
        }
        assert_eq!(got, ["p1", "DCB:1"]);
    }
}
