//! The transfer folder through the service, bound to 127.0.0.1: files go up
//! and come back, are listed and deleted; every route needs the bearer
//! header; ids that try to leave the folder are refused; no size, quota or
//! expiry limits a file, and the flags that once set them are unknown;
//! startup clears the folder's ids; and an ATEM simulated on loopback UDP
//! takes a still and a macro from uploaded files and writes them back into
//! reserved ones.

#![cfg(feature = "all")]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tokio::net::UdpSocket;

struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A sidecar with its own token file and transfer folder, both removed when
/// it is dropped.
struct Service {
    child: KillOnDrop,
    base: String,
    addr: String,
    token: String,
    dir: PathBuf,
    token_file: PathBuf,
    http: reqwest::Client,
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.child.0.kill();
        let _ = self.child.0.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_file(&self.token_file);
    }
}

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("meros-transfer-{name}-{}", std::process::id()))
}

fn start(name: &str, extra: &[&str]) -> Service {
    let dir = scratch(name);
    let token_file = dir.with_extension("token");
    let _ = std::fs::remove_file(&token_file);
    start_in(dir, token_file, extra)
}

fn start_in(dir: PathBuf, token_file: PathBuf, extra: &[&str]) -> Service {
    let mut child = KillOnDrop(
        Command::new(env!("CARGO_BIN_EXE_meros-integrations"))
            .args([
                "serve",
                "--listen",
                "127.0.0.1:0",
                "--bind-address",
                "127.0.0.1",
            ])
            .arg("--token-file")
            .arg(&token_file)
            .arg("--transfer-dir")
            .arg(&dir)
            .args(extra)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let mut line = String::new();
    BufReader::new(child.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let first: Value = serde_json::from_str(&line).unwrap();
    let addr = first["listening"].as_str().unwrap().to_string();
    Service {
        child,
        base: format!("http://{addr}"),
        addr,
        token: std::fs::read_to_string(&token_file).unwrap(),
        dir,
        token_file,
        http: reqwest::Client::new(),
    }
}

impl Service {
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    async fn upload(&self, name: &str, data: Vec<u8>) -> (u16, Value) {
        let response = self
            .http
            .post(self.url(&format!("/v1/files?name={name}")))
            .bearer_auth(&self.token)
            .body(data)
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        (status, response.json().await.unwrap())
    }

    async fn get(&self, path: &str) -> (u16, Vec<u8>) {
        let response = self
            .http
            .get(self.url(path))
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        (status, response.bytes().await.unwrap().to_vec())
    }

    async fn get_json(&self, path: &str) -> Value {
        let (status, body) = self.get(path).await;
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
        serde_json::from_slice(&body).unwrap()
    }

    async fn post(&self, path: &str, body: Value) -> (u16, Value) {
        let response = self
            .http
            .post(self.url(path))
            .bearer_auth(&self.token)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        (status, response.json().await.unwrap())
    }

    async fn delete(&self, path: &str) -> (u16, Value) {
        let response = self
            .http
            .delete(self.url(path))
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        (status, response.json().await.unwrap())
    }

    /// The raw request, for what reqwest will not send (a chunked body, a
    /// path it would normalize).
    fn raw(&self, request: &[u8]) -> String {
        let mut stream = std::net::TcpStream::connect(&self.addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        stream.write_all(request).unwrap();
        let mut out = Vec::new();
        let _ = stream.read_to_end(&mut out);
        String::from_utf8_lossy(&out).into_owned()
    }
}

/// Bytes that do not repeat in a way a mistake could hide.
fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i * 7 + i / 251) as u8).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn files_go_up_come_back_are_listed_and_deleted() {
    let service = start("roundtrip", &[]);

    // 3 MiB, over axum's default body limit: the body is streamed to disk.
    let data = pattern(3 * 1024 * 1024 + 17);
    let (status, up) = service.upload("Lower third.png", data.clone()).await;
    assert_eq!(status, 201, "{up}");
    let id = up["file"].as_str().unwrap().to_string();
    assert_eq!(id.len(), 32);
    assert_eq!(up["name"], "Lower third.png");
    assert_eq!(up["path"], format!("transfer:{id}"));
    assert_eq!(up["size"], data.len());

    let list = service.get_json("/v1/files").await;
    assert_eq!(list["files"].as_array().unwrap().len(), 1);
    assert_eq!(list["files"][0]["file"], id.as_str());
    assert_eq!(list["files"][0]["written"], true);
    assert_eq!(list["used"], data.len());
    // No limits to report: the list is the files and their total.
    for gone in ["quota", "max_file", "expiry_s"] {
        assert!(list.get(gone).is_none(), "{gone} in {list}");
    }
    assert!(list["files"][0].get("idle_ms").is_none(), "{list}");

    let response = service
        .http
        .get(service.url(&format!("/v1/files/{id}")))
        .bearer_auth(&service.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()["content-type"],
        "application/octet-stream"
    );
    assert_eq!(
        response.headers()["content-disposition"],
        "attachment; filename=\"Lower third.png\""
    );
    assert_eq!(response.bytes().await.unwrap().to_vec(), data);

    // Reserved outputs are listed as not written, and have nothing to fetch.
    let (status, reserved) = service
        .post(
            "/v1/files/reserve",
            json!({"count": 3, "name": "frame.png"}),
        )
        .await;
    assert_eq!(status, 201, "{reserved}");
    let outputs = reserved["files"].as_array().unwrap();
    assert_eq!(outputs.len(), 3);
    assert_eq!(outputs[0]["written"], false);
    assert_eq!(outputs[0]["name"], "frame.png");
    let output = outputs[0]["file"].as_str().unwrap();
    assert_eq!(service.get(&format!("/v1/files/{output}")).await.0, 404);
    let (status, refused) = service.post("/v1/files/reserve", json!({"count": 0})).await;
    assert_eq!(status, 400, "{refused}");

    // Deleted, then gone.
    let (status, deleted) = service.delete(&format!("/v1/files/{id}")).await;
    assert_eq!((status, deleted), (200, json!({"deleted": true})));
    assert_eq!(service.get(&format!("/v1/files/{id}")).await.0, 404);
    assert_eq!(service.delete(&format!("/v1/files/{id}")).await.0, 404);
    assert!(!service.dir.join(&id).exists());
    let list = service.get_json("/v1/files").await;
    assert_eq!(list["files"].as_array().unwrap().len(), 3);
    assert_eq!(list["used"], 0);

    // Fetched with ?delete=true, it is removed once sent in full.
    let (_, up) = service.upload("once.bin", pattern(200_000)).await;
    let once = up["file"].as_str().unwrap().to_string();
    let (status, body) = service.get(&format!("/v1/files/{once}?delete=true")).await;
    assert_eq!((status, body.len()), (200, 200_000));
    let deadline = Instant::now() + Duration::from_secs(5);
    while service.dir.join(&once).exists() {
        assert!(Instant::now() < deadline, "not removed after the fetch");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(service.get(&format!("/v1/files/{once}")).await.0, 404);

    // A chunked body, with no length given up front.
    let body = pattern(10_000);
    let mut request = format!(
        "POST /v1/files?name=chunked.bin HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
        service.token
    )
    .into_bytes();
    for part in body.chunks(3000) {
        request.extend_from_slice(format!("{:x}\r\n", part.len()).as_bytes());
        request.extend_from_slice(part);
        request.extend_from_slice(b"\r\n");
    }
    request.extend_from_slice(b"0\r\n\r\n");
    let reply = service.raw(&request);
    assert!(reply.starts_with("HTTP/1.1 201"), "{reply}");
    assert!(reply.contains("\"size\":10000"), "{reply}");
}

#[tokio::test(flavor = "multi_thread")]
async fn every_file_route_needs_the_token() {
    let service = start("auth", &[]);
    let id = "0123456789abcdef0123456789abcdef";
    let requests = [
        service.http.get(service.url("/v1/files")),
        service.http.post(service.url("/v1/files")).body("x"),
        service
            .http
            .post(service.url("/v1/files/reserve"))
            .json(&json!({})),
        service.http.get(service.url(&format!("/v1/files/{id}"))),
        service.http.delete(service.url(&format!("/v1/files/{id}"))),
    ];
    for request in requests {
        let anonymous = request.try_clone().unwrap().send().await.unwrap();
        assert_eq!(anonymous.status(), 401);
        let wrong = request.bearer_auth("wrong").send().await.unwrap();
        assert_eq!(wrong.status(), 401);
    }
    // Not in the query either, as an <img> stream would take it.
    let url = service.url(&format!("/v1/files?access_token={}", service.token));
    assert_eq!(service.http.get(url).send().await.unwrap().status(), 401);
    // Nothing was stored.
    assert_eq!(service.get_json("/v1/files").await["files"], json!([]));
}

#[tokio::test(flavor = "multi_thread")]
async fn paths_out_of_the_folder_are_refused() {
    let service = start("traversal", &[]);
    // A file beside the folder that a traversal would reach.
    let secret = service
        .dir
        .parent()
        .unwrap()
        .join(format!("meros-transfer-secret-{}.txt", std::process::id()));
    std::fs::write(&secret, b"secret").unwrap();
    let secret_name = secret.file_name().unwrap().to_str().unwrap().to_string();

    for target in [
        format!("/v1/files/..%2F{secret_name}"),
        format!("/v1/files/..%5C{secret_name}"),
        "/v1/files/..".to_string(),
        "/v1/files/%2e%2e".to_string(),
        "/v1/files/0123456789ABCDEF0123456789ABCDEF".to_string(),
    ] {
        for method in ["GET", "DELETE"] {
            let reply = service.raw(
                format!(
                    "{method} {target} HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",
                    service.token
                )
                .as_bytes(),
            );
            let status = &reply[9..12];
            assert!(
                ["400", "404", "405"].contains(&status),
                "{method} {target}: {reply}"
            );
            assert!(!reply.contains("secret\r\n") && !reply.ends_with("secret"));
        }
    }
    assert!(secret.exists());

    // An upload's name never leaves its folder.
    let (status, up) = service.upload("..%2F..%2Fescaped.txt", b"x".to_vec()).await;
    assert_eq!(status, 201, "{up}");
    assert_eq!(up["name"], "escaped.txt");
    let id = up["file"].as_str().unwrap();
    assert!(service.dir.join(id).join("escaped.txt").exists());
    assert!(!service.dir.parent().unwrap().join("escaped.txt").exists());

    // In a command, only ids this service handed out resolve: the command
    // is refused before it reaches the core (device 999 is not open).
    for path in [
        "transfer:../../etc/passwd".to_string(),
        format!("transfer:{id}/../{id}"),
        "transfer:0123456789abcdef0123456789abcdef".to_string(),
        format!("transfer:{}", id.to_uppercase()),
    ] {
        let (_, refused) = service
            .post(
                "/v1/execute",
                json!({"device": 999, "command": "upload_still", "params": {"still": 1, "path": path}}),
            )
            .await;
        assert_eq!(refused["error"]["error"], "invalid_params", "{path}");
    }
    // A known id passes on to the core, which then refuses device 999.
    let (_, passed) = service
        .post(
            "/v1/execute",
            json!({"device": 999, "command": "upload_still",
                   "params": {"still": 1, "path": format!("transfer:{id}")}}),
        )
        .await;
    assert!(passed["error"].is_object(), "{passed}");
    assert_ne!(passed["error"]["error"], "invalid_params", "{passed}");
    let _ = std::fs::remove_file(&secret);
}

/// A link planted in the folder is never followed.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn a_link_in_the_folder_is_not_followed() {
    let service = start("symlink", &[]);
    let (_, up) = service.upload("a.bin", b"mine".to_vec()).await;
    let id = up["file"].as_str().unwrap();
    let outside = scratch("symlink-outside");
    std::fs::write(&outside, b"outside").unwrap();
    let file = service.dir.join(id).join("a.bin");
    std::fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink(&outside, &file).unwrap();
    assert_eq!(service.get(&format!("/v1/files/{id}")).await.0, 404);
    // Deleting it removes the link, not what it points to.
    assert_eq!(service.delete(&format!("/v1/files/{id}")).await.0, 200);
    assert_eq!(std::fs::read(&outside).unwrap(), b"outside");
    let _ = std::fs::remove_file(&outside);
}

#[tokio::test(flavor = "multi_thread")]
async fn no_size_or_quota_limits_an_upload() {
    let service = start("limits", &[]);

    // A body declared larger than the old 512 MiB default is taken as it
    // arrives, not refused up front: the service answers only once the
    // client stops sending, here part way, so nothing of it is kept.
    let declared: u64 = 600 << 20;
    let mut stream = std::net::TcpStream::connect(&service.addr).unwrap();
    write!(
        stream,
        "POST /v1/files?name=clip.mov HTTP/1.1\r\nHost: x\r\nAuthorization: Bearer {}\r\nContent-Length: {declared}\r\n\r\n",
        service.token
    )
    .unwrap();
    stream.write_all(&pattern(256 * 1024)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let mut early = [0u8; 64];
    let read = stream.read(&mut early);
    assert!(
        matches!(&read, Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut)),
        "answered while the body was still coming: {read:?} {}",
        String::from_utf8_lossy(&early)
    );
    // Part of it is on disk, counted but not yet listed.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let on_disk = std::fs::read_dir(&service.dir).unwrap().count();
        if on_disk == 1 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the upload never reached the folder"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let list = service.get_json("/v1/files").await;
    assert_eq!(list["files"].as_array().unwrap().len(), 0, "{list}");
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut reply = Vec::new();
    let _ = stream.read_to_end(&mut reply);
    let reply = String::from_utf8_lossy(&reply);
    assert!(reply.starts_with("HTTP/1.1 400"), "{reply}");
    assert!(reply.contains("cut off"), "{reply}");
    assert!(!reply.contains("too_large"), "{reply}");
    let deadline = Instant::now() + Duration::from_secs(5);
    while std::fs::read_dir(&service.dir).unwrap().count() != 0 {
        assert!(Instant::now() < deadline, "the cut-off upload was kept");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Files add up past the old quota's share without a refusal, and
    // reservations are handed out whatever the folder holds.
    for i in 0..4 {
        let (status, up) = service.upload(&format!("{i}.bin"), pattern(300_000)).await;
        assert_eq!(status, 201, "{up}");
    }
    let (status, reserved) = service.post("/v1/files/reserve", json!({"count": 2})).await;
    assert_eq!(status, 201, "{reserved}");
    let list = service.get_json("/v1/files").await;
    assert_eq!(list["used"], 1_200_000);
    assert_eq!(list["files"].as_array().unwrap().len(), 6);

    // A command's JSON body carries a binary param's bytes: larger than
    // axum's 2 MB default, it still reaches the command (refused here only
    // because no such device is open).
    let blob = "A".repeat(3 * 1024 * 1024);
    let (status, refused) = service
        .post(
            "/v1/execute",
            json!({"device": 987_654, "command": "upload_media", "params": {"file": blob}}),
        )
        .await;
    assert_ne!(status, 413, "{refused}");
    assert!(refused.get("error").is_some(), "{refused}");
}

#[test]
fn the_removed_limit_flags_are_unknown() {
    for flag in [
        "--transfer-max-file",
        "--transfer-quota",
        "--transfer-expiry",
    ] {
        let dir = scratch("flags");
        let token_file = dir.with_extension("token");
        let out = Command::new(env!("CARGO_BIN_EXE_meros-integrations"))
            .args([
                "serve",
                "--listen",
                "127.0.0.1:0",
                "--bind-address",
                "127.0.0.1",
                flag,
                "1000",
            ])
            .arg("--token-file")
            .arg(&token_file)
            .arg("--transfer-dir")
            .arg(&dir)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{flag}");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(&format!("unknown flag {flag}")),
            "{flag}"
        );
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(&token_file);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn startup_clears_the_folder_and_files_stay_until_deleted() {
    // What an earlier run left: an id's folder goes, anything else stays.
    let dir = scratch("startup");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("0123456789abcdef0123456789abcdef")).unwrap();
    std::fs::write(dir.join("0123456789abcdef0123456789abcdef/old.png"), b"x").unwrap();
    std::fs::write(dir.join("keep.txt"), b"not the service's").unwrap();
    let token_file = dir.with_extension("token");
    let service = start_in(dir, token_file, &[]);
    assert!(!service
        .dir
        .join("0123456789abcdef0123456789abcdef")
        .exists());
    assert!(service.dir.join("keep.txt").exists());

    // Left alone, a file is not removed by the service.
    let (_, up) = service.upload("a.bin", pattern(10)).await;
    let id = up["file"].as_str().unwrap().to_string();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let list = service.get_json("/v1/files").await;
    assert_eq!(list["files"][0]["file"], id.as_str(), "{list}");
    assert_eq!(
        service.get(&format!("/v1/files/{id}")).await,
        (200, pattern(10))
    );
}

// --- A simulated ATEM: enough of the switcher for stills and macros. ---

const SESSION: u16 = 0x8123;
const RLE_MARKER: [u8; 8] = [0xFE; 8];
const CHUNK: usize = 1392;

fn packet(flags: u8, ack: u16, id: u16, payload: &[u8]) -> Vec<u8> {
    let length = 12 + payload.len() as u16;
    let mut p = Vec::new();
    p.extend_from_slice(&(((flags as u16) << 11) | length).to_be_bytes());
    p.extend_from_slice(&SESSION.to_be_bytes());
    p.extend_from_slice(&ack.to_be_bytes());
    p.extend_from_slice(&[0, 0, 0, 0]);
    p.extend_from_slice(&id.to_be_bytes());
    p.extend_from_slice(payload);
    p
}

fn cmd(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut c = ((8 + body.len()) as u16).to_be_bytes().to_vec();
    c.extend_from_slice(&[0xff, 0xff]);
    c.extend_from_slice(name);
    c.extend_from_slice(body);
    c
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

/// The bytes run-length encoded data expands to.
fn expanded(data: &[u8]) -> usize {
    let (mut at, mut n) = (0, 0);
    while at + 8 <= data.len() {
        if data[at..at + 8] == RLE_MARKER {
            n += 8 * u64::from_be_bytes(data[at + 8..at + 16].try_into().unwrap()) as usize;
            at += 24;
        } else {
            n += 8;
            at += 8;
        }
    }
    n
}

struct Upload {
    id: u16,
    store: u16,
    index: u16,
    size: usize,
    data: Vec<u8>,
    budget: u32,
}

#[derive(Default)]
struct Sim {
    stills: HashMap<u16, Vec<u8>>,
    macros: HashMap<u16, Vec<u8>>,
    names: HashMap<u16, String>,
    locked: bool,
    upload: Option<Upload>,
}

struct Switcher {
    socket: UdpSocket,
    client: Option<SocketAddr>,
    next_id: u16,
    last_from_client: u16,
    sim: Arc<Mutex<Sim>>,
}

fn mpfe(slot: u16) -> Vec<u8> {
    let mut b = vec![0u8; 24];
    b[2..4].copy_from_slice(&slot.to_be_bytes());
    b[4] = 1;
    b[23] = 4;
    b.extend_from_slice(b"pict");
    cmd(b"MPfe", &b)
}

impl Switcher {
    async fn send(&mut self, payload: &[u8]) {
        let p = packet(0x01, 0, self.next_id, payload);
        self.next_id += 1;
        if let Some(to) = self.client {
            self.socket.send_to(&p, to).await.unwrap();
        }
    }

    async fn dump(&mut self) {
        let mut pin = vec![0u8; 44];
        pin[..9].copy_from_slice(b"ATEM Mini");
        pin[40] = 13;
        let mut top = vec![0u8; 24];
        top[..4].copy_from_slice(&[1, 2, 1, 1]);
        let mut media = vec![
            cmd(b"VidM", &[0, 0, 0, 0]),
            cmd(b"_mpl", &[20, 0, 0, 0]),
            cmd(b"_MAC", &[10, 0, 0, 0]),
        ];
        let macros: Vec<u16> = self.sim.lock().unwrap().macros.keys().copied().collect();
        for slot in macros {
            let mut mprp = vec![0u8; 8];
            mprp[0..2].copy_from_slice(&slot.to_be_bytes());
            mprp[2] = 1;
            mprp[5] = 2;
            mprp.extend_from_slice(b"Go");
            media.push(cmd(b"MPrp", &mprp));
        }
        for payload in [
            [
                cmd(b"_ver", &[0, 2, 0, 30]),
                cmd(b"_pin", &pin),
                cmd(b"_top", &top),
            ]
            .concat(),
            [cmd(b"_MeC", &[0, 1, 0, 0]), media.concat()].concat(),
            cmd(b"InCm", &[1, 0, 0, 0]),
        ] {
            self.send(&payload).await;
        }
    }

    async fn budget(&mut self, id: u16) {
        let mut b = vec![0u8; 12];
        b[0..2].copy_from_slice(&id.to_be_bytes());
        b[6..8].copy_from_slice(&1396u16.to_be_bytes());
        b[8..10].copy_from_slice(&64u16.to_be_bytes());
        self.send(&cmd(b"FTCD", &b)).await;
    }

    async fn command(&mut self, name: &[u8], b: &[u8]) {
        match name {
            b"LOCK" => {
                let on = b[2] == 1;
                let was = std::mem::replace(&mut self.sim.lock().unwrap().locked, on);
                if on && !was {
                    self.send(&[cmd(b"LKOB", &[0, 0, 0, 0]), cmd(b"LKST", &[0, 0, 1, 0])].concat())
                        .await;
                } else if !on && was {
                    self.send(&cmd(b"LKST", &[0, 0, 0, 0])).await;
                }
            }
            b"FTSD" => {
                self.sim.lock().unwrap().upload = Some(Upload {
                    id: u16_at(b, 0),
                    store: u16_at(b, 2),
                    index: u16_at(b, 6),
                    size: u32::from_be_bytes([b[8], b[9], b[10], b[11]]) as usize,
                    data: Vec::new(),
                    budget: 64,
                });
                self.budget(u16_at(b, 0)).await;
            }
            b"FTFD" => {
                let name: Vec<u8> = b[2..66].iter().copied().take_while(|c| *c != 0).collect();
                let mut sim = self.sim.lock().unwrap();
                if let Some(index) = sim.upload.as_ref().map(|u| u.index) {
                    sim.names
                        .insert(index, String::from_utf8_lossy(&name).into_owned());
                }
            }
            b"FTDa" => {
                let chunk = &b[4..4 + u16_at(b, 2) as usize];
                let done = {
                    let mut sim = self.sim.lock().unwrap();
                    let Some(u) = sim.upload.as_mut() else {
                        return;
                    };
                    u.data.extend_from_slice(chunk);
                    u.budget -= 1;
                    let got = if u.store == 0xFFFF {
                        u.data.len()
                    } else {
                        expanded(&u.data)
                    };
                    if got >= u.size {
                        let u = sim.upload.take().unwrap();
                        let still = (u.store == 0).then_some(u.index);
                        if u.store == 0xFFFF {
                            sim.macros.insert(u.index, u.data);
                        } else {
                            sim.stills.insert(u.index, u.data);
                        }
                        Some(Ok((u.id, still)))
                    } else if u.budget == 0 {
                        u.budget = 64;
                        Some(Err(u.id))
                    } else {
                        None
                    }
                };
                match done {
                    Some(Ok((id, still))) => {
                        self.send(&cmd(b"FTDC", &[(id >> 8) as u8, id as u8, 0, 0]))
                            .await;
                        if let Some(slot) = still {
                            self.send(&mpfe(slot)).await;
                        }
                    }
                    Some(Err(id)) => self.budget(id).await,
                    None => {}
                }
            }
            b"FTSU" => {
                let (id, store, index) = (u16_at(b, 0), u16_at(b, 2), u16_at(b, 6));
                let data = {
                    let sim = self.sim.lock().unwrap();
                    if store == 0xFFFF {
                        sim.macros.get(&index).cloned()
                    } else {
                        sim.stills.get(&index).cloned()
                    }
                };
                let idb = [(id >> 8) as u8, id as u8];
                let Some(data) = data else {
                    self.send(&cmd(b"FTDE", &[idb[0], idb[1], 2, 0])).await;
                    return;
                };
                for c in data.chunks(CHUNK) {
                    let mut body = idb.to_vec();
                    body.extend_from_slice(&(c.len() as u16).to_be_bytes());
                    body.extend_from_slice(c);
                    self.send(&cmd(b"FTDa", &body)).await;
                }
                self.send(&cmd(b"FTDC", &[idb[0], idb[1], 0, 0])).await;
            }
            _ => {}
        }
    }

    async fn run(mut self) {
        let mut buf = [0u8; 2048];
        loop {
            let Ok((n, from)) = self.socket.recv_from(&mut buf).await else {
                return;
            };
            let data = buf[..n].to_vec();
            let flags = data[0] >> 3;
            if flags & 0x02 != 0 {
                self.client = Some(from);
                self.next_id = 1;
                self.last_from_client = 0;
                let reply = packet(0x02, 0, 0, &[2, 0, 0, 0, 0, 0, 0, 0]);
                let reply = [&reply[..2], &0x53abu16.to_be_bytes(), &reply[4..]].concat();
                self.socket.send_to(&reply, from).await.unwrap();
                continue;
            }
            if flags == 0x10 && self.next_id == 1 && self.client == Some(from) {
                self.dump().await;
                continue;
            }
            if flags & 0x01 != 0 {
                let id = u16_at(&data, 10);
                if id != self.last_from_client.wrapping_add(1) {
                    if id <= self.last_from_client {
                        let ack = packet(0x10, self.last_from_client, 0, &[]);
                        self.socket.send_to(&ack, from).await.unwrap();
                    }
                    continue;
                }
                self.last_from_client = id;
                self.socket
                    .send_to(&packet(0x10, id, 0, &[]), from)
                    .await
                    .unwrap();
                let mut at = 12;
                while at + 8 <= data.len() {
                    let len = u16_at(&data, at) as usize;
                    let name = data[at + 4..at + 8].to_vec();
                    let body = data[at + 8..at + len].to_vec();
                    self.command(&name, &body).await;
                    at += len;
                }
            }
        }
    }
}

async fn simulated_atem(sim: Arc<Mutex<Sim>>) -> u16 {
    let socket = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = socket.local_addr().unwrap().port();
    tokio::spawn(
        Switcher {
            socket,
            client: None,
            next_id: 1,
            last_from_client: 0,
            sim,
        }
        .run(),
    );
    port
}

/// Every string in a value, to check none shows a host path.
fn strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(a) => a.iter().for_each(|v| strings(v, out)),
        Value::Object(m) => m.values().for_each(|v| strings(v, out)),
        _ => {}
    }
}

struct Events<'a> {
    service: &'a Service,
    seen: Vec<Value>,
}

impl Events<'_> {
    async fn until(&mut self, mut pred: impl FnMut(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(found) = self.seen.iter().find(|e| pred(e)) {
                return found.clone();
            }
            assert!(
                Instant::now() < deadline,
                "no such event in {:?}",
                self.seen
            );
            let batch = self
                .service
                .get_json("/v1/events?max=256&wait_ms=200")
                .await;
            self.seen.extend(batch.as_array().unwrap().iter().cloned());
        }
    }

    async fn finished(&mut self, transfer: &Value) -> Value {
        self.until(|e| {
            e["event"] == "message"
                && e["address"] == "transfer_finished"
                && e["args"]["transfer"] == *transfer
        })
        .await["args"]
            .clone()
    }
}

/// A 640 by 480 RGBA picture: flat bands, so it compresses.
fn picture() -> Vec<u8> {
    let mut rgba = Vec::with_capacity(640 * 480 * 4);
    for i in 0..640 * 480 {
        let band = ((i / 640) / 60) as u8;
        rgba.extend_from_slice(&[band * 30, 200 - band * 20, 90, 255]);
    }
    rgba
}

#[tokio::test(flavor = "multi_thread")]
async fn an_atem_takes_uploaded_files_and_writes_reserved_ones() {
    let sim: Arc<Mutex<Sim>> = Arc::default();
    sim.lock()
        .unwrap()
        .macros
        .insert(0, b"\x00\x0c\x00\x00macro bytes!".to_vec());
    let port = simulated_atem(sim.clone()).await;
    let service = start("atem", &[]);
    let mut events = Events {
        service: &service,
        seen: Vec::new(),
    };
    let (_, opened) = service
        .post(
            "/v1/open",
            json!({"device": "blackmagic-atem", "model": "atem-mini",
                   "host": "127.0.0.1", "port": port}),
        )
        .await;
    let device = opened["device"].clone();
    assert!(device.is_u64(), "{opened}");
    events
        .until(|e| {
            e["event"] == "connection"
                && e["device"] == device
                && e["connection"]["status"] == "connected"
        })
        .await;

    // A still goes up from an uploaded file, named after it.
    let rgba = picture();
    let (status, up) = service.upload("Lower third.rgba", rgba.clone()).await;
    assert_eq!(status, 201, "{up}");
    let (_, started) = service
        .post(
            "/v1/execute",
            json!({"device": device, "command": "upload_still",
                   "params": {"still": 3, "path": up["path"], "format": "rgba"}}),
        )
        .await;
    let transfer = started["ok"]["value"]["transfer"].clone();
    assert!(transfer.is_u64(), "{started}");
    let done = events.finished(&transfer).await;
    assert_eq!(done["status"], "done", "{done}");
    assert_eq!(sim.lock().unwrap().names[&2], "Lower third.rgba");

    // It comes back into a reserved file, which the client fetches.
    let (_, reserved) = service
        .post("/v1/files/reserve", json!({"name": "still.rgba"}))
        .await;
    let output = reserved["files"][0].clone();
    let (_, started) = service
        .post(
            "/v1/execute",
            json!({"device": device, "command": "download_still",
                   "params": {"still": 3, "path": output["path"], "format": "rgba"}}),
        )
        .await;
    let transfer = started["ok"]["value"]["transfer"].clone();
    assert_eq!(events.finished(&transfer).await["status"], "done");
    let (status, got) = service
        .get(&format!("/v1/files/{}", output["file"].as_str().unwrap()))
        .await;
    assert_eq!(status, 200);
    assert_eq!(got.len(), rgba.len());
    assert!(got
        .iter()
        .zip(&rgba)
        .all(|(a, b)| (*a as i16 - *b as i16).abs() <= 2));

    // A macro comes down into a reserved file and goes up again from it.
    let (_, reserved) = service.post("/v1/files/reserve", json!({})).await;
    let macro_file = reserved["files"][0].clone();
    let (_, started) = service
        .post(
            "/v1/execute",
            json!({"device": device, "command": "download_macro",
                   "params": {"macro": 1, "path": macro_file["path"]}}),
        )
        .await;
    let transfer = started["ok"]["value"]["transfer"].clone();
    assert_eq!(events.finished(&transfer).await["status"], "done");
    let (_, started) = service
        .post(
            "/v1/execute",
            json!({"device": device, "command": "upload_macro",
                   "params": {"macro": 5, "path": macro_file["path"], "name": "Copy"}}),
        )
        .await;
    let transfer = started["ok"]["value"]["transfer"].clone();
    assert_eq!(events.finished(&transfer).await["status"], "done");
    assert_eq!(
        sim.lock().unwrap().macros[&4],
        b"\x00\x0c\x00\x00macro bytes!"
    );

    // Neither the events nor the snapshot show where the files are.
    let snapshot = service.get_json(&format!("/v1/snapshot/{device}")).await;
    let mut all = Vec::new();
    strings(&snapshot, &mut all);
    events.seen.iter().for_each(|e| strings(e, &mut all));
    let root = std::path::absolute(&service.dir).unwrap();
    let root = root.to_string_lossy();
    assert!(all.iter().all(|s| !s.contains(root.as_ref())), "{all:?}");

    service.post("/v1/close", json!({"device": device})).await;
}
