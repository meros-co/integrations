//! Finding DMXControl 3's Umbra servers.
//!
//! Umbra announces itself with UDP datagrams to the multicast group
//! 225.68.67.3 on port 17474 (DMXControl's network interface repository,
//! csharp/src/NetConstants.cs: "225.D.C.3"). Each datagram is one
//! `UmbraUdpBroadcast` protobuf message and nothing else
//! (csharp/src/UmbraDiscoveryClient.cs reads it whole with
//! `CodedInputStream`): Umbra's program and client information (client name,
//! host name, IP addresses, network ID, runtime ID, broker ports, version).
//!
//! Rules, in the spirit of the other discovery protocols:
//!
//! - Discovery listens on UDP 17474, bound so other programs on the host
//!   (DMXControl's own GUI and Kernel) can listen there too (SO_REUSEADDR,
//!   and SO_REUSEPORT where the system has it), and joins the group on every
//!   usable IPv4 interface, as the reference client does. A core bound to a
//!   loopback address listens there alone.
//! - Umbra has no query to answer: a scan is listening. Umbras that announce
//!   themselves are reported as they are heard.
//! - Each Umbra is reported once by runtime ID, and again only when its
//!   address, port or name changes; at most 256 are remembered until
//!   discovery stops. A datagram that is not an UmbraUdpBroadcast with a
//!   runtime ID is ignored.
//! - The address reported is the datagram's source, and the port Umbra's
//!   broker port as it announces it (17475 when it gives none). No connection
//!   is made: opening the device logs in.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use socket2::{Domain, Protocol, SockRef, Socket, Type};
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

use crate::events::Event;
use crate::modules::dmxcontrol3_schema::SCHEMA;
use crate::session::Services;

pub(crate) const PROTOCOL: &str = "umbra";
/// DMXCNET_MULTICAST_PORT and DMXCNET_MULTICAST_ADDRESS.
const PORT: u16 = 17474;
const GROUP: Ipv4Addr = Ipv4Addr::new(225, 68, 67, 3);
/// UMBRA_LISTEN_PORT, for an Umbra that announces no port.
const UMBRA_PORT: u16 = 17475;
const SPEC: &str = "dmxcontrol3";
const MODEL: &str = "dmxcontrol3";
const MAX_KNOWN: usize = 256;

/// What a datagram says about one Umbra.
#[derive(Debug, Clone, PartialEq)]
struct Found {
    runtime_id: String,
    name: String,
    port: u16,
    evidence: Value,
}

fn parse(data: &[u8]) -> Option<Found> {
    let v = SCHEMA
        .decode("LumosProtobuf.UmbraUdpBroadcast", data)
        .ok()?;
    let info = &v["umbra_server"];
    let c = &info["client_info"];
    let runtime_id = c["runtimeid"]
        .as_str()
        .filter(|s| !s.is_empty())?
        .to_string();
    let port = c["umbra_port"]
        .as_i64()
        .filter(|p| (1..=65535).contains(p))
        .map(|p| p as u16)
        .unwrap_or(UMBRA_PORT);
    Some(Found {
        runtime_id: runtime_id.clone(),
        name: c["clientname"].as_str().unwrap_or("").to_string(),
        port,
        evidence: json!({
            "runtime_id": runtime_id,
            "network_id": c["networkid"],
            "hostname": c["hostname"],
            "ips": c["ips"],
            "project_name": c["project_name"],
            "secure_port": c["umbra_secure_port"],
            "program_version": info["program_info"]["program_version"].as_str().unwrap_or(""),
            "protocol": "an UmbraUdpBroadcast on UDP 17474 (multicast 225.68.67.3), as DMXControl's Umbra announces itself",
        }),
    })
}

#[derive(Default)]
struct Known {
    seen: HashMap<String, (IpAddr, u16, String)>,
    full_reported: bool,
}

enum Seen {
    New,
    Repeat,
    Full { first: bool },
}

impl Known {
    fn see(&mut self, address: IpAddr, found: &Found) -> Seen {
        let now = (address, found.port, found.name.clone());
        let full = self.seen.len() >= MAX_KNOWN;
        match self.seen.get_mut(&found.runtime_id) {
            Some(old) if *old == now => Seen::Repeat,
            Some(old) => {
                *old = now;
                Seen::New
            }
            None if full => {
                let first = !self.full_reported;
                self.full_reported = true;
                Seen::Full { first }
            }
            None => {
                self.seen.insert(found.runtime_id.clone(), now);
                Seen::New
            }
        }
    }
}

#[derive(Default)]
struct State {
    listener: Option<(Arc<UdpSocket>, JoinHandle<()>)>,
    joined: HashSet<Ipv4Addr>,
}

#[derive(Default)]
pub(crate) struct UmbraDiscovery {
    state: Mutex<State>,
}

impl UmbraDiscovery {
    /// Listen on UDP 17474 and join the group. A port that cannot be had is
    /// reported, not an error, so other protocols in the same request run.
    pub(crate) fn listen(&self, services: &Arc<Services>) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        if state.listener.is_none() {
            let socket = match bind(services.bind_address) {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    log(services, format!("cannot listen on UDP {PORT} ({e}); Umbra servers can still be added by address"));
                    return Ok(());
                }
            };
            let events = services.events.clone();
            let receiver = socket.clone();
            let task = tokio::spawn(async move {
                let mut known = Known::default();
                let mut buf = vec![0u8; 65_536];
                loop {
                    match receiver.recv_from(&mut buf).await {
                        Ok((n, from)) => {
                            let Some(found) = parse(&buf[..n]) else {
                                continue;
                            };
                            match known.see(from.ip(), &found) {
                                Seen::New => events.push(discovered(from.ip(), found)),
                                Seen::Repeat => {}
                                Seen::Full { first: true } => events.push(Event::Discovery {
                                    protocol: PROTOCOL.into(),
                                    message: format!(
                                        "more than {MAX_KNOWN} Umbra servers announced themselves; further ones are \
                                         ignored until discovery is stopped"
                                    ),
                                }),
                                Seen::Full { first: false } => {}
                            }
                        }
                        // Windows reports an ICMP port unreachable from an earlier send.
                        Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => continue,
                        Err(_) => return,
                    }
                }
            });
            state.listener = Some((socket, task));
            state.joined.clear();
        }
        if services.bind_address.is_loopback() {
            return Ok(());
        }
        let socket = state
            .listener
            .as_ref()
            .map(|(s, _)| s.clone())
            .expect("listening");
        for (ip, _) in crate::discovery::interfaces(services.bind_address) {
            if state.joined.contains(&ip) {
                continue;
            }
            match SockRef::from(&*socket).join_multicast_v4(&GROUP, &ip) {
                Ok(()) => {
                    state.joined.insert(ip);
                }
                Err(e) => log(
                    services,
                    format!("cannot join {GROUP} on {ip} ({e}); Umbra servers announcing there will not be heard"),
                ),
            }
        }
        Ok(())
    }

    /// Umbra answers no query: a scan listens.
    pub(crate) fn scan(&self, services: &Arc<Services>) -> Result<(), String> {
        self.listen(services)?;
        log(
            services,
            "Umbra servers announce themselves and answer no query; they are reported as they are heard".into(),
        );
        Ok(())
    }

    /// Stop listening, and forget every Umbra, so the next session reports
    /// them again.
    pub(crate) fn stop(&self) {
        let mut state = self.state.lock().unwrap();
        if let Some((_, task)) = state.listener.take() {
            task.abort();
        }
        state.joined.clear();
    }
}

/// UDP 17474, shared with DMXControl's own programs on this host.
fn bind(bind: IpAddr) -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(all(
        unix,
        not(any(target_os = "solaris", target_os = "illumos", target_os = "cygwin"))
    ))]
    let _ = socket.set_reuse_port(true);
    socket.set_nonblocking(true)?;
    let address = if bind.is_loopback() {
        bind
    } else {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    };
    socket.bind(&SocketAddr::new(address, PORT).into())?;
    UdpSocket::from_std(socket.into())
}

fn log(services: &Services, message: String) {
    services.events.push(Event::Discovery {
        protocol: PROTOCOL.into(),
        message,
    });
}

fn discovered(address: IpAddr, found: Found) -> Event {
    Event::Discovered {
        protocol: PROTOCOL.into(),
        address: address.to_string(),
        port: found.port,
        device: SPEC.into(),
        models: vec![MODEL.into()],
        name: (!found.name.is_empty()).then_some(found.name),
        evidence: found.evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An UmbraUdpBroadcast stated by hand from the message's fields:
    /// umbraServer (2) { clientInfo (3) { clientname (4) "Umbra-1",
    /// networkid (20) "LIGHT-Net", runtimeid (21) "r-1", umbraPort (30) 17475 } }.
    fn datagram(runtime_id: &str, port: u16) -> Vec<u8> {
        let mut info = Vec::new();
        info.extend_from_slice(&[0x22, 7]);
        info.extend_from_slice(b"Umbra-1");
        info.extend_from_slice(&[0xa2, 0x01, 9]);
        info.extend_from_slice(b"LIGHT-Net");
        info.extend_from_slice(&[0xaa, 0x01, runtime_id.len() as u8]);
        info.extend_from_slice(runtime_id.as_bytes());
        info.extend_from_slice(&[0xf0, 0x01]);
        crate::protobuf::varint(u64::from(port), &mut info);
        let mut program = vec![0x1a, info.len() as u8];
        program.extend(info);
        let mut out = vec![0x12, program.len() as u8];
        out.extend(program);
        out
    }

    #[test]
    fn discovery_lists_what_this_protocol_reports() {
        assert!(crate::discovery::PROTOCOLS.contains(&(PROTOCOL, &[SPEC][..])));
    }

    #[test]
    fn an_announcement_names_the_umbra_and_its_port() {
        let found = parse(&datagram("r-1", 17475)).unwrap();
        assert_eq!(
            (found.runtime_id.as_str(), found.name.as_str(), found.port),
            ("r-1", "Umbra-1", 17475)
        );
        assert_eq!(found.evidence["network_id"], "LIGHT-Net");
        assert_eq!(
            parse(&datagram("r-1", 0)).unwrap().port,
            UMBRA_PORT,
            "no port announced"
        );
        assert_eq!(parse(&datagram("", 17475)), None, "no runtime ID");
        assert_eq!(parse(b"\xff\xff\xff"), None);
        assert_eq!(parse(b""), None);
    }

    #[test]
    fn each_umbra_is_reported_once_and_memory_is_bounded() {
        let mut known = Known::default();
        let a = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let found = parse(&datagram("r-1", 17475)).unwrap();
        assert!(matches!(known.see(a, &found), Seen::New));
        assert!(matches!(known.see(a, &found), Seen::Repeat));
        let moved = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        assert!(
            matches!(known.see(moved, &found), Seen::New),
            "a new address is reported"
        );
        for n in 1..MAX_KNOWN {
            let f = parse(&datagram(&format!("x{n}"), 17475)).unwrap();
            assert!(matches!(known.see(a, &f), Seen::New));
        }
        let over = parse(&datagram("over", 17475)).unwrap();
        assert!(matches!(known.see(a, &over), Seen::Full { first: true }));
        assert!(matches!(known.see(a, &over), Seen::Full { first: false }));
    }

    #[test]
    fn the_event_carries_the_address_heard_and_the_evidence() {
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
            parse(&datagram("r-1", 17475)).unwrap(),
        )
        else {
            panic!("expected Discovered");
        };
        assert_eq!(
            (protocol.as_str(), address.as_str(), port, device.as_str()),
            ("umbra", "10.0.0.9", 17475, "dmxcontrol3")
        );
        assert_eq!(models, ["dmxcontrol3"]);
        assert_eq!(name.as_deref(), Some("Umbra-1"));
        assert_eq!(evidence["runtime_id"], "r-1");
    }
}
