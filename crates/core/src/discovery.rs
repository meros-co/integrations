//! Finding devices on the network.
//!
//! The core decides how a scan is done, so no host can flood a venue's
//! network; the host decides when, which is product policy. Results are
//! events: what was found, where, which spec and models it can be, and what
//! the identification rests on.
//!
//! Sennheiser G3/G4 (MCP), from TI 1254 and RFDeck's discovery, which was
//! hardened on real rigs (RFDeck review items K and N):
//!
//! - Discovery listens on the same UDP 53212 socket as open G3/G4 devices:
//!   datagrams from addresses no device session claims come here. Two sockets
//!   on 53212 would each receive an undefined share.
//! - A scan broadcasts the probe on every interface, which reaches devices
//!   anywhere in the L2 domain, then sweeps by unicast only the /24s of the
//!   server's own interfaces and of addresses the host says devices were last
//!   seen at, at most eight. Walking a whole /16 once took a venue's network
//!   down: every empty address costs a broadcast ARP.
//! - The probe is a five-second subscription (`Push 5 500 3`) and `Name`, the
//!   least the protocol offers that makes a G3/G4 answer, paced at 32
//!   addresses per 50 ms.
//! - A request (a bare `Name` or `Push ...`, from us or another scanner) is
//!   not a device. A reply carrying a cycle telegram is.
//! - Identification never guesses: a G3 and a G4 cannot be told apart over
//!   MCP, so both receiver models are offered. Receiver or IEM transmitter is
//!   read from the telegrams (RF, RF1/RF2 and Bat are receiver-only; an IEM
//!   transmitter reports Af). The name is operator-set, so it is weak
//!   evidence of identity.
//!
//! Sony cameras (SSDP) are found by `crate::ssdp`, and PJLink Class 2
//! projectors (SRCH/ACKN and LKUP on UDP 4352) by `crate::pjlink_discovery`,
//! whose rules are there.
//! Protocols are independent: each listens, scans and stops on its own.

use std::collections::{BTreeSet, HashMap};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::catalog::Catalog;
use crate::events::Event;
use crate::session::Services;

pub const MCP_PORT: u16 = 53212;

/// Each discovery protocol and the specs whose devices it reports. A core
/// runs a protocol only when its catalogue holds one of them: a build
/// without the family, or a core started for other devices, has no use for
/// the protocol's traffic, and asking for it by name is an error.
pub const PROTOCOLS: &[(&str, &[&str])] = &[
    ("mcp", &[MCP_SPEC]),
    ("ssdp", &["sony-camera"]),
    ("pjlink", &["pjlink"]),
];
const MCP_SPEC: &str = "sennheiser-ew-g3-g4";
const MCP_PROBE: &str = "Push 5 500 3\r";
const MCP_NAME: &str = "Name\r";
const SWEEP_BATCH: usize = 32;
const SWEEP_PAUSE: Duration = Duration::from_millis(50);
const SWEEP_MAX_SUBNETS: usize = 8;

/// What a host asks of discovery.
#[derive(Debug, Clone, Deserialize)]
pub struct DiscoverRequest {
    /// `listen` (passively), `scan` (listen, and probe now) or `stop`.
    pub action: DiscoverAction,
    /// Which discovery protocols: `mcp` (Sennheiser G3/G4), `ssdp` (Sony
    /// cameras) and `pjlink` (PJLink Class 2 projectors). Empty means every
    /// protocol that finds a device in this core's catalogue; naming one that
    /// finds none of them is an error.
    #[serde(default)]
    pub protocols: Vec<String>,
    /// Addresses where devices were last seen. An MCP scan also sweeps their
    /// /24s; an SSDP scan also asks each by unicast M-SEARCH, which reaches
    /// cameras multicast does not (across a router); a PJLink scan also
    /// sends each a unicast `%2SRCH`.
    #[serde(default)]
    pub hints: Vec<Ipv4Addr>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverAction {
    Listen,
    Scan,
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Family {
    Receiver,
    IemTransmitter,
}

/// What one MCP datagram says about its sender.
#[derive(Debug, Default, PartialEq)]
struct McpReply {
    name: Option<String>,
    family: Option<Family>,
}

/// An MCP device's reply, or `None` for anything else (a request, or not
/// MCP at all).
fn classify(data: &[u8]) -> Option<McpReply> {
    let text = String::from_utf8_lossy(data);
    let trimmed = text.trim();
    if trimmed == "Name" || trimmed == "Push" || trimmed.starts_with("Push ") {
        return None;
    }
    let mut reply = McpReply::default();
    let mut is_mcp = false;
    for line in text.split(['\r', '\n']) {
        let line = line.trim();
        let (keyword, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        if rest.is_empty() {
            continue;
        }
        match keyword {
            "Name" => {
                is_mcp = true;
                reply.name = Some(rest.to_string());
            }
            "RF" | "RF1" | "RF2" | "Bat" => {
                is_mcp = true;
                reply.family = Some(Family::Receiver);
            }
            "Af" => {
                is_mcp = true;
                if reply.family.is_none() {
                    reply.family = Some(Family::IemTransmitter);
                }
            }
            "States" | "AF" | "Frequency" | "Msg" => is_mcp = true,
            _ => {}
        }
    }
    is_mcp.then_some(reply)
}

/// The broadcast addresses and the /24s a scan walks: each interface's own,
/// then each hint's not already covered, at most eight. The server's own
/// addresses are left out.
fn sweep_targets(
    interfaces: &[(Ipv4Addr, Ipv4Addr)],
    hints: &[Ipv4Addr],
) -> (Vec<Ipv4Addr>, Vec<Vec<Ipv4Addr>>, usize) {
    let own: BTreeSet<Ipv4Addr> = interfaces.iter().map(|(ip, _)| *ip).collect();
    let mut broadcasts = Vec::new();
    for (ip, mask) in interfaces {
        let b = Ipv4Addr::from(u32::from(*ip) | !u32::from(*mask));
        if !broadcasts.contains(&b) {
            broadcasts.push(b);
        }
    }
    let mut bases: Vec<[u8; 3]> = Vec::new();
    for ip in interfaces.iter().map(|(ip, _)| ip).chain(hints) {
        let o = ip.octets();
        let base = [o[0], o[1], o[2]];
        if !bases.contains(&base) {
            bases.push(base);
        }
    }
    let skipped = bases.len().saturating_sub(SWEEP_MAX_SUBNETS);
    bases.truncate(SWEEP_MAX_SUBNETS);
    let subnets = bases
        .iter()
        .map(|b| {
            (1..=254u8)
                .map(|h| Ipv4Addr::new(b[0], b[1], b[2], h))
                .filter(|a| !own.contains(a))
                .collect()
        })
        .collect();
    (broadcasts, subnets, skipped)
}

/// The IPv4 interfaces to use: the one the core is bound to, or every
/// non-loopback one.
pub(crate) fn interfaces(bind: IpAddr) -> Vec<(Ipv4Addr, Ipv4Addr)> {
    let Ok(all) = if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    all.into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.addr {
            if_addrs::IfAddr::V4(v4) => Some((v4.ip, v4.netmask)),
            _ => None,
        })
        .filter(|(ip, _)| bind.is_unspecified() || bind == IpAddr::V4(*ip))
        .collect()
}

#[derive(Default)]
struct State {
    listener: Option<JoinHandle<()>>,
    scan: Option<JoinHandle<()>>,
}

pub(crate) struct Discovery {
    state: Mutex<State>,
    #[cfg(feature = "sony")]
    ssdp: crate::ssdp::Ssdp,
    #[cfg(feature = "pjlink")]
    pjlink: crate::pjlink_discovery::PjLinkDiscovery,
    /// The protocols that find a device in the core's catalogue.
    available: Vec<&'static str>,
}

/// The protocols that find at least one device in `catalog`.
pub(crate) fn protocols_for(catalog: &Catalog) -> Vec<&'static str> {
    PROTOCOLS
        .iter()
        .filter(|(_, specs)| specs.iter().any(|s| catalog.device(s).is_some()))
        .map(|(protocol, _)| *protocol)
        .collect()
}

impl Discovery {
    pub(crate) fn new(catalog: &Catalog) -> Discovery {
        Discovery {
            state: Default::default(),
            #[cfg(feature = "sony")]
            ssdp: Default::default(),
            #[cfg(feature = "pjlink")]
            pjlink: Default::default(),
            available: protocols_for(catalog),
        }
    }

    pub(crate) fn protocols(&self) -> &[&'static str] {
        &self.available
    }

    /// Which protocols a request asks for, or why it cannot be met.
    fn wanted(&self, request: &DiscoverRequest) -> Result<Vec<&'static str>, String> {
        for p in &request.protocols {
            let Some((_, specs)) = PROTOCOLS.iter().find(|(name, _)| name == p) else {
                return Err(format!("unknown discovery protocol '{p}'"));
            };
            if !self.available.contains(&p.as_str()) {
                return Err(format!(
                    "discovery protocol '{p}' finds only {}, which this core does not include",
                    specs.join(", ")
                ));
            }
        }
        Ok(self
            .available
            .iter()
            .copied()
            .filter(|p| request.protocols.is_empty() || request.protocols.iter().any(|q| q == p))
            .collect())
    }

    /// Must be called from within the runtime.
    pub(crate) fn handle(
        &self,
        services: &Arc<Services>,
        request: DiscoverRequest,
    ) -> Result<(), String> {
        let wanted = self.wanted(&request)?;
        let wants = |p: &str| wanted.contains(&p);
        // A protocol whose family is not built in is never available, so its
        // branches below are compiled only with the family.
        let (mcp, ssdp, pjlink) = (wants("mcp"), wants("ssdp"), wants("pjlink"));
        let _ = (ssdp, pjlink);
        match request.action {
            DiscoverAction::Stop => {
                if mcp {
                    self.stop(services);
                }
                #[cfg(feature = "sony")]
                if ssdp {
                    self.ssdp.stop();
                }
                #[cfg(feature = "pjlink")]
                if pjlink {
                    self.pjlink.stop(services);
                }
                Ok(())
            }
            DiscoverAction::Listen => {
                if mcp {
                    self.listen(services)?;
                }
                #[cfg(feature = "sony")]
                if ssdp {
                    self.ssdp.listen(services)?;
                }
                #[cfg(feature = "pjlink")]
                if pjlink {
                    self.pjlink.listen(services)?;
                }
                Ok(())
            }
            DiscoverAction::Scan => {
                if mcp {
                    self.listen(services)?;
                    self.scan(services, request.hints.clone());
                }
                #[cfg(feature = "sony")]
                if ssdp {
                    self.ssdp.listen(services)?;
                    self.ssdp.scan(services, &request.hints)?;
                }
                #[cfg(feature = "pjlink")]
                if pjlink {
                    self.pjlink.listen(services)?;
                    self.pjlink.scan(services, &request.hints);
                }
                Ok(())
            }
        }
    }

    fn listen(&self, services: &Arc<Services>) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();
        if state.listener.is_some() {
            return Ok(());
        }
        let (tx, mut rx) = mpsc::channel::<(SocketAddr, Vec<u8>)>(1024);
        services
            .shared_udp
            .set_fallback(services.bind_address, MCP_PORT, Some(tx))?;
        let events = services.events.clone();
        state.listener = Some(tokio::spawn(async move {
            let mut seen: HashMap<IpAddr, (Option<String>, Option<Family>)> = HashMap::new();
            while let Some((from, data)) = rx.recv().await {
                let Some(reply) = classify(&data) else {
                    continue;
                };
                let record = seen.entry(from.ip()).or_default();
                let mut changed = record.0.is_none() && record.1.is_none();
                if reply.name.is_some() && reply.name != record.0 {
                    record.0 = reply.name;
                    changed = true;
                }
                if reply.family.is_some() && reply.family != record.1 {
                    record.1 = reply.family;
                    changed = true;
                }
                if changed {
                    events.push(discovered(from.ip(), &record.0, record.1));
                }
            }
        }));
        Ok(())
    }

    fn scan(&self, services: &Arc<Services>, hints: Vec<Ipv4Addr>) {
        let mut state = self.state.lock().unwrap();
        if state.scan.as_ref().is_some_and(|t| !t.is_finished()) {
            return;
        }
        let services = services.clone();
        state.scan = Some(tokio::spawn(async move {
            let ifaces = interfaces(services.bind_address);
            let (broadcasts, subnets, skipped) = sweep_targets(&ifaces, &hints);
            if skipped > 0 {
                log(
                    &services,
                    format!(
                        "devices span more than {SWEEP_MAX_SUBNETS} /24s; {skipped} not swept by \
                         unicast (broadcast still reaches them, and devices can be added by address)"
                    ),
                );
            }
            for b in &broadcasts {
                for probe in [MCP_PROBE, MCP_NAME] {
                    let to = SocketAddr::new(IpAddr::V4(*b), MCP_PORT);
                    let _ = services
                        .shared_udp
                        .send(MCP_PORT, to, probe.as_bytes())
                        .await;
                }
            }
            let mut in_batch = 0;
            for address in subnets.iter().flatten() {
                for probe in [MCP_PROBE, MCP_NAME] {
                    let to = SocketAddr::new(IpAddr::V4(*address), MCP_PORT);
                    let _ = services
                        .shared_udp
                        .send(MCP_PORT, to, probe.as_bytes())
                        .await;
                }
                in_batch += 1;
                if in_batch == SWEEP_BATCH {
                    in_batch = 0;
                    tokio::time::sleep(SWEEP_PAUSE).await;
                }
            }
        }));
    }

    fn stop(&self, services: &Arc<Services>) {
        let mut state = self.state.lock().unwrap();
        if let Some(t) = state.scan.take() {
            t.abort();
        }
        if let Some(t) = state.listener.take() {
            t.abort();
            let _ = services
                .shared_udp
                .set_fallback(services.bind_address, MCP_PORT, None);
        }
    }
}

fn log(services: &Services, message: String) {
    services.events.push(Event::Discovery {
        protocol: "mcp".into(),
        message,
    });
}

fn discovered(address: IpAddr, name: &Option<String>, family: Option<Family>) -> Event {
    let (models, family_evidence) = match family {
        Some(Family::Receiver) => (
            vec!["em-300-500-g4", "em-300-500-g3"],
            "receiver: it reports RF, RF1/RF2 or Bat telegrams. G3 and G4 cannot be told apart over MCP",
        ),
        Some(Family::IemTransmitter) => (
            vec!["sr-iem-g4"],
            "IEM transmitter: it reports Af telegrams and no RF or battery",
        ),
        None => (
            vec!["em-300-500-g4", "em-300-500-g3", "sr-iem-g4"],
            "not yet known: no cycle telegram identifying a receiver or transmitter has arrived",
        ),
    };
    Event::Discovered {
        protocol: "mcp".into(),
        address: address.to_string(),
        port: MCP_PORT,
        device: MCP_SPEC.into(),
        models: models.into_iter().map(String::from).collect(),
        name: name.clone(),
        evidence: json!({
            "protocol": "answered with MCP cycle telegrams on UDP 53212",
            "family": family_evidence,
            "name": "set by the operator; not unique and can change, so weak evidence of identity",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(ids: &[&str]) -> Catalog {
        let all = Catalog::source_tree();
        Catalog {
            devices: ids
                .iter()
                .map(|id| (id.to_string(), all.device(id).unwrap().clone()))
                .collect(),
        }
    }

    fn request(protocols: &[&str]) -> DiscoverRequest {
        DiscoverRequest {
            action: DiscoverAction::Scan,
            protocols: protocols.iter().map(|p| p.to_string()).collect(),
            hints: vec![],
        }
    }

    #[test]
    fn a_core_runs_only_the_protocols_that_find_its_devices() {
        let rfdeck = Discovery::new(&catalog(&[
            "sennheiser-ew-g3-g4",
            "sennheiser-ew-dx",
            "shure-wireless",
        ]));
        assert_eq!(rfdeck.protocols(), ["mcp"]);
        assert_eq!(rfdeck.wanted(&request(&[])).unwrap(), ["mcp"]);
        assert_eq!(rfdeck.wanted(&request(&["mcp"])).unwrap(), ["mcp"]);
        let err = rfdeck.wanted(&request(&["ssdp"])).unwrap_err();
        assert!(err.contains("finds only sony-camera"), "{err}");
        let err = rfdeck.wanted(&request(&["bonjour"])).unwrap_err();
        assert!(err.contains("unknown discovery protocol"), "{err}");

        let none = Discovery::new(&catalog(&["shure-wireless"]));
        assert!(none.wanted(&request(&[])).unwrap().is_empty());
        assert!(none.wanted(&request(&["mcp"])).is_err());

        let projectors = Discovery::new(&catalog(&["pjlink", "kramer-p3000"]));
        assert_eq!(projectors.wanted(&request(&[])).unwrap(), ["pjlink"]);

        let every = Discovery::new(&Catalog::source_tree());
        assert_eq!(every.protocols(), ["mcp", "ssdp", "pjlink"]);
    }

    #[test]
    fn the_protocol_table_names_real_specs() {
        let all = Catalog::source_tree();
        for (protocol, specs) in PROTOCOLS {
            for spec in *specs {
                assert!(all.device(spec).is_some(), "{protocol}: {spec}");
            }
        }
    }

    #[test]
    fn requests_are_not_devices_and_replies_identify_the_family() {
        assert_eq!(classify(b"Name\r"), None);
        assert_eq!(classify(b"Push 5 500 3\r"), None);
        assert_eq!(classify(b"hello"), None);
        assert_eq!(
            classify(b"Name Vocal 1\r"),
            Some(McpReply {
                name: Some("Vocal 1".into()),
                family: None
            })
        );
        assert_eq!(
            classify(b"RF1 25 65 1\rStates 3 2\rBat 70\r")
                .unwrap()
                .family,
            Some(Family::Receiver)
        );
        assert_eq!(
            classify(b"Af 15 25 40 38 5\rStates 0 2\r").unwrap().family,
            Some(Family::IemTransmitter)
        );
    }

    #[test]
    fn the_sweep_is_bounded_to_known_subnets() {
        let ifaces = [(Ipv4Addr::new(10, 2, 0, 5), Ipv4Addr::new(255, 255, 0, 0))];
        let hints = [Ipv4Addr::new(10, 2, 5, 9), Ipv4Addr::new(10, 2, 0, 77)];
        let (broadcasts, subnets, skipped) = sweep_targets(&ifaces, &hints);
        // Broadcast covers the whole /16; unicast walks two /24s, not 65,534.
        assert_eq!(broadcasts, [Ipv4Addr::new(10, 2, 255, 255)]);
        assert_eq!(subnets.len(), 2);
        assert_eq!(
            subnets[0].len(),
            253,
            "the server's own address is left out"
        );
        assert_eq!(subnets[1][0], Ipv4Addr::new(10, 2, 5, 1));
        assert_eq!(skipped, 0);

        let many: Vec<Ipv4Addr> = (1..=20).map(|n| Ipv4Addr::new(10, 3, n, 1)).collect();
        let (_, subnets, skipped) = sweep_targets(&ifaces, &many);
        assert_eq!(subnets.len(), SWEEP_MAX_SUBNETS);
        assert_eq!(skipped, 13);
    }

    #[test]
    fn a_scan_costs_at_most_two_datagrams_per_address_at_a_bounded_rate() {
        let max_addresses = SWEEP_MAX_SUBNETS * 254;
        let datagrams = max_addresses * 2;
        let per_second = (SWEEP_BATCH * 2) as f64 / SWEEP_PAUSE.as_secs_f64();
        assert!(datagrams <= 4_064);
        assert!(per_second <= 1_300.0, "{per_second} datagrams a second");
    }
}
