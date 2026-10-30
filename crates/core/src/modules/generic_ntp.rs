//! An NTP server's status, read with the client request of RFC 5905: a
//! 48-byte mode 3 packet to UDP 123, answered by the server's mode 4 packet.
//!
//! Each answer gives the server's leap indicator, stratum, poll interval,
//! precision, root delay and dispersion, reference ID and reference time
//! (RFC 5905 §7.3), and, from the four timestamps of the exchange (§8), the
//! offset between the server's clock and this host's and the round-trip
//! delay. A kiss-o'-death answer (stratum 0, §7.4) is reported with its code.
//!
//! The server is asked every `poll_ms` (64 s by default, NTP's own shortest
//! poll interval being 16 s). Opened for commands only (`monitor` false),
//! only `get_status` asks, and once a minute while idle as the liveness
//! check. Every answer reports its round trip.

use std::net::SocketAddr;

use crate::{json, value::Value};

use crate::catalog::Params;
use crate::module::{
    Bind, CommandError, CommandId, Connection, Cx, Key, Level, Millis, Module, OpenContext,
    Outcome, PollStamps,
};

const SOCKET: Key = "ntp";
const REPLY_TIMER: Key = "reply";
const POLL_TIMER: Key = "poll";
const PROBE_TIMER: Key = "probe";

pub(crate) const DEFAULT_PORT: u16 = 123;

const REPLY_TIMEOUT: Millis = 2_000;
const MISSES: u32 = 3;
const PROBE_EVERY: Millis = 60_000;

/// Seconds from the NTP era's start (1900) to the Unix epoch (1970).
const NTP_UNIX: u64 = 2_208_988_800;

/// A Unix time in milliseconds as an NTP timestamp (§6): seconds since 1900
/// and a binary fraction.
fn ntp_timestamp(unix_ms: u64) -> u64 {
    let secs = unix_ms / 1000 + NTP_UNIX;
    let frac = ((unix_ms % 1000) << 32) / 1000;
    (secs << 32) | frac
}

/// An NTP timestamp as Unix milliseconds, as a float to keep the fraction.
fn unix_ms(ts: u64) -> f64 {
    let secs = (ts >> 32) as f64 - NTP_UNIX as f64;
    let frac = (ts & 0xFFFF_FFFF) as f64 / 4_294_967_296.0;
    (secs + frac) * 1000.0
}

/// The client request (§7.3): LI 0, version 4, mode 3, and our transmit time,
/// which the server returns as the origin timestamp.
pub(crate) fn request(unix_ms: u64) -> [u8; 48] {
    let mut p = [0u8; 48];
    p[0] = 0x23;
    p[40..48].copy_from_slice(&ntp_timestamp(unix_ms).to_be_bytes());
    p
}

fn short_ms(b: &[u8]) -> f64 {
    // NTP short format: 16.16 seconds (§6).
    f64::from(u32::from_be_bytes([b[0], b[1], b[2], b[3]])) / 65_536.0 * 1000.0
}

fn round(v: f64, places: i32) -> f64 {
    let f = 10f64.powi(places);
    (v * f).round() / f
}

/// The server's answer as state, given when the request left (`t1`) and the
/// answer came (`t4`), both Unix milliseconds of this host. `None` for a
/// packet that is not the answer to `sent`.
fn decode(p: &[u8], sent: u64, t1: u64, t4: u64) -> Option<Value> {
    if p.len() < 48 {
        return None;
    }
    let mode = p[0] & 0x07;
    if mode != 4 {
        return None;
    }
    let ts = |at: usize| u64::from_be_bytes(p[at..at + 8].try_into().unwrap());
    if ts(24) != sent {
        return None;
    }
    let leap = match p[0] >> 6 {
        0 => "none",
        1 => "add_second",
        2 => "delete_second",
        _ => "unsynchronized",
    };
    let version = (p[0] >> 3) & 0x07;
    let stratum = p[1];
    let refid = &p[12..16];
    let reference_id = if stratum <= 1 {
        String::from_utf8_lossy(refid)
            .trim_end_matches('\0')
            .trim()
            .to_string()
    } else {
        format!("{}.{}.{}.{}", refid[0], refid[1], refid[2], refid[3])
    };
    let t2 = unix_ms(ts(32));
    let t3 = unix_ms(ts(40));
    let (t1, t4) = (t1 as f64, t4 as f64);
    // §8: offset ((T2 - T1) + (T3 - T4)) / 2, delay (T4 - T1) - (T3 - T2).
    let offset = ((t2 - t1) + (t3 - t4)) / 2.0;
    let delay = ((t4 - t1) - (t3 - t2)).max(0.0);
    let kiss = (stratum == 0).then(|| reference_id.clone());
    let synchronized = leap != "unsynchronized" && (1..=15).contains(&stratum);
    let reference_time = unix_ms(ts(16));
    let mut state = json!({
        "leap": leap,
        "version": version,
        "stratum": stratum,
        "poll_exponent": p[2] as i8,
        "precision_exponent": p[3] as i8,
        "root_delay_ms": round(short_ms(&p[4..8]), 3),
        "root_dispersion_ms": round(short_ms(&p[8..12]), 3),
        "reference_id": reference_id,
        "reference_time_ms": reference_time.round() as i64,
        "offset_ms": round(offset, 3),
        "delay_ms": round(delay, 3),
        "synchronized": synchronized,
        "kiss_code": kiss.unwrap_or_default(),
    });
    if stratum == 0 {
        // A kiss-o'-death carries no time.
        let o = state.as_object_mut().unwrap();
        o.remove("offset_ms");
        o.remove("delay_ms");
        o.remove("reference_time_ms");
    }
    Some(state)
}

pub(crate) struct GenericNtp {
    to: SocketAddr,
    monitor: bool,
    /// When each poll was last answered (`polled.<group>`, SPEC.md §8).
    stamps: PollStamps,
    poll_ms: Millis,
    reachable: bool,
    misses: u32,
    /// The command waiting for the answer, if a command asked.
    waiting: Vec<CommandId>,
    /// The request in flight: its transmit timestamp, and when it left.
    in_flight: Option<(u64, u64, Millis)>,
}

impl GenericNtp {
    pub(crate) fn new(ctx: OpenContext) -> Result<GenericNtp, String> {
        if ctx.host.is_unspecified() {
            return Err("give the NTP server's address as host".into());
        }
        let poll_ms = ctx
            .settings
            .get("poll_ms")
            .and_then(Value::as_u64)
            .unwrap_or(64_000)
            .max(1_000);
        Ok(GenericNtp {
            to: SocketAddr::new(ctx.host, ctx.port.unwrap_or(DEFAULT_PORT)),
            monitor: ctx.monitor,
            stamps: PollStamps::default(),
            poll_ms,
            reachable: false,
            misses: 0,
            waiting: Vec::new(),
            in_flight: None,
        })
    }

    fn ask(&mut self, cx: &mut Cx) {
        if self.in_flight.is_some() {
            return;
        }
        let t1 = cx.unix_millis();
        let packet = request(t1);
        let sent = ntp_timestamp(t1);
        cx.udp_send(SOCKET, self.to, packet.to_vec());
        cx.set_timer(REPLY_TIMER, REPLY_TIMEOUT);
        self.in_flight = Some((sent, t1, cx.now()));
    }
}

impl Module for GenericNtp {
    fn start(&mut self, cx: &mut Cx) {
        cx.connection(Connection::Connecting);
        cx.udp_open(SOCKET, Bind::Ephemeral);
        self.ask(cx);
        if self.monitor {
            cx.set_timer(POLL_TIMER, self.poll_ms);
        } else {
            cx.set_timer(PROBE_TIMER, PROBE_EVERY);
        }
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, _params: &Params) {
        match name {
            "get_status" => {
                self.waiting.push(id);
                self.ask(cx);
            }
            other => cx.complete(
                id,
                Err(CommandError::UnknownCommand {
                    command: other.into(),
                }),
            ),
        }
    }

    fn datagram(&mut self, cx: &mut Cx, _socket: Key, from: SocketAddr, data: &[u8]) {
        if from.ip() != self.to.ip() {
            return;
        }
        let Some((sent, t1, sent_at)) = self.in_flight else {
            return;
        };
        let Some(state) = decode(data, sent, t1, cx.unix_millis()) else {
            return;
        };
        self.in_flight = None;
        cx.cancel_timer(REPLY_TIMER);
        cx.round_trip(cx.now().saturating_sub(sent_at));
        self.misses = 0;
        cx.alive();
        if !self.reachable {
            self.reachable = true;
            cx.connection(Connection::Connected);
        }
        cx.state(state.clone());
        if self.monitor {
            self.stamps.answered(cx, "status");
        }
        for id in self.waiting.drain(..) {
            cx.complete(
                id,
                Ok(Outcome::Value {
                    value: state.clone(),
                }),
            );
        }
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        match key {
            REPLY_TIMER => {
                self.in_flight = None;
                for id in self.waiting.drain(..) {
                    cx.complete(id, Err(CommandError::Timeout));
                }
                self.misses += 1;
                if self.misses >= MISSES && self.reachable {
                    self.reachable = false;
                    cx.log(
                        Level::Warning,
                        "the NTP server answered none of three requests",
                    );
                    cx.connection(Connection::Disconnected {
                        reason: "no reply from the NTP server".into(),
                    });
                }
            }
            POLL_TIMER if self.monitor => {
                self.ask(cx);
                cx.set_timer(POLL_TIMER, self.poll_ms);
            }
            PROBE_TIMER => {
                self.ask(cx);
                cx.set_timer(PROBE_TIMER, PROBE_EVERY);
            }
            _ => {}
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        cx.udp_close(SOCKET);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{validate, Catalog};
    use crate::module::{Action, CommandResult};
    use std::net::{IpAddr, Ipv4Addr};

    const HOST: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 123));
    /// 2026-10-28T12:00:00Z.
    const NOW: u64 = 1_793_188_800_000;

    fn module(monitor: bool) -> GenericNtp {
        let spec = Catalog::source_tree().devices["generic-ntp"].clone();
        let settings = validate(&spec.settings, &Params::new()).unwrap();
        GenericNtp::new(OpenContext {
            host: HOST,
            host_name: None,
            port: None,
            model: "ntp-server".into(),
            channels: None,
            settings,
            monitor,
        })
        .unwrap()
    }

    /// A stratum 1 GPS server's answer to `req`: received 10 ms after it
    /// was sent by its clock, which runs 5 ms ahead, answered 1 ms later.
    fn answer(req: &[u8], t1: u64) -> Vec<u8> {
        let mut p = vec![0u8; 48];
        p[0] = 0x24; // LI 0, version 4, mode 4
        p[1] = 1;
        p[2] = 6;
        p[3] = 0xEC; // 2^-20 s
        p[4..8].copy_from_slice(&0x0000_0010u32.to_be_bytes());
        p[8..12].copy_from_slice(&0x0000_0020u32.to_be_bytes());
        p[12..16].copy_from_slice(b"GPS\0");
        p[16..24].copy_from_slice(&ntp_timestamp(t1 - 1000).to_be_bytes());
        p[24..32].copy_from_slice(&req[40..48]);
        p[32..40].copy_from_slice(&ntp_timestamp(t1 + 10 + 5).to_be_bytes());
        p[40..48].copy_from_slice(&ntp_timestamp(t1 + 11 + 5).to_be_bytes());
        p
    }

    fn sent(actions: &[Action]) -> Vec<Vec<u8>> {
        actions
            .iter()
            .filter_map(|a| match a {
                Action::UdpSend { data, to, .. } => {
                    assert_eq!(*to, SocketAddr::new(HOST, 123));
                    Some(data.clone())
                }
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

    #[test]
    fn the_request_is_a_version_4_client_packet() {
        let p = request(NOW);
        assert_eq!(p[0], 0x23);
        assert!(p[1..40].iter().all(|b| *b == 0));
        assert_eq!(
            u32::from_be_bytes(p[40..44].try_into().unwrap()),
            (NOW / 1000 + NTP_UNIX) as u32
        );
    }

    #[test]
    fn an_answer_gives_offset_delay_and_the_server_state() {
        let mut m = module(true);
        let mut cx = Cx::at(0, NOW);
        m.start(&mut cx);
        let req = sent(&cx.take()).remove(0);
        let mut cx = Cx::at(12, NOW + 12);
        m.datagram(
            &mut cx,
            SOCKET,
            SocketAddr::new(HOST, 123),
            &answer(&req, NOW),
        );
        let a = cx.take();
        assert!(a.contains(&Action::Connection(Connection::Connected)));
        assert!(a.contains(&Action::RoundTrip(12)));
        let state = a
            .iter()
            .find_map(|x| match x {
                Action::State(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(state["stratum"], json!(1));
        assert_eq!(state["reference_id"], json!("GPS"));
        assert_eq!(state["leap"], json!("none"));
        assert_eq!(state["synchronized"], json!(true));
        assert_eq!(state["precision_exponent"], json!(-20));
        // Offset ((15 - 0) + (16 - 12)) / 2 = 9.5 ms; delay 12 - 1 = 11 ms.
        let offset = state["offset_ms"].as_f64().unwrap();
        let delay = state["delay_ms"].as_f64().unwrap();
        assert!((offset - 9.5).abs() < 0.01, "{offset}");
        assert!((delay - 11.0).abs() < 0.01, "{delay}");
    }

    #[test]
    fn a_stray_or_kiss_packet_is_handled() {
        let mut m = module(false);
        let mut cx = Cx::at(0, NOW);
        m.start(&mut cx);
        let req = sent(&cx.take()).remove(0);
        // Another request's answer is not this one's.
        let mut other = answer(&req, NOW);
        other[24] ^= 1;
        let mut cx = Cx::at(5, NOW + 5);
        m.datagram(&mut cx, SOCKET, SocketAddr::new(HOST, 123), &other);
        assert!(!cx.take().iter().any(|a| matches!(a, Action::State(_))));
        // A kiss-o'-death: stratum 0, RATE.
        let mut kiss = answer(&req, NOW);
        kiss[1] = 0;
        kiss[12..16].copy_from_slice(b"RATE");
        let mut cx = Cx::at(6, NOW + 6);
        m.command(&mut cx, 1, "get_status", &Params::new());
        m.datagram(&mut cx, SOCKET, SocketAddr::new(HOST, 123), &kiss);
        let a = cx.take();
        match completed(&a, 1) {
            Some(Ok(Outcome::Value { value })) => {
                assert_eq!(value["kiss_code"], json!("RATE"));
                assert_eq!(value["synchronized"], json!(false));
                assert!(value.get("offset_ms").is_none());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn opened_for_commands_only_it_does_not_poll() {
        let mut m = module(false);
        let mut cx = Cx::at(0, NOW);
        m.start(&mut cx);
        let a = cx.take();
        assert!(!a.iter().any(|x| matches!(
            x,
            Action::SetTimer {
                key: POLL_TIMER,
                ..
            }
        )));
        assert!(a.iter().any(|x| matches!(
            x,
            Action::SetTimer {
                key: PROBE_TIMER,
                ..
            }
        )));
    }
}
