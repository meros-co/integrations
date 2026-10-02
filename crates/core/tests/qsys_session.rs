//! A Q-SYS Core simulated on real TCP, driven through the public API: QRC's
//! JSON-RPC 2.0 with NUL-terminated messages (QSC Q-SYS Help 10.5.0, "QRC
//! Commands").
//!
//! The simulator pushes EngineStatus on connecting, as the help says a Core
//! does, accepts one Administrator user, answers StatusGet, Control.Set,
//! ChangeGroup.AddControl and ChangeGroup.AutoPoll, and pushes the change
//! group's results when a control it holds changes. An unknown control is
//! answered with error 8, a command before logon with error 10.

#![cfg(feature = "qsys")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type Log = Arc<Mutex<Vec<u8>>>;

fn framed(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec(v).unwrap();
    b.push(0);
    b
}

#[derive(Default)]
struct CoreState {
    logged_on: bool,
    gain: f64,
    group: Vec<String>,
    auto_poll_id: Option<Value>,
}

/// One reply (or none) and any pushes for a request.
fn answer(core: &mut CoreState, req: &Value) -> Vec<Value> {
    let id = req["id"].clone();
    let method = req["method"].as_str().unwrap_or("");
    let ok = |result: Value| json!({"jsonrpc": "2.0", "id": id, "result": result});
    let err = |code: i64, message: &str| json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}});
    if method == "Logon" {
        if req["params"] == json!({"User": "control", "Password": "1234"}) {
            core.logged_on = true;
            return vec![ok(json!(true))];
        }
        return vec![err(10, "Logon required")];
    }
    if !core.logged_on {
        return vec![err(10, "Logon required")];
    }
    let changes = |core: &CoreState| {
        json!({"Id": "meros", "Changes": [
            {"Name": "MainGain", "Value": core.gain, "String": format!("{:.1}dB", core.gain),
             "Position": (core.gain + 100.0) / 120.0}
        ]})
    };
    match method {
        "StatusGet" => vec![ok(json!({
            "Platform": "Core 510i", "State": "Active", "DesignName": "SAF-MainPA",
            "DesignCode": "qALFilm6IcAz", "IsRedundant": false, "IsEmulator": false,
            "Status": {"Code": 0, "String": "OK"}
        }))],
        "NoOp" => vec![ok(json!(true))],
        "ChangeGroup.AddControl" => {
            for c in req["params"]["Controls"].as_array().unwrap() {
                core.group.push(c.as_str().unwrap().to_string());
            }
            vec![ok(json!(true))]
        }
        "ChangeGroup.AutoPoll" => {
            core.auto_poll_id = Some(id.clone());
            // The first poll reports every control in the group.
            vec![ok(changes(core))]
        }
        "Control.Set" => {
            if req["params"]["Name"] != "MainGain" {
                return vec![err(8, "Unknown control")];
            }
            core.gain = req["params"]["Value"].as_f64().unwrap();
            let mut out = vec![ok(json!(true))];
            // The next automatic poll carries the change, as AutoPoll's
            // documented response: a result with the AutoPoll request's id.
            if let (Some(poll), true) = (
                &core.auto_poll_id,
                core.group.iter().any(|c| c == "MainGain"),
            ) {
                out.push(json!({"jsonrpc": "2.0", "id": poll, "result": changes(core)}));
            }
            out
        }
        _ => vec![err(-32601, "Method not found")],
    }
}

/// Accept one connection, push EngineStatus, answer every request.
async fn simulate() -> (u16, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let log: Log = Arc::default();
    let record = log.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        // QRC Commands, EngineStatus: sent whenever a client connects.
        let status = json!({"jsonrpc": "2.0", "method": "EngineStatus", "params": {
            "State": "Active", "DesignName": "SAF-MainPA", "DesignCode": "qALFilm6IcAz",
            "IsRedundant": false, "IsEmulator": false}});
        stream.write_all(&framed(&status)).await.unwrap();
        let mut core = CoreState {
            gain: -100.0,
            ..CoreState::default()
        };
        let mut buf = [0u8; 4096];
        let mut pending = Vec::new();
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            record.lock().unwrap().extend_from_slice(&buf[..n]);
            pending.extend_from_slice(&buf[..n]);
            let mut reply = Vec::new();
            while let Some(end) = pending.iter().position(|&b| b == 0) {
                let msg: Vec<u8> = pending.drain(..=end).collect();
                let req: Value = serde_json::from_slice(&msg[..msg.len() - 1]).unwrap();
                for out in answer(&mut core, &req) {
                    reply.extend(framed(&out));
                }
            }
            if !reply.is_empty() {
                stream.write_all(&reply).await.unwrap();
            }
        }
    });
    (port, log)
}

async fn wait_for(core: &Core, mut pred: impl FnMut(&Event) -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if core.next_events(64).await.iter().any(&mut pred) {
                return;
            }
        }
    })
    .await
    .expect("event within 10 s")
}

async fn wait_for_state(
    core: &Core,
    id: meros_integrations::DeviceId,
    pred: impl Fn(&Value) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if pred(&core.snapshot(id).unwrap().state) {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 10 s")
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

/// The core's stream split at its NUL terminators.
fn requests(log: &[u8]) -> Vec<Value> {
    assert_eq!(log.last(), Some(&0), "every message ends in NUL");
    log.split(|&b| b == 0)
        .filter(|m| !m.is_empty())
        .map(|m| serde_json::from_slice(m).unwrap())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn qrc_end_to_end() {
    let (port, log) = simulate().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "qsys".into(),
            model: "core".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"username": "control", "pin": "1234", "auto_poll_rate": 0.5})),
        })
        .unwrap();

    // Logon accepted, then StatusGet's answer.
    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
    })
    .await;
    wait_for_state(&core, id, |s| {
        s["engine"]["platform"] == "Core 510i" && s["engine"]["state"] == "Active"
    })
    .await;

    // A change group of one named control; AutoPoll starts by itself and its
    // first result reports the control.
    assert_eq!(
        core.execute(
            id,
            "change_group_add_controls",
            params(json!({"controls": ["MainGain"]}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["controls"]["MainGain"]["value"] == -100.0).await;

    // Control.Set answered by the Core; the change arrives by AutoPoll.
    assert_eq!(
        core.execute(
            id,
            "control_set",
            params(json!({"name": "MainGain", "value": -12.0, "ramp": 1.5}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| {
        s["controls"]["MainGain"]["value"] == -12.0
            && s["controls"]["MainGain"]["string"] == "-12.0dB"
    })
    .await;

    // An error reply.
    assert_eq!(
        core.execute(
            id,
            "control_set",
            params(json!({"name": "Nope", "value_bool": true}))
        )
        .await,
        Err(CommandError::DeviceError {
            code: Some("8".into()),
            message: "Unknown control".into()
        })
    );

    let sent = requests(&log.lock().unwrap().clone());
    let methods: Vec<&str> = sent.iter().map(|r| r["method"].as_str().unwrap()).collect();
    assert_eq!(
        &methods[..6],
        [
            "Logon",
            "StatusGet",
            "ChangeGroup.AddControl",
            "ChangeGroup.AutoPoll",
            "Control.Set",
            "Control.Set"
        ]
    );
    assert_eq!(
        sent[0],
        json!({"jsonrpc": "2.0", "id": 1, "method": "Logon",
            "params": {"User": "control", "Password": "1234"}})
    );
    assert_eq!(sent[3]["params"], json!({"Id": "meros", "Rate": 0.5}));
    assert_eq!(
        sent[4]["params"],
        json!({"Name": "MainGain", "Value": -12.0, "Ramp": 1.5})
    );
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_logon_is_terminal() {
    let (port, _log) = simulate().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "qsys".into(),
            model: "designer-emulation".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"username": "control", "pin": "9999"})),
        })
        .unwrap();
    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Unauthorized { .. } } if *device == id)
    })
    .await;
    assert!(matches!(
        core.execute(id, "status_get", params(json!({}))).await,
        Err(CommandError::Auth { .. })
    ));
    core.close(id).await;
}
