//! An Epson projector simulated on real TCP, driven through the public API.
//!
//! The simulator takes ESC/VP.net's 16-byte connect header and answers with
//! one beginning "ESC/VP.net", then answers ESC/VP21 commands as the guide
//! describes: `NAME=value` and CR for a query, then a colon; a colon alone
//! for a set; ERR and CR then a colon for what it does not take.

#![cfg(feature = "epson-projector")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{CommandError, Core, OpenRequest, Outcome};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct Projector {
    power: Mutex<&'static str>,
    brightness: Mutex<String>,
    lines: Mutex<Vec<String>>,
    headers: Mutex<usize>,
}

impl Projector {
    fn answer(&self, line: &str) -> String {
        let on = *self.power.lock().unwrap() == "01";
        let reply = match line {
            "" => String::new(),
            "PWR?" => format!("PWR={}\r", self.power.lock().unwrap()),
            "PWR ON" => {
                *self.power.lock().unwrap() = "01";
                String::new()
            }
            "PWR OFF" => {
                *self.power.lock().unwrap() = "04";
                String::new()
            }
            "ERR?" => "ERR=00\r".into(),
            "LAMP?" => "LAMP=1520\r".into(),
            "SNO?" => "SNO=X4CJ8800123\r".into(),
            "SOURCE?" if on => "SOURCE=30\r".into(),
            "BRIGHT?" if on => format!("BRIGHT={}\r", self.brightness.lock().unwrap()),
            l if on && l.starts_with("BRIGHT ") => {
                *self.brightness.lock().unwrap() = l[7..].to_string();
                String::new()
            }
            _ => "ERR\r".into(),
        };
        format!("{reply}:")
    }

    async fn serve(self: Arc<Self>, mut stream: TcpStream) {
        let mut header = [0u8; 16];
        if stream.read_exact(&mut header).await.is_err() {
            return;
        }
        assert_eq!(&header[..10], b"ESC/VP.net");
        assert_eq!(&header[10..12], &[0x10, 0x03]);
        *self.headers.lock().unwrap() += 1;
        let mut answer = b"ESC/VP.net".to_vec();
        answer.extend_from_slice(&[0x10, 0x03, 0x00, 0x00, 0x20, 0x00]);
        stream.write_all(&answer).await.unwrap();
        let mut pending = Vec::new();
        let mut buf = [0u8; 256];
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            pending.extend_from_slice(&buf[..n]);
            while let Some(end) = pending.iter().position(|&b| b == b'\r') {
                let raw: Vec<u8> = pending.drain(..=end).collect();
                let line = String::from_utf8(raw[..raw.len() - 1].to_vec()).unwrap();
                self.lines.lock().unwrap().push(line.clone());
                let out = self.answer(&line);
                stream.write_all(out.as_bytes()).await.unwrap();
            }
        }
    }
}

async fn simulate() -> (u16, Arc<Projector>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let projector = Arc::new(Projector {
        power: Mutex::new("04"),
        brightness: Mutex::new("128".into()),
        lines: Mutex::default(),
        headers: Mutex::new(0),
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

fn open(core: &Core, port: u16, monitor: bool) -> meros_integrations::DeviceId {
    core.open(OpenRequest {
        device: "epson-projector".into(),
        model: "eb-l1xxx-series-2019".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(json!({})),
        monitor,
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_projector_is_polled_and_controlled_over_esc_vp_net() {
    let (port, projector) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, true);

    wait_for_state(&core, id, |s| {
        s["power"] == "standby_network_on" && s["error"]["code"] == "00" && s["lamp_hours"] == 1520
    })
    .await;

    assert_eq!(
        core.execute(id, "set_power", params(json!({"on": true})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["power"] == "on").await;

    assert_eq!(
        core.execute(id, "set_brightness", params(json!({"value": "200"})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["settings"]["brightness"] == "200").await;

    // ERR is a device error.
    assert!(matches!(
        core.execute(id, "set_aspect", params(json!({"value": "99"})))
            .await,
        Err(CommandError::DeviceError { .. })
    ));
    let lines = projector.lines.lock().unwrap().clone();
    assert!(lines.contains(&"PWR ON".to_string()));
    assert!(lines.contains(&"BRIGHT 200".to_string()));
    assert_eq!(*projector.headers.lock().unwrap(), 1);
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn commands_only_sends_the_null_command_between_commands() {
    let (port, projector) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, port, false);
    assert_eq!(
        core.execute(id, "get_lamp_hours", params(json!({}))).await,
        Ok(Outcome::Value {
            value: json!("1520")
        })
    );
    let lines = projector.lines.lock().unwrap().clone();
    assert!(
        lines.iter().all(|l| l.is_empty() || l == "LAMP?"),
        "{lines:?}"
    );
    core.close(id).await;
}
