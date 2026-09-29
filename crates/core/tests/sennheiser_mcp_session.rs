//! A G4 receiver simulated on real UDP, driven through the public API.
//!
//! The simulated device listens on 127.0.0.2:53212 so that it and the core's
//! shared socket (0.0.0.0:53212) can coexist on one machine. It answers the way
//! TI 1254 describes: echoes set instructions, streams cyclic attributes while
//! subscribed, and rejects out-of-range values with numbered errors.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::net::UdpSocket;

const DEVICE_IP: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);

async fn simulated_g4() -> tokio::task::JoinHandle<()> {
    let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(DEVICE_IP), 53212))
        .await
        .expect("bind simulated device on 127.0.0.2:53212");
    tokio::spawn(async move {
        let mut buf = [0u8; 1500];
        loop {
            let Ok((n, from)) = socket.recv_from(&mut buf).await else {
                return;
            };
            let text = String::from_utf8_lossy(&buf[..n])
                .trim_end_matches('\r')
                .to_string();
            let reply = match text.split_whitespace().collect::<Vec<_>>().as_slice() {
                ["Push", ..] => {
                    // Echo, then one cycle of cyclic attributes and the
                    // configuration attributes.
                    let _ = socket.send_to(format!("{text}\r").as_bytes(), from).await;
                    let _ = socket
                        .send_to(b"Name Vocal 1\rFrequency 606000 0 0\rMute 0\r", from)
                        .await;
                    "RF1 25 65 1\rRF2 28 78 0\rStates 0 1\rRF 50 1 1\rAF 40 65 0\rBat 70\rMsg OK\rConfig 12\r".to_string()
                }
                ["Name"] => "Name Vocal 1\r".into(),
                ["Frequency"] => "Frequency 606000 0 0\r".into(),
                ["Mute", v] => format!("Mute {v}\r"),
                ["AfOut", v] => match v.parse::<i32>() {
                    Ok(level) if (-24..=24).contains(&level) && level % 3 == 0 => {
                        format!("AfOut {level}\r")
                    }
                    _ => format!("1020: Value out of range [ {text} ] \r"),
                },
                _ => continue,
            };
            let _ = socket.send_to(reply.as_bytes(), from).await;
        }
    })
}

async fn wait_for(core: &Core, mut pred: impl FnMut(&Event) -> bool) -> Event {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            for event in core.next_events(64).await {
                if pred(&event) {
                    return event;
                }
            }
        }
    })
    .await
    .expect("event within 5 s")
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn g4_receiver_end_to_end() {
    let _device = simulated_g4().await;
    let core = Core::new().unwrap();

    let id = core
        .open(OpenRequest {
            device: "sennheiser-ew-g3-g4".into(),
            model: "em-300-500-g4".into(),
            host: DEVICE_IP.to_string(),
            settings: Default::default(),
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
    })
    .await;

    // Telemetry and configuration arrive and merge into one snapshot.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let state = core.snapshot(id).unwrap().state;
    let ch = &state["channels"]["1"];
    assert_eq!(ch["name"], "Vocal 1");
    assert_eq!(ch["frequency_khz"], 606000);
    assert_eq!(ch["battery_percent"], 70);
    assert_eq!(ch["tx_mute"], false);

    // A set is acknowledged by the device's echo.
    let outcome = core
        .execute(id, "mute", params(json!({"muted": true})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));

    // A value the device refuses is reported with its code.
    let outcome = core
        .execute(id, "set_af_out", params(json!({"level_db": 5})))
        .await;
    assert!(matches!(
        outcome,
        Err(CommandError::DeviceError { code: Some(ref c), .. }) if c == "1020"
    ));

    // Validation happens before anything is sent.
    let outcome = core
        .execute(id, "set_af_out", params(json!({"level_db": 99})))
        .await;
    assert!(matches!(outcome, Err(CommandError::InvalidParams { .. })));
    let outcome = core.execute(id, "rf_mute", params(json!({}))).await;
    assert!(matches!(
        outcome,
        Err(CommandError::UnsupportedForModel { .. })
    ));

    core.close(id).await;
    wait_for(
        &core,
        |e| matches!(e, Event::Closed { device } if *device == id),
    )
    .await;
}
