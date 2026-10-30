//! A Panasonic projector simulated on real TCP, driven through the public
//! API.
//!
//! The simulator speaks NTCONTROL as the Operating Instructions describe:
//! each connection is greeted "NTCONTROL 1 <random>" (protect mode), the
//! command must carry the MD5 hash of user, password and that random number
//! and "00", the projector answers "00" and the data, and closes the
//! connection; a wrong hash is answered ERRA.

#![cfg(feature = "panasonic-projector")]

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use md5::{Digest, Md5};
use meros_integrations::{json, value::Value};
use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const USER: &str = "dispadmin";
const PASSWORD: &str = "@Panasonic";

struct Projector {
    power: Mutex<&'static str>,
    input: Mutex<String>,
    contrast: Mutex<String>,
    connections: AtomicUsize,
    commands: Mutex<Vec<String>>,
}

impl Projector {
    fn answer(&self, command: &str) -> String {
        let on = *self.power.lock().unwrap() == "001";
        match command {
            "QPW" => self.power.lock().unwrap().to_string(),
            "PON" => {
                *self.power.lock().unwrap() = "001";
                "PON".into()
            }
            "POF" => {
                *self.power.lock().unwrap() = "000";
                "POF".into()
            }
            "QIN" => self.input.lock().unwrap().clone(),
            "QVR" => self.contrast.lock().unwrap().clone(),
            c if on && c.starts_with("IIS:") => {
                *self.input.lock().unwrap() = c[4..].to_string();
                c.into()
            }
            c if on && c.starts_with("VCN:") => {
                *self.contrast.lock().unwrap() = c[4..].to_string();
                c.into()
            }
            _ => "ERR1".into(),
        }
    }

    async fn serve(self: Arc<Self>, mut stream: TcpStream) {
        let n = self.connections.fetch_add(1, Ordering::SeqCst);
        let random = format!("{:08x}", 0x2318_1e1e_u32.wrapping_add(n as u32));
        stream
            .write_all(format!("NTCONTROL 1 {random}\r").as_bytes())
            .await
            .unwrap();
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            if stream.read(&mut byte).await.unwrap_or(0) == 0 {
                return;
            }
            if byte[0] == b'\r' {
                break;
            }
            line.push(byte[0]);
        }
        let line = String::from_utf8(line).unwrap();
        let hash: String = Md5::digest(format!("{USER}:{PASSWORD}:{random}").as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let reply = if line.len() > 34 && line[..32] == hash && &line[32..34] == "00" {
            let command = &line[34..];
            self.commands.lock().unwrap().push(command.to_string());
            let a = self.answer(command);
            if a.starts_with("ERR") {
                a
            } else {
                format!("00{a}")
            }
        } else {
            "ERRA".into()
        };
        let _ = stream.write_all(format!("{reply}\r").as_bytes()).await;
        // The projector disconnects right after answering.
    }
}

async fn simulate() -> (u16, Arc<Projector>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let projector = Arc::new(Projector {
        power: Mutex::new("000"),
        input: Mutex::new("HD1".into()),
        contrast: Mutex::new("032".into()),
        connections: AtomicUsize::new(0),
        commands: Mutex::default(),
    });
    let p = projector.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(p.clone().serve(stream));
        }
    });
    (port, projector)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, port: u16, password: &str) -> meros_integrations::DeviceId {
    core.open(OpenRequest {
        device: "panasonic-projector".into(),
        model: "pt-rz990".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(json!({"password": password})),
        monitor: true,
    })
    .unwrap()
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

#[tokio::test(flavor = "multi_thread")]
async fn each_command_is_an_authenticated_connection_of_its_own() {
    let (port, projector) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, PASSWORD);
    wait_for_state(&core, id, |s| s["power"] == "standby").await;

    assert_eq!(
        core.execute(id, "set_power", params(json!({"on": true})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["power"] == "on").await;

    assert_eq!(
        core.execute(id, "set_input_select", params(json!({"value": "HD2"})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["settings"]["input_select"] == "HD2").await;

    assert_eq!(
        core.execute(id, "get_contrast", params(json!({}))).await,
        Ok(Outcome::Value {
            value: json!("032")
        })
    );
    // ERR1 is a device error.
    assert!(matches!(
        core.execute(id, "set_shutter", params(json!({"value": "1"})))
            .await,
        Err(CommandError::DeviceError { .. })
    ));
    let commands = projector.commands.lock().unwrap().clone();
    assert!(commands.contains(&"PON".to_string()));
    assert!(commands.contains(&"IIS:HD2".to_string()));
    assert!(projector.connections.load(Ordering::SeqCst) >= commands.len());
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_password_is_never_retried() {
    let (port, projector) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, "wrong");
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let events = core.next_events(64).await;
            if events.iter().any(|e| {
                matches!(e, Event::Connection { device, connection: Connection::Unauthorized { .. } }
                    if *device == id)
            }) {
                return;
            }
        }
    })
    .await
    .expect("unauthorized within 10 s");
    let before = projector.connections.load(Ordering::SeqCst);
    assert!(matches!(
        core.execute(id, "get_power", params(json!({}))).await,
        Err(CommandError::Auth { .. })
    ));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(projector.connections.load(Ordering::SeqCst), before);
    core.close(id).await;
}
