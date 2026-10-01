//! A Sony camera speaking PTP-IP, simulated on real TCP, driven through the
//! public API.
//!
//! The simulator accepts the command connection (Init Command Request and
//! Ack), then the event connection (Init Event Request and Ack), answers
//! OpenSession, DeviceInfo, the three SDIO_Connect phases and
//! SDIO_GetExtDeviceInfo (empty the first time, as a camera that is not
//! ready yet does), and serves its properties to SDIO_GetAllExtDevicePropInfo,
//! all of them or only those changed since the last read. It applies
//! SDIO_SetExtDevicePropValue writes, and it can change a property as if an
//! operator had pressed a button on the body, announcing that on the event
//! connection.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{CommandError, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

const UINT8: u16 = 0x0002;
const UINT16: u16 = 0x0004;
const INT8: u16 = 0x0001;

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
    props: BTreeMap<u16, Prop>,
    /// Changed since the last read.
    dirty: Vec<u16>,
    /// (property, value bytes) of every write received.
    writes: Vec<(u16, Vec<u8>)>,
    /// Every operation code received, in order.
    operations: Vec<u16>,
    /// Data-flag parameter of every property read: 0 all, 1 changes.
    reads: Vec<u32>,
    ext_info_asked: u32,
}

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

fn ext_device_info(props: &[u16], controls: &[u16]) -> Vec<u8> {
    let mut b = 0x012Cu16.to_le_bytes().to_vec();
    for list in [props, controls] {
        b.extend_from_slice(&(list.len() as u32).to_le_bytes());
        for c in list {
            b.extend_from_slice(&c.to_le_bytes());
        }
    }
    b
}

fn device_info() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(&100u16.to_le_bytes());
    b.extend_from_slice(&0x11u32.to_le_bytes());
    b.extend_from_slice(&300u16.to_le_bytes());
    b.extend(ptp_string("Sony PTP Extensions"));
    b.extend_from_slice(&0u16.to_le_bytes());
    for _ in 0..5 {
        b.extend_from_slice(&0u32.to_le_bytes());
    }
    for s in ["Sony Corporation", "ILCE-7SM3", "3.00", "0123456789"] {
        b.extend(ptp_string(s));
    }
    b
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
    assert_eq!(String::from_utf16(&name).unwrap(), "Imperio Cam Desk");
    let mut ack = 7u32.to_le_bytes().to_vec();
    ack.extend_from_slice(&[0xAB; 16]);
    ack.extend(utf16z("ILCE-7SM3"));
    ack.extend_from_slice(&0x0001_0000u32.to_le_bytes());
    stream.write_all(&packet(2, &ack)).await.unwrap();

    let mut pending_write: Option<(u32, u16)> = None;
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
                camera.lock().unwrap().operations.push(code);
                let reply = match code {
                    0x1002 => response(0x2001, t, &[]),
                    0x1001 => [data_in(t, &device_info()), response(0x2001, t, &[])].concat(),
                    0x9201 => [data_in(t, &[0; 8]), response(0x2001, t, &[])].concat(),
                    0x9202 => {
                        let mut c = camera.lock().unwrap();
                        c.ext_info_asked += 1;
                        if c.ext_info_asked == 1 {
                            // Not ready yet: no data.
                            response(0x2001, t, &[300])
                        } else {
                            let codes: Vec<u16> = c.props.keys().copied().collect();
                            [
                                data_in(t, &ext_device_info(&codes, &[0xD2C1, 0xD2C2, 0xD2C8])),
                                response(0x2001, t, &[300]),
                            ]
                            .concat()
                        }
                    }
                    0x9209 => {
                        let mut c = camera.lock().unwrap();
                        let diff = params.first().copied().unwrap_or(0);
                        c.reads.push(diff);
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
                        [data_in(t, &array), response(0x2001, t, &[])].concat()
                    }
                    0x9205 | 0x9207 => {
                        // The value follows in Start Data and End Data.
                        pending_write = Some((t, params[0] as u16));
                        continue;
                    }
                    0x1003 => response(0x2001, t, &[]),
                    _ => response(0x2005, t, &[]),
                };
                stream.write_all(&reply).await.unwrap();
            }
            // Start Data: the length is all we need to know.
            9 => {}
            // End Data: the value of the pending write.
            12 => {
                let (t, code) = pending_write.take().expect("a pending write");
                let data = body[4..].to_vec();
                let changed = {
                    let mut c = camera.lock().unwrap();
                    c.writes.push((code, data.clone()));
                    match c.props.get_mut(&code) {
                        Some(p) => {
                            let mut raw = [0u8; 8];
                            raw[..data.len()].copy_from_slice(&data);
                            p.value = i64::from_le_bytes(raw);
                            c.dirty.push(code);
                            true
                        }
                        None => false,
                    }
                };
                stream.write_all(&response(0x2001, t, &[])).await.unwrap();
                if changed {
                    let _ = events.send(event(0xC203, &[])).await;
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

/// Starts the camera; returns its port, its state, and a way to send events.
async fn simulated_camera() -> (u16, Shared, mpsc::Sender<Vec<u8>>) {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let camera: Shared = Arc::default();
    {
        let mut c = camera.lock().unwrap();
        let p = |datatype, writable, value, options: &[i64]| Prop {
            datatype,
            writable,
            value,
            options: options.to_vec(),
        };
        // White balance: auto; daylight, shade and colour temperature allowed.
        c.props.insert(
            0x5005,
            p(UINT16, true, 0x0002, &[0x0002, 0x0004, 0x8011, 0x8012]),
        );
        // F-number 2.8.
        c.props
            .insert(0x5007, p(UINT16, true, 280, &[280, 400, 560]));
        // Movie recording state: not recording.
        c.props.insert(0xD21D, p(UINT8, false, 0, &[]));
        // Battery 64 %.
        c.props.insert(0xD218, p(INT8, false, 64, &[]));
        // Media slot 1: OK.
        c.props.insert(0xD248, p(UINT8, false, 1, &[]));
    }
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

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn sony_camera_over_ptp_ip_end_to_end() {
    let (port, camera, events) = simulated_camera().await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "sony-camera".into(),
            model: "ilce-7sm3".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"friendly_name": "Imperio Cam Desk", "poll_ms": 60000})),
        })
        .unwrap();

    // Connected once the handshake is done and every property has been read.
    wait_for(&core, |e| {
        matches!(e, Event::Connection { device, connection } if *device == id
            && *connection == meros_integrations::Connection::Connected)
    })
    .await;
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["white_balance"]["mode"], "auto");
    assert_eq!(state["exposure"]["iris"], 2.8);
    assert_eq!(state["recording"]["state"], "not_recording");
    assert_eq!(state["power"]["battery"]["percent"], 64);
    assert_eq!(state["media"]["slot1"]["status"], "ok");
    assert_eq!(state["device"]["model"], "ILCE-7SM3");
    assert_eq!(state["device"]["serial"], "0123456789");
    assert_eq!(
        state["properties"]["5005"]["options"],
        json!([2, 4, 0x8011, 0x8012])
    );
    {
        let c = camera.lock().unwrap();
        assert_eq!(
            c.operations[..7],
            [0x1002, 0x1001, 0x9201, 0x9201, 0x9202, 0x9202, 0x9201],
            "OpenSession, DeviceInfo, Connect 1 and 2, extension info until it has data, Connect 3"
        );
        assert_eq!(c.reads.first(), Some(&0), "the first read is of everything");
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
        camera.lock().unwrap().reads.iter().skip(1).all(|r| *r == 1),
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

    core.close(id).await;
}
