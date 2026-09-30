//! Spec-driven devices simulated on real sockets, driven through the public API.

use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
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

    let core = Core::new().unwrap();
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

/// An X32: OSC over UDP, replying to the sender.
#[tokio::test(flavor = "multi_thread")]
async fn x32_over_udp() {
    fn osc_string(s: &str) -> Vec<u8> {
        let mut b = s.as_bytes().to_vec();
        b.push(0);
        while b.len() % 4 != 0 {
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
                _ => continue,
            };
            socket.send_to(&reply, from).await.unwrap();
        }
    });

    let core = Core::new().unwrap();
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

/// QLab never replies to the sender: the connection is unmonitored, not
/// connected, and every command is unverified.
#[tokio::test(flavor = "multi_thread")]
async fn qlab_is_unmonitored() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let core = Core::new().unwrap();
    let id = open(&core, "qlab", "qlab-5", port);
    assert_eq!(
        core.execute(id, "go", params(json!({}))).await,
        Ok(Outcome::Unverified)
    );
    assert_eq!(
        core.snapshot(id).unwrap().connection,
        Connection::Unmonitored
    );
    // The datagram really went out.
    let mut buf = [0u8; 64];
    let (n, _) = tokio::time::timeout(Duration::from_secs(2), socket.recv_from(&mut buf))
        .await
        .unwrap()
        .unwrap();
    assert!(buf[..n].starts_with(b"/go\0"));
}
