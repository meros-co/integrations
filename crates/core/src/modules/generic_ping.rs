//! Network presence: whether a host answers, how quickly, and how many of
//! its probes are lost, plus Wake-on-LAN to power it on.
//!
//! - A probe is an ICMP echo request (RFC 792, RFC 4443) sent the way the
//!   operating system allows without elevated privileges (src/icmp.rs), or a
//!   TCP connection to a chosen port (RFC 9293), opened and closed at once.
//!   A connection the host refuses (a reset) also proves it is there: only
//!   a running network stack answers. `auto` uses ICMP and falls back to
//!   TCP where the system refuses ICMP (Linux outside
//!   `net.ipv4.ping_group_range`).
//! - One probe is sent every `interval_ms`, never two at once. Reachable,
//!   the last round trip, and the minimum, mean and maximum round trip and
//!   the loss over the last `statistics_window` probes are kept in state.
//!   The host is reported unreachable (disconnected) after `down_after`
//!   probes in a row go unanswered, and connected again at the first reply.
//! - Wake-on-LAN sends AMD's magic packet (modules/wol.rs) to a broadcast
//!   address and to the host, with a SecureOn password when one is set.
//! - Opening for commands only changes nothing: probing is the liveness
//!   check and the whole purpose, so it runs either way.

use std::collections::VecDeque;
use std::net::{IpAddr, SocketAddr};

use serde_json::{json, Value};

use super::wol;
use crate::catalog::Params;
use crate::module::{
    CommandError, CommandId, Connection, Cx, EchoResult, Key, Level, Millis, Module, OpenContext,
    Outcome, TcpInput,
};

const ICMP: Key = "icmp";
const TCP: Key = "tcp";
const WOL: Key = "wol";
const POLL: Key = "poll";
const TCP_TIMEOUT: Key = "tcp_timeout";

#[derive(Debug, Clone, Copy, PartialEq)]
enum Method {
    Auto,
    Icmp,
    Tcp,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Via {
    Icmp,
    Tcp,
}

impl Via {
    fn name(self) -> &'static str {
        match self {
            Via::Icmp => "icmp",
            Via::Tcp => "tcp",
        }
    }
}

/// The probe in flight.
struct Probe {
    via: Via,
    sent_at: Millis,
}

/// What one probe found.
enum Found {
    Reply { rtt_ms: f64, ttl: Option<u8> },
    Lost { reason: String },
}

pub(crate) struct GenericPing {
    host: IpAddr,
    method: Method,
    tcp_port: Option<u16>,
    interval: Millis,
    timeout: Millis,
    size: u16,
    window: usize,
    down_after: u32,
    mac: Option<[u8; 6]>,
    password: Option<Vec<u8>>,
    wol_to: SocketAddr,
    /// How probes go now; `auto` moves from ICMP to TCP when ICMP is refused.
    via: Option<Via>,
    probe: Option<Probe>,
    /// The last `window` probes: a round trip in ms, or `None` if lost.
    history: VecDeque<Option<f64>>,
    sent: u64,
    received: u64,
    failures: u32,
    reachable: Option<bool>,
    /// `ping` commands answered by the probe in flight.
    waiting: Vec<CommandId>,
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

impl GenericPing {
    pub(crate) fn new(ctx: OpenContext) -> Result<GenericPing, String> {
        if ctx.host.is_unspecified() {
            return Err("give the address of the host to check".into());
        }
        let s = &ctx.settings;
        let text = |k: &str| s.get(k).and_then(Value::as_str).unwrap_or("").trim();
        let num = |k: &str, d: u64| s.get(k).and_then(Value::as_u64).unwrap_or(d);
        let method = match text("method") {
            "" | "auto" => Method::Auto,
            "icmp" => Method::Icmp,
            "tcp" => Method::Tcp,
            other => return Err(format!("method '{other}' is not auto, icmp or tcp")),
        };
        if method == Method::Tcp && ctx.port.is_none() {
            return Err("the tcp method needs the port to connect to: give it as the port".into());
        }
        let mac = match text("mac_address") {
            "" => None,
            m => Some(
                wol::parse_mac(m)
                    .ok_or_else(|| format!("mac_address '{m}' is not a MAC address"))?,
            ),
        };
        let password = match text("secureon_password") {
            "" => None,
            p => Some(wol::parse_password(p).ok_or(
                "secureon_password is not 4 or 6 bytes in hex, such as 01:23:45:67:89:ab",
            )?),
        };
        let wol_address: IpAddr = match text("wol_address") {
            "" => IpAddr::V4(std::net::Ipv4Addr::BROADCAST),
            a => a
                .parse()
                .map_err(|_| format!("wol_address '{a}' is not an IP address"))?,
        };
        let wol_port = num("wol_port", 9).clamp(1, 65535) as u16;
        Ok(GenericPing {
            host: ctx.host,
            method,
            tcp_port: ctx.port,
            interval: num("interval_ms", 1000).max(100),
            timeout: num("timeout_ms", 1000).max(1),
            size: num("payload_size", 32).min(65_000) as u16,
            window: num("statistics_window", 20).max(1) as usize,
            down_after: num("down_after", 3).max(1) as u32,
            mac,
            password,
            wol_to: SocketAddr::new(wol_address, wol_port),
            via: match method {
                Method::Tcp => Some(Via::Tcp),
                _ => Some(Via::Icmp),
            },
            probe: None,
            history: VecDeque::new(),
            sent: 0,
            received: 0,
            failures: 0,
            reachable: None,
            waiting: Vec::new(),
        })
    }

    fn send_probe(&mut self, cx: &mut Cx) {
        let Some(via) = self.via else {
            return;
        };
        self.probe = Some(Probe {
            via,
            sent_at: cx.now(),
        });
        self.sent += 1;
        match via {
            Via::Icmp => cx.icmp_echo(ICMP, self.host, self.timeout, self.size),
            Via::Tcp => {
                let port = self.tcp_port.expect("TCP is used only with a port");
                cx.tcp_open(TCP, SocketAddr::new(self.host, port));
                cx.set_timer(TCP_TIMEOUT, self.timeout);
            }
        }
    }

    /// ICMP is refused here: fall back to TCP when allowed and possible,
    /// or stop probing and say why.
    fn icmp_refused(&mut self, cx: &mut Cx, message: &str) {
        self.probe = None;
        self.sent -= 1;
        match (self.method, self.tcp_port) {
            (Method::Auto, Some(port)) => {
                cx.log(
                    Level::Info,
                    format!("ICMP echo is not allowed here ({message}); checking TCP port {port} instead"),
                );
                self.via = Some(Via::Tcp);
                self.send_probe(cx);
            }
            _ => {
                self.via = None;
                let reason = format!(
                    "ICMP echo is not allowed here ({message}); give a port to check with a TCP connection instead"
                );
                cx.state(json!({ "method": "none", "last_error": reason }));
                cx.connection(Connection::Disconnected {
                    reason: reason.clone(),
                });
                for id in self.waiting.drain(..) {
                    cx.complete(
                        id,
                        Err(CommandError::Transport {
                            message: reason.clone(),
                        }),
                    );
                }
            }
        }
    }

    fn finish(&mut self, cx: &mut Cx, found: Found) {
        let Some(probe) = self.probe.take() else {
            return;
        };
        cx.cancel_timer(TCP_TIMEOUT);
        if self.history.len() == self.window {
            self.history.pop_front();
        }
        let mut patch = json!({ "method": probe.via.name(), "sent": self.sent });
        let answer = match &found {
            Found::Reply { rtt_ms, ttl } => {
                self.received += 1;
                self.failures = 0;
                self.history.push_back(Some(*rtt_ms));
                cx.alive();
                cx.round_trip(rtt_ms.round() as Millis);
                patch["rtt_ms"] = json!(rtt_ms);
                patch["ttl"] = json!(ttl);
                patch["last_reply"] = json!(cx.unix_millis());
                patch["last_error"] = Value::Null;
                if self.reachable != Some(true) {
                    self.reachable = Some(true);
                    patch["reachable"] = json!(true);
                    cx.connection(Connection::Connected);
                }
                json!({ "reachable": true, "rtt_ms": rtt_ms, "ttl": ttl, "method": probe.via.name() })
            }
            Found::Lost { reason } => {
                self.failures += 1;
                self.history.push_back(None);
                patch["rtt_ms"] = Value::Null;
                patch["ttl"] = Value::Null;
                patch["last_error"] = json!(reason);
                if self.failures >= self.down_after && self.reachable != Some(false) {
                    self.reachable = Some(false);
                    patch["reachable"] = json!(false);
                    cx.connection(Connection::Disconnected {
                        reason: format!("{} probes in a row unanswered: {reason}", self.failures),
                    });
                }
                json!({ "reachable": false, "error": reason, "method": probe.via.name() })
            }
        };
        patch["received"] = json!(self.received);
        patch["consecutive_failures"] = json!(self.failures);
        let answered: Vec<f64> = self.history.iter().flatten().copied().collect();
        let lost = self.history.len() - answered.len();
        patch["loss_percent"] = json!(100.0 * lost as f64 / self.history.len() as f64);
        if answered.is_empty() {
            patch["rtt_min_ms"] = Value::Null;
            patch["rtt_avg_ms"] = Value::Null;
            patch["rtt_max_ms"] = Value::Null;
        } else {
            let min = answered.iter().copied().fold(f64::INFINITY, f64::min);
            let max = answered.iter().copied().fold(0.0, f64::max);
            let avg = answered.iter().sum::<f64>() / answered.len() as f64;
            patch["rtt_min_ms"] = json!(min);
            patch["rtt_avg_ms"] = json!(avg);
            patch["rtt_max_ms"] = json!(max);
        }
        cx.state(patch);
        for id in self.waiting.drain(..) {
            cx.complete(
                id,
                Ok(Outcome::Value {
                    value: answer.clone(),
                }),
            );
        }
    }
}

impl Module for GenericPing {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.state(json!({ "reachable": Value::Null, "sent": 0, "received": 0 }));
        self.send_probe(cx);
        cx.set_timer(POLL, self.interval);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        match name {
            "ping" => {
                if self.via.is_none() {
                    cx.complete(
                        id,
                        Err(CommandError::Transport {
                            message: "ICMP echo is not allowed here and no TCP port is given"
                                .into(),
                        }),
                    );
                    return;
                }
                self.waiting.push(id);
                if self.probe.is_none() {
                    self.send_probe(cx);
                }
            }
            "wake_on_lan" => {
                let mac = match params
                    .get("mac_address")
                    .and_then(Value::as_str)
                    .map(str::trim)
                {
                    Some(m) if !m.is_empty() => match wol::parse_mac(m) {
                        Some(mac) => mac,
                        None => {
                            cx.complete(id, Err(invalid(format!("'{m}' is not a MAC address"))));
                            return;
                        }
                    },
                    _ => match self.mac {
                        Some(mac) => mac,
                        None => {
                            cx.complete(
                                id,
                                Err(invalid("set the mac_address setting, or give mac_address")),
                            );
                            return;
                        }
                    },
                };
                let packet = wol::magic_packet(mac, self.password.as_deref());
                wol::send(cx, WOL, packet, self.wol_to, Some(self.host));
                cx.complete(id, Ok(Outcome::Unverified));
            }
            "reset_statistics" => {
                self.history.clear();
                self.sent = u64::from(self.probe.is_some());
                self.received = 0;
                cx.state(json!({
                    "sent": self.sent, "received": 0, "loss_percent": Value::Null,
                    "rtt_min_ms": Value::Null, "rtt_avg_ms": Value::Null, "rtt_max_ms": Value::Null,
                }));
                cx.complete(id, Ok(Outcome::Ack));
            }
            other => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: other.into(),
                }),
            ),
        }
    }

    fn icmp(&mut self, cx: &mut Cx, key: Key, result: EchoResult) {
        if key != ICMP || !matches!(self.probe, Some(Probe { via: Via::Icmp, .. })) {
            return;
        }
        match result {
            EchoResult::Reply { rtt_micros, ttl } => self.finish(
                cx,
                Found::Reply {
                    rtt_ms: rtt_micros as f64 / 1000.0,
                    ttl,
                },
            ),
            EchoResult::Timeout => self.finish(
                cx,
                Found::Lost {
                    reason: "no echo reply within the timeout".into(),
                },
            ),
            EchoResult::Unreachable { message } => self.finish(cx, Found::Lost { reason: message }),
            EchoResult::Unavailable { message } => self.icmp_refused(cx, &message),
        }
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        if socket != TCP {
            return;
        }
        let Some(Probe {
            via: Via::Tcp,
            sent_at,
        }) = self.probe
        else {
            return;
        };
        let rtt_ms = cx.now().saturating_sub(sent_at) as f64;
        match input {
            TcpInput::Connected => {
                cx.tcp_close(TCP);
                self.finish(cx, Found::Reply { rtt_ms, ttl: None });
            }
            // A refusal is the host's own TCP stack answering with a reset.
            TcpInput::Closed { reason } if reason.to_ascii_lowercase().contains("refused") => {
                self.finish(cx, Found::Reply { rtt_ms, ttl: None });
            }
            TcpInput::Closed { reason } => self.finish(cx, Found::Lost { reason }),
            TcpInput::Data(_) => {}
        }
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        // Only the Wake-on-LAN socket is UDP; a failed send is worth a log
        // line, not a change of the host's reachability.
        cx.log(Level::Warning, format!("{socket}: {message}"));
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            POLL => {
                cx.set_timer(POLL, self.interval);
                if self.probe.is_none() {
                    self.send_probe(cx);
                }
            }
            TCP_TIMEOUT => {
                cx.tcp_close(TCP);
                self.finish(
                    cx,
                    Found::Lost {
                        reason: "no TCP answer within the timeout".into(),
                    },
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::Action;
    use std::net::Ipv4Addr;

    const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 10));

    fn open(settings: Value, port: Option<u16>) -> GenericPing {
        GenericPing::new(OpenContext {
            host: HOST,
            host_name: None,
            port,
            model: "generic".into(),
            channels: None,
            settings: settings.as_object().unwrap().clone(),
            monitor: true,
        })
        .unwrap()
    }

    fn state(actions: &[Action]) -> Value {
        let mut out = json!({});
        for a in actions {
            if let Action::State(p) = a {
                crate::session::merge_patch(&mut out, p);
            }
        }
        out
    }

    #[test]
    fn replies_and_losses_make_the_statistics() {
        let mut m = open(json!({ "down_after": 2, "statistics_window": 4 }), None);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        assert!(cx.take().iter().any(|a| matches!(
            a,
            Action::IcmpEcho { key: ICMP, request } if request.to == HOST && request.size == 32
        )));
        let mut cx = Cx::new(3);
        m.icmp(
            &mut cx,
            ICMP,
            EchoResult::Reply {
                rtt_micros: 2500,
                ttl: Some(64),
            },
        );
        let actions = cx.take();
        assert!(actions.contains(&Action::Connection(Connection::Connected)));
        assert!(actions.contains(&Action::RoundTrip(3)));
        let s = state(&actions);
        assert_eq!(s["reachable"], true);
        assert_eq!(s["rtt_ms"], 2.5);
        assert_eq!(s["ttl"], 64);
        assert_eq!(s["loss_percent"], 0.0);
        // Two losses: unreachable at the second.
        for at in [1000, 2000] {
            let mut cx = Cx::new(at);
            m.timer(&mut cx, POLL);
            m.icmp(&mut cx, ICMP, EchoResult::Timeout);
            let actions = cx.take();
            let down = actions
                .iter()
                .any(|a| matches!(a, Action::Connection(Connection::Disconnected { .. })));
            assert_eq!(down, at == 2000);
        }
        let mut cx = Cx::new(3000);
        m.timer(&mut cx, POLL);
        m.icmp(
            &mut cx,
            ICMP,
            EchoResult::Reply {
                rtt_micros: 500,
                ttl: None,
            },
        );
        let s = state(&cx.take());
        assert_eq!(s["loss_percent"], 50.0);
        assert_eq!(s["rtt_min_ms"], 0.5);
        assert_eq!(s["rtt_max_ms"], 2.5);
        assert_eq!(s["rtt_avg_ms"], 1.5);
        assert_eq!(s["sent"], 4);
        assert_eq!(s["received"], 2);
        assert_eq!(s["reachable"], true);
    }

    #[test]
    fn refused_icmp_falls_back_to_tcp_and_a_reset_is_an_answer() {
        let mut m = open(json!({}), Some(80));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(1);
        m.icmp(
            &mut cx,
            ICMP,
            EchoResult::Unavailable {
                message: "Permission denied".into(),
            },
        );
        assert!(cx.take().contains(&Action::TcpOpen {
            socket: TCP,
            to: SocketAddr::new(HOST, 80)
        }));
        let mut cx = Cx::new(5);
        m.tcp(
            &mut cx,
            TCP,
            TcpInput::Closed {
                reason: "connect: Connection refused (os error 111)".into(),
            },
        );
        let s = state(&cx.take());
        assert_eq!(s["reachable"], true);
        assert_eq!(s["method"], "tcp");
        assert_eq!(s["rtt_ms"], 4.0);
        // Later probes stay on TCP; a timeout is a loss.
        let mut cx = Cx::new(1000);
        m.timer(&mut cx, POLL);
        assert!(cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::TcpOpen { .. })));
        let mut cx = Cx::new(2000);
        m.timer(&mut cx, TCP_TIMEOUT);
        let actions = cx.take();
        assert!(actions.contains(&Action::TcpClose { socket: TCP }));
        assert_eq!(state(&actions)["consecutive_failures"], 1);
    }

    #[test]
    fn without_icmp_or_a_port_the_reason_is_given() {
        let mut m = open(json!({ "method": "icmp" }), Some(80));
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        m.icmp(
            &mut cx,
            ICMP,
            EchoResult::Unavailable {
                message: "EACCES".into(),
            },
        );
        let actions = cx.take();
        assert!(!actions.iter().any(|a| matches!(a, Action::TcpOpen { .. })));
        assert!(actions
            .iter()
            .any(|a| matches!(a, Action::Connection(Connection::Disconnected { reason }) if reason.contains("EACCES"))));
        let mut cx = Cx::new(1000);
        m.timer(&mut cx, POLL);
        assert!(!cx
            .take()
            .iter()
            .any(|a| matches!(a, Action::IcmpEcho { .. })));
    }

    #[test]
    fn the_ping_command_waits_for_the_probe_in_flight() {
        let mut m = open(json!({}), None);
        let mut cx = Cx::new(0);
        m.start(&mut cx);
        cx.take();
        let mut cx = Cx::new(1);
        m.command(&mut cx, 7, "ping", &Params::new());
        assert!(
            !cx.take()
                .iter()
                .any(|a| matches!(a, Action::IcmpEcho { .. })),
            "one probe at a time"
        );
        let mut cx = Cx::new(2);
        m.icmp(
            &mut cx,
            ICMP,
            EchoResult::Reply {
                rtt_micros: 1000,
                ttl: Some(128),
            },
        );
        let done = cx.take().into_iter().find_map(|a| match a {
            Action::Complete { id: 7, result } => Some(result),
            _ => None,
        });
        assert_eq!(
            done,
            Some(Ok(Outcome::Value {
                value: json!({ "reachable": true, "rtt_ms": 1.0, "ttl": 128, "method": "icmp" })
            }))
        );
    }

    #[test]
    fn wake_on_lan_broadcasts_the_magic_packet_with_the_password() {
        let mut m = open(
            json!({
                "mac_address": "00:11:22:33:44:55",
                "secureon_password": "01:02:03:04:05:06",
                "wol_address": "192.0.2.255",
                "wol_port": 7,
            }),
            None,
        );
        let mut cx = Cx::new(0);
        m.command(&mut cx, 1, "wake_on_lan", &Params::new());
        let actions = cx.take();
        let sends: Vec<(SocketAddr, &Vec<u8>)> = actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { to, data, .. } => Some((*to, data)),
                _ => None,
            })
            .collect();
        let packet =
            wol::magic_packet([0, 0x11, 0x22, 0x33, 0x44, 0x55], Some(&[1, 2, 3, 4, 5, 6]));
        assert_eq!(
            sends,
            vec![
                ("192.0.2.255:7".parse().unwrap(), &packet),
                (SocketAddr::new(HOST, 7), &packet)
            ]
        );
        assert!(actions.contains(&Action::Complete {
            id: 1,
            result: Ok(Outcome::Unverified)
        }));
        // Without a MAC address anywhere, it is refused.
        let mut m = open(json!({}), None);
        let mut cx = Cx::new(0);
        m.command(&mut cx, 2, "wake_on_lan", &Params::new());
        assert!(matches!(
            &cx.take()[..],
            [Action::Complete {
                result: Err(CommandError::InvalidParams { .. }),
                ..
            }]
        ));
    }
}
