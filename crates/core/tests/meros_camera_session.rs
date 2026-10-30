//! Meros Camera simulated as its control protocol's HTTP/1.1 server bound to
//! 127.0.0.1, driven through the public API.
//!
//! The fake camera answers the hello without a key and everything else only
//! with the fleet key (401 otherwise); serves the status, and the event
//! stream as NDJSON, a line per change and a keepalive, ending the first
//! stream after its first line; carries out commands, refusing ops it does
//! not list and a preset it does not have, each answer carrying only its own
//! error, and an OMT quality outside its limits; gives talkback a secret
//! address; serves clips with byte ranges
//! (206, and 416 past the end); and streams preview frames while watched.

#![cfg(feature = "meros-camera")]

mod common;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use meros_integrations::{json, value::Value};
use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

const KEY: &str = "0123456789abcdef0123456789abcdef";
const CLIP: &[u8] = b"0123456789abcdefghij";
const TALKBACK: &str = "https://127.0.0.1:8791/talkback-s3cr3t/";
/// An RTMP stream key the fake wrongly puts in its status.
const STREAM_KEY: &str = "live_rtmp_k3y_leak";
/// The camera's refusal of a format change mid-clip.
const MID_CLIP: &str = "Stop recording to change the lens, format or recording settings";
/// The OMT qualities the fake lists in its limits.
const OMT_QUALITY: [&str; 3] = ["low", "standard", "high"];
/// Every op of protocol version 1.
const OPS: [&str; 15] = [
    "capabilities",
    "settings",
    "record",
    "preset",
    "lens",
    "focusAt",
    "look",
    "monitoring",
    "outputs",
    "slate",
    "circle",
    "tally",
    "talkback",
    "return",
    "visca",
];

#[derive(Default)]
struct Counts {
    status: AtomicUsize,
    events: AtomicUsize,
    commands: AtomicUsize,
    previews: AtomicUsize,
}

struct Camera {
    status: Mutex<Value>,
    changed: watch::Sender<u64>,
    counts: Counts,
}

impl Camera {
    fn update(&self, f: impl FnOnce(&mut Value)) {
        f(&mut self.status.lock().unwrap());
        self.changed.send_modify(|n| *n += 1);
    }
}

fn initial_status() -> Value {
    json!({
        "id": "00112233aabbccdd", "name": "Camera A", "model": "SM-S926U",
        "recording": false, "clip": null, "error": null, "warnings": [],
        "tally": "off", "lens": "0", "lenses": [{"id": "0", "name": "Wide"}],
        "settings": {"lensId": "0", "width": 1920, "height": 1080, "fps": 25,
                     "whiteBalance": {"mode": "auto", "kelvin": 5600, "tint": 0}},
        "running": {"width": 1920, "height": 1080, "fps": 25, "highSpeed": false},
        "resolved": {"width": 1920, "height": 1080, "fps": 25, "highSpeed": false},
        "presets": ["Wide"], "luts": ["rec709"],
        "clips": 1, "battery": 80, "thermal": "none", "freeBytes": 64_000_000_000i64,
        "outputs": {"on": [], "problems": {}},
        "monitoring": {"zebra": false, "lut": "none"},
        "outputSettings": {"rtmp": {"on": false, "url": "", "keySet": false},
                           "omt": {"on": false, "name": "Camera A", "quality": "standard"},
                           "format": {"height": 1080, "bitrateMbps": 8, "halfRate": false}},
        "talkback": null, "talkbackOn": false, "talkbackLatch": false,
        "previewCodec": "h264", "previewNote": null
    })
}

async fn read_request(
    stream: &mut TcpStream,
) -> Option<(String, String, Vec<(String, String)>, Vec<u8>)> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while !buf.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).await.ok()? == 0 {
            return None;
        }
        buf.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&buf).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_string();
    let target = first.next()?.to_string();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let length = headers
        .iter()
        .find(|(k, _)| k == "content-length")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    stream.read_exact(&mut body).await.ok()?;
    Some((method, target, headers, body))
}

async fn json_reply(stream: &mut TcpStream, status: u16, value: &Value) {
    let body = value.to_string();
    let head = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(body.as_bytes()).await;
}

fn frame(pts: i64, keyframe: bool, data: &[u8]) -> Vec<u8> {
    let mut f = (data.len() as u32).to_be_bytes().to_vec();
    f.extend_from_slice(&pts.to_be_bytes());
    f.push(keyframe as u8);
    f.extend_from_slice(data);
    f
}

async fn serve(camera: Arc<Camera>, mut stream: TcpStream) {
    let Some((method, target, headers, body)) = read_request(&mut stream).await else {
        return;
    };
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    if method == "GET" && path == "/v1/hello" {
        let hello = json!({"id": "00112233aabbccdd", "name": "Camera A", "model": "SM-S926U", "app": "1.0.0", "protocol": 1});
        return json_reply(&mut stream, 200, &hello).await;
    }
    let bearer = headers
        .iter()
        .any(|(k, v)| k == "authorization" && v == &format!("Bearer {KEY}"));
    if !bearer && query != format!("key={KEY}") {
        return json_reply(
            &mut stream,
            401,
            &json!({"error": "this camera's fleet key is needed"}),
        )
        .await;
    }
    match (method.as_str(), path) {
        ("GET", "/v1/status") => {
            camera.counts.status.fetch_add(1, Ordering::SeqCst);
            let status = camera.status.lock().unwrap().clone();
            json_reply(&mut stream, 200, &status).await;
        }
        ("GET", "/v1/events") => {
            let n = camera.counts.events.fetch_add(1, Ordering::SeqCst);
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n")
                .await;
            let mut changed = camera.changed.subscribe();
            loop {
                let line = format!("{}\n", camera.status.lock().unwrap());
                if stream.write_all(line.as_bytes()).await.is_err() {
                    return;
                }
                // The first stream is dropped after its first line.
                if n == 0 {
                    return;
                }
                // A line per change, or the same line again as a keepalive.
                let _ = tokio::time::timeout(Duration::from_secs(5), changed.changed()).await;
            }
        }
        ("POST", "/v1/command") => {
            camera.counts.commands.fetch_add(1, Ordering::SeqCst);
            let command: Value = serde_json::from_slice(&body).unwrap();
            let answer = match command["op"].as_str().unwrap() {
                "capabilities" => json!({
                    "protocol": 1,
                    "ops": OPS,
                    "capabilities": {"model": "SM-S926U", "platform": "android", "lenses": [{
                        "id": "0", "name": "Wide", "facing": "back",
                        "exposure": {"iso": {"min": 50, "max": 3200},
                                     "exposureTimeNs": {"min": 1e5, "max": 1e8},
                                     "ev": {"min": -2, "max": 2}, "manual": true},
                        "whiteBalance": {"presets": [], "kelvin": {"min": 2000, "max": 10000}},
                        "zoom": {"min": 1, "max": 8},
                        "videoFormats": [{"width": 1920, "height": 1080, "fps": [24, 25, 30]}]
                    }]},
                    "settings": camera.status.lock().unwrap()["settings"].clone(),
                    "resolved": camera.status.lock().unwrap()["settings"].clone(),
                    "monitoring": camera.status.lock().unwrap()["monitoring"].clone(),
                    "outputs": camera.status.lock().unwrap()["outputSettings"].clone(),
                    "luts": ["rec709"], "presets": ["Wide"],
                    "limits": {
                        "outputs": {"heights": [1080, 720, 540], "bitrateMbps": {"min": 1, "max": 50},
                                    "srtLatencyMs": {"min": 20, "max": 8000},
                                    "port": {"min": 1024, "max": 65535},
                                    "omtQuality": OMT_QUALITY},
                        "monitoring": {"zebraLevel": {"min": 0.5, "max": 1},
                                       "zebra2Level": {"min": 0.3, "max": 1},
                                       "customGuide": {"min": 0.25, "max": 4},
                                       "desqueeze": {"min": 1, "max": 2}},
                        "recording": {"gainDb": {"min": -24, "max": 24}},
                        "whiteBalance": {"tint": {"min": -100, "max": 100}}
                    }
                }),
                "record" => {
                    let on = command["on"] == true;
                    camera.update(|s| {
                        s["recording"] = json!(on);
                        s["clip"] = if on {
                            json!("A001C002.mov")
                        } else {
                            Value::Null
                        };
                    });
                    json!({"recording": on, "error": null})
                }
                "tally" => {
                    let light = command["light"].clone();
                    camera.update(|s| {
                        s["tally"] = light.clone();
                        s["error"] = Value::Null;
                    });
                    json!({"tally": light})
                }
                "preset" => {
                    let error = format!("no preset {}", command["name"].as_str().unwrap());
                    camera.update(|s| s["error"] = json!(error));
                    json!({"error": error})
                }
                "talkback" => {
                    let on = command["on"].as_bool();
                    camera.update(|s| {
                        if let Some(on) = on {
                            s["talkbackOn"] = json!(on);
                            s["talkback"] = if on { json!(TALKBACK) } else { Value::Null };
                        }
                        s["error"] = Value::Null;
                    });
                    let s = camera.status.lock().unwrap().clone();
                    json!({"on": s["talkbackOn"], "latch": s["talkbackLatch"],
                           "url": s["talkback"], "error": null})
                }
                "settings" => {
                    // A clip fixes the lens, format and recording settings.
                    let patch = command["settings"].as_object().cloned().unwrap_or_default();
                    let fixed = ["lensId", "width", "height", "fps", "recording"];
                    let s = camera.status.lock().unwrap().clone();
                    let error = (s["recording"] == true
                        && patch
                            .iter()
                            .any(|(k, v)| fixed.contains(&k.as_str()) && s["settings"][k] != *v))
                    .then_some(MID_CLIP);
                    camera.update(|s| {
                        if error.is_none() {
                            for (k, v) in &patch {
                                s["settings"][k] = v.clone();
                            }
                        }
                        s["error"] = json!(error);
                    });
                    let resolved = camera.status.lock().unwrap()["settings"].clone();
                    json!({"resolved": resolved, "error": error})
                }
                "monitoring" => {
                    let patch = command["monitoring"]
                        .as_object()
                        .cloned()
                        .unwrap_or_default();
                    camera.update(|s| {
                        for (k, v) in &patch {
                            s["monitoring"][k] = v.clone();
                        }
                        s["error"] = Value::Null;
                    });
                    json!({"monitoring": camera.status.lock().unwrap()["monitoring"].clone(), "error": null})
                }
                "outputs" => {
                    // Wrongly, the key set is carried into the status as sent.
                    let patch = command["outputs"].as_object().cloned().unwrap_or_default();
                    let quality = &command["outputs"]["omt"]["quality"];
                    if !quality.is_null()
                        && !quality.as_str().is_some_and(|q| OMT_QUALITY.contains(&q))
                    {
                        let error =
                            format!("omt quality {quality} is not one of low, standard, high");
                        return json_reply(&mut stream, 200, &json!({"error": error})).await;
                    }
                    camera.update(|s| {
                        for (kind, fields) in &patch {
                            for (k, v) in fields.as_object().unwrap() {
                                s["outputSettings"][kind][k] = v.clone();
                            }
                        }
                        s["error"] = Value::Null;
                    });
                    json!({"outputs": camera.status.lock().unwrap()["outputSettings"].clone(),
                           "problems": {}, "error": null})
                }
                other => json!({"error": format!("unknown op {other}")}),
            };
            json_reply(&mut stream, 200, &answer).await;
        }
        ("GET", "/v1/clips") => {
            let list = json!([{"name": "A001C001.mov", "size": CLIP.len(), "modified": 1790000000, "circled": false}]);
            json_reply(&mut stream, 200, &list).await;
        }
        ("GET", "/v1/clips/A001C001.mov") => {
            let range = headers
                .iter()
                .find(|(k, _)| k == "range")
                .and_then(|(_, v)| v.strip_prefix("bytes="))
                .and_then(|r| r.split_once('-'))
                .map(|(a, b)| (a.parse::<usize>().unwrap(), b.parse::<usize>().unwrap()));
            let total = CLIP.len();
            match range {
                Some((a, _)) if a >= total => {
                    let head = format!(
                        "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{total}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    let _ = stream.write_all(head.as_bytes()).await;
                }
                Some((a, b)) => {
                    let b = b.min(total - 1);
                    let part = &CLIP[a..=b];
                    let head = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Type: video/quicktime\r\nAccept-Ranges: bytes\r\nContent-Range: bytes {a}-{b}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        part.len()
                    );
                    let _ = stream.write_all(head.as_bytes()).await;
                    let _ = stream.write_all(part).await;
                }
                None => json_reply(&mut stream, 400, &json!({"error": "a range"})).await,
            }
        }
        ("GET", "/v1/preview") => {
            camera.counts.previews.fetch_add(1, Ordering::SeqCst);
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/x-meros-preview\r\nConnection: close\r\n\r\n")
                .await;
            for n in 0i64.. {
                let f = frame(
                    n * 40_000,
                    n % 25 == 0,
                    &[0, 0, 0, 1, if n % 25 == 0 { 0x65 } else { 0x41 }, n as u8],
                );
                if stream.write_all(&f).await.is_err() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
        _ => json_reply(&mut stream, 404, &json!({"error": "not found"})).await,
    }
}

async fn simulated_camera() -> (u16, Arc<Camera>) {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let camera = Arc::new(Camera {
        status: Mutex::new(initial_status()),
        changed: watch::channel(0).0,
        counts: Counts::default(),
    });
    let shared = camera.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(serve(shared.clone(), stream));
        }
    });
    (port, camera)
}

fn open(core: &Core, port: u16, key: &str) -> u64 {
    core.open(OpenRequest {
        device: "meros-camera".into(),
        model: "meros-camera".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: json!({ "fleet_key": key }).as_object().unwrap().clone(),
        monitor: true,
    })
    .unwrap()
}

async fn until(what: &str, mut ok: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !ok() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what} within 10 s"));
}

async fn run(core: &Core, id: u64, command: &str, params: Value) -> Result<Value, CommandError> {
    core.execute(id, command, params.as_object().unwrap().clone())
        .await
        .map(|o| match o {
            Outcome::Value { value } => value,
            other => panic!("{other:?}"),
        })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_camera_is_monitored_and_controlled() {
    let (port, camera) = simulated_camera().await;
    let core = common::core().unwrap();
    let id = open(&core, port, KEY);
    let state = |p: &str| core.snapshot(id).unwrap().state.pointer(p).cloned();
    until("connected", || {
        core.snapshot(id).unwrap().connection == Connection::Connected
    })
    .await;
    until("capabilities and clips", || {
        state("/ops").is_some() && state("/clips/A001C001.mov/size").is_some()
    })
    .await;
    assert_eq!(state("/name"), Some(json!("Camera A")));
    assert_eq!(state("/free_bytes"), Some(json!(64_000_000_000i64)));
    assert_eq!(state("/output_settings/rtmp/keySet"), Some(json!(false)));
    assert_eq!(state("/clips/A001C001.mov/size"), Some(json!(CLIP.len())));
    assert!(core.snapshot(id).unwrap().latency_ms.is_some());

    // The first event stream ends after its first line: the status is read
    // again and the stream reopened, while the camera stays connected.
    until("the stream reopened", || {
        camera.counts.events.load(Ordering::SeqCst) >= 2
    })
    .await;
    assert!(camera.counts.status.load(Ordering::SeqCst) >= 2);
    assert_eq!(core.snapshot(id).unwrap().connection, Connection::Connected);

    // A change on the camera reaches state through the stream.
    camera.update(|s| s["warnings"] = json!(["Low storage"]));
    until("warnings", || {
        state("/warnings") == Some(json!(["Low storage"]))
    })
    .await;

    // Commands, and what their answers and the stream carry.
    let answer = run(&core, id, "set_tally", json!({"light": "program"}))
        .await
        .unwrap();
    assert_eq!(answer["tally"], "program");
    let answer = run(&core, id, "start_recording", json!({})).await.unwrap();
    assert_eq!(answer["recording"], true);
    until("recording", || {
        state("/clip") == Some(json!("A001C002.mov"))
    })
    .await;

    // A refusal in the answer fails the command.
    assert_eq!(
        run(&core, id, "recall_preset", json!({"name": "Nope"})).await,
        Err(CommandError::DeviceError {
            code: None,
            message: "no preset Nope".into()
        })
    );

    assert_eq!(state("/error"), Some(json!("no preset Nope")));
    // A command that succeeds clears it.
    run(&core, id, "set_tally", json!({"light": "preview"}))
        .await
        .unwrap();
    until("the error cleared", || state("/error").is_none()).await;

    // An op the camera does not list, and a value outside the lens's range,
    // fail before anything is sent.
    let sent = camera.counts.commands.load(Ordering::SeqCst);
    match run(&core, id, "send_op", json!({"op": "future"})).await {
        Err(CommandError::DeviceError { code, .. }) => {
            assert_eq!(code.as_deref(), Some("unsupported_op"))
        }
        other => panic!("{other:?}"),
    }
    match run(&core, id, "set_exposure", json!({"iso": 6400})).await {
        Err(CommandError::InvalidParams { message }) => {
            assert!(message.contains("50 to 3200"), "{message}")
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(camera.counts.commands.load(Ordering::SeqCst), sent);
    assert_eq!(
        state("/limits/outputs/srtLatencyMs"),
        Some(json!({"min": 20, "max": 8000}))
    );

    // Talkback: on, its address kept out of state and answers, and given
    // only by get_talkback_url.
    let answer = run(&core, id, "set_talkback", json!({"enabled": true}))
        .await
        .unwrap();
    assert_eq!(
        (answer["on"].clone(), answer["url"].clone()),
        (json!(true), json!("set"))
    );
    until("talkback on", || {
        state("/talkback") == Some(json!({"on": true, "latch": false, "url_set": true}))
    })
    .await;
    let status = run(&core, id, "get_status", json!({})).await.unwrap();
    assert_eq!(status["talkback"], "set");
    assert_eq!(
        run(&core, id, "get_talkback_url", json!({})).await.unwrap(),
        json!({"on": true, "url": TALKBACK})
    );
    assert!(!core
        .snapshot(id)
        .unwrap()
        .state
        .to_string()
        .contains("s3cr3t"));

    // Clips: a range, then past the end.
    let chunk = run(
        &core,
        id,
        "read_clip",
        json!({"name": "A001C001.mov", "offset": 4, "length": 8}),
    )
    .await
    .unwrap();
    let data = base64::engine::general_purpose::STANDARD
        .decode(chunk["data"].as_str().unwrap())
        .unwrap();
    assert_eq!(data, &CLIP[4..12]);
    assert_eq!(
        (chunk["total"].clone(), chunk["eof"].clone()),
        (json!(20), json!(false))
    );
    let tail = run(
        &core,
        id,
        "read_clip",
        json!({"name": "A001C001.mov", "offset": 16, "length": 8}),
    )
    .await
    .unwrap();
    assert_eq!(
        (tail["length"].clone(), tail["eof"].clone()),
        (json!(4), json!(true))
    );
    let past = run(
        &core,
        id,
        "read_clip",
        json!({"name": "A001C001.mov", "offset": 20}),
    )
    .await
    .unwrap();
    assert_eq!(
        (
            past["length"].clone(),
            past["eof"].clone(),
            past["total"].clone()
        ),
        (json!(0), json!(true), json!(20))
    );

    // A clip URL opens without a header.
    let url = run(&core, id, "clip_url", json!({"name": "A001C001.mov"}))
        .await
        .unwrap()["url"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(url.ends_with(&format!("/v1/clips/A001C001.mov?key={KEY}")));

    // The preview flows only while watched, each frame as the camera sent it.
    assert_eq!(camera.counts.previews.load(Ordering::SeqCst), 0);
    let live = core.open_stream(id, "live").unwrap();
    assert_eq!(live.format(), "meros-preview");
    let first = tokio::time::timeout(Duration::from_secs(5), live.next_frame())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.data[12] & 1, 1, "starts on a keyframe");
    assert_eq!(&first.data[..4], &6u32.to_be_bytes());
    core.close_stream(live);

    // Derived events and logs: what changed, and never the key.
    let events = core.poll_events(10_000);
    let messages: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::Message {
                device, address, ..
            } if *device == id => Some(address.as_str()),
            _ => None,
        })
        .collect();
    for want in ["warning", "tally_changed", "record_started"] {
        assert!(messages.contains(&want), "{want} in {messages:?}");
    }
    assert!(messages.contains(&"talkback_changed"), "{messages:?}");
    for e in &events {
        let text = serde_json::to_string(e).unwrap();
        assert!(!text.contains(KEY), "the key in {text}");
        assert!(!text.contains("s3cr3t"), "the talkback address in {text}");
    }

    // A fleet link is decoded without the camera.
    let packed =
        miniz_oxide::deflate::compress_to_vec(json!({ "key": KEY }).to_string().as_bytes(), 9);
    let link = format!(
        "meros-camera://fleet?d={}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(packed)
    );
    assert_eq!(
        run(&core, id, "decode_fleet_link", json!({ "link": link }))
            .await
            .unwrap(),
        json!({ "key": KEY })
    );
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_wrong_fleet_key_is_unauthorized() {
    let (port, camera) = simulated_camera().await;
    let core = common::core().unwrap();
    let id = open(&core, port, "ffffffffffffffffffffffffffffffff");
    until("unauthorized", || {
        matches!(
            core.snapshot(id).unwrap().connection,
            Connection::Unauthorized { .. }
        )
    })
    .await;
    // The hello still identified it.
    assert_eq!(core.snapshot(id).unwrap().state["id"], "00112233aabbccdd");
    assert!(matches!(
        run(&core, id, "start_recording", json!({})).await,
        Err(CommandError::Auth { .. })
    ));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(camera.counts.status.load(Ordering::SeqCst), 0);
    assert_eq!(camera.counts.commands.load(Ordering::SeqCst), 0);
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_camera_opened_for_commands_reads_its_capabilities_for_the_first_op() {
    let (port, camera) = simulated_camera().await;
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "meros-camera".into(),
            model: "meros-camera".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: json!({ "fleet_key": KEY }).as_object().unwrap().clone(),
            monitor: false,
        })
        .unwrap();
    until("connected", || {
        core.snapshot(id).unwrap().connection == Connection::Connected
    })
    .await;
    assert_eq!(camera.counts.commands.load(Ordering::SeqCst), 0);
    assert_eq!(camera.counts.status.load(Ordering::SeqCst), 0);
    // The capabilities, then the op.
    let answer = run(&core, id, "set_tally", json!({"light": "program"}))
        .await
        .unwrap();
    assert_eq!(answer["tally"], "program");
    assert_eq!(camera.counts.commands.load(Ordering::SeqCst), 2);
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["tally"], "program");
    assert_eq!(state["ops"].as_array().unwrap().len(), OPS.len());
    // Ranges are known from then on.
    assert!(matches!(
        run(&core, id, "set_zoom", json!({"ratio": 9})).await,
        Err(CommandError::InvalidParams { .. })
    ));
    assert_eq!(camera.counts.events.load(Ordering::SeqCst), 0);
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn monitoring_and_output_settings_follow_the_status_without_credentials() {
    let (port, camera) = simulated_camera().await;
    let core = common::core().unwrap();
    let id = open(&core, port, KEY);
    let state = |p: &str| core.snapshot(id).unwrap().state.pointer(p).cloned();
    until("capabilities", || state("/ops").is_some()).await;
    until("the stream reopened", || {
        camera.counts.events.load(Ordering::SeqCst) >= 2
    })
    .await;
    until("the outputs document", || {
        state("/output_settings/format/height") == Some(json!(1080))
    })
    .await;

    // The operator changes the monitor and an output on the camera.
    camera.update(|s| {
        s["monitoring"]["zebra"] = json!(true);
        s["outputSettings"]["format"]["height"] = json!(720);
    });
    until("monitoring and outputs from the stream", || {
        state("/monitoring/zebra") == Some(json!(true))
            && state("/output_settings/format/height") == Some(json!(720))
    })
    .await;

    // A stream key set through the driver, which the camera wrongly sends
    // back in its status: only keySet reaches state.
    run(
        &core,
        id,
        "set_outputs",
        json!({"outputs": {"rtmp": {"url": "rtmp://127.0.0.1/live", "key": STREAM_KEY}}}),
    )
    .await
    .unwrap();
    until("keySet", || {
        state("/output_settings/rtmp/keySet") == Some(json!(true))
            && state("/output_settings/rtmp/url") == Some(json!("rtmp://127.0.0.1/live"))
    })
    .await;
    assert_eq!(state("/output_settings/rtmp/key"), None);
    let status = run(&core, id, "get_status", json!({})).await.unwrap();
    assert_eq!(status["outputSettings"]["rtmp"]["keySet"], true);
    assert!(!status.to_string().contains(STREAM_KEY));

    // A format change mid-clip is refused: the command fails with the
    // camera's sentence, which state shows until a command succeeds.
    run(&core, id, "start_recording", json!({})).await.unwrap();
    assert_eq!(
        run(&core, id, "set_settings", json!({"settings": {"fps": 30}})).await,
        Err(CommandError::DeviceError {
            code: None,
            message: MID_CLIP.into()
        })
    );
    assert_eq!(state("/error"), Some(json!(MID_CLIP)));
    assert_eq!(state("/settings/fps"), Some(json!(25)));
    run(&core, id, "stop_recording", json!({})).await.unwrap();
    run(&core, id, "set_settings", json!({"settings": {"fps": 30}}))
        .await
        .unwrap();
    until("the error cleared and the rate changed", || {
        state("/error").is_none() && state("/settings/fps") == Some(json!(30))
    })
    .await;

    let events = core.poll_events(10_000);
    let changed: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            Event::Message { address, args, .. } if address == "settings_changed" => {
                Some(args["paths"].to_string())
            }
            _ => None,
        })
        .collect();
    let all = changed.join(" ");
    for want in [
        "monitoring.zebra",
        "output_settings.format.height",
        "output_settings.rtmp.keySet",
        "\"fps\"",
    ] {
        assert!(all.contains(want), "{want} in {all}");
    }
    for e in &events {
        let text = serde_json::to_string(e).unwrap();
        assert!(!text.contains(STREAM_KEY), "the stream key in {text}");
    }
    assert!(!core
        .snapshot(id)
        .unwrap()
        .state
        .to_string()
        .contains(STREAM_KEY));
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn omt_quality_is_set_checked_and_followed() {
    let (port, camera) = simulated_camera().await;
    let core = common::core().unwrap();
    let id = open(&core, port, KEY);
    let state = |p: &str| core.snapshot(id).unwrap().state.pointer(p).cloned();
    until("capabilities", || state("/ops").is_some()).await;
    assert_eq!(
        state("/limits/outputs/omtQuality"),
        Some(json!(["low", "standard", "high"]))
    );
    until("the OMT quality", || {
        state("/output_settings/omt/quality") == Some(json!("standard"))
    })
    .await;

    for q in ["low", "high", "standard"] {
        let answer = run(&core, id, "set_omt_output", json!({"quality": q}))
            .await
            .unwrap();
        assert_eq!(answer["outputs"]["omt"]["quality"], q);
        until(q, || {
            state("/output_settings/omt/quality") == Some(json!(q))
        })
        .await;
    }

    // Outside the list: refused before anything is sent.
    let sent = camera.counts.commands.load(Ordering::SeqCst);
    for (name, params) in [
        (
            "set_outputs",
            json!({"outputs": {"omt": {"quality": "ultra"}}}),
        ),
        (
            "send_op",
            json!({"op": "outputs", "fields": {"outputs": {"omt": {"quality": "best"}}}}),
        ),
    ] {
        match run(&core, id, name, params).await {
            Err(CommandError::InvalidParams { message }) => {
                assert!(message.contains("low, standard, high"), "{message}")
            }
            other => panic!("{name}: {other:?}"),
        }
    }
    assert_eq!(camera.counts.commands.load(Ordering::SeqCst), sent);

    // The operator changes it on the camera.
    camera.update(|s| s["outputSettings"]["omt"]["quality"] = json!("low"));
    until("the operator's quality", || {
        state("/output_settings/omt/quality") == Some(json!("low"))
    })
    .await;
    let all: Vec<String> = core
        .poll_events(10_000)
        .iter()
        .filter_map(|e| match e {
            Event::Message { address, args, .. } if address == "settings_changed" => {
                Some(args["paths"].to_string())
            }
            _ => None,
        })
        .collect();
    assert!(
        all.iter()
            .any(|p| p.contains("output_settings.omt.quality")),
        "{all:?}"
    );
    core.close(id).await;
}
