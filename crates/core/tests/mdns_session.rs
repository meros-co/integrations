//! mDNS discovery of Blackmagic devices through the public API. A core bound
//! to 127.0.0.1 scans with a hint; a simulated responder at 127.0.0.2 answers
//! the unicast query to UDP 5353 by unicast to the query's source port, as
//! RFC 6762 §5.5 and §6.7 have it. It answers for an ATEM in full (PTR, with
//! SRV, TXT and A as additional records) and for a HyperDeck with the PTR
//! alone, so its SRV must be asked for in a later round. Multicast is not
//! used: the core bound to loopback has no interface to multicast on, and
//! loopback multicast is unreliable across the platforms the tests run on.

#![cfg(all(feature = "blackmagic-atem", feature = "blackmagic-hyperdeck"))]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use meros_integrations::{Core, CoreOptions, DiscoverAction, DiscoverRequest, Event};
use serde_json::Value;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;

const RESPONDER: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);

const BLACKMAGIC: &[u8] = b"\x0b_blackmagic\x04_tcp\x05local\x00";
const HYPERDECK: &[u8] = b"\x0f_hyperdeck_ctrl\x04_tcp\x05local\x00";
const DECK_INSTANCE: &[u8] = b"\x0aHyperDeck1\x0f_hyperdeck_ctrl\x04_tcp\x05local\x00";

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// A response header (RFC 6762 §18): ID 0, QR and AA, no questions.
fn header(answers: u16, additional: u16) -> Vec<u8> {
    let mut h = vec![0, 0, 0x84, 0x00, 0, 0];
    h.extend_from_slice(&answers.to_be_bytes());
    h.extend_from_slice(&[0, 0]);
    h.extend_from_slice(&additional.to_be_bytes());
    h
}

fn rr(out: &mut Vec<u8>, rtype: u16, ttl: u32, rdata: &[u8]) {
    out.extend_from_slice(&rtype.to_be_bytes());
    out.extend_from_slice(&[0x00, 0x01]);
    out.extend_from_slice(&ttl.to_be_bytes());
    out.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
    out.extend_from_slice(rdata);
}

/// The ATEM in full: PTR answer; SRV, TXT and A additional.
fn atem() -> Vec<u8> {
    let mut p = header(1, 3);
    p.extend_from_slice(BLACKMAGIC); // 12..36
    rr(&mut p, 12, 4500, b"\x0bStudio ATEM\xc0\x0c"); // instance at 46
    p.extend_from_slice(&[0xc0, 46]);
    rr(
        &mut p,
        33,
        120,
        b"\x00\x00\x00\x00\x26\xb6\x04atem\x05local\x00",
    );
    p.extend_from_slice(&[0xc0, 46]);
    let mut txt = Vec::new();
    for s in [
        "class=AtemSwitcher",
        "name=Blackmagic ATEM Television Studio HD8 ISO",
    ] {
        txt.push(s.len() as u8);
        txt.extend_from_slice(s.as_bytes());
    }
    rr(&mut p, 16, 4500, &txt);
    p.extend_from_slice(b"\x04atem\x05local\x00");
    rr(&mut p, 1, 120, &RESPONDER.octets());
    p
}

/// The HyperDeck's PTR alone.
fn deck_ptr() -> Vec<u8> {
    let mut p = header(1, 0);
    p.extend_from_slice(HYPERDECK);
    rr(&mut p, 12, 4500, b"\x0aHyperDeck1\xc0\x0c");
    p
}

/// The HyperDeck's SRV, answering a question for it, with its A record.
fn deck_srv() -> Vec<u8> {
    let mut p = header(1, 1);
    p.extend_from_slice(DECK_INSTANCE);
    rr(
        &mut p,
        33,
        120,
        b"\x00\x00\x00\x00\x27\x09\x04deck\x05local\x00",
    );
    p.extend_from_slice(b"\x04deck\x05local\x00");
    rr(&mut p, 1, 120, &RESPONDER.octets());
    p
}

/// One `Discovered` event: device, port, address, models, name, evidence.
type Found = (String, u16, String, Vec<String>, Option<String>, Value);

/// UDP 5353 on the responder's address, shared as the core's listener shares it.
fn responder_socket() -> UdpSocket {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
    socket.set_reuse_address(true).unwrap();
    socket.set_nonblocking(true).unwrap();
    socket
        .bind(&SocketAddr::new(IpAddr::V4(RESPONDER), 5353).into())
        .unwrap();
    UdpSocket::from_std(socket.into()).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn blackmagic_devices_answer_a_query_and_are_identified() {
    let responder = responder_socket();
    let srv_asked = Arc::new(AtomicBool::new(false));
    let asked = srv_asked.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 9000];
        loop {
            let Ok((n, from)) = responder.recv_from(&mut buf).await else {
                continue;
            };
            let q = &buf[..n];
            // Queries only (QR clear), as a responder answers.
            if n < 12 || q[2] & 0x80 != 0 {
                continue;
            }
            if contains(q, BLACKMAGIC) {
                let _ = responder.send_to(&atem(), from).await;
            }
            if contains(q, HYPERDECK) {
                let _ = responder.send_to(&deck_ptr(), from).await;
            }
            if contains(q, DECK_INSTANCE) {
                asked.store(true, Ordering::SeqCst);
                let _ = responder.send_to(&deck_srv(), from).await;
            }
        }
    });

    let core = Core::with_options(CoreOptions::new().bind_address(IpAddr::V4(Ipv4Addr::LOCALHOST)))
        .unwrap();
    assert!(core.discovery_protocols().contains(&"mdns"));
    core.discover(DiscoverRequest {
        action: DiscoverAction::Scan,
        protocols: vec!["mdns".into()],
        hints: vec![RESPONDER],
    })
    .unwrap();

    // Every Discovered event from the responder until the scan (5 s) is over.
    let mut found: Vec<Found> = Vec::new();
    let until = tokio::time::Instant::now() + Duration::from_secs(8);
    while let Ok(events) = tokio::time::timeout_at(until, core.next_events(64)).await {
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
                if address == "127.0.0.2" {
                    assert_eq!(protocol, "mdns");
                    found.push((device, port, address, models, name, evidence));
                }
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(found.len(), 2, "each device once: {found:?}");

    let (device, port, _, models, name, evidence) = &found[0];
    assert_eq!((device.as_str(), *port), ("blackmagic-atem", 9910));
    assert_eq!(models, &["atem-television-studio-hd8-iso"]);
    assert_eq!(name.as_deref(), Some("Studio ATEM"));
    assert_eq!(evidence["txt"]["class"], "AtemSwitcher");
    assert_eq!(evidence["srv"]["target"], "atem.local");
    assert!(evidence["address"]
        .as_str()
        .unwrap()
        .starts_with("the A record"));

    let (device, port, _, models, name, evidence) = &found[1];
    assert_eq!((device.as_str(), *port), ("blackmagic-hyperdeck", 9993));
    assert!(models.len() > 1, "a HyperDeck's model is not advertised");
    assert_eq!(name.as_deref(), Some("HyperDeck1"));
    assert_eq!(evidence["srv"]["port"], 9993);
    assert!(
        srv_asked.load(Ordering::SeqCst),
        "the SRV was asked for after the PTR"
    );

    core.discover(DiscoverRequest {
        action: DiscoverAction::Stop,
        protocols: vec!["mdns".into()],
        hints: vec![],
    })
    .unwrap();
}
