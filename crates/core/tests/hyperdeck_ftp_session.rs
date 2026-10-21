//! HyperDeck files over FTP (the hyperdeck-ftp extension) against an FTP
//! server simulated on loopback, as the HyperDeck manuals describe the
//! deck's: anonymous login, the media as top folders, passive data
//! connections. Its PASV replies name 0.0.0.0, as servers that bind every
//! interface do, so the data connections must go to the control
//! connection's peer. Switches make it refuse the login, refuse STOR, not
//! know MLSD, drop a download's data connection halfway, or send a download
//! slowly enough to cancel.

#![cfg(feature = "blackmagic-hyperdeck")]

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Core, DeviceId, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

#[derive(Default)]
struct Deck {
    files: BTreeMap<String, Vec<u8>>,
    folders: Vec<String>,
    refuse_login: bool,
    refuse_stor: bool,
    no_mlsd: bool,
    drop_retr: bool,
    slow_retr: bool,
    /// Every command line received, passwords included.
    commands: Vec<String>,
}

type Shared = Arc<Mutex<Deck>>;

async fn reply(w: &mut tokio::net::tcp::OwnedWriteHalf, line: &str) -> Option<()> {
    w.write_all(format!("{line}\r\n").as_bytes()).await.ok()
}

fn listing(deck: &Deck, folder: &str, mlsd: bool) -> String {
    let prefix = if folder.is_empty() {
        String::new()
    } else {
        format!("{folder}/")
    };
    let mut out = String::new();
    if mlsd {
        out.push_str("type=cdir;modify=20260101000000; .\r\n");
    }
    let mut seen = Vec::new();
    for f in &deck.folders {
        if let Some(name) = f.strip_prefix(&prefix).filter(|n| !n.contains('/')) {
            seen.push(name.to_string());
            if mlsd {
                out.push_str(&format!("type=dir;modify=20260101000000; {name}\r\n"));
            } else {
                out.push_str(&format!("drwxr-xr-x 1 ftp ftp 0 Jan 01 00:00 {name}\r\n"));
            }
        }
    }
    for (path, data) in &deck.files {
        if let Some(name) = path.strip_prefix(&prefix).filter(|n| !n.contains('/')) {
            if mlsd {
                out.push_str(&format!(
                    "type=file;size={};modify=20260102030405; {name}\r\n",
                    data.len()
                ));
            } else {
                out.push_str(&format!(
                    "-rw-r--r-- 1 ftp ftp {} Jan 02 03:04 {name}\r\n",
                    data.len()
                ));
            }
        }
    }
    out
}

fn path_of(arg: &str) -> String {
    arg.trim_start_matches('/').to_string()
}

async fn session(deck: Shared, stream: TcpStream) -> Option<()> {
    let (r, mut w) = stream.into_split();
    let mut lines = BufReader::new(r).lines();
    reply(&mut w, "220-Welcome to the deck").await?;
    reply(&mut w, "220 Ready").await?;
    let mut data: Option<oneshot::Receiver<TcpStream>> = None;
    let mut rename_from = None;
    while let Ok(Some(line)) = lines.next_line().await {
        deck.lock().unwrap().commands.push(line.clone());
        let (verb, arg) = line.split_once(' ').unwrap_or((line.as_str(), ""));
        let arg = arg.to_string();
        match verb {
            "USER" => reply(&mut w, "331 Password required").await?,
            "PASS" => {
                if deck.lock().unwrap().refuse_login {
                    reply(&mut w, "530 Login incorrect.").await?;
                } else {
                    reply(&mut w, "230 Logged in").await?;
                }
            }
            "TYPE" => reply(&mut w, "200 Type set to I").await?,
            "SIZE" => {
                let size = deck.lock().unwrap().files.get(&path_of(&arg)).map(Vec::len);
                match size {
                    Some(n) => reply(&mut w, &format!("213 {n}")).await?,
                    None => reply(&mut w, "550 No such file").await?,
                }
            }
            "PASV" => {
                let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
                let port = listener.local_addr().ok()?.port();
                let (tx, rx) = oneshot::channel();
                tokio::spawn(async move {
                    if let Ok(Ok((s, _))) =
                        tokio::time::timeout(Duration::from_secs(10), listener.accept()).await
                    {
                        let _ = tx.send(s);
                    }
                });
                data = Some(rx);
                let text = format!(
                    "227 Entering Passive Mode (0,0,0,0,{},{})",
                    port / 256,
                    port % 256
                );
                reply(&mut w, &text).await?;
            }
            "MLSD" | "LIST" => {
                let mlsd = verb == "MLSD";
                if mlsd && deck.lock().unwrap().no_mlsd {
                    reply(&mut w, "500 Unknown command").await?;
                    continue;
                }
                let mut s = data.take()?.await.ok()?;
                reply(&mut w, "150 Here comes the listing").await?;
                let text = listing(&deck.lock().unwrap(), &path_of(&arg), mlsd);
                s.write_all(text.as_bytes()).await.ok()?;
                drop(s);
                reply(&mut w, "226 Transfer complete").await?;
            }
            "RETR" => {
                let file = deck.lock().unwrap().files.get(&path_of(&arg)).cloned();
                let Some(file) = file else {
                    reply(&mut w, "550 No such file").await?;
                    continue;
                };
                let mut s = data.take()?.await.ok()?;
                reply(&mut w, "150 Opening BINARY mode data connection").await?;
                let (drop_half, slow) = {
                    let d = deck.lock().unwrap();
                    (d.drop_retr, d.slow_retr)
                };
                if drop_half {
                    s.write_all(&file[..file.len() / 2]).await.ok()?;
                    drop(s);
                    reply(&mut w, "426 Connection closed; transfer aborted.").await?;
                    continue;
                }
                if slow {
                    // Slow enough to cancel: the client closes the data
                    // connection and sends ABOR.
                    for chunk in file.chunks(16 * 1024) {
                        if s.write_all(chunk).await.is_err() {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    continue;
                }
                s.write_all(&file).await.ok()?;
                drop(s);
                reply(&mut w, "226 Transfer complete").await?;
            }
            "STOR" => {
                let path = path_of(&arg);
                let refuse = deck.lock().unwrap().refuse_stor || !path.contains('/');
                if refuse {
                    reply(&mut w, "550 Permission denied").await?;
                    continue;
                }
                let mut s = data.take()?.await.ok()?;
                reply(&mut w, "150 Ok to send data").await?;
                let mut got = Vec::new();
                s.read_to_end(&mut got).await.ok()?;
                deck.lock().unwrap().files.insert(path, got);
                reply(&mut w, "226 Transfer complete").await?;
            }
            "DELE" => {
                let gone = deck.lock().unwrap().files.remove(&path_of(&arg)).is_some();
                if gone {
                    reply(&mut w, "250 Deleted").await?;
                } else {
                    reply(&mut w, "550 No such file").await?;
                }
            }
            "MKD" => {
                deck.lock().unwrap().folders.push(path_of(&arg));
                reply(&mut w, &format!("257 \"{arg}\" created")).await?;
            }
            "RNFR" => {
                let there = deck.lock().unwrap().files.contains_key(&path_of(&arg));
                if there {
                    rename_from = Some(path_of(&arg));
                    reply(&mut w, "350 Ready for RNTO").await?;
                } else {
                    reply(&mut w, "550 No such file").await?;
                }
            }
            "RNTO" => {
                let Some(from) = rename_from.take() else {
                    reply(&mut w, "503 RNFR first").await?;
                    continue;
                };
                {
                    let mut d = deck.lock().unwrap();
                    let file = d.files.remove(&from).unwrap();
                    d.files.insert(path_of(&arg), file);
                }
                reply(&mut w, "250 Renamed").await?;
            }
            "ABOR" => {
                reply(&mut w, "426 Transfer aborted").await?;
                reply(&mut w, "226 ABOR successful").await?;
            }
            "QUIT" => {
                reply(&mut w, "221 Goodbye").await?;
                return Some(());
            }
            _ => reply(&mut w, "502 Not implemented").await?,
        }
    }
    Some(())
}

/// The deck: its FTP server and its protocol port (9993), which answers
/// every line with 200 so the engine is content.
async fn simulate() -> (u16, u16, Shared) {
    let ftp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let protocol = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ports = (
        protocol.local_addr().unwrap().port(),
        ftp.local_addr().unwrap().port(),
    );
    let mut deck = Deck {
        folders: vec!["sd1".into(), "sd2".into(), "usb".into()],
        ..Deck::default()
    };
    deck.files.insert(
        "sd1/Clip 0001.mov".into(),
        (0..200_000u32).map(|i| i as u8).collect(),
    );
    deck.files.insert("sd1/Short.mp4".into(), b"tiny".to_vec());
    let deck = Arc::new(Mutex::new(deck));
    let shared = deck.clone();
    tokio::spawn(async move {
        while let Ok((s, _)) = ftp.accept().await {
            tokio::spawn(session(shared.clone(), s));
        }
    });
    tokio::spawn(async move {
        while let Ok((s, _)) = protocol.accept().await {
            tokio::spawn(async move {
                let (r, mut w) = s.into_split();
                let mut lines = BufReader::new(r).lines();
                while let Ok(Some(_)) = lines.next_line().await {
                    if w.write_all(b"200 ok\r\n").await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    (ports.0, ports.1, deck)
}

fn params(v: Value) -> serde_json::Map<String, Value> {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, protocol: u16, ftp: u16, monitor: bool) -> DeviceId {
    core.open(OpenRequest {
        device: "blackmagic-hyperdeck".into(),
        model: "hyperdeck-studio".into(),
        host: "127.0.0.1".into(),
        port: Some(protocol),
        settings: params(json!({ "ftp_port": ftp })),
        monitor,
    })
    .unwrap()
}

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("meros-hyperdeck-ftp-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

async fn run(core: &Core, id: DeviceId, name: &str, p: Value) -> Result<Outcome, CommandError> {
    core.execute(id, name, params(p)).await
}

async fn start(core: &Core, id: DeviceId, name: &str, p: Value) -> u64 {
    match run(core, id, name, p).await {
        Ok(Outcome::Value { value }) => value["transfer"].as_u64().unwrap(),
        other => panic!("{name}: {other:?}"),
    }
}

/// The transfer's events, until it finishes: what transfer_started and
/// transfer_finished said, and the transfer's state as last patched.
struct Ended {
    started: Value,
    finished: Value,
    state: serde_json::Map<String, Value>,
}

/// Every event seen, so one waited for is never lost to another wait.
struct Seen<'a> {
    core: &'a Core,
    events: Vec<Event>,
}

impl Seen<'_> {
    async fn more(&mut self) {
        let batch = self.core.next_events(64).await;
        self.events.extend(batch);
    }

    async fn ended(&mut self, transfer: u64) -> Ended {
        let key = transfer.to_string();
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                let mut started = Value::Null;
                let mut state = serde_json::Map::new();
                for e in &self.events {
                    match e {
                        Event::State { patch, .. } => {
                            if let Some(Value::Object(t)) =
                                patch.pointer(&format!("/transfers/{key}"))
                            {
                                for (k, v) in t {
                                    state.insert(k.clone(), v.clone());
                                }
                            }
                        }
                        Event::Message { address, args, .. } if args["transfer"] == transfer => {
                            if address == "transfer_started" {
                                started = args.clone();
                            } else if address == "transfer_finished" {
                                return Ended {
                                    started,
                                    finished: args.clone(),
                                    state,
                                };
                            }
                        }
                        _ => {}
                    }
                }
                self.more().await;
            }
        })
        .await
        .expect("the transfer ends within 20 s")
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn files_are_listed_downloaded_and_uploaded() {
    let (protocol, ftp, deck) = simulate().await;
    let core = common::core().unwrap();
    let id = open(&core, protocol, ftp, false);
    let dir = scratch("updown");
    let mut seen = Seen {
        core: &core,
        events: Vec::new(),
    };

    // The root lists the media; a folder its files, with sizes.
    let out = run(&core, id, "list_files", json!({})).await;
    let Ok(Outcome::Value { value }) = out else {
        panic!("list_files: {out:?}");
    };
    let names: Vec<&str> = value["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["sd1", "sd2", "usb"]);
    assert_eq!(value["entries"][0]["type"], "folder");
    let out = run(&core, id, "list_files", json!({"path": "sd1"})).await;
    let Ok(Outcome::Value { value }) = out else {
        panic!("list_files sd1: {out:?}");
    };
    assert_eq!(
        value["entries"],
        json!([
            {"name": "Clip 0001.mov", "type": "file", "size": 200000, "modified": "2026-01-02T03:04:05Z"},
            {"name": "Short.mp4", "type": "file", "size": 4, "modified": "2026-01-02T03:04:05Z"},
        ])
    );

    // A download comes back whole; the command answers at once.
    let target = dir.join("clip.mov");
    let t = start(
        &core,
        id,
        "download_file",
        json!({"remote": "sd1/Clip 0001.mov", "path": target.to_str().unwrap()}),
    )
    .await;
    let end = seen.ended(t).await;
    assert_eq!(end.started["kind"], "download_file");
    assert_eq!(end.started["remote"], "sd1/Clip 0001.mov");
    assert_eq!(end.finished["status"], "done", "{:?}", end.finished);
    assert_eq!(end.finished["bytes"], 200_000);
    assert_eq!(end.state["status"], "done");
    assert_eq!(end.state["progress"], 1.0);
    assert_eq!(end.state["size"], 200_000);
    let expected: Vec<u8> = (0..200_000u32).map(|i| i as u8).collect();
    assert_eq!(std::fs::read(&target).unwrap(), expected);

    // An upload streams the host file into the folder under its own name.
    let source = dir.join("Opener.mov");
    let big: Vec<u8> = (0..1_500_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&source, &big).unwrap();
    let t = start(
        &core,
        id,
        "upload_file",
        json!({"path": source.to_str().unwrap(), "folder": "sd2"}),
    )
    .await;
    let end = seen.ended(t).await;
    assert_eq!(end.finished["status"], "done", "{:?}", end.finished);
    assert_eq!(end.finished["remote"], "sd2/Opener.mov");
    assert_eq!(end.finished["bytes"], 1_500_000);
    assert_eq!(end.state["size"], 1_500_000);
    assert_eq!(deck.lock().unwrap().files["sd2/Opener.mov"], big);

    // Again without overwrite: refused before anything is sent.
    let stors = |d: &Shared| {
        d.lock()
            .unwrap()
            .commands
            .iter()
            .filter(|c| c.starts_with("STOR"))
            .count()
    };
    let before = stors(&deck);
    let t = start(
        &core,
        id,
        "upload_file",
        json!({"path": source.to_str().unwrap(), "folder": "sd2"}),
    )
    .await;
    let end = seen.ended(t).await;
    assert_eq!(end.finished["status"], "failed");
    assert!(
        end.finished["error"]
            .as_str()
            .unwrap()
            .contains("already exists"),
        "{:?}",
        end.finished
    );
    assert_eq!(stors(&deck), before);
    // With overwrite, under another name too.
    std::fs::write(&source, b"replaced").unwrap();
    let t = start(
        &core,
        id,
        "upload_file",
        json!({"path": source.to_str().unwrap(), "folder": "/sd2/", "name": "Opener.mov", "overwrite": true}),
    )
    .await;
    assert_eq!(seen.ended(t).await.finished["status"], "done");
    assert_eq!(deck.lock().unwrap().files["sd2/Opener.mov"], b"replaced");

    // The anonymous login, data connections to the peer whatever PASV names.
    {
        let d = deck.lock().unwrap();
        assert!(d.commands.contains(&"USER anonymous".to_string()));
        assert!(d.commands.contains(&"TYPE I".to_string()));
        assert!(d.commands.contains(&"MLSD /sd1".to_string()));
        assert!(d.commands.contains(&"RETR /sd1/Clip 0001.mov".to_string()));
        assert!(d.commands.contains(&"QUIT".to_string()));
    }

    // Rename, folders and delete.
    let out = run(
        &core,
        id,
        "rename_file",
        json!({"remote": "sd1/Short.mp4", "to": "sd1/Sting.mp4"}),
    )
    .await;
    assert_eq!(out, Ok(Outcome::Ack));
    let out = run(
        &core,
        id,
        "rename_file",
        json!({"remote": "sd1/Sting.mp4", "to": "sd2/Opener.mov"}),
    )
    .await;
    assert!(
        matches!(out, Err(CommandError::InvalidParams { .. })),
        "{out:?}"
    );
    assert_eq!(
        run(&core, id, "create_folder", json!({"remote": "sd1/Show"})).await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        run(&core, id, "delete_file", json!({"remote": "sd1/Sting.mp4"})).await,
        Ok(Outcome::Ack)
    );
    let out = run(&core, id, "delete_file", json!({"remote": "sd1/Sting.mp4"})).await;
    assert!(
        matches!(&out, Err(CommandError::DeviceError { code: Some(c), .. }) if c == "550"),
        "{out:?}"
    );
    {
        let d = deck.lock().unwrap();
        assert!(d.folders.contains(&"sd1/Show".to_string()));
        assert!(!d.files.contains_key("sd1/Sting.mp4"));
        assert!(!d.files.contains_key("sd1/Short.mp4"));
    }
    // A missing file fails its download without creating the host file.
    let missing = dir.join("missing.mov");
    let t = start(
        &core,
        id,
        "download_file",
        json!({"remote": "sd1/None.mov", "path": missing.to_str().unwrap()}),
    )
    .await;
    assert_eq!(seen.ended(t).await.finished["status"], "failed");
    assert!(!missing.exists());

    core.close(id).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn listings_fall_back_to_list_and_refusals_fail() {
    let (protocol, ftp, deck) = simulate().await;
    deck.lock().unwrap().no_mlsd = true;
    let core = common::core().unwrap();
    let id = open(&core, protocol, ftp, true);
    let dir = scratch("refusals");
    let mut seen = Seen {
        core: &core,
        events: Vec::new(),
    };

    let out = run(&core, id, "list_files", json!({"path": "sd1"})).await;
    let Ok(Outcome::Value { value }) = out else {
        panic!("list_files: {out:?}");
    };
    assert_eq!(value["entries"][0]["name"], "Clip 0001.mov");
    assert_eq!(value["entries"][0]["size"], 200000);
    assert!(deck
        .lock()
        .unwrap()
        .commands
        .contains(&"LIST /sd1".to_string()));

    // A refused STOR fails the transfer, not the command.
    deck.lock().unwrap().refuse_stor = true;
    let source = dir.join("a.mov");
    std::fs::write(&source, b"abc").unwrap();
    let t = start(
        &core,
        id,
        "upload_file",
        json!({"path": source.to_str().unwrap(), "folder": "sd1"}),
    )
    .await;
    let end = seen.ended(t).await;
    assert_eq!(end.finished["status"], "failed");
    assert!(
        end.finished["error"].as_str().unwrap().starts_with("550"),
        "{:?}",
        end.finished
    );

    // A download whose data connection drops halfway fails.
    deck.lock().unwrap().drop_retr = true;
    let target = dir.join("half.mov");
    let t = start(
        &core,
        id,
        "download_file",
        json!({"remote": "sd1/Clip 0001.mov", "path": target.to_str().unwrap()}),
    )
    .await;
    let end = seen.ended(t).await;
    assert_eq!(end.finished["status"], "failed", "{:?}", end.finished);
    assert!(
        end.finished["error"].as_str().unwrap().contains("cut off")
            || end.finished["error"].as_str().unwrap().contains("received"),
        "{:?}",
        end.finished
    );

    // A refused login fails commands with an authentication error, and
    // transfers with the reason.
    deck.lock().unwrap().refuse_login = true;
    let out = run(&core, id, "list_files", json!({})).await;
    assert!(matches!(out, Err(CommandError::Auth { .. })), "{out:?}");
    let t = start(
        &core,
        id,
        "download_file",
        json!({"remote": "sd1/Short.mp4", "path": target.to_str().unwrap()}),
    )
    .await;
    let end = seen.ended(t).await;
    assert_eq!(end.finished["status"], "failed");
    assert!(
        end.finished["error"].as_str().unwrap().contains("530"),
        "{:?}",
        end.finished
    );

    // Bad paths never reach the deck.
    let out = run(
        &core,
        id,
        "delete_file",
        json!({"remote": "sd1/x\r\nDELE sd1/Clip 0001.mov"}),
    )
    .await;
    assert!(
        matches!(out, Err(CommandError::InvalidParams { .. })),
        "{out:?}"
    );

    core.close(id).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_transfer_is_canceled_midway_and_the_next_one_runs() {
    let (protocol, ftp, deck) = simulate().await;
    {
        let mut d = deck.lock().unwrap();
        d.slow_retr = true;
        d.files.insert("sd1/Long.mov".into(), vec![7; 2_000_000]);
    }
    let core = common::core().unwrap();
    let id = open(&core, protocol, ftp, false);
    let dir = scratch("cancel");
    let target = dir.join("long.mov");
    let mut seen = Seen {
        core: &core,
        events: Vec::new(),
    };

    let first = start(
        &core,
        id,
        "download_file",
        json!({"remote": "sd1/Long.mov", "path": target.to_str().unwrap()}),
    )
    .await;
    let source = dir.join("next.mov");
    std::fs::write(&source, b"next").unwrap();
    let out = run(
        &core,
        id,
        "upload_file",
        json!({"path": source.to_str().unwrap(), "folder": "sd2"}),
    )
    .await;
    let Ok(Outcome::Value { value }) = out else {
        panic!("upload_file: {out:?}");
    };
    assert_eq!(value["ahead"], 1);
    let second = value["transfer"].as_u64().unwrap();

    // Wait until bytes are moving, then cancel the first.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let moving = seen.events.iter().any(|e| match e {
                Event::State { patch, .. } => patch
                    .pointer(&format!("/transfers/{first}/bytes"))
                    .and_then(Value::as_u64)
                    .is_some_and(|b| b > 0),
                _ => false,
            });
            if moving {
                return;
            }
            seen.more().await;
        }
    })
    .await
    .expect("the download moves");
    let out = run(&core, id, "cancel_transfer", json!({"transfer": first})).await;
    assert_eq!(
        out,
        Ok(Outcome::Value {
            value: json!({"canceled": [first]})
        })
    );
    let end = seen.ended(first).await;
    assert_eq!(end.finished["status"], "canceled");
    let end = seen.ended(second).await;
    assert_eq!(end.finished["status"], "done", "{:?}", end.finished);
    assert_eq!(deck.lock().unwrap().files["sd2/next.mov"], b"next");
    assert!(deck.lock().unwrap().commands.contains(&"ABOR".to_string()));
    let out = run(&core, id, "cancel_transfer", json!({})).await;
    assert_eq!(
        out,
        Ok(Outcome::Value {
            value: json!({"canceled": []})
        })
    );

    core.close(id).await;
    let _ = std::fs::remove_dir_all(&dir);
}
