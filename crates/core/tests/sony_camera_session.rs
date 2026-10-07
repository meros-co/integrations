//! A Sony camera speaking PTP-IP, simulated on real TCP, driven through the
//! public API.
//!
//! The simulator accepts the command connection (Init Command Request and
//! Ack), then the event connection (Init Event Request and Ack), answers
//! SDIO_OpenSession or OpenSession, DeviceInfo, the three SDIO_Connect phases
//! and SDIO_GetExtDeviceInfo (empty the first time, as a camera that is not
//! ready yet does), and serves its properties to SDIO_GetAllExtDevicePropInfo,
//! all of them or only those changed since the last read. It applies
//! SDIO_SetExtDevicePropValue writes and step controls, and it can change a
//! property as if an operator had pressed a button on the body, announcing
//! that on the event connection. It also serves a live view frame, a content
//! list and a file in parts, an FTP server list it updates on writes, and an
//! FTP job list. One simulator speaks Camera Control PTP 3, another PTP 2.

#![cfg(feature = "sony-camera")]

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use meros_integrations::{CommandError, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

const UINT8: u16 = 0x0002;
const UINT16: u16 = 0x0004;
const INT8: u16 = 0x0001;
const UINT32: u16 = 0x0006;

/// The file the simulated camera has on its card.
const CONTENT: &[u8] = b"pretend this is a JPEG from slot one";

/// One property: datatype, writable, current value, accepted values.
#[derive(Clone)]
struct Prop {
    datatype: u16,
    writable: bool,
    value: i64,
    options: Vec<i64>,
}

#[derive(Default)]
struct Camera {
    /// Camera Control PTP 2 rather than 3.
    ptp2: bool,
    props: BTreeMap<u16, Prop>,
    controls: Vec<u16>,
    /// Changed since the last read.
    dirty: Vec<u16>,
    /// (code, value bytes) of every property write and control received.
    writes: Vec<(u16, Vec<u8>)>,
    /// Every operation code received, in order, with its parameters.
    operations: Vec<(u16, Vec<u32>)>,
    /// Parameters of every property read.
    reads: Vec<Vec<u32>>,
    ext_info_asked: u32,
    /// FTP servers: id, name, host, and the password the camera keeps.
    ftp: Vec<(u16, String, String, Option<String>)>,
    /// FTP jobs: id and clip.
    jobs: Vec<(u32, String)>,
    /// Parameters and bytes of every partial upload.
    parts: Vec<(Vec<u32>, Vec<u8>)>,
    /// How long each content data part takes, as over a slow link.
    part_delay_ms: u64,
}

/// The camera-setting file the simulated camera saves.
const SETTINGS_FILE: &[u8] = b"camera settings, binary in a real camera";

type Shared = Arc<Mutex<Camera>>;

fn int_bytes(datatype: u16, v: i64) -> Vec<u8> {
    let size = match datatype {
        INT8 | UINT8 => 1,
        UINT16 => 2,
        _ => 4,
    };
    v.to_le_bytes()[..size].to_vec()
}

fn ptp_string(text: &str) -> Vec<u8> {
    if text.is_empty() {
        return vec![0];
    }
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut b = vec![units.len() as u8 + 1];
    for u in units {
        b.extend_from_slice(&u.to_le_bytes());
    }
    b.extend_from_slice(&[0, 0]);
    b
}

fn utf16z(text: &str) -> Vec<u8> {
    let mut b = Vec::new();
    for u in text.encode_utf16() {
        b.extend_from_slice(&u.to_le_bytes());
    }
    b.extend_from_slice(&[0, 0]);
    b
}

/// A property description: code, type, get/set, enabled, default, current,
/// then an enumeration form listing the options twice.
fn prop_dataset(code: u16, p: &Prop) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&code.to_le_bytes());
    b.extend_from_slice(&p.datatype.to_le_bytes());
    b.push(p.writable as u8);
    b.push(1);
    b.extend(int_bytes(p.datatype, 0));
    b.extend(int_bytes(p.datatype, p.value));
    b.push(2);
    for _ in 0..2 {
        b.extend_from_slice(&(p.options.len() as u16).to_le_bytes());
        for o in &p.options {
            b.extend(int_bytes(p.datatype, *o));
        }
    }
    b
}

fn packet(kind: u32, body: &[u8]) -> Vec<u8> {
    let mut b = ((8 + body.len()) as u32).to_le_bytes().to_vec();
    b.extend_from_slice(&kind.to_le_bytes());
    b.extend_from_slice(body);
    b
}

fn response(code: u16, transaction: u32, params: &[u32]) -> Vec<u8> {
    let mut body = code.to_le_bytes().to_vec();
    body.extend_from_slice(&transaction.to_le_bytes());
    for p in params {
        body.extend_from_slice(&p.to_le_bytes());
    }
    packet(7, &body)
}

/// Start Data and End Data carrying a whole data phase.
fn data_in(transaction: u32, payload: &[u8]) -> Vec<u8> {
    let mut start = transaction.to_le_bytes().to_vec();
    start.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    let mut end = transaction.to_le_bytes().to_vec();
    end.extend_from_slice(payload);
    let mut out = packet(9, &start);
    out.extend(packet(12, &end));
    out
}

fn event(code: u16, params: &[u32]) -> Vec<u8> {
    let mut body = code.to_le_bytes().to_vec();
    body.extend_from_slice(&0u32.to_le_bytes());
    for p in params {
        body.extend_from_slice(&p.to_le_bytes());
    }
    packet(8, &body)
}

/// Reads one PTP-IP packet: (type, body).
async fn read_packet(stream: &mut TcpStream) -> Option<(u32, Vec<u8>)> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header).await.ok()?;
    let length = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
    let kind = u32::from_le_bytes(header[4..].try_into().unwrap());
    let mut body = vec![0u8; length - 8];
    stream.read_exact(&mut body).await.ok()?;
    Some((kind, body))
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn ext_device_info(version: u16, props: &[u16], controls: &[u16]) -> Vec<u8> {
    let mut b = version.to_le_bytes().to_vec();
    for list in [props, controls] {
        b.extend_from_slice(&(list.len() as u32).to_le_bytes());
        for c in list {
            b.extend_from_slice(&c.to_le_bytes());
        }
    }
    b
}

fn device_info(operations: &[u16]) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&100u16.to_le_bytes());
    b.extend_from_slice(&0x11u32.to_le_bytes());
    b.extend_from_slice(&300u16.to_le_bytes());
    b.extend(ptp_string("Sony PTP Extensions"));
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&(operations.len() as u32).to_le_bytes());
    for op in operations {
        b.extend_from_slice(&op.to_le_bytes());
    }
    for _ in 0..4 {
        b.extend_from_slice(&0u32.to_le_bytes());
    }
    for s in ["Sony Corporation", "ILCE-7SM3", "3.00", "0123456789"] {
        b.extend(ptp_string(s));
    }
    b
}

/// A JPEG reduced to its start-of-frame header: enough to have a size.
fn tiny_jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut b = vec![0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x0B, 0x08];
    b.extend_from_slice(&height.to_be_bytes());
    b.extend_from_slice(&width.to_be_bytes());
    b.extend_from_slice(&[0x01, 0x01, 0x11, 0x00, 0xFF, 0xD9]);
    b
}

/// The live view dataset: image offset and size (and on PTP 3 the focus
/// frame offset and size), some padding, then the JPEG.
fn live_view(jpeg: &[u8], ptp2: bool) -> Vec<u8> {
    let header = if ptp2 { 8u32 } else { 16 };
    let offset = header + 4;
    let mut b = offset.to_le_bytes().to_vec();
    b.extend_from_slice(&(jpeg.len() as u32).to_le_bytes());
    if !ptp2 {
        b.extend_from_slice(&[0; 8]);
    }
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(jpeg);
    b
}

/// A content list with one still holding one file of CONTENT's size.
fn content_list() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&100u16.to_le_bytes());
    b.extend_from_slice(&[0, 0]);
    b.extend_from_slice(&1_780_000_000_000u64.to_le_bytes());
    b.extend_from_slice(&[0; 128]);
    b.extend_from_slice(&1u32.to_le_bytes()); // slot
    b.extend_from_slice(&1u32.to_le_bytes()); // one content
    for v in [1u32, 7, 100, 12, 0, 0, 1] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    for _ in 0..4 {
        b.extend_from_slice(&1_780_000_000_000u64.to_le_bytes());
    }
    for v in [0u32, 0, 0, 0] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&1u32.to_le_bytes()); // one file
    b.extend_from_slice(&3u16.to_le_bytes());
    b.extend_from_slice(&[0, 0]);
    let path = b"DCIM/100MSDCF/DSC00012.JPG\0";
    b.extend_from_slice(&(path.len() as u32).to_le_bytes());
    b.extend_from_slice(path);
    b.extend_from_slice(&0x3801u32.to_le_bytes());
    b.extend_from_slice(&(CONTENT.len() as u64).to_le_bytes());
    b.extend_from_slice(&[0x11; 32]);
    for v in [0u32, 0, 0] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

fn text16(b: &mut Vec<u8>, text: &str) {
    if text.is_empty() {
        b.extend_from_slice(&0u16.to_le_bytes());
    } else {
        b.extend_from_slice(&(text.len() as u16 + 1).to_le_bytes());
        b.extend_from_slice(text.as_bytes());
        b.push(0);
    }
}

fn read_text16(b: &[u8], at: &mut usize) -> String {
    let n = u16_at(b, *at) as usize;
    *at += 2;
    let s = String::from_utf8_lossy(&b[*at..*at + n])
        .trim_end_matches('\0')
        .to_string();
    *at += n;
    s
}

/// The server list as the camera sends it (version 1.01, no passwords),
/// inside its offset-and-size wrapper.
fn ftp_list(servers: &[(u16, String, String, Option<String>)]) -> Vec<u8> {
    let mut l = 101u16.to_le_bytes().to_vec();
    l.extend_from_slice(&[0, 0]);
    l.extend_from_slice(&(servers.len() as u32).to_le_bytes());
    for (id, name, host, password) in servers {
        l.extend_from_slice(&id.to_le_bytes());
        l.push(1);
        text16(&mut l, name);
        text16(&mut l, host);
        l.extend_from_slice(&21u16.to_le_bytes());
        text16(&mut l, "cam");
        l.push(password.is_some() as u8);
        text16(&mut l, "");
        l.push(2);
        text16(&mut l, "/in");
        l.extend_from_slice(&[1, 1, 2, 2]);
    }
    let mut b = 8u32.to_le_bytes().to_vec();
    b.extend_from_slice(&(l.len() as u32).to_le_bytes());
    b.extend(l);
    b
}

/// Reads the one server of a written list: id, name, host, password.
fn parse_ftp_write(data: &[u8]) -> (u16, String, String, Option<String>) {
    let l = &data[u32_at(data, 0) as usize..];
    let mut at = 8;
    let id = u16_at(l, at);
    at += 3;
    let name = read_text16(l, &mut at);
    let host = read_text16(l, &mut at);
    at += 2;
    read_text16(l, &mut at);
    let has_password = l[at] == 1;
    at += 1;
    let password = read_text16(l, &mut at);
    (id, name, host, has_password.then_some(password))
}

fn job_list(jobs: &[(u32, String)], sync: u32) -> Vec<u8> {
    let mut l = 101u16.to_le_bytes().to_vec();
    l.extend_from_slice(&[0, 0]);
    l.extend_from_slice(&sync.to_le_bytes());
    l.extend_from_slice(&(jobs.len() as u32).to_le_bytes());
    for (id, clip) in jobs {
        let mut e = Vec::new();
        for v in [*id, 1, 1, 0x100, 1] {
            e.extend_from_slice(&v.to_le_bytes());
        }
        e.extend_from_slice(&1000u64.to_le_bytes());
        e.extend_from_slice(&0u64.to_le_bytes());
        for text in [clip.as_str(), clip.as_str(), ""] {
            if text.is_empty() {
                e.push(0);
            } else {
                e.push(text.len() as u8 + 1);
                e.extend_from_slice(text.as_bytes());
                e.push(0);
            }
        }
        l.extend_from_slice(&((e.len() + 4) as u32).to_le_bytes());
        l.extend(e);
    }
    let mut b = 8u32.to_le_bytes().to_vec();
    b.extend_from_slice(&(l.len() as u32).to_le_bytes());
    b.extend(l);
    b
}

/// The clip path of a written job (version 1.01 layout).
fn parse_job_add(data: &[u8]) -> String {
    let l = &data[u32_at(data, 0) as usize..];
    // version, reserved, count, entry size, server, slot
    let at = 4 + 4 + 4 + 4 + 4;
    let n = l[at] as usize;
    String::from_utf8_lossy(&l[at + 1..at + n])
        .trim_end_matches('\0')
        .to_string()
}

async fn command_connection(mut stream: TcpStream, camera: Shared, events: mpsc::Sender<Vec<u8>>) {
    // Init Command Request: GUID, friendly name, version.
    let (kind, body) = read_packet(&mut stream).await.unwrap();
    assert_eq!(kind, 1, "Init Command Request first");
    let name: Vec<u16> = body[16..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .take_while(|u| *u != 0)
        .collect();
    assert_eq!(String::from_utf16(&name).unwrap(), "Camera Desk");
    let mut ack = 7u32.to_le_bytes().to_vec();
    ack.extend_from_slice(&[0xAB; 16]);
    ack.extend(utf16z("ILCE-7SM3"));
    ack.extend_from_slice(&0x0001_0000u32.to_le_bytes());
    stream.write_all(&packet(2, &ack)).await.unwrap();

    let mut pending: Option<(u32, u16, Vec<u32>)> = None;
    while let Some((kind, body)) = read_packet(&mut stream).await {
        match kind {
            // Operation request.
            6 => {
                let code = u16_at(&body, 4);
                let t = u32_at(&body, 6);
                let params: Vec<u32> = body[10..]
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_le_bytes(*c))
                    .collect();
                let ok = || response(0x2001, t, &[]);
                let reply = {
                    let mut c = camera.lock().unwrap();
                    c.operations.push((code, params.clone()));
                    let version: u16 = if c.ptp2 { 0x00C8 } else { 0x012C };
                    let vendor: &[u32] = if c.ptp2 { &[] } else { &[300] };
                    match code {
                        0x1002 | 0x1003 => ok(),
                        0x9210 if !c.ptp2 => ok(),
                        0x1001 => [
                            data_in(t, &device_info(&[0x1001, 0x1002, 0x1009, 0x9217])),
                            ok(),
                        ]
                        .concat(),
                        0x9201 => [data_in(t, &[0; 8]), ok()].concat(),
                        0x9202 => {
                            c.ext_info_asked += 1;
                            if c.ext_info_asked == 1 {
                                // Not ready yet: no data.
                                response(0x2001, t, vendor)
                            } else {
                                let codes: Vec<u16> = c.props.keys().copied().collect();
                                [
                                    data_in(t, &ext_device_info(version, &codes, &c.controls)),
                                    response(0x2001, t, vendor),
                                ]
                                .concat()
                            }
                        }
                        0x9209 => {
                            c.reads.push(params.clone());
                            let diff = params.first().copied().unwrap_or(0);
                            let codes: Vec<u16> = if diff == 1 {
                                std::mem::take(&mut c.dirty)
                            } else {
                                c.dirty.clear();
                                c.props.keys().copied().collect()
                            };
                            let mut array = (codes.len() as u64).to_le_bytes().to_vec();
                            for code in codes {
                                array.extend(prop_dataset(code, &c.props[&code]));
                            }
                            [data_in(t, &array), ok()].concat()
                        }
                        // Live view.
                        0x1009 if params.first() == Some(&0xFFFF_C002) => {
                            [data_in(t, &live_view(&tiny_jpeg(640, 360), c.ptp2)), ok()].concat()
                        }
                        // Content list and content data, in parts.
                        0x923C => [data_in(t, &content_list()), ok()].concat(),
                        0x923D => {
                            assert_eq!(
                                params[0..2],
                                [7, (1 << 24) | 3],
                                "content 7, slot 1, file 3"
                            );
                            let offset = params[2] as usize;
                            let max = (params[4] & 0x7FFF_FFFF) as usize;
                            // Serve at most 10 bytes a time, as a camera may.
                            let end = (offset + max.min(10)).min(CONTENT.len());
                            [data_in(t, &CONTENT[offset..end]), ok()].concat()
                        }
                        // The camera-setting file: its ObjectInfo, then the file.
                        0x1008 if params.first() == Some(&0xFFFF_C004) => {
                            let mut info = vec![0u8; 52];
                            info.extend_from_slice(&[0, 0]);
                            [data_in(t, &info), ok()].concat()
                        }
                        0x1009 if params.first() == Some(&0xFFFF_C004) => {
                            [data_in(t, SETTINGS_FILE), ok()].concat()
                        }
                        // FTP.
                        0x921F => [data_in(t, &ftp_list(&c.ftp)), ok()].concat(),
                        0x9217 => [data_in(t, &job_list(&c.jobs, 2)), ok()].concat(),
                        // Data follows in Start Data and End Data.
                        0x9205 | 0x9207 | 0x9220 | 0x9218 | 0x9229 | 0x921B => {
                            pending = Some((t, code, params.clone()));
                            continue;
                        }
                        _ => response(0x2005, t, &[]),
                    }
                };
                let delay = camera.lock().unwrap().part_delay_ms;
                if code == 0x923D && delay > 0 {
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
                stream.write_all(&reply).await.unwrap();
            }
            // Start Data: the length is all we need to know.
            9 => {}
            // End Data: the data of the pending operation.
            12 => {
                let (t, code, params) = pending.take().expect("a pending operation");
                let data = body[4..].to_vec();
                let mut notify = None;
                {
                    let mut c = camera.lock().unwrap();
                    match code {
                        0x9220 => {
                            let server = parse_ftp_write(&data);
                            c.ftp.retain(|s| s.0 != server.0);
                            c.ftp.push(server);
                        }
                        0x9229 => c.parts.push((params.clone(), data[12..].to_vec())),
                        // Applying an upload: the result follows as an event.
                        0x921B => notify = Some(event(0xC214, &[1, params[0]])),
                        0x9218 => {
                            if params[0] == 1 {
                                let next = c.jobs.len() as u32 + 1;
                                c.jobs.push((next, parse_job_add(&data)));
                            }
                            notify = Some(event(0xC211, &[1, params[0]]));
                        }
                        _ => {
                            let target = params[0] as u16;
                            c.writes.push((target, data.clone()));
                            let ptp2 = c.ptp2;
                            if let Some(p) = c.props.get_mut(&target) {
                                if code == 0x9207 && ptp2 {
                                    // A step control: move along the list.
                                    let steps = data[0] as i8 as i64;
                                    let at = p.options.iter().position(|o| *o == p.value).unwrap()
                                        as i64;
                                    p.value = p.options[(at + steps) as usize];
                                } else {
                                    let mut raw = [0u8; 8];
                                    raw[..data.len()].copy_from_slice(&data);
                                    p.value = i64::from_le_bytes(raw);
                                }
                                c.dirty.push(target);
                                notify = Some(event(0xC203, &[]));
                            }
                        }
                    }
                }
                stream.write_all(&response(0x2001, t, &[])).await.unwrap();
                if let Some(e) = notify {
                    let _ = events.send(e).await;
                }
            }
            _ => {}
        }
    }
}

async fn event_connection(mut stream: TcpStream, mut events: mpsc::Receiver<Vec<u8>>) {
    let (kind, body) = read_packet(&mut stream).await.unwrap();
    assert_eq!(kind, 3, "Init Event Request");
    assert_eq!(
        u32_at(&body, 0),
        7,
        "with the connection number from the ack"
    );
    stream.write_all(&packet(4, &[])).await.unwrap();
    while let Some(bytes) = events.recv().await {
        if stream.write_all(&bytes).await.is_err() {
            return;
        }
    }
}

fn prop(datatype: u16, writable: bool, value: i64, options: &[i64]) -> Prop {
    Prop {
        datatype,
        writable,
        value,
        options: options.to_vec(),
    }
}

/// Starts a camera; returns its port, its state, and a way to send events.
async fn simulated_camera(setup: impl FnOnce(&mut Camera)) -> (u16, Shared, mpsc::Sender<Vec<u8>>) {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let camera: Shared = Arc::default();
    setup(&mut camera.lock().unwrap());
    let (tx, rx) = mpsc::channel(16);
    let shared = camera.clone();
    let events_tx = tx.clone();
    tokio::spawn(async move {
        let (cmd, _) = listener.accept().await.unwrap();
        tokio::spawn(command_connection(cmd, shared, events_tx));
        let (evt, _) = listener.accept().await.unwrap();
        tokio::spawn(event_connection(evt, rx));
    });
    (port, camera, tx)
}

/// The PTP 3 body: Alpha 7S III properties and controls.
fn ptp3_body(c: &mut Camera) {
    // White balance: auto; daylight, shade and colour temperature allowed.
    c.props.insert(
        0x5005,
        prop(UINT16, true, 0x0002, &[0x0002, 0x0004, 0x8011, 0x8012]),
    );
    // F-number 2.8.
    c.props
        .insert(0x5007, prop(UINT16, true, 280, &[280, 400, 560]));
    // Movie recording state: not recording.
    c.props.insert(0xD21D, prop(UINT8, false, 0, &[]));
    // Battery 64 %.
    c.props.insert(0xD218, prop(INT8, false, 64, &[]));
    // Media slot 1: OK.
    c.props.insert(0xD248, prop(UINT8, false, 1, &[]));
    // Live view available.
    c.props.insert(0xD221, prop(UINT8, false, 1, &[]));
    // FTP settings may be written.
    c.props.insert(0xD09A, prop(UINT8, false, 1, &[]));
    c.controls = vec![0xD2C1, 0xD2C2, 0xD2C8, 0xD313];
    c.ftp = vec![(1, "News".into(), "ftp.example".into(), Some("old".into()))];
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

async fn connected(core: &Core, model: &str, port: u16, settings: Value) -> u64 {
    let id = core
        .open(OpenRequest {
            device: "sony-camera".into(),
            model: model.into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(settings),
            monitor: true,
        })
        .unwrap();
    // Connected once the handshake is done and every property has been read.
    wait_for(core, |e| {
        matches!(e, Event::Connection { device, connection } if *device == id
            && *connection == meros_integrations::Connection::Connected)
    })
    .await;
    id
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn codes(camera: &Shared) -> Vec<u16> {
    camera
        .lock()
        .unwrap()
        .operations
        .iter()
        .map(|(c, _)| *c)
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn sony_camera_over_ptp_ip_end_to_end() {
    let (port, camera, events) = simulated_camera(ptp3_body).await;
    let core = Core::new().unwrap();
    let id = connected(
        &core,
        "ilce-7sm3",
        port,
        json!({"friendly_name": "Camera Desk", "poll_ms": 60000}),
    )
    .await;
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["white_balance"]["mode"], "auto");
    assert_eq!(state["exposure"]["iris"], 2.8);
    assert_eq!(state["recording"]["state"], "not_recording");
    assert_eq!(state["power"]["battery"]["percent"], 64);
    assert_eq!(state["media"]["slot1"]["status"], "ok");
    assert_eq!(state["device"]["model"], "ILCE-7SM3");
    assert_eq!(state["device"]["serial"], "0123456789");
    assert_eq!(state["session"]["function_mode"], "remote_with_transfer");
    assert_eq!(
        state["properties"]["5005"]["options"],
        json!([2, 4, 0x8011, 0x8012])
    );
    {
        let c = camera.lock().unwrap();
        assert_eq!(
            c.operations[0],
            (0x9210, vec![1, 2]),
            "remote control with transfer"
        );
        let first: Vec<u16> = c.operations[1..7].iter().map(|(c, _)| *c).collect();
        assert_eq!(
            first,
            [0x1001, 0x9201, 0x9201, 0x9202, 0x9202, 0x9201],
            "DeviceInfo, Connect 1 and 2, extension info until it has data, Connect 3"
        );
        assert_eq!(
            c.reads.first().map(|r| r[0]),
            Some(0),
            "the first read is of everything"
        );
    }

    // A typed write, sent as the property's own 16-bit type.
    assert_eq!(
        core.execute(id, "set_white_balance", params(json!({"value": "shade"})))
            .await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        camera.lock().unwrap().writes.last(),
        Some(&(0x5005, vec![0x11, 0x80]))
    );
    wait_for(&core, |_| {
        core.snapshot(id).unwrap().state["white_balance"]["mode"] == "shade"
    })
    .await;

    // A value the camera does not list is refused before anything is sent.
    assert!(matches!(
        core.execute(id, "set_white_balance", params(json!({"value": "cloudy"})))
            .await,
        Err(CommandError::InvalidParams { .. })
    ));
    assert!(matches!(
        core.execute(id, "set_iris", params(json!({"value": 3.5})))
            .await,
        Err(CommandError::InvalidParams { .. })
    ));
    assert_eq!(
        core.execute(id, "set_iris", params(json!({"value": 4.0})))
            .await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        camera.lock().unwrap().writes.last(),
        Some(&(0x5007, vec![0x90, 0x01]))
    );

    // Recording starts on the camera body; the change event brings it in.
    {
        let mut c = camera.lock().unwrap();
        c.props.get_mut(&0xD21D).unwrap().value = 1;
        c.dirty.push(0xD21D);
    }
    events.send(event(0xC203, &[])).await.unwrap();
    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["recording"]["state"] == "recording")
    })
    .await;
    assert!(
        camera
            .lock()
            .unwrap()
            .reads
            .iter()
            .skip(1)
            .all(|r| r[0] == 1),
        "reads after the first ask for changes only"
    );

    // A generic read of what the camera reports.
    match core
        .execute(id, "get_property", params(json!({"code": "D218"})))
        .await
    {
        Ok(Outcome::Value { value }) => assert_eq!(value["value"], 64),
        other => panic!("expected a value, got {other:?}"),
    }

    // A live view frame: live view is turned on first in this mode.
    match core
        .execute(id, "get_live_view_image", params(json!({})))
        .await
    {
        Ok(Outcome::Value { value }) => {
            assert_eq!(
                (value["width"].as_u64(), value["height"].as_u64()),
                (Some(640), Some(360))
            );
            let jpeg = base64::engine::general_purpose::STANDARD
                .decode(value["image"].as_str().unwrap())
                .unwrap();
            assert_eq!(jpeg, tiny_jpeg(640, 360));
        }
        other => panic!("expected a frame, got {other:?}"),
    }
    assert!(camera
        .lock()
        .unwrap()
        .writes
        .iter()
        .any(|(c, v)| *c == 0xD313 && v == &vec![2, 0]));

    // The card's content, then one file downloaded to a temporary file.
    let listing = match core
        .execute(id, "list_content", params(json!({"slot": 1})))
        .await
    {
        Ok(Outcome::Value { value }) => value,
        other => panic!("expected a listing, got {other:?}"),
    };
    let file = &listing["items"][0]["files"][0];
    assert_eq!(file["path"], "DCIM/100MSDCF/DSC00012.JPG");
    assert_eq!(file["size"], CONTENT.len());
    let path = std::env::temp_dir().join(format!("meros-sony-{}.jpg", std::process::id()));
    let result = core
        .execute(
            id,
            "download_content",
            params(json!({"id": file["id"], "path": path.to_string_lossy()})),
        )
        .await;
    assert_eq!(
        result,
        Ok(Outcome::Value {
            value: json!({"path": path.to_string_lossy(), "bytes": CONTENT.len()})
        })
    );
    assert_eq!(std::fs::read(&path).unwrap(), CONTENT);
    let _ = std::fs::remove_file(&path);
    let parts = codes(&camera).iter().filter(|c| **c == 0x923D).count();
    assert_eq!(
        parts,
        CONTENT.len().div_ceil(10),
        "read in parts until the size is reached"
    );

    // FTP: the server list, a server written with its password, read back
    // without it.
    match core
        .execute(id, "get_ftp_settings", params(json!({})))
        .await
    {
        Ok(Outcome::Value { value }) => {
            assert_eq!(value["servers"][0]["host"], "ftp.example");
            assert_eq!(value["servers"][0]["password_set"], true);
        }
        other => panic!("expected settings, got {other:?}"),
    }
    assert_eq!(
        core.execute(
            id,
            "set_ftp_server",
            params(
                json!({"server_id": 2, "name": "Desk", "host": "10.0.0.20", "password": "s3cret"})
            )
        )
        .await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        camera.lock().unwrap().ftp.last().unwrap().3.as_deref(),
        Some("s3cret"),
        "the password reached the camera"
    );
    wait_for(&core, |_| {
        core.snapshot(id).unwrap().state["ftp"]["servers"]["2"]["host"] == "10.0.0.20"
    })
    .await;
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["ftp"]["servers"]["2"]["password_set"], true);
    assert!(!state.to_string().contains("s3cret"), "never in the state");

    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn camera_control_ptp2_body() {
    let (port, camera, _events) = simulated_camera(|c| {
        c.ptp2 = true;
        // F-number reported only, changed by steps.
        c.props
            .insert(0x5007, prop(UINT16, false, 280, &[280, 400, 560]));
        c.props.insert(0x5005, prop(UINT16, true, 2, &[2, 4]));
        c.props.insert(0xD221, prop(UINT8, false, 1, &[]));
        c.controls = vec![0x5007, 0xD2C1, 0xD2C2, 0xD2C7];
    })
    .await;
    let core = Core::new().unwrap();
    let id = connected(
        &core,
        "ilce-7m3",
        port,
        json!({"friendly_name": "Camera Desk", "poll_ms": 60000}),
    )
    .await;
    {
        let c = camera.lock().unwrap();
        assert_eq!(c.operations[0], (0x1002, vec![1]), "plain OpenSession");
        assert!(
            c.operations.contains(&(0x9202, vec![0x00C8])),
            "announces 2.00"
        );
        assert_eq!(c.reads[0], Vec::<u32>::new(), "no read options in PTP 2");
    }
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["session"]["protocol"], "ptp2");
    assert_eq!(state["exposure"]["iris"], 2.8);

    // Two steps along the camera's list reach 5.6.
    assert_eq!(
        core.execute(id, "set_iris", params(json!({"value": 5.6})))
            .await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        camera.lock().unwrap().writes.last(),
        Some(&(0x5007, vec![2]))
    );
    wait_for(&core, |_| {
        core.snapshot(id).unwrap().state["exposure"]["iris"] == 5.6
    })
    .await;

    // Live view from the PTP 2 dataset.
    match core
        .execute(id, "get_live_view_image", params(json!({})))
        .await
    {
        Ok(Outcome::Value { value }) => assert_eq!(value["width"], 640),
        other => panic!("expected a frame, got {other:?}"),
    }
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ftp_jobs_on_a_video_body() {
    let (port, camera, _events) = simulated_camera(|c| {
        c.props.insert(0xD02A, prop(UINT32, false, 2, &[]));
        c.jobs = vec![(1, "C0001.MXF".into())];
    })
    .await;
    let core = Core::new().unwrap();
    let id = connected(
        &core,
        "ilme-fx6",
        port,
        json!({"friendly_name": "Camera Desk", "poll_ms": 60000}),
    )
    .await;
    // The job list is read as soon as its sync id is known.
    wait_for(&core, |_| {
        core.snapshot(id).unwrap().state["ftp"]["jobs"]["1"]["name"] == "C0001.MXF"
    })
    .await;
    assert_eq!(
        core.execute(
            id,
            "add_ftp_job",
            params(json!({"server_id": 1, "slot": 1, "clip_path": "/Clip/C0002.MXF"}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    // The camera's job event brings the new job in.
    wait_for(&core, |_| {
        core.snapshot(id).unwrap().state["ftp"]["jobs"]["2"]["name"] == "/Clip/C0002.MXF"
    })
    .await;
    assert!(camera
        .lock()
        .unwrap()
        .operations
        .contains(&(0x9218, vec![1, 0])));
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn uploads_and_setting_files() {
    let (port, camera, _events) = simulated_camera(|c| {
        // Custom grid line import enabled, and the camera's largest
        // operation small enough that a 20-byte file goes in three parts.
        c.props.insert(0xE110, prop(UINT32, false, 100, &[]));
        c.props.insert(0xD0C1, prop(UINT32, false, 20, &[]));
        // Camera-setting file can be saved.
        c.props.insert(0xD271, prop(UINT8, false, 1, &[]));
        // LUT import switched off on the camera.
        c.props.insert(0xD08B, prop(UINT8, false, 0, &[]));
    })
    .await;
    let core = Core::new().unwrap();
    let id = connected(
        &core,
        "ilce-7sm3",
        port,
        json!({"friendly_name": "Camera Desk", "poll_ms": 60000, "session_mode": "remote"}),
    )
    .await;
    let dir = std::env::temp_dir();
    let tag = std::process::id();

    // An upload in parts, applied, with the camera's result event.
    let grid = dir.join(format!("meros-sony-grid-{tag}.png"));
    std::fs::write(&grid, b"0123456789abcdefghij").unwrap();
    let result = core
        .execute(
            id,
            "import_grid_line_file",
            params(json!({"path": grid.to_string_lossy(), "custom": 1})),
        )
        .await;
    let _ = std::fs::remove_file(&grid);
    assert!(
        matches!(result, Ok(Outcome::Value { ref value }) if value["result"] == "ok"),
        "{result:?}"
    );
    {
        let c = camera.lock().unwrap();
        let parts: Vec<(Vec<u32>, usize)> =
            c.parts.iter().map(|(p, d)| (p.clone(), d.len())).collect();
        assert_eq!(
            parts,
            [
                (vec![0x0009_0001, 0, 0, 8, 0], 8),
                (vec![0x0009_0001, 8, 0, 8, 0], 8),
                (vec![0x0009_0001, 16, 0, 4, 1], 4)
            ]
        );
        let joined: Vec<u8> = c.parts.iter().flat_map(|(_, d)| d.clone()).collect();
        assert_eq!(joined, b"0123456789abcdefghij");
        assert!(c
            .operations
            .iter()
            .any(|(code, p)| *code == 0x921B && p == &vec![0x0009_0000]));
    }

    // The camera-setting file, saved to a temporary file.
    let saved = dir.join(format!("meros-sony-settings-{tag}.dat"));
    let result = core
        .execute(
            id,
            "export_camera_settings",
            params(json!({"path": saved.to_string_lossy()})),
        )
        .await;
    assert!(
        matches!(result, Ok(Outcome::Value { ref value }) if value["bytes"] == SETTINGS_FILE.len()),
        "{result:?}"
    );
    assert_eq!(std::fs::read(&saved).unwrap(), SETTINGS_FILE);
    let _ = std::fs::remove_file(&saved);

    // LUT import is off on the camera: refused before anything is read or sent.
    let before = camera.lock().unwrap().operations.len();
    match core
        .execute(
            id,
            "import_lut",
            params(json!({"path": "/nowhere/Show.cube", "user_base_look": 1})),
        )
        .await
    {
        Err(CommandError::DeviceError { code, message }) => {
            assert_eq!(code.as_deref(), Some("not_available"));
            assert!(message.contains("0xD08B"), "{message}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(camera.lock().unwrap().operations.len(), before);
    core.close(id).await;
}

fn live_requests(camera: &Shared) -> usize {
    camera
        .lock()
        .unwrap()
        .operations
        .iter()
        .filter(|(c, p)| *c == 0x1009 && p.first() == Some(&0xFFFF_C002))
        .count()
}

#[tokio::test(flavor = "multi_thread")]
async fn live_view_stream_while_watched() {
    let (port, camera, _events) = simulated_camera(ptp3_body).await;
    let core = Core::new().unwrap();
    let id = connected(
        &core,
        "ilce-7sm3",
        port,
        json!({"friendly_name": "Camera Desk", "session_mode": "remote", "poll_ms": 60000, "live_interval_ms": 20}),
    )
    .await;
    let streams = &meros_integrations::json::catalog(&core)["devices"]["sony-camera"]["streams"];
    assert_eq!(streams["live"]["format"], "jpeg");

    // Not watched: no live view requests.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(live_requests(&camera), 0);

    let live = core.open_stream(id, "live").unwrap();
    for _ in 0..3 {
        let frame = tokio::time::timeout(Duration::from_secs(5), live.next_frame())
            .await
            .expect("a frame within 5 s")
            .unwrap();
        assert_eq!(frame.format, "jpeg");
        assert!(frame.data.starts_with(&[0xFF, 0xD8]));
    }
    // Commands still go through while frames flow.
    assert_eq!(
        core.execute(
            id,
            "set_property",
            params(json!({"code": "0x5005", "value": 4}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(5), live.next_frame())
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        core.snapshot(id).unwrap().state["live_view"]["stream"],
        "running"
    );

    // Unwatched: requests stop, apart from one already in flight.
    core.close_stream(live);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let stopped_at = live_requests(&camera);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(live_requests(&camera) <= stopped_at + 1);
    core.close(id).await;
}

/// The order of live view requests (L) and content data parts (P).
fn transfer_order(camera: &Shared) -> String {
    camera
        .lock()
        .unwrap()
        .operations
        .iter()
        .filter_map(|(c, p)| match c {
            0x1009 if p.first() == Some(&0xFFFF_C002) => Some('L'),
            0x923D => Some('P'),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn live_view_during_downloads_is_the_operators_choice() {
    let (port, camera, _events) = simulated_camera(|c| {
        ptp3_body(c);
        c.part_delay_ms = 120;
    })
    .await;
    let core = Core::new().unwrap();
    let id = connected(
        &core,
        "ilce-7sm3",
        port,
        json!({"friendly_name": "Camera Desk", "poll_ms": 60000, "live_interval_ms": 20}),
    )
    .await;
    assert_eq!(
        core.snapshot(id).unwrap().state["live_view"]["during_transfers"],
        "keep"
    );
    let file = match core
        .execute(id, "list_content", params(json!({"slot": 1})))
        .await
    {
        Ok(Outcome::Value { value }) => value["items"][0]["files"][0].clone(),
        other => panic!("expected a content list, got {other:?}"),
    };
    let live = core.open_stream(id, "live").unwrap();
    tokio::time::timeout(Duration::from_secs(5), live.next_frame())
        .await
        .unwrap()
        .unwrap();
    let path = std::env::temp_dir().join(format!("meros-sony-live-{}.bin", std::process::id()));

    // Kept: frames go between the parts, and both finish.
    let mark = transfer_order(&camera).len();
    let download = core.execute(
        id,
        "download_content",
        params(json!({"id": file["id"], "path": path.to_string_lossy()})),
    );
    let (result, frames) = tokio::join!(download, async {
        let mut n = 0;
        let deadline = tokio::time::Instant::now() + Duration::from_millis(400);
        while tokio::time::Instant::now() < deadline {
            if tokio::time::timeout(Duration::from_millis(50), live.next_frame())
                .await
                .is_ok()
            {
                n += 1;
            }
        }
        n
    });
    assert!(matches!(result, Ok(Outcome::Value { .. })));
    assert!(frames >= 1, "frames during the transfer");
    let order = transfer_order(&camera)[mark..].to_string();
    let first = order.find('P').unwrap();
    let last = order.rfind('P').unwrap();
    assert!(
        order[first..last].contains('L'),
        "a frame between two parts: {order}"
    );

    // Paused: the parts go back to back.
    assert_eq!(
        core.execute(
            id,
            "set_live_view_during_transfers",
            params(json!({"mode": "pause"}))
        )
        .await,
        Ok(Outcome::Ack)
    );
    let mark = transfer_order(&camera).len();
    let result = core
        .execute(
            id,
            "download_content",
            params(json!({"id": file["id"], "path": path.to_string_lossy()})),
        )
        .await;
    assert!(matches!(result, Ok(Outcome::Value { .. })));
    let order = transfer_order(&camera)[mark..].to_string();
    let first = order.find('P').unwrap();
    let last = order.rfind('P').unwrap();
    assert!(
        !order[first..last].contains('L'),
        "no frame between parts: {order}"
    );
    // And the picture comes back afterwards.
    while live.try_frame().is_some() {}
    assert!(
        tokio::time::timeout(Duration::from_secs(5), live.next_frame())
            .await
            .unwrap()
            .is_some()
    );
    let _ = std::fs::remove_file(&path);
    core.close(id).await;
}
