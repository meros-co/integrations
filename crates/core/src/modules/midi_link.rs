//! The link a MIDI integration reaches its device over, chosen by the `link`
//! setting: an RTP-MIDI session on the network that the core joins (`rtp`)
//! or hosts (`rtp_host`, [`super::rtp_midi`]). Either way the module sends
//! and receives MIDI messages. The device is `connected` while the session
//! is joined and its clock synchronization is answered.

use std::net::SocketAddr;

use crate::value::Value;

use super::rtp_midi::{self, Rtp};
use crate::catalog::Params;
use crate::midi::{Message, Parser, Rate, Timecode};
use crate::module::{CommandError, Cx, Key, OpenContext};

#[derive(Debug)]
pub(crate) struct MidiLink {
    rtp: Box<Rtp>,
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
    /// device's host and port (5004 unless given), or `rtp_host` on
    /// `local_port` (5004 unless given) for the device's host (or any) to
    /// join.
    pub(crate) fn from_context(ctx: &OpenContext) -> Result<MidiLink, String> {
        let name = text_setting(ctx, "session_name").unwrap_or_else(|| "Meros".into());
        let local = ctx
            .settings
            .get("local_port")
            .and_then(Value::as_u64)
            .and_then(|p| u16::try_from(p).ok())
            .filter(|p| *p > 0 && *p < u16::MAX);
        let rtp = match ctx.settings.get("link").and_then(Value::as_str) {
            None | Some("rtp") => {
                if ctx.host.is_unspecified() {
                    return Err(
                        "an RTP-MIDI session needs the host offering it; to host one, use link rtp_host"
                            .into(),
                    );
                }
                let peer = SocketAddr::new(ctx.host, ctx.port.unwrap_or(rtp_midi::DEFAULT_PORT));
                Rtp::new(peer, &name, local, seed())
            }
            Some("rtp_host") => {
                let only = (!ctx.host.is_unspecified()).then_some(ctx.host);
                Rtp::host(local.unwrap_or(rtp_midi::DEFAULT_PORT), only, &name, seed())
            }
            Some(other) => return Err(format!("link '{other}' is not rtp or rtp_host")),
        };
        Ok(MidiLink {
            rtp: Box::new(rtp),
            parser: Parser::default(),
        })
    }

    pub(crate) fn start(&mut self, cx: &mut Cx) {
        self.rtp.start(cx);
    }

    /// `rtp` or `rtp_host`.
    pub(crate) fn kind(&self) -> &'static str {
        if self.rtp.hosting() {
            "rtp_host"
        } else {
            "rtp"
        }
    }

    /// Where received messages come from, for message events.
    pub(crate) fn source(&self) -> SocketAddr {
        self.rtp.source()
    }

    /// Send messages to the device: `not_connected` when the session is not
    /// up.
    pub(crate) fn send(&mut self, cx: &mut Cx, messages: &[Message]) -> Result<(), CommandError> {
        self.rtp.send(cx, messages)
    }

    pub(crate) fn datagram(
        &mut self,
        cx: &mut Cx,
        socket: Key,
        from: SocketAddr,
        data: &[u8],
    ) -> Vec<Message> {
        let bytes = self.rtp.datagram(cx, socket, from, data);
        self.parser.feed(&bytes)
    }

    /// A timer of the link's; false for any other.
    pub(crate) fn timer(&mut self, cx: &mut Cx, key: Key) -> bool {
        self.rtp.timer(cx, key)
    }

    /// A socket of the link's failed; false for any other.
    pub(crate) fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) -> bool {
        self.rtp.socket_error(cx, socket, message)
    }

    pub(crate) fn stop(&mut self, cx: &mut Cx) {
        self.rtp.stop(cx);
    }

    /// Close and open the link again now.
    pub(crate) fn reconnect(&mut self, cx: &mut Cx) {
        self.parser = Parser::default();
        self.rtp.reconnect(cx);
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
