//! Ultimatte 12 preset archive and restore (the ultimatte-preset-archive
//! extension) against a keyer simulated on loopback: its text protocol on
//! one port, sending the status dump with a FILE LIST, and its binary
//! settings channel on another, holding presets, GPISettings and
//! SavedSettings and answering one request per connection as bmdwire's
//! ultimattewire (MIT) recorded it. Its first settings connection is dropped
//! unanswered, as a cold unit's often is.

#![cfg(feature = "blackmagic-ultimatte")]

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use meros_integrations::{CommandError, Core, DeviceId, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const PREFIX: &str =
    "/private/var/folders/00/000000000000000000000000000000/T/Ultimatte Software Control-PYREST/";

#[derive(Default)]
struct Unit {
    store: BTreeMap<String, Vec<u8>>,
    /// Every write, in order: (opcode, name field).
    writes: Vec<(u16, String)>,
    connections: usize,
}

async fn control(listener: TcpListener) {
    loop {
        let Ok((mut stream, _)) = listener.accept().await else {
            return;
        };
        tokio::spawn(async move {
            let dump = "PROTOCOL PREAMBLE:\nVersion: 2.3\n\nFILE LIST:\nGreen Set\nBlue Set\n\nEND PRELUDE:\n\n";
            if stream.write_all(dump.as_bytes()).await.is_err() {
                return;
            }
            let mut buf = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let Ok(n) = stream.read(&mut chunk).await else {
                    return;
                };
                if n == 0 {
                    return;
                }
                buf.extend_from_slice(&chunk[..n]);
                while let Some(at) = buf.windows(2).position(|w| w == b"\n\n") {
                    buf.drain(..at + 2);
                    if stream.write_all(b"ACK\n\n").await.is_err() {
                        return;
                    }
                }
            }
        });
    }
}

async fn read_u32(stream: &mut TcpStream) -> Option<u32> {
    let mut b = [0u8; 4];
    stream.read_exact(&mut b).await.ok()?;
    Some(u32::from_be_bytes(b))
}

async fn settings_request(unit: Arc<Mutex<Unit>>, mut stream: TcpStream) -> Option<()> {
    let mut op = [0u8; 2];
    stream.read_exact(&mut op).await.ok()?;
    let opcode = u16::from_be_bytes(op);
    let len = read_u32(&mut stream).await? as usize;
    let mut name = vec![0u8; len];
    stream.read_exact(&mut name).await.ok()?;
    let name = String::from_utf8(name).ok()?;
    match opcode {
        0x0000 | 0x0003 => {
            let body = unit.lock().unwrap().store.get(&name).cloned();
            match body {
                Some(body) => {
                    let mut out = (body.len() as u32).to_be_bytes().to_vec();
                    out.extend_from_slice(&body);
                    stream.write_all(&out).await.ok()?;
                }
                // An unknown name: the unit closes without an answer.
                None => return None,
            }
        }
        0x0100 | 0x0103 => {
            let len = read_u32(&mut stream).await? as usize;
            let mut data = vec![0u8; len];
            stream.read_exact(&mut data).await.ok()?;
            let mut end = [0u8; 1];
            stream.read_exact(&mut end).await.ok()?;
            assert_eq!(end, [0], "a write ends with a zero byte");
            let key = name.rsplit('/').next().unwrap().to_string();
            if opcode == 0x0100 {
                assert!(
                    name.contains("/presets/"),
                    "a preset's name holds presets/: {name}"
                );
            }
            let refuse = key == "Locked";
            {
                let mut u = unit.lock().unwrap();
                u.writes.push((opcode, name.clone()));
                if !refuse {
                    u.store.insert(key, data);
                }
            }
            // The unit closes so soon after its acknowledgement that only
            // part of it may arrive: send four of its six bytes.
            let ack: &[u8] = if refuse { &[0, 1] } else { &[0, 0, 0, 0] };
            stream.write_all(ack).await.ok()?;
        }
        _ => return None,
    }
    let _ = stream.shutdown().await;
    Some(())
}

async fn settings(listener: TcpListener, unit: Arc<Mutex<Unit>>) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let first = {
            let mut u = unit.lock().unwrap();
            u.connections += 1;
            u.connections == 1
        };
        if first {
            drop(stream);
            continue;
        }
        tokio::spawn(settings_request(unit.clone(), stream));
    }
}

async fn simulate() -> (u16, u16, Arc<Mutex<Unit>>) {
    let control_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let settings_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ports = (
        control_listener.local_addr().unwrap().port(),
        settings_listener.local_addr().unwrap().port(),
    );
    let mut unit = Unit::default();
    for (name, data) in [
        ("Green Set", b"green preset".to_vec()),
        ("Blue Set", vec![0xB1; 3000]),
        ("GPISettings", b"gpi".to_vec()),
        ("SavedSettings", b"saved live state".to_vec()),
    ] {
        unit.store.insert(name.into(), data);
    }
    let unit = Arc::new(Mutex::new(unit));
    tokio::spawn(control(control_listener));
    tokio::spawn(settings(settings_listener, unit.clone()));
    (ports.0, ports.1, unit)
}

fn params(v: Value) -> serde_json::Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, control: u16, settings: u16, monitor: bool) -> DeviceId {
    core.open(OpenRequest {
        device: "blackmagic-ultimatte".into(),
        model: "ultimatte-12-4k".into(),
        host: "127.0.0.1".into(),
        port: Some(control),
        settings: params(json!({ "archive_port": settings })),
        monitor,
    })
    .unwrap()
}

/// The zip's member names in order, read from its local headers.
fn members(zip: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut at = 0;
    while zip.get(at..at + 4) == Some(&[0x50, 0x4b, 0x03, 0x04]) {
        let u16_at = |o: usize| u16::from_le_bytes([zip[at + o], zip[at + o + 1]]) as usize;
        let packed = u32::from_le_bytes(zip[at + 18..at + 22].try_into().unwrap()) as usize;
        let (n, e) = (u16_at(26), u16_at(28));
        names.push(String::from_utf8_lossy(&zip[at + 30..at + 30 + n]).into_owned());
        at += 30 + n + e + packed;
    }
    names
}

fn temp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("meros-ultimatte-{}-{name}", std::process::id()))
}

/// Run a command once the unit's status dump has arrived.
async fn run(core: &Core, id: DeviceId, name: &str, p: Value) -> Result<Outcome, CommandError> {
    for _ in 0..100 {
        match core.execute(id, name, params(p.clone())).await {
            Err(CommandError::NotConnected) => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await
            }
            other => return other,
        }
    }
    Err(CommandError::NotConnected)
}

#[tokio::test(flavor = "multi_thread")]
async fn archive_and_restore_presets_over_the_settings_channel() {
    let (control_port, settings_port, unit) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, control_port, settings_port, true);
    let archive = temp("archive.zip");

    // Archive All: the dropped first connection is tried again.
    let out = run(
        &core,
        id,
        "export_presets",
        json!({ "path": archive.to_str().unwrap() }),
    )
    .await;
    let Ok(Outcome::Value { value }) = out else {
        panic!("export_presets: {out:?}");
    };
    assert_eq!(value["presets"], json!(["Green Set", "Blue Set"]));
    assert_eq!(value["settings"], json!(["GPISettings", "SavedSettings"]));
    assert_eq!(value["warnings"], json!([]));
    let zip = std::fs::read(&archive).unwrap();
    assert_eq!(value["bytes"], json!(zip.len()));
    assert_eq!(
        members(&zip),
        [
            "images/",
            "GPISettings",
            "SavedSettings",
            "quickfiles/",
            "presets/",
            "presets/Green Set",
            "presets/Blue Set"
        ]
    );

    // Restore onto a unit whose presets have changed since.
    {
        let mut u = unit.lock().unwrap();
        u.store.insert("Green Set".into(), b"changed".to_vec());
        u.store.insert("SavedSettings".into(), b"changed".to_vec());
    }
    let out = run(
        &core,
        id,
        "import_presets",
        json!({ "path": archive.to_str().unwrap() }),
    )
    .await;
    assert_eq!(
        out,
        Ok(Outcome::Value {
            value: json!({ "presets": ["Blue Set", "Green Set"], "settings": ["GPISettings", "SavedSettings"] })
        })
    );
    {
        let u = unit.lock().unwrap();
        assert_eq!(u.store["Green Set"], b"green preset");
        assert_eq!(u.store["SavedSettings"], b"saved live state");
        assert_eq!(u.store["Blue Set"], vec![0xB1; 3000]);
        let order: Vec<(u16, String)> = u.writes.clone();
        assert_eq!(
            order,
            [
                (0x0100, format!("{PREFIX}presets/Blue Set")),
                (0x0100, format!("{PREFIX}presets/Green Set")),
                (0x0103, format!("{PREFIX}GPISettings")),
                (0x0103, format!("{PREFIX}SavedSettings")),
            ]
        );
    }

    // One preset and one resource out to a file and back under other names.
    let one = temp("preset.bin");
    let out = run(
        &core,
        id,
        "export_preset",
        json!({ "name": "Green Set", "path": one.to_str().unwrap() }),
    )
    .await;
    assert_eq!(
        out,
        Ok(Outcome::Value {
            value: json!({ "path": one.to_str().unwrap(), "bytes": 12 })
        })
    );
    assert_eq!(std::fs::read(&one).unwrap(), b"green preset");
    let out = run(
        &core,
        id,
        "import_preset",
        json!({ "name": "Copy", "path": one.to_str().unwrap() }),
    )
    .await;
    assert_eq!(out, Ok(Outcome::Ack));
    assert_eq!(unit.lock().unwrap().store["Copy"], b"green preset");
    let out = run(
        &core,
        id,
        "export_settings",
        json!({ "resource": "GPISettings", "path": one.to_str().unwrap() }),
    )
    .await;
    assert!(matches!(out, Ok(Outcome::Value { .. })), "{out:?}");
    assert_eq!(std::fs::read(&one).unwrap(), b"gpi");
    let out = run(
        &core,
        id,
        "import_settings",
        json!({ "resource": "SavedSettings", "path": one.to_str().unwrap() }),
    )
    .await;
    assert_eq!(out, Ok(Outcome::Ack));
    assert_eq!(unit.lock().unwrap().store["SavedSettings"], b"gpi");

    // A refused write stops the restore and says what was written before it.
    let out = run(
        &core,
        id,
        "import_preset",
        json!({ "name": "Locked", "path": one.to_str().unwrap() }),
    )
    .await;
    let Err(CommandError::DeviceError { message, .. }) = out else {
        panic!("a refused write: {out:?}");
    };
    assert!(message.contains("Locked"), "{message}");
    // A preset the unit does not hold is an error, not an empty file.
    let out = run(
        &core,
        id,
        "export_preset",
        json!({ "name": "Missing", "path": one.to_str().unwrap() }),
    )
    .await;
    assert!(
        matches!(out, Err(CommandError::DeviceError { .. })),
        "{out:?}"
    );

    core.close(id).await;
    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::remove_file(&one);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_archive_commands_work_when_the_device_is_opened_for_commands_only() {
    let (control_port, settings_port, _unit) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, control_port, settings_port, false);
    let archive = temp("commands-only.zip");
    let out = run(
        &core,
        id,
        "export_presets",
        json!({ "path": archive.to_str().unwrap() }),
    )
    .await;
    let Ok(Outcome::Value { value }) = out else {
        panic!("export_presets: {out:?}");
    };
    assert_eq!(value["presets"], json!(["Green Set", "Blue Set"]));
    assert!(archive.exists());
    core.close(id).await;
    let _ = std::fs::remove_file(&archive);
}
