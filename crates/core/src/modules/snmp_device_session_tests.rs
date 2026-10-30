//! An SNMP profile integration (apc-rack-pdu) through the public API against
//! a fake agent on loopback (snmp.rs's test agent, on a real UDP socket):
//! the rPDU2 walk named into state, a named command's SET read back, and a
//! PowerNet-MIB trap named. Nothing but 127.0.0.1 is bound.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{json, value::Value as Json};
use tokio::net::UdpSocket;

use super::snmp::agent::Agent;
use super::snmp_ber::Value;
use crate::{Core, CoreOptions, DeviceId, OpenRequest, Outcome};

fn core() -> Core {
    Core::with_catalog(
        crate::catalog::Catalog::embedded(),
        CoreOptions::new().bind_address([127, 0, 0, 1].into()),
    )
    .unwrap()
}

/// A Rack PDU 2G with two switched outlets: their names, states and
/// control commands (PowerNet-MIB rPDU2OutletSwitched).
fn fake_pdu() -> Agent {
    let status = "1.3.6.1.4.1.318.1.1.26.9.2.3.1";
    let control = "1.3.6.1.4.1.318.1.1.26.9.2.4.1";
    Agent::new(&[
        (
            "1.3.6.1.2.1.1.1.0",
            Value::OctetString(b"APC Rack PDU".to_vec()),
        ),
        ("1.3.6.1.2.1.1.3.0", Value::TimeTicks(1234)),
        (
            &format!("{status}.3.1"),
            Value::OctetString(b"Encoder".to_vec()),
        ),
        (
            &format!("{status}.3.2"),
            Value::OctetString(b"Switch".to_vec()),
        ),
        (&format!("{status}.5.1"), Value::Integer(1)),
        (&format!("{status}.5.2"), Value::Integer(2)),
        (&format!("{control}.5.1"), Value::Integer(1)),
        (&format!("{control}.5.2"), Value::Integer(2)),
    ])
}

async fn serve(agent: Arc<Mutex<Agent>>) -> Arc<UdpSocket> {
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let s = socket.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        while let Ok((n, from)) = s.recv_from(&mut buf).await {
            let answer = agent.lock().unwrap().handle(&buf[..n]);
            if let Some(answer) = answer {
                let _ = s.send_to(&answer, from).await;
            }
        }
    });
    socket
}

async fn wait_for_state(core: &Core, id: DeviceId, pred: impl Fn(&Json) -> bool) -> Json {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let state = core.snapshot(id).unwrap().state;
            if pred(&state) {
                return state;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 20 s")
}

fn free_udp_port() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pdu_is_walked_switched_and_heard() {
    let agent = Arc::new(Mutex::new(fake_pdu()));
    let socket = serve(agent.clone()).await;
    let port = socket.local_addr().unwrap().port();
    let trap_port = free_udp_port();
    let core = core();
    let id = core
        .open(OpenRequest {
            device: "apc-rack-pdu".into(),
            model: "rack-pdu-2g".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: json!({ "poll_interval_ms": 300, "write_community": "private", "trap_port": trap_port })
                .as_object()
                .unwrap()
                .clone(),
            monitor: true,
        })
        .unwrap();
    let state = wait_for_state(&core, id, |s| {
        s["outlet_switched_status"]["2"]["state"] == "on"
    })
    .await;
    assert_eq!(state["system"]["description"], "APC Rack PDU");
    assert_eq!(state["outlet_switched_status"]["1"]["name"], "Encoder");
    assert_eq!(state["outlet_switched_status"]["1"]["state"], "off");
    assert_eq!(
        state["outlet_switched_control"]["2"]["command"],
        "immediate_off"
    );

    let out = core
        .execute(
            id,
            "set_outlet_switched_control_command",
            json!({ "index": 2, "value": "immediate_on" })
                .as_object()
                .unwrap()
                .clone(),
        )
        .await;
    assert_eq!(out, Ok(Outcome::Ack));
    wait_for_state(&core, id, |s| {
        s["outlet_switched_control"]["2"]["command"] == "immediate_on"
    })
    .await;

    // A PowerNet-MIB trap (rPDUOutletOn, enterprises.318.0.268), named.
    let trap = agent
        .lock()
        .unwrap()
        .notification(false, 7, "1.3.6.1.4.1.318.0.268", vec![], None);
    let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sender
        .send_to(&trap, ("127.0.0.1", trap_port))
        .await
        .unwrap();
    let state = wait_for_state(&core, id, |s| s["traps"]["count"] == 1).await;
    assert_eq!(state["traps"]["last"]["trap_oid"], "1.3.6.1.4.1.318.0.268");
    assert_eq!(state["traps"]["last"]["name"], "rPDUOutletOn");
    core.close(id).await;
}
