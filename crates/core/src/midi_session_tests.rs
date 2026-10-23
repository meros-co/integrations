//! A MIDI integration on a MIDI port of the host, through the public API,
//! with the device played on an in-memory port: messages both ways, state
//! and events, the port listing, and a device unplugged and plugged in
//! again. No real MIDI port is opened.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::catalog::{Catalog, Params};
use crate::midi_ports::MemoryMidi;
use crate::{CommandError, Connection, Core, CoreOptions, Event, OpenRequest, Outcome};

fn core(ports: Arc<MemoryMidi>) -> Core {
    let catalog = Catalog::source_tree();
    let spec = catalog.device("generic-midi").unwrap().clone();
    let catalog = Catalog {
        devices: [(spec.id.clone(), spec)].into(),
    };
    Core::with_catalog(
        catalog,
        CoreOptions::new().bind_address([127, 0, 0, 1].into()),
    )
    .unwrap()
    .with_midi_ports(ports)
}

fn params(v: Value) -> Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core) -> u64 {
    core.open(OpenRequest {
        device: "generic-midi".into(),
        model: "generic".into(),
        // A device on a MIDI port needs no host.
        host: String::new(),
        port: None,
        settings: params(json!({"link": "port", "midi_input": "Keys", "midi_output": "Keys"})),
        monitor: true,
    })
    .unwrap()
}

async fn wait_connection(core: &Core, device: u64, want: fn(&Connection) -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !want(&core.snapshot(device).unwrap().connection) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("connection within 5 s")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_on_a_midi_port_is_played_both_ways_and_found_again() {
    let ports = Arc::new(MemoryMidi::default());
    let mut sent = ports.plug("Keys");
    let core = core(ports.clone());
    assert_eq!(
        core.midi_ports().unwrap(),
        [crate::MidiPortInfo {
            name: "Keys".into(),
            input: true,
            output: true
        }]
    );
    let id = open(&core);
    wait_connection(&core, id, |c| *c == Connection::Connected).await;

    let result = core
        .execute(
            id,
            "note_on",
            params(json!({"channel": 10, "note": 36, "velocity": 127})),
        )
        .await;
    assert_eq!(result, Ok(Outcome::Unverified));
    let got = tokio::time::timeout(Duration::from_secs(5), sent.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, [0x99, 36, 127]);

    // The device sends a controller and an MMC play.
    assert!(ports.play("Keys", &[0xB0, 74, 33]));
    assert!(ports.play("Keys", &[0xF0, 0x7F, 0x7F, 0x06, 0x02, 0xF7]));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let state = core.snapshot(id).unwrap().state;
            if state["mmc"]["command"] == "play" {
                assert_eq!(state["channels"]["1"]["cc"]["74"], 33);
                assert_eq!(state["link"], "port");
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("state within 5 s");
    let mut addresses = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while addresses.len() < 2 {
            for e in core.next_events(64).await {
                if let Event::Message { address, .. } = e {
                    addresses.push(address);
                }
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(addresses, ["control_change", "mmc"]);

    // Unplugged: disconnected, and commands fail until it is back.
    ports.unplug("Keys");
    wait_connection(
        &core,
        id,
        |c| matches!(c, Connection::Disconnected { reason } if reason.starts_with("midi:")),
    )
    .await;
    assert_eq!(
        core.execute(id, "tune_request", Params::new()).await,
        Err(CommandError::NotConnected)
    );
    let mut sent = ports.plug("Keys");
    wait_connection(&core, id, |c| *c == Connection::Connected).await;
    assert_eq!(
        core.execute(id, "tune_request", Params::new()).await,
        Ok(Outcome::Unverified)
    );
    let got = tokio::time::timeout(Duration::from_secs(5), sent.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, [0xF6]);
}

/// Two consecutive ports free on loopback for UDP and TCP: an RTP-MIDI
/// session's control and data ports.
fn free_pair() -> u16 {
    loop {
        let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = tcp.local_addr().unwrap().port();
        if port == u16::MAX {
            continue;
        }
        let free = std::net::UdpSocket::bind(("127.0.0.1", port)).is_ok()
            && std::net::UdpSocket::bind(("127.0.0.1", port + 1)).is_ok()
            && std::net::TcpListener::bind(("127.0.0.1", port + 1)).is_ok();
        if free {
            return port;
        }
    }
}

/// Send `packet` to `to` until an answer whose command is `want` comes back.
async fn ask(socket: &tokio::net::UdpSocket, to: (&str, u16), packet: &[u8], want: &[u8]) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut buf = [0u8; 1500];
        loop {
            socket.send_to(packet, to).await.unwrap();
            let got =
                tokio::time::timeout(Duration::from_millis(200), socket.recv_from(&mut buf)).await;
            if let Ok(Ok((n, _))) = got {
                if n >= 4 && &buf[2..4] == want {
                    return;
                }
            }
        }
    })
    .await
    .expect("an answer within 5 s")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hosted_rtp_midi_session_is_joined_over_loopback() {
    let core = core(Arc::new(MemoryMidi::default()));
    let port = free_pair();
    let id = core
        .open(OpenRequest {
            device: "generic-midi".into(),
            model: "generic".into(),
            // Only this host may join.
            host: "127.0.0.1".into(),
            port: None,
            settings: params(
                json!({"link": "rtp_host", "local_port": port, "session_name": "Core"}),
            ),
            monitor: true,
        })
        .unwrap();
    // The joining host: its control and data sockets.
    let control = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let data = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let mut invite = vec![0xFF, 0xFF, b'I', b'N', 0, 0, 0, 2];
    invite.extend_from_slice(&0x0102_0304u32.to_be_bytes());
    invite.extend_from_slice(&0xCAFEu32.to_be_bytes());
    invite.extend_from_slice(b"Desk\0");
    ask(&control, ("127.0.0.1", port), &invite, b"OK").await;
    ask(&data, ("127.0.0.1", port + 1), &invite, b"OK").await;
    wait_connection(&core, id, |c| *c == Connection::Connected).await;
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["link"], "rtp_host");
    assert_eq!(state["session"]["peer_name"], "Desk");

    // A note goes to the data socket, with a recovery journal.
    let result = core
        .execute(
            id,
            "note_on",
            params(json!({"channel": 1, "note": 60, "velocity": 100})),
        )
        .await;
    assert_eq!(result, Ok(Outcome::Unverified));
    let mut buf = [0u8; 1500];
    let n = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let (n, _) = data.recv_from(&mut buf).await.unwrap();
            // Skip anything but RTP (clock synchronisation is the joiner's).
            if buf[0] >> 6 == 2 && n > 13 && buf[12] & 0x0F == 3 {
                return n;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(buf[12] & 0x40, 0x40, "J set");
    assert_eq!(&buf[13..16], &[0x90, 60, 100]);
    assert!(n >= 19, "a journal follows the list");

    // The joiner sends a controller.
    let mut packet = vec![0x80, 0x61, 0, 1, 0, 0, 0, 1];
    packet.extend_from_slice(&0xCAFEu32.to_be_bytes());
    packet.extend_from_slice(&[3, 0xB0, 7, 99]);
    data.send_to(&packet, ("127.0.0.1", port + 1))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while core.snapshot(id).unwrap().state["channels"]["1"]["cc"]["7"] != 99 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("state within 5 s");
}
