//! A Meros Camera's preview through the service's `.frames` route, against a
//! fake camera bound to 127.0.0.1: the frames arrive in the camera's own
//! framing, start on a keyframe, and after frames a slow reader missed resume
//! only at the next keyframe; the route needs the bearer header, and a stream
//! of another format is refused with 406.

#![cfg(feature = "all")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

const KEY: &str = "0123456789abcdef0123456789abcdef";
/// Frames every 10 ms, a keyframe every 10, each this large so that a reader
/// that stops for a while makes the service drop some.
const PAYLOAD: usize = 128 * 1024;
const GOP: i64 = 10;
const FRAME_US: i64 = 10_000;

/// Also removes the transfer folder the sidecar was given, so a killed
/// sidecar leaves nothing in the temp folder.
struct KillOnDrop(Child, PathBuf);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
        let _ = std::fs::remove_dir_all(&self.1);
    }
}

fn read_request(stream: &mut TcpStream) -> Option<(String, String, Vec<u8>)> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).ok()? == 0 {
            return None;
        }
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_string();
    let target = first.next()?.to_string();
    let length = lines
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).ok()?;
    Some((method, target, body))
}

fn json_reply(stream: &mut TcpStream, status: u16, value: &Value) {
    let body = value.to_string();
    let _ = write!(
        stream,
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
}

fn status() -> Value {
    json!({
        "id": "00112233aabbccdd", "name": "Camera A", "model": "SM-S926U",
        "recording": false, "clip": null, "error": null, "warnings": [],
        "tally": "off", "lens": "0", "lenses": [{"id": "0", "name": "Wide"}],
        "settings": {"lensId": "0", "width": 1920, "height": 1080, "fps": 25},
        "resolved": {"width": 1920, "height": 1080, "fps": 25, "highSpeed": false},
        "presets": [], "luts": [], "clips": 0, "battery": 80, "thermal": "none",
        "freeBytes": 64e9, "outputs": {"on": [], "problems": {}},
        "talkback": null, "previewCodec": "h264", "previewNote": null
    })
}

/// One preview frame: frame `n`, a keyframe at the start of each GOP.
fn preview_frame(n: i64) -> Vec<u8> {
    let key = n % GOP == 0;
    let mut data = vec![0, 0, 0, 1, if key { 0x65 } else { 0x41 }];
    data.resize(PAYLOAD, n as u8);
    let mut f = (data.len() as u32).to_be_bytes().to_vec();
    f.extend_from_slice(&(n * FRAME_US).to_be_bytes());
    f.push(key as u8);
    f.extend_from_slice(&data);
    f
}

fn serve(mut stream: TcpStream, previews: Arc<AtomicUsize>) {
    let Some((method, target, _body)) = read_request(&mut stream) else {
        return;
    };
    let path = target.split('?').next().unwrap_or_default();
    match (method.as_str(), path) {
        ("GET", "/v1/hello") => json_reply(
            &mut stream,
            200,
            &json!({"id": "00112233aabbccdd", "name": "Camera A", "model": "SM-S926U",
                    "app": "1.0.0", "protocol": 1}),
        ),
        ("GET", "/v1/status") => json_reply(&mut stream, 200, &status()),
        ("GET", "/v1/events") => {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n",
            );
            // The status as a keepalive, until the driver hangs up.
            while stream
                .write_all(format!("{}\n", status()).as_bytes())
                .is_ok()
            {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        ("POST", "/v1/command") => json_reply(
            &mut stream,
            200,
            &json!({"protocol": 1, "ops": ["capabilities"], "capabilities": {},
                    "settings": status()["settings"], "presets": [], "luts": []}),
        ),
        ("GET", "/v1/preview") => {
            previews.fetch_add(1, Ordering::SeqCst);
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: application/x-meros-preview\r\nConnection: close\r\n\r\n",
            );
            for n in 0.. {
                if stream.write_all(&preview_frame(n)).is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_micros(FRAME_US as u64));
            }
        }
        _ => json_reply(&mut stream, 404, &json!({"error": "not found"})),
    }
}

fn fake_camera() -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let port = listener.local_addr().unwrap().port();
    let previews = Arc::new(AtomicUsize::new(0));
    let counted = previews.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let counted = counted.clone();
            std::thread::spawn(move || serve(stream, counted));
        }
    });
    (port, previews)
}

/// A frame as the client parsed it.
struct Parsed {
    pts: i64,
    keyframe: bool,
    data: Vec<u8>,
}

/// Takes whole frames off the front of `buf`.
fn parse_frames(buf: &mut Vec<u8>, out: &mut Vec<Parsed>) {
    while buf.len() >= 13 {
        let length = u32::from_be_bytes(buf[..4].try_into().unwrap()) as usize;
        if buf.len() < 13 + length {
            return;
        }
        let pts = i64::from_be_bytes(buf[4..12].try_into().unwrap());
        let flags = buf[12];
        assert_eq!(flags & !1, 0, "only the keyframe flag");
        out.push(Parsed {
            pts,
            keyframe: flags & 1 == 1,
            data: buf[13..13 + length].to_vec(),
        });
        buf.drain(..13 + length);
    }
}

async fn read_frames(
    response: &mut reqwest::Response,
    buf: &mut Vec<u8>,
    count: usize,
) -> Vec<Parsed> {
    let mut frames = Vec::new();
    while frames.len() < count {
        let chunk = tokio::time::timeout(Duration::from_secs(5), response.chunk())
            .await
            .expect("a frame within 5 s")
            .unwrap()
            .expect("the stream stays open");
        buf.extend_from_slice(&chunk);
        parse_frames(buf, &mut frames);
    }
    frames
}

#[tokio::test(flavor = "multi_thread")]
async fn the_preview_is_served_as_frames_from_a_keyframe() {
    let (camera_port, previews) = fake_camera();
    let token_file: PathBuf = std::env::temp_dir().join(format!(
        "meros-integrations-test-preview-{}.token",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&token_file);
    let transfer_dir = token_file.with_extension("transfer");
    let mut sidecar = KillOnDrop(
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
            .arg(&transfer_dir)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
        transfer_dir,
    );
    let mut line = String::new();
    BufReader::new(sidecar.0.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let listening: Value = serde_json::from_str(&line).unwrap();
    let base = format!("http://{}", listening["listening"].as_str().unwrap());
    let token = std::fs::read_to_string(&token_file).unwrap();
    let http = reqwest::Client::new();
    let open = |body: Value| {
        let request = http
            .post(format!("{base}/v1/open"))
            .bearer_auth(&token)
            .json(&body);
        async move {
            let result: Value = request.send().await.unwrap().json().await.unwrap();
            result["device"]
                .as_u64()
                .unwrap_or_else(|| panic!("opened: {result}"))
        }
    };
    let camera = open(json!({"device": "meros-camera", "model": "meros-camera",
        "host": "127.0.0.1", "port": camera_port, "settings": {"fleet_key": KEY}}))
    .await;
    let frames_url = format!("{base}/v1/devices/{camera}/streams/live.frames");

    // The bearer header, and only it.
    assert_eq!(http.get(&frames_url).send().await.unwrap().status(), 401);
    let wrong = http.get(&frames_url).bearer_auth("wrong").send().await;
    assert_eq!(wrong.unwrap().status(), 401);
    let in_query = format!("{frames_url}?access_token={token}");
    assert_eq!(http.get(&in_query).send().await.unwrap().status(), 401);

    // Formats a route cannot carry: 406, and nothing opened on the camera.
    for file in ["live.mjpg", "live.jpg"] {
        let url = format!("{base}/v1/devices/{camera}/streams/{file}");
        let refused = http.get(&url).bearer_auth(&token).send().await.unwrap();
        assert_eq!(refused.status(), 406, "{file}");
        let body: Value = refused.json().await.unwrap();
        assert_eq!(body["error"]["error"], "not_acceptable", "{file}");
    }
    let snapshot = open(json!({"device": "http-snapshot", "model": "generic",
        "host": "127.0.0.1", "port": camera_port, "settings": {"path": "/snapshot.jpg"}}))
    .await;
    let url = format!("{base}/v1/devices/{snapshot}/streams/live.frames");
    let refused = http.get(&url).bearer_auth(&token).send().await.unwrap();
    assert_eq!(refused.status(), 406);
    let url = format!("{base}/v1/devices/{camera}/streams/no_such_stream.frames");
    let refused = http.get(&url).bearer_auth(&token).send().await.unwrap();
    assert_eq!(refused.status(), 404);
    assert_eq!(previews.load(Ordering::SeqCst), 0);

    // The frames, back to back in the camera's framing, chunked.
    let mut response = http
        .get(&frames_url)
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()["content-type"],
        "application/x-meros-preview"
    );
    assert_eq!(response.headers()["transfer-encoding"], "chunked");
    let mut buf = Vec::new();
    let mut frames = read_frames(&mut response, &mut buf, 5).await;
    // A reader that stops for a while misses frames.
    tokio::time::sleep(Duration::from_secs(2)).await;
    frames.extend(read_frames(&mut response, &mut buf, 60).await);

    assert!(frames[0].keyframe, "starts on a keyframe");
    let mut gaps = 0;
    for (i, f) in frames.iter().enumerate() {
        let n = f.pts / FRAME_US;
        assert_eq!(f.keyframe, n % GOP == 0, "frame {n}'s flag");
        assert_eq!(f.data.len(), PAYLOAD, "frame {n}'s length");
        assert_eq!(
            &f.data[..5],
            &[0, 0, 0, 1, if f.keyframe { 0x65 } else { 0x41 }]
        );
        assert!(
            f.data[5..].iter().all(|&b| b == n as u8),
            "frame {n}'s data"
        );
        if i > 0 {
            let before = frames[i - 1].pts / FRAME_US;
            assert!(n > before, "frame {n} after {before}");
            if n != before + 1 {
                gaps += 1;
                assert!(
                    f.keyframe,
                    "frame {n} after a gap from {before} is a keyframe"
                );
            }
        }
    }
    assert!(gaps > 0, "the pause made the service skip frames");
    drop(response);

    let _ = http
        .post(format!("{base}/v1/close"))
        .bearer_auth(&token)
        .json(&json!({"device": camera}))
        .send()
        .await;
    let _ = http
        .post(format!("{base}/v1/close"))
        .bearer_auth(&token)
        .json(&json!({"device": snapshot}))
        .send()
        .await;
    drop(sidecar);
    let _ = std::fs::remove_file(&token_file);
}
