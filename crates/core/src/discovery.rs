//! Finding devices on the network.
//!
//! The core decides how a scan is done, so no host can flood a venue's
//! network; the host decides when, which is product policy. Results are
//! events: what was found, where, which spec and models it can be, and what
//! the identification rests on.
//!
//! Sennheiser EW G3/G4 receivers (MCP) are found by `crate::mcp_discovery`,
//! Sony cameras (SSDP) by `crate::ssdp`, and PJLink Class 2 projectors
//! (SRCH/ACKN and LKUP on UDP 4352) by `crate::pjlink_discovery`, whose rules
//! are there. Each is compiled only with the integration whose devices it
//! finds. Protocols are independent: each listens, scans and stops on its own.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

use serde::Deserialize;

use crate::catalog::Catalog;
use crate::session::Services;

/// Each discovery protocol and the specs whose devices it reports. A core
/// runs a protocol only when its catalogue holds one of them: a build
/// without that integration, or a core started for other devices, has no use
/// for the protocol's traffic, and asking for it by name is an error.
pub const PROTOCOLS: &[(&str, &[&str])] = &[
    ("mcp", &["sennheiser-ew-g3-g4"]),
    ("ssdp", &["sony-camera"]),
    ("pjlink", &["pjlink"]),
];

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

pub(crate) struct Discovery {
    #[cfg(feature = "sennheiser-ew-g3-g4")]
    mcp: crate::mcp_discovery::McpDiscovery,
    #[cfg(feature = "sony-camera")]
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
            #[cfg(feature = "sennheiser-ew-g3-g4")]
            mcp: Default::default(),
            #[cfg(feature = "sony-camera")]
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
        // A protocol whose integration is not built in is never available, so
        // its branches below are compiled only with that integration.
        let (mcp, ssdp, pjlink) = (wants("mcp"), wants("ssdp"), wants("pjlink"));
        let _ = (mcp, ssdp, pjlink);
        match request.action {
            DiscoverAction::Stop => {
                #[cfg(feature = "sennheiser-ew-g3-g4")]
                if mcp {
                    self.mcp.stop(services);
                }
                #[cfg(feature = "sony-camera")]
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
                #[cfg(feature = "sennheiser-ew-g3-g4")]
                if mcp {
                    self.mcp.listen(services)?;
                }
                #[cfg(feature = "sony-camera")]
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
                #[cfg(feature = "sennheiser-ew-g3-g4")]
                if mcp {
                    self.mcp.listen(services)?;
                    self.mcp.scan(services, request.hints.clone());
                }
                #[cfg(feature = "sony-camera")]
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
}
