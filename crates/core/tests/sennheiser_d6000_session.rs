//! An EM 6000 simulated on real UDP, driven through the public API.
//!
//! Modelled on RFDeck's `fakeDigital6000Device.ts`: it answers subscriptions
//! with the channel tree, streams the metering array, answers identity queries,
//! and echoes control writes with the resulting values. Like that simulator it
//! does not reflect `/osc/xid`, so replies are matched by path.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use meros_integrations::{Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::net::UdpSocket;

const DEVICE_IP: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 2);

async fn simulated_em6000() -> (u16, tokio::task::JoinHandle<()>) {
    let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(DEVICE_IP), 0))
        .await
        .unwrap();
    let port = socket.local_addr().unwrap().port();
    let task = tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        let mut mute = [false, false];
        loop {
            let Ok((n, from)) = socket.recv_from(&mut buf).await else {
                return;
            };
            let Ok(msg) = serde_json::from_slice::<Value>(&buf[..n]) else {
                continue;
            };
            let mut replies = Vec::new();
            if let Some(subs) = msg
                .pointer("/osc/state/subscribe")
                .and_then(Value::as_array)
            {
                for tree in subs {
                    if tree.get("mm").is_some() {
                        replies.push(
                            json!({"mm": [[83,0,53,0,1,1,128,165,0],[83,0,53,0,1,1,128,165,0]]}),
                        );
                    }
                    for ch in 1..=2 {
                        if tree.get(format!("rx{ch}")).is_some() {
                            replies.push(json!({ format!("rx{ch}"): {
                                "name": format!("Channel{ch}"),
                                "carrier": 470100 + ch * 25,
                                "audio_mute": mute[ch - 1],
                                "active_warnings": [],
                                "skx": {"battery": ["70%", "5:12"], "name": "SKM", "type": ["SKM 6000"]},
                            }}));
                        }
                    }
                }
                replies.push(json!({"osc": {"state": {"subscribe": subs}}}));
            } else if msg.get("device").is_some() {
                replies.push(json!({"device": {
                    "identity": {"version": "1.1.4.74", "vendor": "Sennheiser", "product": "EM 6000"},
                    "name": "Rack 1",
                }}));
            } else {
                for ch in 1..=2usize {
                    let Some(block) = msg.get(format!("rx{ch}")) else {
                        continue;
                    };
                    if let Some(m) = block.get("audio_mute").and_then(Value::as_bool) {
                        mute[ch - 1] = m;
                        replies.push(json!({ format!("rx{ch}"): {"audio_mute": m} }));
                    }
                    if block.get("identify").is_some() {
                        replies.push(json!({ format!("rx{ch}"): {"identify": true} }));
                    }
                }
            }
            for reply in replies {
                let _ = socket.send_to(reply.to_string().as_bytes(), from).await;
            }
        }
    });
    (port, task)
}

async fn wait_for(core: &Core, mut pred: impl FnMut(&Event) -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if core.next_events(64).await.iter().any(&mut pred) {
                return;
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
async fn em6000_end_to_end() {
    let (port, _device) = simulated_em6000().await;
    let core = Core::new().unwrap();

    let id = core
        .open(OpenRequest {
            device: "sennheiser-digital-6000".into(),
            model: "em-6000".into(),
            host: DEVICE_IP.to_string(),
            port: Some(port),
            settings: Default::default(),
        })
        .unwrap();

    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["device"]["product"], "EM 6000");
    let ch1 = &state["channels"]["1"];
    assert_eq!(ch1["name"], "Channel1");
    assert_eq!(ch1["frequency_khz"], 470125);
    assert_eq!(ch1["rf"]["antenna_a_dbm"], -86.0);
    assert_eq!(
        ch1["transmitter"]["battery"],
        json!({"state": "70%", "minutes": 312})
    );

    let outcome = core
        .execute(id, "mute", params(json!({"channel": 2, "muted": true})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    assert_eq!(
        core.snapshot(id).unwrap().state["channels"]["2"]["mute"],
        true
    );

    let outcome = core
        .execute(id, "identify", params(json!({"channel": 1})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));

    core.close(id).await;
}
