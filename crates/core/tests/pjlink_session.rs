//! PJLink projectors simulated on real TCP, driven through the public API.
//!
//! The simulator greets as the specification does ("PJLINK 1 498e4a67", the
//! random number of its examples), checks the digest the core sends against
//! the password (SHA-256 per version 2.10 when it offers that, MD5 per
//! Class 1 otherwise), answers the commands the core sends one at a time,
//! and refuses a wrong password with "PJLINK ERRA".

#![cfg(feature = "pjlink")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use md5::Md5;
use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const PASSWORD: &str = "JBMIAProjectorLink";
const RANDOM4: &str = "498e4a67";
/// Class 2 v2.10 §5.1: the projector's 16-byte random number.
const RANDOM16: &str = "3db25e10f69c47a85adb24cf361897e0";

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

struct Projector {
    /// Offers the version 2.10 SHA-256 exchange.
    sha256: bool,
    class: u8,
    power: Mutex<String>,
    input: Mutex<String>,
    connections: AtomicUsize,
    /// Every line received, as sent.
    lines: Mutex<Vec<String>>,
    /// Authentications that succeeded, by method.
    authenticated: Mutex<Vec<&'static str>>,
}

impl Projector {
    fn answer(&self, command: &str) -> Option<String> {
        // "%1POWR 1" -> class, body, param.
        if command.len() < 7 || !command.starts_with('%') {
            return None;
        }
        let class = &command[1..2];
        let body = command[2..6].to_ascii_uppercase();
        let param = &command[7..];
        let reply = match (body.as_str(), param) {
            ("POWR", "?") => self.power.lock().unwrap().clone(),
            ("POWR", "1" | "0") => {
                *self.power.lock().unwrap() = param.into();
                "OK".into()
            }
            ("INPT", "?") => self.input.lock().unwrap().clone(),
            ("INPT", code) if ["11", "31", "32"].contains(&code) => {
                *self.input.lock().unwrap() = code.into();
                "OK".into()
            }
            ("INPT", _) => "ERR2".into(),
            ("CLSS", "?") => self.class.to_string(),
            ("NAME", "?") => "Hall Left".into(),
            ("INF1", "?") => "JBMIA".into(),
            ("INF2", "?") => "Simulator".into(),
            ("INFO", "?") => String::new(),
            ("INST", "?") => "11 31 32".into(),
            ("INNM", "?31") => "HDMI 1".into(),
            ("INNM", _) => "Other".into(),
            ("AVMT", "?") => "30".into(),
            ("ERST", "?") => "000010".into(),
            ("LAMP", "?") => "1234 1".into(),
            ("FREZ", "?") if self.class == 2 => "0".into(),
            ("IRES", "?") if self.class == 2 => "1920x1080".into(),
            _ => "ERR1".into(),
        };
        Some(format!("%{class}{body}={reply}\r"))
    }

    async fn serve(self: Arc<Self>, mut stream: TcpStream) {
        stream
            .write_all(format!("PJLINK 1 {RANDOM4}\r").as_bytes())
            .await
            .unwrap();
        let mut sha_level = false;
        let mut authed = false;
        let mut pending = Vec::new();
        let mut buf = [0u8; 1024];
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
                let reply = if line == "PJLINK 2" {
                    if self.sha256 {
                        sha_level = true;
                        Some(format!("PJLINK 2 {RANDOM16}\r"))
                    } else {
                        // An older projector reads it as a bad digest.
                        Some("PJLINK ERRA\r".into())
                    }
                } else if !authed {
                    let (ok, method, command) = if sha_level {
                        // Controller random (32 hex), SHA-256 (64 hex), command.
                        let controller = unhex(&line[..32]);
                        let projector = unhex(RANDOM16);
                        let xor: Vec<u8> = projector
                            .iter()
                            .zip(&controller)
                            .map(|(a, b)| a ^ b)
                            .collect();
                        let expected = hex(&Sha256::digest(
                            format!("{}{PASSWORD}", hex(&xor)).as_bytes(),
                        ));
                        (line[32..96] == expected, "sha256", &line[96..])
                    } else {
                        let expected = hex(&Md5::digest(format!("{RANDOM4}{PASSWORD}").as_bytes()));
                        (
                            line.len() > 32 && line[..32] == expected,
                            "md5",
                            &line[32.min(line.len())..],
                        )
                    };
                    if ok {
                        authed = true;
                        self.authenticated.lock().unwrap().push(method);
                        self.answer(command)
                    } else {
                        Some("PJLINK ERRA\r".into())
                    }
                } else {
                    self.answer(&line)
                };
                if let Some(reply) = reply {
                    stream.write_all(reply.as_bytes()).await.unwrap();
                }
            }
        }
    }
}

async fn simulate(sha256: bool, class: u8) -> (u16, Arc<Projector>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let projector = Arc::new(Projector {
        sha256,
        class,
        power: Mutex::new("0".into()),
        input: Mutex::new("11".into()),
        connections: AtomicUsize::new(0),
        lines: Mutex::default(),
        authenticated: Mutex::default(),
    });
    let p = projector.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            p.connections.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(p.clone().serve(stream));
        }
    });
    (port, projector)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, port: u16, model: &str, password: &str) -> meros_integrations::DeviceId {
    core.open(OpenRequest {
        device: "pjlink".into(),
        model: model.into(),
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
async fn class_2_with_sha256_authentication() {
    let (port, projector) = simulate(true, 2).await;
    let core = Core::new().unwrap();
    let id = open(&core, port, "class-2", PASSWORD);

    // The first poll reads identity and status.
    wait_for_state(&core, id, |s| {
        s["name"] == "Hall Left"
            && s["info"]["class"] == 2
            && s["inputs"]["31"]["name"] == "HDMI 1"
            && s["power"] == "off"
            && s["errors"]["filter"] == "warning"
            && s["lamps"]["1"]["hours"] == 1234
            && s["signal"]["horizontal"] == 1920
    })
    .await;

    // A command, then its follow-up query shows in state.
    assert_eq!(
        core.execute(id, "set_power", params(json!({"on": true})))
            .await,
        Ok(Outcome::Ack)
    );
    wait_for_state(&core, id, |s| s["power"] == "on").await;

    // A query.
    assert_eq!(
        core.execute(id, "get_input", params(json!({}))).await,
        Ok(Outcome::Value {
            value: json!({"code": "11", "source": "rgb", "terminal": "1"})
        })
    );

    // An error reply is a device error carrying its code.
    match core
        .execute(id, "set_input", params(json!({"input": "59"})))
        .await
    {
        Err(CommandError::DeviceError { code, .. }) => assert_eq!(code.as_deref(), Some("ERR2")),
        other => panic!("{other:?}"),
    }

    let auth = projector.authenticated.lock().unwrap().clone();
    assert!(
        !auth.is_empty() && auth.iter().all(|m| *m == "sha256"),
        "{auth:?}"
    );
    let lines = projector.lines.lock().unwrap().clone();
    assert!(lines
        .iter()
        .any(|l| l == "%1POWR 1" || l.ends_with("%1POWR 1")));
    assert!(lines.iter().any(|l| l.ends_with("%2INPT ?")));
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn class_1_falls_back_to_md5() {
    let (port, projector) = simulate(false, 1).await;
    let core = Core::new().unwrap();
    let id = open(&core, port, "class-1", PASSWORD);

    wait_for_state(&core, id, |s| {
        s["name"] == "Hall Left" && s["power"] == "off"
    })
    .await;
    assert_eq!(
        core.execute(id, "get_power", params(json!({}))).await,
        Ok(Outcome::Value {
            value: json!("off")
        })
    );

    // PJLINK 2 was tried once and refused; every session since used MD5,
    // with the specification's digest for its random number and password.
    let lines = projector.lines.lock().unwrap().clone();
    assert_eq!(lines.iter().filter(|l| *l == "PJLINK 2").count(), 1);
    assert!(lines
        .iter()
        .any(|l| l.starts_with("5d8409bc1c3fa39749434aa3a5c38682%1")));
    let auth = projector.authenticated.lock().unwrap().clone();
    assert!(
        !auth.is_empty() && auth.iter().all(|m| *m == "md5"),
        "{auth:?}"
    );
    // Class 1 sends INPT as %1.
    assert!(lines.iter().any(|l| l.ends_with("%1INPT ?")));
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_password_is_never_retried() {
    let (port, projector) = simulate(true, 2).await;
    let core = Core::new().unwrap();
    let id = open(&core, port, "class-2", "wrongpassword");

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

    assert!(matches!(
        core.execute(id, "get_power", params(json!({}))).await,
        Err(CommandError::Auth { .. })
    ));
    // Past the poll interval and any backoff: still the one connection.
    tokio::time::sleep(Duration::from_millis(6_500)).await;
    assert_eq!(projector.connections.load(Ordering::SeqCst), 1);
    assert!(projector.authenticated.lock().unwrap().is_empty());
    core.close(id).await;
}
