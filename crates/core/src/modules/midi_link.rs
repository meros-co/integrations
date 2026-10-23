//! The link a MIDI integration reaches its device over, chosen by the `link`
//! setting: an RTP-MIDI session on the network that the core joins (`rtp`)
//! or hosts (`rtp_host`, [`super::rtp_midi`]), or a MIDI port of the host
//! (`port`: `midi_input` and `midi_output`, opened by the session,
//! `crate::midi_ports`). Either way the module sends
//! and receives MIDI messages; what differs is only what is opened, and how
//! the device's presence shows.
//!
//! A MIDI port gives no sign of the device itself, so the device is
//! `connected` while the port is open; a port that cannot be opened, or goes
//! away, is opened again after 1 s, doubling to 30 s. An RTP-MIDI session is
//! `connected` while it is joined and its clock synchronisation is answered.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde_json::{json, Value};

use super::rtp_midi::{self, Rtp};
use crate::catalog::Params;
use crate::midi::{Message, Parser, Rate, Timecode};
use crate::module::{CommandError, Connection, Cx, Key, MidiPort, Millis, OpenContext, TcpInput};

const PORT: Key = "midi-port";
const PORT_RETRY: Key = "midi-port-retry";
const RETRY_MIN: Millis = 1000;
const RETRY_MAX: Millis = 30_000;
/// What a received message's source reads on a MIDI port, which has no
/// network address.
const NO_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);

#[derive(Debug)]
enum Kind {
    Rtp(Box<Rtp>),
    Port {
        port: MidiPort,
        open: bool,
        retry: Millis,
    },
}

#[derive(Debug)]
pub(crate) struct MidiLink {
    kind: Kind,
    parser: Parser,
}

fn text_setting(ctx: &OpenContext, name: &str) -> Option<String> {
    ctx.settings
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

/// A seed no two sessions share, for the SSRC, token and ports.
fn seed() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or_default(),
    );
    h.finish()
}

impl MidiLink {
    /// The link the settings choose: `link` `rtp` (the default) to the
    /// device's host and port (5004 unless given), `rtp_host` on
    /// `local_port` (5004 unless given) for the device's host (or any) to
    /// join, or `port` with `midi_input` and `midi_output`.
    pub(crate) fn from_context(ctx: &OpenContext) -> Result<MidiLink, String> {
        let kind = match ctx.settings.get("link").and_then(Value::as_str) {
            None | Some("rtp") => {
                if ctx.host.is_unspecified() {
                    return Err(
                        "an RTP-MIDI session needs the host offering it; a MIDI port needs link port"
                            .into(),
                    );
                }
                let peer = SocketAddr::new(ctx.host, ctx.port.unwrap_or(rtp_midi::DEFAULT_PORT));
                let name = text_setting(ctx, "session_name").unwrap_or_else(|| "Meros".into());
                let local = ctx
                    .settings
                    .get("local_port")
                    .and_then(Value::as_u64)
                    .and_then(|p| u16::try_from(p).ok())
                    .filter(|p| *p > 0 && *p < u16::MAX);
                Kind::Rtp(Box::new(Rtp::new(peer, &name, local, seed())))
            }
            Some("rtp_host") => {
                let name = text_setting(ctx, "session_name").unwrap_or_else(|| "Meros".into());
                let local = ctx
                    .settings
                    .get("local_port")
                    .and_then(Value::as_u64)
                    .and_then(|p| u16::try_from(p).ok())
                    .filter(|p| *p > 0 && *p < u16::MAX)
                    .unwrap_or(rtp_midi::DEFAULT_PORT);
                let only = (!ctx.host.is_unspecified()).then_some(ctx.host);
                Kind::Rtp(Box::new(Rtp::host(local, only, &name, seed())))
            }
            Some("port") => {
                let port = MidiPort {
                    input: text_setting(ctx, "midi_input"),
                    output: text_setting(ctx, "midi_output"),
                };
                if port.input.is_none() && port.output.is_none() {
                    return Err(
                        "link port needs midi_input, midi_output or both: the host's MIDI ports"
                            .into(),
                    );
                }
                Kind::Port {
                    port,
                    open: false,
                    retry: RETRY_MIN,
                }
            }
            Some(other) => return Err(format!("link '{other}' is not rtp, rtp_host or port")),
        };
        Ok(MidiLink {
            kind,
            parser: Parser::default(),
        })
    }

    pub(crate) fn start(&mut self, cx: &mut Cx) {
        match &mut self.kind {
            Kind::Rtp(rtp) => rtp.start(cx),
            Kind::Port { port, .. } => {
                cx.connection(Connection::Connecting);
                // "": no port opened that way (a null would be a removal).
                let name = |p: &Option<String>| p.clone().unwrap_or_default();
                cx.state(
                    json!({"port": {"input": name(&port.input), "output": name(&port.output)}}),
                );
                cx.midi_open(PORT, port.clone());
            }
        }
    }

    /// `rtp`, `rtp_host` or `port`.
    pub(crate) fn kind(&self) -> &'static str {
        match &self.kind {
            Kind::Rtp(rtp) if rtp.hosting() => "rtp_host",
            Kind::Rtp(_) => "rtp",
            Kind::Port { .. } => "port",
        }
    }

    /// Where received messages come from, for message events.
    pub(crate) fn source(&self) -> SocketAddr {
        match &self.kind {
            Kind::Rtp(rtp) => rtp.source(),
            Kind::Port { .. } => NO_ADDRESS,
        }
    }

    /// Send messages to the device: `not_connected` when the link is not
    /// up, `invalid_params` on a port with no output set.
    pub(crate) fn send(&mut self, cx: &mut Cx, messages: &[Message]) -> Result<(), CommandError> {
        match &mut self.kind {
            Kind::Rtp(rtp) => rtp.send(cx, messages),
            Kind::Port { port, open, .. } => {
                if port.output.is_none() {
                    return Err(CommandError::InvalidParams {
                        message: "no MIDI output is set (midi_output), so nothing can be sent"
                            .into(),
                    });
                }
                if !*open {
                    return Err(CommandError::NotConnected);
                }
                let bytes: Vec<u8> = messages.iter().flat_map(Message::encode).collect();
                cx.tcp_send(PORT, bytes);
                Ok(())
            }
        }
    }

    fn messages(&mut self, bytes: &[u8]) -> Vec<Message> {
        self.parser.feed(bytes)
    }

    pub(crate) fn datagram(
        &mut self,
        cx: &mut Cx,
        socket: Key,
        from: SocketAddr,
        data: &[u8],
    ) -> Vec<Message> {
        let bytes = match &mut self.kind {
            Kind::Rtp(rtp) => rtp.datagram(cx, socket, from, data),
            Kind::Port { .. } => return Vec::new(),
        };
        self.messages(&bytes)
    }

    pub(crate) fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) -> Vec<Message> {
        let Kind::Port { open, retry, .. } = &mut self.kind else {
            return Vec::new();
        };
        if socket != PORT {
            return Vec::new();
        }
        match input {
            TcpInput::Connected => {
                *open = true;
                *retry = RETRY_MIN;
                cx.connection(Connection::Connected);
                Vec::new()
            }
            TcpInput::Data(bytes) => {
                cx.alive();
                self.messages(&bytes)
            }
            TcpInput::Closed { reason } => {
                *open = false;
                cx.connection(Connection::Disconnected { reason });
                cx.set_timer(PORT_RETRY, *retry);
                *retry = (*retry * 2).min(RETRY_MAX);
                self.parser = Parser::default();
                Vec::new()
            }
        }
    }

    /// A timer of the link's; false for any other.
    pub(crate) fn timer(&mut self, cx: &mut Cx, key: Key) -> bool {
        match &mut self.kind {
            Kind::Rtp(rtp) => rtp.timer(cx, key),
            Kind::Port { port, .. } if key == PORT_RETRY => {
                cx.midi_open(PORT, port.clone());
                true
            }
            Kind::Port { .. } => false,
        }
    }

    /// A socket of the link's failed; false for any other.
    pub(crate) fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) -> bool {
        match &mut self.kind {
            Kind::Rtp(rtp) => rtp.socket_error(cx, socket, message),
            Kind::Port { .. } => false,
        }
    }

    pub(crate) fn stop(&mut self, cx: &mut Cx) {
        match &mut self.kind {
            Kind::Rtp(rtp) => rtp.stop(cx),
            Kind::Port { .. } => cx.tcp_close(PORT),
        }
    }

    /// Close and open the link again now.
    pub(crate) fn reconnect(&mut self, cx: &mut Cx) {
        self.parser = Parser::default();
        match &mut self.kind {
            Kind::Rtp(rtp) => rtp.reconnect(cx),
            Kind::Port {
                port, open, retry, ..
            } => {
                *open = false;
                *retry = RETRY_MIN;
                cx.cancel_timer(PORT_RETRY);
                cx.connection(Connection::Connecting);
                cx.midi_open(PORT, port.clone());
            }
        }
    }
}

/// The time in a command's `hours`, `minutes`, `seconds`, `frames` and
/// `rate`, for the MIDI integrations' time code parameters.
pub(crate) fn timecode(params: &Params) -> Result<Timecode, CommandError> {
    let field = |name: &str| {
        params
            .get(name)
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .clamp(0, 127) as u8
    };
    let invalid = |message: String| CommandError::InvalidParams { message };
    let name = params.get("rate").and_then(Value::as_str).unwrap_or("30");
    let rate = Rate::from_name(name)
        .ok_or_else(|| invalid(format!("rate '{name}' is not one of 24, 25, 30df, 30")))?;
    let (h, m, s, f) = (
        field("hours"),
        field("minutes"),
        field("seconds"),
        field("frames"),
    );
    Timecode::new(h, m, s, f, rate).ok_or_else(|| {
        invalid(format!(
            "{h:02}:{m:02}:{s:02}:{f:02} is not a time at {} fps{}",
            rate.name(),
            if rate == Rate::Fps30Drop {
                " (drop-frame skips frames 0 and 1 of each minute but every tenth)"
            } else {
                ""
            }
        ))
    })
}
