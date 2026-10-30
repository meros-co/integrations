//! A Middle Atlantic RackLink simulated on real TCP, driven through the public
//! API: it accepts the login, pings the controller and expects the pong,
//! answers the outlet and contact counts, states and names, and switches an
//! outlet, as the control protocol manual describes.

#![cfg(feature = "middle-atlantic-racklink")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{Connection, Core, Event, OpenRequest, Outcome};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn frame(envelope: &[u8]) -> Vec<u8> {
    let mut raw = vec![0xFE, envelope.len() as u8];
    raw.extend_from_slice(envelope);
    let sum = raw.iter().map(|b| u32::from(*b)).sum::<u32>() & 0x7F;
    raw.push(sum as u8);
    let mut out = vec![0xFE];
    for &b in &raw[1..] {
        if matches!(b, 0xFD..=0xFF) {
            out.extend([0xFD, !b]);
        } else {
            out.push(b);
        }
    }
    out.push(0xFF);
    out
}

/// What the unit answers to one envelope it received.
fn answer(env: &[u8], outlets: &mut [u8; 2]) -> Vec<u8> {
    let (cmd, sub, data) = (env[1], env[2], &env[3..]);
    match (cmd, sub) {
        (0x02, 0x01) => {
            let accepted = data == b"user|secret";
            frame(&[0x00, 0x02, 0x10, u8::from(accepted)])
        }
        (0x22, 0x02) => {
            let mut e = vec![0x00, 0x22, 0x10];
            e.extend_from_slice(b"CCXXXXXXXXXXXXXX");
            frame(&e)
        }
        (0x32, 0x02) => {
            let mut e = vec![0x00, 0x32, 0x10];
            e.extend_from_slice(b"XXXXXXXX");
            frame(&e)
        }
        (0x20, 0x02) => {
            let n = data[0];
            let mut e = vec![0x00, 0x20, 0x10, n, outlets[(n - 1) as usize]];
            e.extend_from_slice(b"0005");
            frame(&e)
        }
        (0x20, 0x01) => {
            let n = data[0];
            outlets[(n - 1) as usize] = data[1];
            let mut e = vec![0x00, 0x20, 0x10, n, data[1]];
            e.extend_from_slice(&data[2..6]);
            frame(&e)
        }
        (0x21, 0x02) => {
            let mut e = vec![0x00, 0x21, 0x10, data[0]];
            e.extend_from_slice(format!("Outlet {}", data[0]).as_bytes());
            frame(&e)
        }
        // A Select has no advanced commands.
        _ => frame(&[0x00, 0x10, 0x10, 0x04]),
    }
}

async fn simulated_racklink() -> (u16, Arc<Mutex<bool>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let ponged = Arc::new(Mutex::new(false));
    let pong = ponged.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut outlets = [1u8, 0u8];
        let mut buf = [0u8; 2048];
        let mut body = Vec::new();
        let mut escaped = false;
        let mut logged_in = false;
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            let mut replies = Vec::new();
            for &b in &buf[..n] {
                match b {
                    0xFE => body.clear(),
                    0xFF => {
                        // [length, envelope.., checksum]
                        let env = body[1..body.len() - 1].to_vec();
                        body.clear();
                        if env[1] == 0x01 && env[2] == 0x10 {
                            *pong.lock().unwrap() = true;
                            continue;
                        }
                        replies.extend(answer(&env, &mut outlets));
                        if env[1] == 0x02 && !logged_in {
                            logged_in = true;
                            // The unit pings once the controller is in.
                            replies.extend(frame(&[0x00, 0x01, 0x01]));
                        }
                    }
                    0xFD => escaped = true,
                    b => {
                        body.push(if escaped { !b } else { b });
                        escaped = false;
                    }
                }
            }
            if !replies.is_empty() {
                stream.write_all(&replies).await.unwrap();
            }
        }
    });
    (port, ponged)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

async fn wait_for_state(
    core: &Core,
    id: meros_integrations::DeviceId,
    pred: impl Fn(&Value) -> bool,
) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let state = core.snapshot(id).unwrap().state;
            if pred(&state) {
                return state;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 10 s")
}

#[tokio::test(flavor = "multi_thread")]
async fn racklink_end_to_end() {
    let (port, ponged) = simulated_racklink().await;
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "middle-atlantic-racklink".into(),
            model: "racklink-select".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({ "password": "secret" })),
            monitor: true,
        })
        .unwrap();
    let state = wait_for_state(&core, id, |s| s["outlets"]["2"]["name"] == "Outlet 2").await;
    assert_eq!(state["outlets"]["1"]["state"], "on");
    assert_eq!(state["outlets"]["2"]["state"], "off");
    assert_eq!(state["outlets"]["1"]["cycle_time"], 5);
    assert_eq!(core.snapshot(id).unwrap().connection, Connection::Connected);
    assert!(*ponged.lock().unwrap(), "the ping was answered");

    let outcome = core
        .execute(
            id,
            "set_outlet",
            params(json!({ "outlet": 2, "action": "on" })),
        )
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    wait_for_state(&core, id, |s| s["outlets"]["2"]["state"] == "on").await;
    let outcome = core
        .execute(id, "get_outlet_name", params(json!({ "outlet": 1 })))
        .await;
    assert_eq!(
        outcome,
        Ok(Outcome::Value {
            value: json!("Outlet 1")
        })
    );
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_password_is_unauthorized() {
    let (port, _) = simulated_racklink().await;
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "middle-atlantic-racklink".into(),
            model: "racklink-select".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({ "password": "wrong" })),
            monitor: true,
        })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let events = core.next_events(64).await;
            if events.iter().any(|e| {
                matches!(e, Event::Connection { device, connection: Connection::Unauthorized { .. } } if *device == id)
            }) {
                return;
            }
        }
    })
    .await
    .expect("unauthorized within 10 s");
    core.close(id).await;
}
