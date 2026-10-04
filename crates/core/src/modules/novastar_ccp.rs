//! NovaStar central control protocol: COEX controllers over TCP 5200.
//!
//! Protocol from NovaStar's "COEX Central Control Protocol Instructions"
//! (V1.5.0, for the MX40 Pro, MX30, MX20, KU20, CX40 Pro, MX6000 Pro and
//! MX2000 Pro). Section numbers below are that document's.
//!
//! - Every request is one binary frame (§3.1): the header `55 aa`, then ACK,
//!   label, source and destination address, device type, Ethernet port,
//!   broadcast address, code and packet type (the document gives them as the
//!   fixed bytes `00 00 fe ff 01 ff ff ff 01 00` for a write), the register
//!   address and the data length (both little-endian), the data, and a
//!   two-byte little-endian checksum: the sum of every byte after the header,
//!   plus 0x5555. A value in the data changes the checksum, which is why this
//!   protocol is a module rather than a spec.
//! - The controller answers each request with a frame headed `aa 55` that
//!   repeats the register address with no data (§3.1 D). The module sends one
//!   request at a time and matches the answer by its register address; an
//!   ACK byte other than 0 is reported as a device error.
//! - The protocol has no read in the public document, so the module keeps
//!   no state of its own polling: what the controller acknowledged is put in
//!   the state, as it now holds. The TCP connection is the liveness signal.
//!
//! Opened for commands only (`monitor` false), the module behaves the same:
//! it subscribes to and polls nothing in either case. Each answered request
//! reports its round trip.

use std::collections::VecDeque;
use std::net::SocketAddr;

use serde_json::{json, Value};

use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext, Outcome,
    TcpInput,
};

pub(crate) const DEFAULT_PORT: u16 = 5200;
const SOCKET: Key = "novastar-ccp";
const REPLY: Key = "reply";
const RETRY: Key = "retry";
const REPLY_TIMEOUT: Millis = 3_000;
const RETRY_MIN: Millis = 1_000;
const RETRY_MAX: Millis = 30_000;

/// The bytes between the header and the register address of a write (§3.1 B).
const FIXED: [u8; 10] = [0x00, 0x00, 0xfe, 0xff, 0x01, 0xff, 0xff, 0xff, 0x01, 0x00];

const BRIGHTNESS: u32 = 0x0200_0001;
const RECEIVING_BLACK: u32 = 0x0200_0100;
const RECEIVING_FREEZE: u32 = 0x0200_0102;
const PRESET: u32 = 0x0a00_0002;
const LAYER_SOURCE: u32 = 0x0a00_0003;
const LOW_LATENCY: u32 = 0x1000_0111;
const THREE_D: u32 = 0x1000_0116;
const THREE_D_EYE: u32 = 0x1000_1118;
const SENDING_DISPLAY: u32 = 0x1000_0100;
const WORKING_MODE: u32 = 0x0008_fff2;

/// A write request frame for `register` carrying `data` (§3.1).
pub(crate) fn frame(register: u32, data: &[u8]) -> Vec<u8> {
    let mut content = FIXED.to_vec();
    content.extend_from_slice(&register.to_le_bytes());
    content.extend_from_slice(&(data.len() as u16).to_le_bytes());
    content.extend_from_slice(data);
    let sum = content
        .iter()
        .fold(0x5555u32, |s, b| s.wrapping_add(*b as u32)) as u16;
    let mut out = vec![0x55, 0xaa];
    out.extend_from_slice(&content);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

/// An answer frame: its ACK byte and register address, once the checksum is
/// right.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Answer {
    ack: u8,
    register: u32,
}

/// Take one answer frame from the front of `buf`, dropping anything before
/// its header. `None` until a whole frame has arrived.
fn take_answer(buf: &mut Vec<u8>) -> Option<Result<Answer, String>> {
    let start = buf.windows(2).position(|w| w == [0xaa, 0x55]);
    match start {
        Some(i) => {
            buf.drain(..i);
        }
        None => {
            // Keep a lone trailing 0xaa: it may start the next header.
            let keep = usize::from(buf.last() == Some(&0xaa));
            buf.drain(..buf.len() - keep);
            return None;
        }
    }
    if buf.len() < 18 {
        return None;
    }
    let len = u16::from_le_bytes([buf[16], buf[17]]) as usize;
    let total = 18 + len + 2;
    if buf.len() < total {
        return None;
    }
    let frame: Vec<u8> = buf.drain(..total).collect();
    // The document's answer example (§3.1 D) sums the header too, unlike a
    // request's checksum; either is accepted.
    let sum = frame[..total - 2]
        .iter()
        .fold(0x5555u32, |s, b| s.wrapping_add(*b as u32)) as u16;
    let without_header = sum.wrapping_sub(0xaa + 0x55);
    let got = u16::from_le_bytes([frame[total - 2], frame[total - 1]]);
    if sum != got && without_header != got {
        return Some(Err(format!(
            "answer checksum {got:04x}, expected {sum:04x}"
        )));
    }
    Some(Ok(Answer {
        ack: frame[2],
        register: u32::from_le_bytes([frame[12], frame[13], frame[14], frame[15]]),
    }))
}

#[derive(Debug, Clone)]
struct Job {
    id: CommandId,
    register: u32,
    frame: Vec<u8>,
    /// Put in the state once the controller acknowledges.
    state: Value,
}

pub(crate) struct NovastarCcp {
    device: SocketAddr,
    connected: bool,
    connecting: bool,
    buf: Vec<u8>,
    queue: VecDeque<Job>,
    in_flight: Option<Job>,
    sent_at: Millis,
    retry_after: Millis,
}

impl NovastarCcp {
    pub(crate) fn new(ctx: OpenContext) -> NovastarCcp {
        // `monitor` changes nothing: the module neither subscribes nor polls.
        NovastarCcp::with(SocketAddr::new(ctx.host, ctx.port.unwrap_or(DEFAULT_PORT)))
    }

    fn with(device: SocketAddr) -> NovastarCcp {
        NovastarCcp {
            device,
            connected: false,
            connecting: false,
            buf: Vec::new(),
            queue: VecDeque::new(),
            in_flight: None,
            sent_at: 0,
            retry_after: RETRY_MIN,
        }
    }

    fn connect(&mut self, cx: &mut Cx) {
        if self.connected || self.connecting {
            return;
        }
        self.connecting = true;
        cx.tcp_open(SOCKET, self.device);
    }

    fn pump(&mut self, cx: &mut Cx) {
        if !self.connected || self.in_flight.is_some() {
            return;
        }
        if let Some(job) = self.queue.pop_front() {
            cx.tcp_send(SOCKET, job.frame.clone());
            self.sent_at = cx.now();
            cx.set_timer(REPLY, REPLY_TIMEOUT);
            self.in_flight = Some(job);
        }
    }

    fn fail_all(&mut self, cx: &mut Cx, reason: &str) {
        let jobs: Vec<Job> = self
            .in_flight
            .take()
            .into_iter()
            .chain(self.queue.drain(..))
            .collect();
        for job in jobs {
            cx.complete(
                job.id,
                Err(CommandError::Transport {
                    message: reason.into(),
                }),
            );
        }
    }

    fn lost(&mut self, cx: &mut Cx, reason: &str) {
        let was = self.connected || self.connecting;
        if self.connected || self.connecting {
            cx.tcp_close(SOCKET);
        }
        self.connected = false;
        self.connecting = false;
        self.buf.clear();
        cx.cancel_timer(REPLY);
        self.fail_all(cx, reason);
        if was {
            cx.connection(Connection::Disconnected {
                reason: reason.into(),
            });
        }
        cx.log(Level::Debug, format!("NovaStar: {reason}"));
        cx.set_timer(RETRY, self.retry_after);
        self.retry_after = (self.retry_after * 2).min(RETRY_MAX);
    }

    fn answer(&mut self, cx: &mut Cx, answer: Answer) {
        if self.in_flight.as_ref().map(|j| j.register) != Some(answer.register) {
            cx.log(
                Level::Debug,
                format!("NovaStar: unexpected answer for {:08x}", answer.register),
            );
            return;
        }
        let Some(job) = self.in_flight.take() else {
            return;
        };
        cx.cancel_timer(REPLY);
        cx.round_trip(cx.now().saturating_sub(self.sent_at));
        if answer.ack == 0 {
            cx.state(job.state.clone());
            cx.complete(job.id, Ok(Outcome::Ack));
        } else {
            cx.complete(
                job.id,
                Err(CommandError::DeviceError {
                    code: Some(answer.ack.to_string()),
                    message: format!("the controller answered ACK {:#04x}", answer.ack),
                }),
            );
        }
    }
}

fn int(p: &Params, name: &str) -> Result<i64, CommandError> {
    p.get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(format!("'{name}' must be an integer")))
}

fn byte(p: &Params, name: &str) -> Result<u8, CommandError> {
    let v = int(p, name)?;
    u8::try_from(v).map_err(|_| invalid(format!("'{name}' must be 0 to 255")))
}

fn flag(p: &Params, name: &str) -> Result<bool, CommandError> {
    p.get(name)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(format!("'{name}' must be true or false")))
}

fn text<'a>(p: &'a Params, name: &str) -> Result<&'a str, CommandError> {
    p.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("'{name}' is required")))
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

/// The register, data and resulting state of a command.
pub(crate) fn request(name: &str, p: &Params) -> Result<(u32, Vec<u8>, Value), CommandError> {
    Ok(match name {
        "set_brightness" => {
            let level = byte(p, "level")?;
            (BRIGHTNESS, vec![level], json!({ "brightness": level }))
        }
        "set_brightness_percent" => {
            let percent = p
                .get("percent")
                .and_then(Value::as_f64)
                .ok_or_else(|| invalid("'percent' must be a number"))?;
            if !(0.0..=100.0).contains(&percent) {
                return Err(invalid("'percent' must be 0 to 100"));
            }
            // Brightness ratio = value / 0xFF (§3.2.1).
            let level = (percent * 255.0 / 100.0).round() as u8;
            (BRIGHTNESS, vec![level], json!({ "brightness": level }))
        }
        "set_blackout" => {
            let on = flag(p, "enabled")?;
            (
                RECEIVING_BLACK,
                vec![if on { 0xff } else { 0x00 }],
                json!({ "receiving_cards": { "blackout": on } }),
            )
        }
        "set_freeze" => {
            let on = flag(p, "enabled")?;
            (
                RECEIVING_FREEZE,
                vec![if on { 0xff } else { 0x00 }],
                json!({ "receiving_cards": { "frozen": on } }),
            )
        }
        "recall_preset" => {
            let preset = byte(p, "preset")?;
            if preset == 0 {
                return Err(invalid("'preset' must be 1 to 255"));
            }
            (PRESET, vec![preset], json!({ "preset": preset }))
        }
        "set_low_latency" => {
            let on = flag(p, "enabled")?;
            (
                LOW_LATENCY,
                vec![u8::from(on)],
                json!({ "low_latency": on }),
            )
        }
        "set_3d" => {
            let on = flag(p, "enabled")?;
            (
                THREE_D,
                vec![u8::from(on)],
                json!({ "three_d": { "enabled": on } }),
            )
        }
        "set_3d_eye" => {
            let eye = text(p, "eye")?;
            let data = match eye {
                "right" => 0x00,
                "left" => 0x01,
                other => return Err(invalid(format!("unknown eye '{other}'"))),
            };
            (
                THREE_D_EYE,
                vec![data],
                json!({ "three_d": { "eye": eye } }),
            )
        }
        "set_working_mode" => {
            let mode = text(p, "mode")?;
            let data = match mode {
                "send_only" => 0x00,
                "all_in_one" => 0x01,
                other => return Err(invalid(format!("unknown working mode '{other}'"))),
            };
            (WORKING_MODE, vec![data], json!({ "working_mode": mode }))
        }
        "set_output_display" => {
            let card = byte(p, "card")?;
            let mode = text(p, "mode")?;
            let data = match mode {
                "normal" => 0x00,
                "blackout" => 0x01,
                "freeze" => 0x02,
                other => return Err(invalid(format!("unknown display mode '{other}'"))),
            };
            let key = if card == 0xff {
                "all".to_string()
            } else {
                card.to_string()
            };
            (
                SENDING_DISPLAY,
                vec![card, data],
                json!({ "output_cards": { key: { "display": mode } } }),
            )
        }
        "switch_layer_source" => {
            let layer = byte(p, "layer")?;
            let card = byte(p, "input_card")?;
            let port = byte(p, "input_port")?;
            (
                LAYER_SOURCE,
                vec![layer, card, port],
                json!({ "layers": { layer.to_string(): { "input_card": card, "input_port": port } } }),
            )
        }
        other => {
            return Err(CommandError::UnknownCommand {
                command: other.into(),
            })
        }
    })
}

impl Module for NovastarCcp {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        self.connect(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        match request(name, params) {
            Ok((register, data, state)) => {
                if !self.connected && !self.connecting {
                    cx.complete(id, Err(CommandError::NotConnected));
                    return;
                }
                self.queue.push_back(Job {
                    id,
                    register,
                    frame: frame(register, &data),
                    state,
                });
                self.pump(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, _socket: Key, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                self.connecting = false;
                self.connected = true;
                self.retry_after = RETRY_MIN;
                cx.connection(Connection::Connected);
                self.pump(cx);
            }
            TcpInput::Data(data) => {
                cx.alive();
                self.buf.extend_from_slice(&data);
                while let Some(result) = take_answer(&mut self.buf) {
                    match result {
                        Ok(answer) => self.answer(cx, answer),
                        Err(e) => cx.log(Level::Debug, format!("NovaStar: {e}")),
                    }
                }
                self.pump(cx);
            }
            TcpInput::Closed { reason } => self.lost(cx, &reason),
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            REPLY => {
                if let Some(job) = self.in_flight.take() {
                    cx.complete(job.id, Err(CommandError::Timeout));
                }
                self.lost(cx, "the controller stopped answering");
            }
            RETRY => self.connect(cx),
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        if self.connected || self.connecting {
            cx.tcp_close(SOCKET);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::{Action, CommandResult};
    use std::net::{IpAddr, Ipv4Addr};

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    fn module() -> NovastarCcp {
        NovastarCcp::with(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(192, 168, 0, 10)),
            DEFAULT_PORT,
        ))
    }

    fn connected() -> NovastarCcp {
        let mut m = module();
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        m
    }

    fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::TcpSend { data, .. } => Some(data.clone()),
                _ => None,
            })
            .collect()
    }

    fn completed(actions: &[Action], id: CommandId) -> Option<CommandResult> {
        actions.iter().find_map(|a| match a {
            Action::Complete { id: i, result } if *i == id => Some(result.clone()),
            _ => None,
        })
    }

    fn run(m: &mut NovastarCcp, id: CommandId, name: &str, p: Value) -> Vec<Action> {
        let mut cx = Cx::new(0);
        m.command(&mut cx, id, name, p.as_object().unwrap());
        cx.take()
    }

    /// Every example frame of the document, byte for byte.
    #[test]
    fn frames_match_the_documents_examples() {
        let cases: &[(&str, Value, &str)] = &[
            // §3.2.2: brightness 0.
            (
                "set_brightness",
                json!({"level": 0}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 01 00 00 02 01 00 00 55 5a",
            ),
            // §3.2.3, §3.2.6: receiving card black screen and normal display.
            (
                "set_blackout",
                json!({"enabled": true}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 00 01 00 02 01 00 ff 54 5b",
            ),
            (
                "set_blackout",
                json!({"enabled": false}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 00 01 00 02 01 00 00 55 5a",
            ),
            // §3.2.4, §3.2.5: freeze and unfreeze.
            (
                "set_freeze",
                json!({"enabled": true}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 02 01 00 02 01 00 ff 56 5b",
            ),
            (
                "set_freeze",
                json!({"enabled": false}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 02 01 00 02 01 00 00 57 5a",
            ),
            // §3.3.2, §3.3.3: presets 1 and 2.
            (
                "recall_preset",
                json!({"preset": 1}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 02 00 00 0a 01 00 01 5f 5a",
            ),
            (
                "recall_preset",
                json!({"preset": 2}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 02 00 00 0a 01 00 02 60 5a",
            ),
            // §3.4.2-§3.4.9.
            (
                "set_low_latency",
                json!({"enabled": true}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 11 01 00 10 01 00 01 75 5a",
            ),
            (
                "set_low_latency",
                json!({"enabled": false}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 11 01 00 10 01 00 00 74 5a",
            ),
            (
                "set_3d",
                json!({"enabled": true}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 16 01 00 10 01 00 01 7a 5a",
            ),
            (
                "set_3d",
                json!({"enabled": false}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 16 01 00 10 01 00 00 79 5a",
            ),
            (
                "set_3d_eye",
                json!({"eye": "right"}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 18 11 00 10 01 00 00 8b 5a",
            ),
            (
                "set_3d_eye",
                json!({"eye": "left"}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 18 11 00 10 01 00 01 8c 5a",
            ),
            (
                "set_working_mode",
                json!({"mode": "all_in_one"}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 f2 ff 08 00 01 00 01 4c 5c",
            ),
            (
                "set_working_mode",
                json!({"mode": "send_only"}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 f2 ff 08 00 01 00 00 4b 5c",
            ),
            // §3.5.2-§3.5.5: sending card display, all cards and card 6.
            (
                "set_output_display",
                json!({"card": 255, "mode": "blackout"}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 00 01 00 10 02 00 ff 01 64 5b",
            ),
            (
                "set_output_display",
                json!({"card": 6, "mode": "blackout"}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 00 01 00 10 02 00 06 01 6b 5a",
            ),
            (
                "set_output_display",
                json!({"card": 6, "mode": "freeze"}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 00 01 00 10 02 00 06 02 6c 5a",
            ),
            (
                "set_output_display",
                json!({"card": 6, "mode": "normal"}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 00 01 00 10 02 00 06 00 6a 5a",
            ),
            // §3.6.2: layer 1 to the first source of input card 1.
            (
                "switch_layer_source",
                json!({"layer": 1, "input_card": 1, "input_port": 0}),
                "55 aa 00 00 fe ff 01 ff ff ff 01 00 03 00 00 0a 03 00 01 01 00 63 5a",
            ),
        ];
        for (name, params, expected) in cases {
            let (register, data, _) = request(name, params.as_object().unwrap()).unwrap();
            assert_eq!(frame(register, &data), hex(expected), "{name} {params}");
        }
    }

    #[test]
    fn brightness_percent_scales_to_ff() {
        let (_, data, _) = request(
            "set_brightness_percent",
            json!({"percent": 50.0}).as_object().unwrap(),
        )
        .unwrap();
        // Brightness ratio = value / 0xFF (§3.2.1): 50 % is 127.5, rounded.
        assert_eq!(data, vec![0x80]);
        let (_, data, _) = request(
            "set_brightness_percent",
            json!({"percent": 100.0}).as_object().unwrap(),
        )
        .unwrap();
        assert_eq!(data, vec![0xff]);
    }

    #[test]
    fn an_answer_completes_the_command_and_reports_state_and_round_trip() {
        let mut m = connected();
        let a = run(&mut m, 1, "set_working_mode", json!({"mode": "send_only"}));
        assert_eq!(sent(&a).len(), 1);
        // §3.1 D: the documented answer to the send-only request.
        let mut cx = Cx::new(40);
        m.tcp(
            &mut cx,
            SOCKET,
            TcpInput::Data(hex(
                "aa 55 00 00 ff fe 01 ff ff ff 01 00 f2 ff 08 00 00 00 49 5d",
            )),
        );
        let a = cx.take();
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert!(a.contains(&Action::RoundTrip(40)));
        assert!(a.contains(&Action::State(json!({"working_mode": "send_only"}))));
    }

    #[test]
    fn requests_go_one_at_a_time() {
        let mut m = connected();
        let a = run(&mut m, 1, "set_brightness", json!({"level": 10}));
        assert_eq!(sent(&a).len(), 1);
        let a = run(&mut m, 2, "set_blackout", json!({"enabled": true}));
        assert!(sent(&a).is_empty());
        let answer = {
            // The answer to a brightness write: the register, no data.
            let mut f = frame(BRIGHTNESS, &[]);
            f[0] = 0xaa;
            f[1] = 0x55;
            f
        };
        let mut cx = Cx::new(5);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(answer));
        let a = cx.take();
        assert_eq!(completed(&a, 1), Some(Ok(Outcome::Ack)));
        assert_eq!(sent(&a), vec![frame(RECEIVING_BLACK, &[0xff])]);
    }

    #[test]
    fn a_non_zero_ack_is_a_device_error() {
        let mut m = connected();
        run(&mut m, 1, "recall_preset", json!({"preset": 3}));
        let mut f = frame(PRESET, &[]);
        f[0] = 0xaa;
        f[1] = 0x55;
        f[2] = 0x01;
        let n = f.len();
        let sum = f[2..n - 2].iter().fold(0x5555u32, |s, b| s + *b as u32) as u16;
        f[n - 2..].copy_from_slice(&sum.to_le_bytes());
        let mut cx = Cx::new(0);
        m.tcp(&mut cx, SOCKET, TcpInput::Data(f));
        assert!(matches!(
            completed(&cx.take(), 1),
            Some(Err(CommandError::DeviceError { .. }))
        ));
    }

    #[test]
    fn opened_for_commands_only_it_sends_nothing_on_connecting() {
        let mut m = NovastarCcp::new(OpenContext {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            host_name: None,
            port: None,
            model: "mx40-pro".into(),
            channels: None,
            settings: Params::new(),
            monitor: false,
        });
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.tcp(&mut cx, SOCKET, TcpInput::Connected);
        let a = cx.take();
        assert!(sent(&a).is_empty());
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        // Commands still work.
        let a = run(&mut m, 1, "set_brightness", json!({"level": 255}));
        assert_eq!(sent(&a), vec![frame(BRIGHTNESS, &[0xff])]);
    }

    #[test]
    fn a_silent_controller_times_out_and_is_reconnected() {
        let mut m = connected();
        run(&mut m, 1, "set_3d", json!({"enabled": true}));
        let mut cx = Cx::new(3_000);
        m.timer(&mut cx, REPLY);
        let a = cx.take();
        assert_eq!(completed(&a, 1), Some(Err(CommandError::Timeout)));
        assert!(a
            .iter()
            .any(|x| matches!(x, Action::SetTimer { key: RETRY, .. })));
        let mut cx = Cx::new(4_000);
        m.timer(&mut cx, RETRY);
        assert!(cx
            .take()
            .iter()
            .any(|x| matches!(x, Action::TcpOpen { .. })));
    }

    #[test]
    fn noise_before_an_answer_is_skipped_and_split_answers_are_joined() {
        let mut buf = hex("00 13 aa");
        assert!(take_answer(&mut buf).is_none());
        buf.extend(hex("55 00 00 ff fe 01 ff ff ff 01 00 f2 ff 08 00"));
        assert!(take_answer(&mut buf).is_none());
        buf.extend(hex("00 00 49 5d"));
        assert_eq!(
            take_answer(&mut buf),
            Some(Ok(Answer {
                ack: 0,
                register: WORKING_MODE
            }))
        );
        assert!(buf.is_empty());
    }
}
