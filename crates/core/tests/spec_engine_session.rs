//! Spec-driven devices simulated on real sockets, driven through the public API.

#![cfg(all(
    feature = "behringer-x32",
    feature = "blackmagic-hyperdeck",
    feature = "qlab"
))]

mod common;

use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, UdpSocket};

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

async fn wait_connected(core: &Core, id: u64) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let events = core.next_events(64).await;
            if events.iter().any(|e| {
                matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
            }) {
                return;
            }
        }
    })
    .await
    .expect("connected within 5 s")
}

fn open(core: &Core, device: &str, model: &str, port: u16) -> u64 {
    core.open(OpenRequest {
        device: device.into(),
        model: model.into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: Default::default(),
        monitor: true,
    })
    .unwrap()
}

/// A HyperDeck: greets with an asynchronous 500 block, answers commands with
/// numbered replies, and pushes an unsolicited 508 transport block before
/// answering `transport info`, which must not be taken as the reply.
#[tokio::test(flavor = "multi_thread")]
async fn hyperdeck_over_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (read, mut write) = stream.into_split();
        write
            .write_all(b"500 connection info:\r\nprotocol version: 1.11\r\nmodel: HyperDeck Studio\r\n\r\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(read).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let reply: &[u8] = match line.trim() {
                "record" => b"200 ok\r\n",
                "stop" => b"111 remote control disabled\r\n",
                "transport info" => {
                    b"508 transport info:\r\nstatus: stopped\r\n\r\n\
                    208 transport info:\r\nstatus: play\r\nspeed: 100\r\n\r\n"
                }
                "ping" => b"200 ok\r\n",
                _ => b"100 syntax error\r\n",
            };
            write.write_all(reply).await.unwrap();
        }
    });

    let core = common::core().unwrap();
    let id = open(&core, "blackmagic-hyperdeck", "hyperdeck-1-11", port);
    wait_connected(&core, id).await;

    assert_eq!(
        core.execute(id, "record", params(json!({}))).await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        core.execute(id, "stop", params(json!({}))).await,
        Err(CommandError::DeviceError {
            code: Some("111".into()),
            message: "remote control disabled".into()
        })
    );
    assert_eq!(
        core.execute(id, "get_transport_info", params(json!({})))
            .await,
        Ok(Outcome::Value {
            value: json!({"status": "play", "speed": "100"})
        })
    );
    // Protocol 1.8 has no playrange: refused before anything is sent.
    core.close(id).await;
    let id = open(&core, "blackmagic-hyperdeck", "hyperdeck-1-8", port);
    assert!(matches!(
        core.execute(id, "set_playrange_clip", params(json!({"clip_id": 1})))
            .await,
        Err(CommandError::UnsupportedForModel { .. })
    ));
}

/// A HyperDeck's configuration API beside its Ethernet Protocol: a write
/// and its read-back go over HTTP on the admin port, and still go once the
/// protocol's connection is gone (the write turned it off).
#[tokio::test(flavor = "multi_thread")]
async fn hyperdeck_configuration_api_beside_the_protocol() {
    let protocol = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = protocol.local_addr().unwrap().port();
    let (drop_tx, drop_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let (stream, _) = protocol.accept().await.unwrap();
        let (read, mut write) = stream.into_split();
        write
            .write_all(b"500 connection info:\r\nprotocol version: 1.11\r\nmodel: HyperDeck Studio HD Mini\r\n\r\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(read).lines();
        tokio::select! {
            _ = async { while let Ok(Some(_)) = lines.next_line().await {
                write.write_all(b"200 ok\r\n").await.unwrap();
            } } => {}
            // Turned off: the deck closes the protocol port.
            _ = drop_rx => {}
        }
    });
    let admin = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let admin_port = admin.local_addr().unwrap().port();
    let (seen_tx, mut seen) = tokio::sync::mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        let mut drop_tx = Some(drop_tx);
        loop {
            let (mut stream, _) = admin.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            let (head, body) = loop {
                let n = stream.read(&mut buf).await.unwrap();
                if n == 0 {
                    break (String::new(), String::new());
                }
                request.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&request).to_string();
                if let Some((head, body)) = text.split_once("\r\n\r\n") {
                    let length = head
                        .lines()
                        .find_map(|l| {
                            let (k, v) = l.split_once(':')?;
                            k.eq_ignore_ascii_case("content-length")
                                .then(|| v.trim().parse::<usize>().ok())?
                        })
                        .unwrap_or(0);
                    if body.len() >= length {
                        break (head.to_string(), body.to_string());
                    }
                }
            };
            let line = head.lines().next().unwrap_or("").to_string();
            seen_tx.send(format!("{line} {body}")).unwrap();
            let reply = if line.starts_with("PUT /admin/api/v1/hyperDeckEthernetProtocol ") {
                if let Some(tx) = drop_tx.take() {
                    let _ = tx.send(());
                }
                r#"{"success":true}"#
            } else if line.starts_with("GET /admin/api/v1/hyperDeckEthernetProtocol ") {
                r#"{"response":{"state":"Disabled"},"success":true}"#
            } else {
                r#"{"success":false}"#
            };
            let answer = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                reply.len()
            );
            stream.write_all(answer.as_bytes()).await.unwrap();
        }
    });

    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "blackmagic-hyperdeck".into(),
            model: "hyperdeck-studio".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"admin_port": admin_port})),
            monitor: false,
        })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let events = core.next_events(64).await;
            if events.iter().any(|e| {
                matches!(e, Event::Connection { device, connection: Connection::Unmonitored | Connection::Connected } if *device == id)
            }) {
                return;
            }
        }
    })
    .await
    .expect("connected within 5 s");

    assert_eq!(
        core.execute(
            id,
            "set_ethernet_protocol",
            params(json!({"state": "Disabled"}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        seen.recv().await.unwrap(),
        r#"PUT /admin/api/v1/hyperDeckEthernetProtocol HTTP/1.1 {"state":"Disabled"}"#
    );
    assert_eq!(
        seen.recv().await.unwrap(),
        "GET /admin/api/v1/hyperDeckEthernetProtocol HTTP/1.1 "
    );
    // The read-back reached state, though the device is not monitored.
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["admin"]["ethernet_protocol"], json!("Disabled"));
    // The protocol port is gone; the configuration API still answers.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        core.execute(id, "get_ethernet_protocol", params(json!({})))
            .await,
        Ok(Outcome::Value {
            value: json!("Disabled")
        })
    );
    core.close(id).await;
}

/// An X32: OSC over UDP, replying to the sender.
#[tokio::test(flavor = "multi_thread")]
async fn x32_over_udp() {
    fn osc_string(s: &str) -> Vec<u8> {
        let mut b = s.as_bytes().to_vec();
        b.push(0);
        while !b.len().is_multiple_of(4) {
            b.push(0);
        }
        b
    }
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    tokio::spawn(async move {
        let mut buf = [0u8; 1500];
        loop {
            let (n, from) = socket.recv_from(&mut buf).await.unwrap();
            let address_end = buf[..n].iter().position(|b| *b == 0).unwrap();
            let address = String::from_utf8_lossy(&buf[..address_end]).to_string();
            let reply = match address.as_str() {
                "/info" => [osc_string("/info"), osc_string(",s"), osc_string("V2.07")].concat(),
                "/ch/03/config/name" => [
                    osc_string("/ch/03/config/name"),
                    osc_string(",s"),
                    osc_string("Snare"),
                ]
                .concat(),
                // Like the console, answer any other query with its value.
                _ => [
                    osc_string(&address),
                    osc_string(",i"),
                    0i32.to_be_bytes().to_vec(),
                ]
                .concat(),
            };
            socket.send_to(&reply, from).await.unwrap();
        }
    });

    let core = common::core().unwrap();
    let id = open(&core, "behringer-x32", "x32", port);
    wait_connected(&core, id).await;
    assert_eq!(
        core.execute(id, "get_channel_name", params(json!({"channel": 3})))
            .await,
        Ok(Outcome::Value {
            value: json!("Snare")
        })
    );
    assert_eq!(
        core.execute(id, "mute_channel", params(json!({"channel": 3})))
            .await,
        Ok(Outcome::Unverified)
    );
}

/// QLab: OSC over TCP with double-END SLIP framing. Queries are answered on
/// /reply/<address> with a JSON string; actions are not answered.
#[tokio::test(flavor = "multi_thread")]
async fn qlab_over_tcp_slip() {
    fn osc_string(s: &str) -> Vec<u8> {
        let mut b = s.as_bytes().to_vec();
        b.resize((b.len() / 4 + 1) * 4, 0);
        b
    }
    fn slip(packet: &[u8]) -> Vec<u8> {
        let mut out = vec![0xC0];
        for &b in packet {
            match b {
                0xC0 => out.extend([0xDB, 0xDC]),
                0xDB => out.extend([0xDB, 0xDD]),
                b => out.push(b),
            }
        }
        out.push(0xC0);
        out
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (heard, mut addresses) = tokio::sync::mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 1024];
        loop {
            let n = match stream.read(&mut chunk).await {
                Ok(0) | Err(_) => return,
                Ok(n) => n,
            };
            buf.extend_from_slice(&chunk[..n]);
            // Frames are END ... END; the addresses used here need no unescaping.
            while let Some(end) = buf.iter().skip(1).position(|&b| b == 0xC0).map(|i| i + 1) {
                let packet: Vec<u8> = buf.drain(..=end).filter(|&b| b != 0xC0).collect();
                if packet.is_empty() {
                    continue;
                }
                let nul = packet.iter().position(|&b| b == 0).unwrap_or(packet.len());
                let address = String::from_utf8_lossy(&packet[..nul]).into_owned();
                if address == "/version" {
                    let json =
                        r#"{"workspace_id":"","address":"/version","status":"ok","data":"5.5.1"}"#;
                    let reply = [
                        osc_string("/reply/version"),
                        osc_string(",s"),
                        osc_string(json),
                    ]
                    .concat();
                    stream.write_all(&slip(&reply)).await.unwrap();
                }
                let _ = heard.send(address);
            }
        }
    });

    let core = common::core().unwrap();
    let id = open(&core, "qlab", "qlab-5", port);
    wait_connected(&core, id).await;
    assert_eq!(
        core.execute(id, "go", params(json!({}))).await,
        Ok(Outcome::Unverified)
    );
    tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(address) = addresses.recv().await {
            if address == "/go" {
                return;
            }
        }
    })
    .await
    .expect("/go reached QLab, SLIP-framed");
}
