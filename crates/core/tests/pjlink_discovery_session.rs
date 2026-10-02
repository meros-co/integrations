//! PJLink discovery on real UDP 4352, through the public API. A core bound to
//! 127.0.0.1 scans with a hint; a simulated Class 2 projector at 127.0.0.2
//! answers the unicast `%2SRCH` with `%2ACKN` to the controller's UDP 4352,
//! as PJLink v2.10 §3.2 has it answer the broadcast. Broadcast is not used:
//! the core bound to loopback has no interface to broadcast on, and loopback
//! broadcast is unreliable across the platforms the tests run on.
//!
//! A projector at 127.0.0.3 announces `%2LKUP`, and one at 127.0.0.4 is open
//! as a device with notifications on: its LKUP must reach its session, not
//! discovery.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use meros_integrations::{Core, CoreOptions, DiscoverAction, DiscoverRequest, Event, OpenRequest};
use serde_json::{json, Value};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;

const SEARCHED: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);
const ANNOUNCING: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 3);
const OPEN: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 4);
const CONTROLLER: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4352);

/// UDP 4352 on a projector's address.
fn projector_socket(ip: Ipv4Addr) -> UdpSocket {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
    socket.set_reuse_address(true).unwrap();
    socket.set_nonblocking(true).unwrap();
    socket
        .bind(&SocketAddr::new(IpAddr::V4(ip), 4352).into())
        .unwrap();
    UdpSocket::from_std(socket.into()).unwrap()
}

/// Every `Discovered` event as (address, via, mac, models), until `quiet`
/// passes without one or `limit` has passed.
async fn discovered(
    core: &Core,
    quiet: Duration,
    limit: Duration,
) -> Vec<(String, Value, Value, Vec<String>)> {
    let mut found = Vec::new();
    let until = tokio::time::Instant::now() + limit;
    loop {
        let wait = quiet.min(until.saturating_duration_since(tokio::time::Instant::now()));
        let Ok(events) = tokio::time::timeout(wait, core.next_events(64)).await else {
            return found;
        };
        for e in events {
            if let Event::Discovered {
                protocol,
                address,
                port,
                device,
                models,
                name,
                evidence,
            } = e
            {
                assert_eq!(protocol, "pjlink");
                assert_eq!(port, 4352);
                assert_eq!(device, "pjlink");
                assert_eq!(name, None);
                found.push((
                    address,
                    evidence["via"].clone(),
                    evidence["mac"].clone(),
                    models,
                ));
            }
        }
        if tokio::time::Instant::now() >= until {
            return found;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn class_2_projectors_are_found_by_search_and_link_up() {
    let core = Core::with_options(CoreOptions {
        bind_address: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
    })
    .unwrap();

    // Open first, so its notification route exists before discovery listens.
    // Nothing listens on its TCP port; only the UDP route matters here.
    let open_id = core
        .open(OpenRequest {
            device: "pjlink".into(),
            model: "class-2".into(),
            host: OPEN.to_string(),
            port: Some(9),
            settings: json!({"notifications": true}).as_object().unwrap().clone(),
        })
        .unwrap();
    // The session registers its UDP 4352 route as it starts.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let searched = projector_socket(SEARCHED);
    tokio::spawn(async move {
        let mut buf = [0u8; 512];
        loop {
            let Ok((n, from)) = searched.recv_from(&mut buf).await else {
                continue;
            };
            if &buf[..n] != b"%2SRCH\r" {
                continue;
            }
            // §3.2: the answer goes to UDP 4352 of the searcher.
            let to = SocketAddr::new(from.ip(), 4352);
            let _ = searched.send_to(b"%2ACKN=00:11:22:AA:BB:01\r", to).await;
        }
    });

    core.discover(DiscoverRequest {
        action: DiscoverAction::Scan,
        protocols: vec!["pjlink".into()],
        hints: vec![SEARCHED],
    })
    .unwrap();

    let announcing = projector_socket(ANNOUNCING);
    announcing
        .send_to(b"%2LKUP=00:11:22:aa:bb:02\r", CONTROLLER)
        .await
        .unwrap();
    // Status notifications from a projector nothing has open identify nothing.
    announcing.send_to(b"%2POWR=1\r", CONTROLLER).await.unwrap();

    let open = projector_socket(OPEN);
    open.send_to(b"%2LKUP=00:11:22:aa:bb:04\r", CONTROLLER)
        .await
        .unwrap();

    // Three SRCH rounds a second apart, one answer each: reported once.
    let mut found = discovered(&core, Duration::from_secs(4), Duration::from_secs(10)).await;
    found.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        found,
        [
            (
                "127.0.0.2".to_string(),
                json!("ACKN"),
                json!("00:11:22:aa:bb:01"),
                vec!["class-2".to_string()]
            ),
            (
                "127.0.0.3".to_string(),
                json!("LKUP"),
                json!("00:11:22:aa:bb:02"),
                vec!["class-2".to_string()]
            ),
        ],
        "the open projector's LKUP went to its session, not discovery"
    );
    assert_eq!(
        core.snapshot(open_id).unwrap().state["mac_address"],
        "00:11:22:aa:bb:04"
    );

    // A second scan while the first collects is ignored; nothing is re-reported.
    core.discover(DiscoverRequest {
        action: DiscoverAction::Scan,
        protocols: vec!["pjlink".into()],
        hints: vec![SEARCHED],
    })
    .unwrap();
    assert!(
        discovered(&core, Duration::from_secs(2), Duration::from_secs(2))
            .await
            .is_empty()
    );

    core.discover(DiscoverRequest {
        action: DiscoverAction::Stop,
        protocols: vec!["pjlink".into()],
        hints: vec![],
    })
    .unwrap();
    core.close(open_id).await;
}
