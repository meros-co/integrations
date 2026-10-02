//! MCP discovery on real UDP, through the public API: a core bound to
//! 127.0.0.1 listens on 53212, and a simulated receiver at 127.0.0.2 answers
//! as a G3/G4 would.

#![cfg(feature = "sennheiser-ew-g3-g4")]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use meros_integrations::{Core, CoreOptions, DiscoverAction, DiscoverRequest, Event};
use tokio::net::UdpSocket;

#[tokio::test(flavor = "multi_thread")]
async fn a_g3_g4_is_discovered_and_identified_as_a_receiver() {
    let core = Core::with_options(CoreOptions::new().bind_address(IpAddr::V4(Ipv4Addr::LOCALHOST)))
        .unwrap();
    core.discover(DiscoverRequest {
        action: DiscoverAction::Listen,
        protocols: vec!["mcp".into()],
        hints: vec![],
    })
    .unwrap();

    let device = UdpSocket::bind(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
        53212,
    ))
    .await
    .unwrap();
    let core_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 53212);
    // A request from another scanner is not a device.
    device.send_to(b"Name\r", core_addr).await.unwrap();
    device
        .send_to(
            b"Name Vocal 1\rRF1 25 65 1\rStates 3 2\rBat 70\r",
            core_addr,
        )
        .await
        .unwrap();

    let found = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            for e in core.next_events(64).await {
                if let Event::Discovered {
                    address,
                    models,
                    name,
                    device,
                    ..
                } = e
                {
                    return (address, models, name, device);
                }
            }
        }
    })
    .await
    .expect("discovered within 10 s");
    assert_eq!(found.0, "127.0.0.2");
    assert_eq!(found.3, "sennheiser-ew-g3-g4");
    assert_eq!(found.1, ["em-300-500-g4", "em-300-500-g3"]);
    assert_eq!(found.2.as_deref(), Some("Vocal 1"));

    // Unknown protocols are refused rather than ignored.
    assert!(core
        .discover(DiscoverRequest {
            action: DiscoverAction::Scan,
            protocols: vec!["nope".into()],
            hints: vec![],
        })
        .is_err());
    core.discover(DiscoverRequest {
        action: DiscoverAction::Stop,
        protocols: vec![],
        hints: vec![],
    })
    .unwrap();
}
