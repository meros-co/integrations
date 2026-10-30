//! generic-snmp through the public API against a fake agent on loopback
//! (snmp.rs's test agent, on a real UDP socket): v2c reads, writes and
//! informs, and v3 at authPriv with engine discovery, a walk, a SET read
//! back into state, and an authenticated, encrypted trap. Nothing but
//! 127.0.0.1 is bound.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{json, value::Value as Json};
use tokio::net::UdpSocket;

use super::snmp::agent::Agent;
use super::snmp::{Level, User, VarBind};
use super::snmp_ber::{Oid, Value};
use super::snmp_usm::{Auth, Privacy};
use crate::{Connection, Core, CoreOptions, DeviceId, Event, OpenRequest, Outcome};

fn core() -> Core {
    Core::with_catalog(
        crate::catalog::Catalog::embedded(),
        CoreOptions::new().bind_address([127, 0, 0, 1].into()),
    )
    .unwrap()
}

fn fake_agent() -> Agent {
    Agent::new(&[
        (
            "1.3.6.1.2.1.1.1.0",
            Value::OctetString(b"Fake UPS".to_vec()),
        ),
        ("1.3.6.1.2.1.1.3.0", Value::TimeTicks(777)),
        ("1.3.6.1.2.1.1.5.0", Value::OctetString(b"ups-a".to_vec())),
        ("1.3.6.1.4.1.99.2.1.0", Value::Gauge32(98)),
        ("1.3.6.1.4.1.99.2.2.0", Value::Integer(2)),
        ("1.3.6.1.4.1.99.2.3.0", Value::Counter64(1 << 33)),
    ])
}

/// Serve the agent on a loopback socket until the test ends.
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

fn open(core: &Core, port: u16, settings: Json) -> DeviceId {
    core.open(OpenRequest {
        device: "generic-snmp".into(),
        model: "generic".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: settings.as_object().unwrap().clone(),
        monitor: true,
    })
    .unwrap()
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

fn params(v: Json) -> crate::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn v2c_polls_reads_writes_and_answers_informs() {
    let agent = Arc::new(Mutex::new(fake_agent()));
    let socket = serve(agent.clone()).await;
    let port = socket.local_addr().unwrap().port();
    let trap_port = free_udp_port();
    let core = core();
    let id = open(
        &core,
        port,
        json!({
            "poll_oids": "charge=1.3.6.1.4.1.99.2.1.0 1.3.6.1.4.1.99.2.3.0",
            "poll_interval_ms": 200,
            "write_community": "private",
            "trap_port": trap_port,
        }),
    );
    let state = wait_for_state(&core, id, |s| s["values"]["charge"]["number"] == 98).await;
    assert_eq!(state["system"]["name"], "ups-a");
    assert_eq!(
        state["values"]["1.3.6.1.4.1.99.2.3.0"]["number"],
        1u64 << 33
    );
    assert_eq!(core.snapshot(id).unwrap().connection, Connection::Connected);
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());

    // The agent's value changes; the next poll has it.
    agent.lock().unwrap().table.insert(
        Oid::parse("1.3.6.1.4.1.99.2.1.0").unwrap(),
        Value::Gauge32(97),
    );
    wait_for_state(&core, id, |s| s["values"]["charge"]["number"] == 97).await;

    let out = core
        .execute(
            id,
            "set",
            params(json!({ "oid": "1.3.6.1.2.1.1.5.0", "type": "string", "value": "ups-b" })),
        )
        .await
        .unwrap();
    assert_eq!(
        out,
        Outcome::Value {
            value: json!({ "varbinds": [{ "oid": "1.3.6.1.2.1.1.5.0", "type": "octet_string", "value": "ups-b", "hex": "7570732d62" }] })
        }
    );
    let out = core
        .execute(
            id,
            "get_bulk",
            params(json!({ "oids": "1.3.6.1.4.1.99.2", "max_repetitions": 2 })),
        )
        .await
        .unwrap();
    let Outcome::Value { value } = out else {
        panic!()
    };
    assert_eq!(value["varbinds"].as_array().unwrap().len(), 2);

    // An inform from the agent: answered, and reported.
    let inform = agent.lock().unwrap().notification(
        true,
        41,
        "1.3.6.1.4.1.99.0.1",
        vec![VarBind {
            oid: Oid::parse("1.3.6.1.4.1.99.2.1.0").unwrap(),
            value: Value::Gauge32(50),
        }],
        None,
    );
    let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sender
        .send_to(&inform, ("127.0.0.1", trap_port))
        .await
        .unwrap();
    let mut buf = [0u8; 1500];
    let (n, _) = tokio::time::timeout(Duration::from_secs(5), sender.recv_from(&mut buf))
        .await
        .expect("the inform is answered")
        .unwrap();
    assert!(n > 0);
    let state = wait_for_state(&core, id, |s| s["traps"]["count"] == 1).await;
    assert_eq!(state["traps"]["last"]["kind"], "inform");
    assert_eq!(state["traps"]["last"]["trap_oid"], "1.3.6.1.4.1.99.0.1");
    // The polled object it carried.
    assert_eq!(state["values"]["charge"]["number"], 50);
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn v3_auth_priv_discovers_walks_sets_and_receives_traps() {
    let user = User::new(
        "operator",
        Level::AuthPriv,
        Some((Auth::Sha256, "maplesyrup")),
        Some((Privacy::Aes128, "pancakes99")),
    )
    .unwrap();
    let mut a = fake_agent();
    a.user = Some(user);
    let agent = Arc::new(Mutex::new(a));
    let socket = serve(agent.clone()).await;
    let port = socket.local_addr().unwrap().port();
    let trap_port = free_udp_port();
    let core = core();
    let id = open(
        &core,
        port,
        json!({
            "version": "v3",
            "user": "operator",
            "security_level": "auth_priv",
            "auth_protocol": "sha256",
            "auth_password": "maplesyrup",
            "priv_protocol": "aes",
            "priv_password": "pancakes99",
            "poll_oids": "mode=1.3.6.1.4.1.99.2.2.0",
            "trap_port": trap_port,
        }),
    );
    let state = wait_for_state(&core, id, |s| s["values"]["mode"]["number"] == 2).await;
    assert_eq!(state["version"], "v3");
    assert_eq!(state["engine"]["id"], "800000000466616b65");
    assert_eq!(state["system"]["description"], "Fake UPS");

    let out = core
        .execute(id, "walk", params(json!({ "oid": "1.3.6.1.4.1.99" })))
        .await
        .unwrap();
    let Outcome::Value { value } = out else {
        panic!()
    };
    assert_eq!(value["varbinds"].as_array().unwrap().len(), 3);

    core.execute(
        id,
        "set_multiple",
        params(json!({ "varbinds": [{ "oid": "1.3.6.1.4.1.99.2.2.0", "type": "integer", "value": "4" }] })),
    )
    .await
    .unwrap();
    assert_eq!(
        core.snapshot(id).unwrap().state["values"]["mode"]["number"],
        4
    );

    // A trap, authenticated and encrypted with the agent's engine's keys.
    let trap = agent
        .lock()
        .unwrap()
        .notification(false, 5, "1.3.6.1.6.3.1.1.5.3", vec![], None);
    let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    sender
        .send_to(&trap, ("127.0.0.1", trap_port))
        .await
        .unwrap();
    let event = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            for e in core.next_events(64).await {
                if let Event::Message {
                    device,
                    address,
                    args,
                    ..
                } = e
                {
                    if device == id {
                        return (address, args);
                    }
                }
            }
        }
    })
    .await
    .expect("a message event");
    assert_eq!(event.0, "1.3.6.1.6.3.1.1.5.3");
    assert_eq!(event.1["version"], "v3");
    assert_eq!(event.1["user"], "operator");
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_v3_password_is_never_answered() {
    let mut a = fake_agent();
    a.user = Some(
        User::new(
            "operator",
            Level::AuthNoPriv,
            Some((Auth::Sha1, "the right one")),
            None,
        )
        .unwrap(),
    );
    let socket = serve(Arc::new(Mutex::new(a))).await;
    let port = socket.local_addr().unwrap().port();
    let core = core();
    let id = open(
        &core,
        port,
        json!({
            "version": "v3", "user": "operator", "security_level": "auth_no_priv",
            "auth_password": "the wrong one", "timeout_ms": 200, "retries": 0,
        }),
    );
    let err = core
        .execute(id, "get", params(json!({ "oids": "1.3.6.1.2.1.1.5.0" })))
        .await;
    assert_eq!(err, Err(crate::CommandError::Timeout));
    core.close(id).await;
}
