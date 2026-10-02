//! Finding PJLink Class 2 projectors and displays.
//!
//! PJLink Specifications Version 2.10, Class 2 (JBMIA), §3.2 and §3.3: a
//! controller broadcasts `%2SRCH` to UDP 4352; every Class 2 projector answers
//! within 0-10 s with `%2ACKN=xx:xx:xx:xx:xx:xx` (its MAC address) sent to UDP
//! 4352 of the controller, which collects answers for 30 s. A projector also
//! sends `%2LKUP=<MAC>` to a controller address registered on it when its
//! PJLink becomes ready. Neither needs authentication (§6).
//!
//! Rules, in the spirit of MCP and SSDP discovery's:
//!
//! - UDP 4352 is the one socket the PJLink module receives notifications on
//!   (`Bind::Shared`, routed by source address). Discovery is its fallback: it
//!   hears only datagrams from addresses no open device claims, so an open
//!   projector's notifications still reach its device session. The other side
//!   of that rule: an open projector with notifications on answers SRCH into
//!   its own session, not into discovery, so a scan does not report it (the
//!   host already has it by address).
//! - A scan sends `%2SRCH` to the subnet broadcast address of every usable
//!   interface, once to 255.255.255.255 (whose interface the OS picks), and by
//!   unicast to each hint (at most 32; unicast SRCH is not in the
//!   specification, but reaches projectors across a router that answer it),
//!   three times one second apart: at most 3 x (interfaces + 33) datagrams.
//!   It then collects answers for 30 s from the first send. A scan requested
//!   while one is collecting is ignored.
//! - Only `%2ACKN` and `%2LKUP` lines carrying a well-formed MAC address are
//!   devices. A SRCH (ours, echoed back by the broadcast, or another
//!   controller's) and status notifications (ERST, POWR, INPT, which carry no
//!   identity) are not.
//! - ACKN means Class 2 (Class 1 has no search), so the model is `class-2`.
//!   Each address is reported once, and again only if its MAC address
//!   changes; at most 256 addresses are remembered until discovery stops.
//! - No TCP connection is made. Querying CLSS, NAME, INF1 and INF2 would name
//!   the projector, but many projectors accept one connection at a time (§6
//!   "Simultaneous connection"), so a scan connecting to every projector it
//!   finds would interrupt whichever controller (another system, or a device
//!   session in this core) is polling it at that moment. Opening the device
//!   reads its identity.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::events::Event;
use crate::session::Services;

pub(crate) const PROTOCOL: &str = "pjlink";
/// UDP 4352 (§3.2, §3.3), and TCP 4352 where a found projector is opened.
const PORT: u16 = 4352;
const SPEC: &str = "pjlink";
const MODEL: &str = "class-2";
const SRCH: &[u8] = b"%2SRCH\r";
const LIMITED_BROADCAST: Ipv4Addr = Ipv4Addr::BROADCAST;

const SEARCH_REPEATS: usize = 3;
const SEARCH_SPACING: Duration = Duration::from_secs(1);
/// How long a controller collects ACKN after searching (§3.2).
const COLLECT: Duration = Duration::from_secs(30);
const MAX_HINTS: usize = 32;
const MAX_KNOWN: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Via {
    /// `%2ACKN`, answering a search.
    Ackn,
    /// `%2LKUP`, announcing PJLink is ready.
    Lkup,
}

impl Via {
    fn as_str(self) -> &'static str {
        match self {
            Via::Ackn => "ACKN",
            Via::Lkup => "LKUP",
        }
    }
}

/// The ACKN or LKUP in `data`, with its MAC address in lower case, or `None`
/// for anything else. Lines end in CR (§2.1); LF and CRLF are accepted.
fn parse_datagram(data: &[u8]) -> Option<(Via, String)> {
    let text = String::from_utf8_lossy(data);
    text.split(['\r', '\n']).find_map(parse_line)
}

fn parse_line(line: &str) -> Option<(Via, String)> {
    let line = line.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    let rest = line.strip_prefix("%2")?;
    let (body, param) = rest.split_once('=')?;
    let via = if body.eq_ignore_ascii_case("ACKN") {
        Via::Ackn
    } else if body.eq_ignore_ascii_case("LKUP") {
        Via::Lkup
    } else {
        return None;
    };
    Some((via, parse_mac(param)?))
}

/// `xx:xx:xx:xx:xx:xx`, six pairs of hexadecimal digits (§3.2).
fn parse_mac(param: &str) -> Option<String> {
    let parts: Vec<&str> = param.trim().split(':').collect();
    let ok = parts.len() == 6
        && parts
            .iter()
            .all(|p| p.len() == 2 && p.bytes().all(|b| b.is_ascii_hexdigit()));
    ok.then(|| parts.join(":").to_ascii_lowercase())
}

/// Where a scan sends SRCH: each interface's subnet broadcast (none for a
/// /31 or /32, which have none), 255.255.255.255 when there is any
/// interface, and the hints not already among those, at most `MAX_HINTS`.
/// Returns the targets and how many hints were left out.
fn search_targets(
    interfaces: &[(Ipv4Addr, Ipv4Addr)],
    hints: &[Ipv4Addr],
) -> (Vec<Ipv4Addr>, usize) {
    let mut targets = Vec::new();
    for (ip, mask) in interfaces {
        if u32::from(*mask) >= 0xffff_fffe {
            continue;
        }
        let b = Ipv4Addr::from(u32::from(*ip) | !u32::from(*mask));
        if !targets.contains(&b) {
            targets.push(b);
        }
    }
    if !interfaces.is_empty() {
        targets.push(LIMITED_BROADCAST);
    }
    let own: Vec<Ipv4Addr> = interfaces.iter().map(|(ip, _)| *ip).collect();
    let mut unique: Vec<Ipv4Addr> = Vec::new();
    for h in hints {
        if !unique.contains(h) && !own.contains(h) && !targets.contains(h) {
            unique.push(*h);
        }
    }
    let skipped = unique.len().saturating_sub(MAX_HINTS);
    unique.truncate(MAX_HINTS);
    targets.extend(unique);
    (targets, skipped)
}

/// The projectors one discovery session (listen to stop) has reported.
#[derive(Default)]
struct Known {
    macs: HashMap<IpAddr, String>,
    full_reported: bool,
}

enum Seen {
    New,
    Repeat,
    Full { first: bool },
}

impl Known {
    fn see(&mut self, address: IpAddr, mac: &str) -> Seen {
        let full = self.macs.len() >= MAX_KNOWN;
        match self.macs.get_mut(&address) {
            Some(m) if m == mac => Seen::Repeat,
            Some(m) => {
                *m = mac.to_string();
                Seen::New
            }
            None if full => {
                let first = !self.full_reported;
                self.full_reported = true;
                Seen::Full { first }
            }
            None => {
                self.macs.insert(address, mac.to_string());
                Seen::New
            }
        }
    }
}

#[derive(Default)]
struct State {
    listener: Option<JoinHandle<()>>,
    scan: Option<JoinHandle<()>>,
}

#[derive(Default)]
pub(crate) struct PjLinkDiscovery {
    state: Mutex<State>,
}

impl PjLinkDiscovery {
    /// Hear ACKN and LKUP on UDP 4352. If the port cannot be had, that is
    /// reported and not an error, so other protocols in the same request
    /// still run; a scan then sends nothing, since answers come to 4352.
    pub(crate) fn listen(&self, services: &Arc<Services>) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        if state.listener.is_some() {
            return Ok(());
        }
        let (tx, mut rx) = mpsc::channel::<(SocketAddr, Vec<u8>)>(1024);
        if let Err(e) = services
            .shared_udp
            .set_fallback(services.bind_address, PORT, Some(tx))
        {
            log(
                services,
                format!(
                    "cannot listen on UDP {PORT} ({e}); another program may hold it exclusively. \
                     PJLink projectors can still be added by address"
                ),
            );
            return Ok(());
        }
        let events = services.events.clone();
        state.listener = Some(tokio::spawn(async move {
            let mut known = Known::default();
            while let Some((from, data)) = rx.recv().await {
                let Some((via, mac)) = parse_datagram(&data) else {
                    continue;
                };
                match known.see(from.ip(), &mac) {
                    Seen::New => events.push(discovered(from.ip(), &mac, via)),
                    Seen::Repeat => {}
                    Seen::Full { first } => {
                        if first {
                            events.push(Event::Discovery {
                                protocol: PROTOCOL.into(),
                                message: format!(
                                    "more than {MAX_KNOWN} projectors answered; further ones are \
                                     ignored until discovery is stopped"
                                ),
                            });
                        }
                    }
                }
            }
        }));
        Ok(())
    }

    pub(crate) fn scan(&self, services: &Arc<Services>, hints: &[Ipv4Addr]) {
        let mut state = self.state.lock().unwrap();
        if state.listener.is_none() || state.scan.as_ref().is_some_and(|t| !t.is_finished()) {
            return;
        }
        let interfaces = crate::discovery::interfaces(services.bind_address);
        let (targets, skipped) = search_targets(&interfaces, hints);
        if skipped > 0 {
            log(
                services,
                format!("{skipped} hints beyond the first {MAX_HINTS} are not searched by unicast"),
            );
        }
        if targets.is_empty() {
            log(
                services,
                "no IPv4 interface to search on, and no hints".into(),
            );
            return;
        }
        let services = services.clone();
        state.scan = Some(tokio::spawn(async move {
            let started = tokio::time::Instant::now();
            for round in 0..SEARCH_REPEATS {
                for t in &targets {
                    let to = SocketAddr::new(IpAddr::V4(*t), PORT);
                    let _ = services.shared_udp.send(PORT, to, SRCH).await;
                }
                if round + 1 < SEARCH_REPEATS {
                    tokio::time::sleep(SEARCH_SPACING).await;
                }
            }
            // Answers arrive at the listener; this holds off another scan
            // until the collection time is over.
            tokio::time::sleep_until(started + COLLECT).await;
            log(
                &services,
                format!("search ended after {} s", COLLECT.as_secs()),
            );
        }));
    }

    /// Stop listening and searching, and forget every projector, so the next
    /// session reports them again.
    pub(crate) fn stop(&self, services: &Arc<Services>) {
        let mut state = self.state.lock().unwrap();
        if let Some(t) = state.scan.take() {
            t.abort();
        }
        if let Some(t) = state.listener.take() {
            t.abort();
            let _ = services
                .shared_udp
                .set_fallback(services.bind_address, PORT, None);
        }
    }
}

fn log(services: &Services, message: String) {
    services.events.push(Event::Discovery {
        protocol: PROTOCOL.into(),
        message,
    });
}

fn discovered(address: IpAddr, mac: &str, via: Via) -> Event {
    Event::Discovered {
        protocol: PROTOCOL.into(),
        address: address.to_string(),
        port: PORT,
        device: SPEC.into(),
        models: vec![MODEL.into()],
        name: None,
        evidence: json!({
            "mac": mac,
            "via": via.as_str(),
            "protocol": match via {
                Via::Ackn => "answered a PJLink %2SRCH with %2ACKN on UDP 4352 (§3.2)",
                _ => "announced %2LKUP (PJLink ready) to UDP 4352 (§3.3)",
            },
            "class": "2: only Class 2 projectors search and notify; the projector's CLSS answer, read when it is opened, is authoritative",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_lists_what_this_protocol_reports() {
        assert!(crate::discovery::PROTOCOLS.contains(&(PROTOCOL, &[SPEC][..])));
    }

    #[test]
    fn ackn_and_lkup_are_devices_and_nothing_else_is() {
        assert_eq!(
            parse_datagram(b"%2ACKN=00:11:22:AA:bb:Cc\r"),
            Some((Via::Ackn, "00:11:22:aa:bb:cc".into()))
        );
        assert_eq!(
            parse_datagram(b"%2lkup=00:11:22:33:44:55\r\n"),
            Some((Via::Lkup, "00:11:22:33:44:55".into()))
        );
        assert_eq!(
            parse_datagram(b"%2ACKN=00:11:22:33:44:55"),
            Some((Via::Ackn, "00:11:22:33:44:55".into())),
            "an unterminated line is still read"
        );
        // A search (ours echoed back, or another controller's) is not a device.
        assert_eq!(parse_datagram(SRCH), None);
        // Status notifications carry no identity.
        assert_eq!(parse_datagram(b"%2POWR=1\r"), None);
        assert_eq!(parse_datagram(b"%2ERST=000000\r"), None);
        // Class 1 has no search; malformed MAC addresses are refused.
        assert_eq!(parse_datagram(b"%1ACKN=00:11:22:33:44:55\r"), None);
        assert_eq!(parse_datagram(b"%2ACKN=00:11:22:33:44\r"), None);
        assert_eq!(parse_datagram(b"%2ACKN=00-11-22-33-44-55\r"), None);
        assert_eq!(parse_datagram(b"%2ACKN=0g:11:22:33:44:55\r"), None);
        assert_eq!(parse_datagram(b"%2ACKN=ERR3\r"), None);
        assert_eq!(parse_datagram(b"\xff\xfe"), None);
    }

    #[test]
    fn a_scan_searches_each_subnet_and_bounded_hints() {
        let ifaces = [
            (Ipv4Addr::new(10, 2, 0, 5), Ipv4Addr::new(255, 255, 0, 0)),
            (Ipv4Addr::new(10, 2, 0, 6), Ipv4Addr::new(255, 255, 0, 0)),
            (
                Ipv4Addr::new(100, 64, 0, 1),
                Ipv4Addr::new(255, 255, 255, 255),
            ),
        ];
        let hints = [
            Ipv4Addr::new(10, 9, 0, 7),
            Ipv4Addr::new(10, 9, 0, 7),
            Ipv4Addr::new(10, 2, 0, 5),
        ];
        let (targets, skipped) = search_targets(&ifaces, &hints);
        assert_eq!(
            targets,
            [
                Ipv4Addr::new(10, 2, 255, 255),
                LIMITED_BROADCAST,
                Ipv4Addr::new(10, 9, 0, 7)
            ],
            "one broadcast per subnet, none for a /32, the server's own address left out"
        );
        assert_eq!(skipped, 0);

        let many: Vec<Ipv4Addr> = (1..=40).map(|n| Ipv4Addr::new(10, 3, 0, n)).collect();
        let (targets, skipped) = search_targets(&[], &many);
        assert_eq!(
            targets.len(),
            MAX_HINTS,
            "no broadcast without an interface"
        );
        assert_eq!(skipped, 8);
    }

    #[test]
    fn a_scan_is_bounded() {
        // Four interfaces on separate subnets and far too many hints: one
        // broadcast each, the limited broadcast, 32 hints, three rounds.
        let ifaces: Vec<(Ipv4Addr, Ipv4Addr)> = (1..=4)
            .map(|n| (Ipv4Addr::new(10, n, 0, 1), Ipv4Addr::new(255, 255, 255, 0)))
            .collect();
        let hints: Vec<Ipv4Addr> = (1..=200).map(|n| Ipv4Addr::new(10, 9, 0, n)).collect();
        let (targets, _) = search_targets(&ifaces, &hints);
        assert_eq!(targets.len(), 4 + 1 + MAX_HINTS);
        assert!(targets.len() * SEARCH_REPEATS <= 111, "datagrams per scan");
        // The search rounds end well within the projectors' 0-10 s to answer,
        // and answers are collected for the specification's 30 s.
        const {
            assert!(SEARCH_SPACING.as_secs() * (SEARCH_REPEATS as u64 - 1) < 10);
            assert!(COLLECT.as_secs() == 30);
        };
    }

    #[test]
    fn each_address_is_reported_once_and_memory_is_bounded() {
        let mut known = Known::default();
        let a = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        assert!(matches!(known.see(a, "00:11:22:33:44:55"), Seen::New));
        assert!(matches!(known.see(a, "00:11:22:33:44:55"), Seen::Repeat));
        assert!(matches!(known.see(a, "00:11:22:33:44:66"), Seen::New));
        for n in 1..MAX_KNOWN as u32 {
            let ip = IpAddr::V4(Ipv4Addr::from(0x0a01_0000 + n));
            assert!(matches!(known.see(ip, "00:00:00:00:00:01"), Seen::New));
        }
        let over = IpAddr::V4(Ipv4Addr::new(10, 9, 9, 9));
        assert!(matches!(
            known.see(over, "00:00:00:00:00:02"),
            Seen::Full { first: true }
        ));
        assert!(matches!(
            known.see(over, "00:00:00:00:00:02"),
            Seen::Full { first: false }
        ));
        assert!(matches!(known.see(a, "00:11:22:33:44:66"), Seen::Repeat));
    }

    #[test]
    fn the_event_carries_the_mac_and_how_it_was_learnt() {
        let Event::Discovered {
            protocol,
            address,
            port,
            device,
            models,
            name,
            evidence,
        } = discovered(
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)),
            "00:11:22:33:44:55",
            Via::Lkup,
        )
        else {
            panic!("expected Discovered");
        };
        assert_eq!(
            (protocol.as_str(), address.as_str(), port, device.as_str()),
            ("pjlink", "10.0.0.9", 4352, "pjlink")
        );
        assert_eq!(models, ["class-2"]);
        assert_eq!(name, None);
        assert_eq!(evidence["mac"], "00:11:22:33:44:55");
        assert_eq!(evidence["via"], "LKUP");
    }
}
