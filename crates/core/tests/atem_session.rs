//! An ATEM switcher simulated on real UDP, driven through the public API.
//!
//! The simulator answers the hello, sends a small initial state from `_ver` to
//! `InCm` as reliable packets, acknowledges the client's packets, and applies
//! program changes by sending the resulting `PrgI` and `TlSr`. It deliberately
//! loses its first copy of one state packet, so the client has to leave the gap
//! and accept the resend.
//!
//! It also runs the file transfer protocol as the module expects a switcher
//! to (see `src/modules/atem_transfer.rs`): the stills lock (`LOCK`, granted
//! with `LKOB` and announced with `LKST`, queued while another client holds
//! it), uploads (`FTSD`, budgets of chunks in `FTCD`, `FTFD`, `FTDa`, ended
//! by `FTDC` once the run-length encoded data expands to the size asked),
//! downloads (`FTSU`, `FTDa`, `FTDC`) and macros, which take no lock. It can
//! hold the stills lock for another client for a while, and can fall silent
//! in the middle of an upload; a new session then finds the old one's lock
//! freed, as a switcher frees a dead session's.

#![cfg(feature = "blackmagic-atem")]

mod common;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use meros_integrations::{CommandError, Connection, Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::net::UdpSocket;

const SESSION: u16 = 0x8123;
const RLE_MARKER: [u8; 8] = [0xFE; 8];
const CHUNK: usize = 1392;

fn packet(flags: u8, session: u16, ack: u16, id: u16, payload: &[u8]) -> Vec<u8> {
    let length = 12 + payload.len() as u16;
    let mut p = Vec::new();
    p.extend_from_slice(&(((flags as u16) << 11) | length).to_be_bytes());
    p.extend_from_slice(&session.to_be_bytes());
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

fn input(id: u16, long: &str, short: &str) -> Vec<u8> {
    let mut b = vec![0u8; 36];
    b[0..2].copy_from_slice(&id.to_be_bytes());
    b[2..2 + long.len()].copy_from_slice(long.as_bytes());
    b[22..22 + short.len()].copy_from_slice(short.as_bytes());
    cmd(b"InPr", &b)
}

fn tally(program: u16, preview: u16) -> Vec<u8> {
    let mut b = vec![0, 2];
    for source in [1u16, 2] {
        b.extend_from_slice(&source.to_be_bytes());
        b.push((source == program) as u8 | (((source == preview) as u8) << 1));
    }
    b.extend_from_slice(&[0, 0]);
    cmd(b"TlSr", &b)
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The bytes run-length encoded data expands to.
fn expanded(data: &[u8]) -> usize {
    let mut at = 0;
    let mut n = 0;
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

#[derive(Clone, Copy, PartialEq, Debug)]
enum Holder {
    Client,
    Other,
}

struct Upload {
    id: u16,
    store: u16,
    index: u16,
    size: usize,
    rle: bool,
    label: bool,
    data: Vec<u8>,
    budget: u32,
}

/// What the simulated switcher holds and saw, for the tests to check.
#[derive(Default)]
struct Sim {
    /// Stills by slot from 0, as uploaded (run-length encoded).
    stills: HashMap<u16, Vec<u8>>,
    macros: HashMap<u16, Vec<u8>>,
    names: HashMap<u16, String>,
    holder: Option<Holder>,
    /// The client asked while another client held the stills.
    queued: bool,
    /// When the other client lets go.
    other_until: Option<Instant>,
    /// Fall silent after this many chunks of an upload.
    drop_after: Option<usize>,
    silent: bool,
    acks: usize,
    /// Goodbyes received.
    goodbyes: usize,
    /// Answer an upload request with no budget, so the upload holds its
    /// lock and waits.
    stall: bool,
    /// Be an older switcher (a TVS Pro HD, model 9), which takes
    /// multiviewer labels as bitmaps; its 1080p50 multiviewer runs in
    /// 1080p50 and 720p50.
    older: bool,
    /// Labels uploaded, by source.
    labels: HashMap<u16, Vec<u8>>,
    /// Clears (CSTL) ignored for want of the stills lock.
    bare_clears: usize,
    upload: Option<Upload>,
}

struct Switcher {
    socket: UdpSocket,
    client: Option<SocketAddr>,
    next_id: u16,
    /// The client's last packet taken, so a resend is not applied twice.
    last_from_client: u16,
    sim: Arc<Mutex<Sim>>,
}

impl Switcher {
    async fn send(&mut self, payload: &[u8]) {
        let p = packet(0x01, SESSION, 0, self.next_id, payload);
        self.next_id += 1;
        if let Some(to) = self.client {
            self.socket.send_to(&p, to).await.unwrap();
        }
    }

    async fn dump(&mut self, from: SocketAddr) {
        let mut pin = vec![0u8; 44];
        pin[..9].copy_from_slice(b"ATEM Mini");
        let older = self.sim.lock().unwrap().older;
        pin[40] = if older { 9 } else { 13 };
        let mut top = vec![0u8; 24];
        top[0] = 1;
        top[1] = 2;
        top[2] = 1;
        top[3] = 1;
        let (stills, macros): (Vec<u16>, Vec<u16>) = {
            let sim = self.sim.lock().unwrap();
            (
                sim.stills.keys().copied().collect(),
                sim.macros.keys().copied().collect(),
            )
        };
        let mut media: Vec<Vec<u8>> = vec![
            cmd(b"VidM", &[0, 0, 0, 0]),
            cmd(b"_mpl", &[20, 0, 0, 0]),
            cmd(b"_MAC", &[10, 0, 0, 0]),
            cmd(b"_VMC", &{
                let mut vmc = vec![0, 1, 0, 0, 12, 0, 0, 0];
                vmc.extend_from_slice(&((1u32 << 12) | (1 << 4)).to_be_bytes());
                vmc.extend_from_slice(&[0; 8]);
                vmc
            }),
        ];
        for slot in stills {
            let mut mpfe = vec![0u8; 24];
            mpfe[2..4].copy_from_slice(&slot.to_be_bytes());
            mpfe[4] = 1;
            mpfe[23] = 4;
            mpfe.extend_from_slice(b"pict");
            media.push(cmd(b"MPfe", &mpfe));
        }
        for slot in macros {
            let mut mprp = vec![0u8; 8];
            mprp[0..2].copy_from_slice(&slot.to_be_bytes());
            mprp[2] = 1;
            mprp[5] = 2;
            mprp.extend_from_slice(b"Go");
            media.push(cmd(b"MPrp", &mprp));
        }
        let packets = [
            [
                cmd(b"_ver", &[0, 2, 0, 30]),
                cmd(b"_pin", &pin),
                cmd(b"_top", &top),
            ]
            .concat(),
            [
                cmd(b"_MeC", &[0, 1, 0, 0]),
                input(1, "Camera 1", "CAM1"),
                input(2, "Camera 2", "CAM2"),
            ]
            .concat(),
            [
                cmd(b"PrgI", &[0, 0, 0, 1]),
                cmd(b"PrvI", &[0, 0, 0, 2, 0, 0, 0, 0]),
                tally(1, 2),
                media.concat(),
            ]
            .concat(),
            cmd(b"InCm", &[1, 0, 0, 0]),
        ];
        for (i, payload) in packets.iter().enumerate() {
            let p = packet(0x01, SESSION, 0, self.next_id, payload);
            self.next_id += 1;
            // Lose the second packet's first copy: the client must
            // drop what follows the gap until the resend arrives.
            if i == 1 {
                continue;
            }
            self.socket.send_to(&p, from).await.unwrap();
        }
        let resend = packet(0x01 | 0x04, SESSION, 0, 2, &packets[1]);
        self.socket.send_to(&resend, from).await.unwrap();
        for (i, payload) in packets.iter().enumerate().skip(2) {
            let p = packet(0x01 | 0x04, SESSION, 0, i as u16 + 1, payload);
            self.socket.send_to(&p, from).await.unwrap();
        }
    }

    async fn grant(&mut self) {
        self.sim.lock().unwrap().holder = Some(Holder::Client);
        self.send(&[cmd(b"LKOB", &[0, 0, 0, 0]), cmd(b"LKST", &[0, 0, 1, 0])].concat())
            .await;
    }

    /// One command from the client.
    async fn command(&mut self, name: &[u8], b: &[u8]) {
        match name {
            b"CPgI" => {
                let source = u16_at(b, 2);
                let payload = [cmd(b"PrgI", &[0, 0, 0, source as u8]), tally(source, 2)].concat();
                self.send(&payload).await;
            }
            b"LOCK" => {
                let (store, on) = (u16_at(b, 0), b[2] == 1);
                assert_eq!(store, 0, "only the stills are locked in these tests");
                let holder = self.sim.lock().unwrap().holder;
                match (on, holder) {
                    (true, None) => self.grant().await,
                    (true, Some(Holder::Other)) => self.sim.lock().unwrap().queued = true,
                    (true, Some(Holder::Client)) => {}
                    (false, Some(Holder::Client)) => {
                        self.sim.lock().unwrap().holder = None;
                        self.send(&cmd(b"LKST", &[0, 0, 0, 0])).await;
                    }
                    (false, _) => {}
                }
            }
            b"FTSD" => {
                let (id, store, index) = (u16_at(b, 0), u16_at(b, 2), u16_at(b, 6));
                let locked = self.sim.lock().unwrap().holder == Some(Holder::Client);
                if store != 0xFFFF && !locked {
                    self.send(&cmd(b"FTDE", &[b[0], b[1], 5, 0])).await;
                    return;
                }
                self.sim.lock().unwrap().upload = Some(Upload {
                    id,
                    store,
                    index,
                    size: u32_at(b, 8) as usize,
                    rle: store != 0xFFFF,
                    label: u16_at(b, 12) == 0x0201,
                    data: Vec::new(),
                    budget: 64,
                });
                if !self.sim.lock().unwrap().stall {
                    self.budget(id).await;
                }
            }
            b"FTFD" => {
                let name: Vec<u8> = b[2..66].iter().copied().take_while(|c| *c != 0).collect();
                let mut sim = self.sim.lock().unwrap();
                if let Some(u) = &sim.upload {
                    let index = u.index;
                    sim.names
                        .insert(index, String::from_utf8_lossy(&name).into_owned());
                }
            }
            b"FTDa" => {
                let chunk = b[4..4 + u16_at(b, 2) as usize].to_vec();
                let done = {
                    let mut sim = self.sim.lock().unwrap();
                    let drop_after = sim.drop_after;
                    let Some(u) = sim.upload.as_mut() else {
                        return;
                    };
                    u.data.extend_from_slice(&chunk);
                    u.budget -= 1;
                    let got = if u.rle {
                        expanded(&u.data)
                    } else {
                        u.data.len()
                    };
                    let chunks = u.data.len().div_ceil(CHUNK);
                    if drop_after.is_some_and(|n| chunks >= n) {
                        sim.silent = true;
                        return;
                    }
                    if got >= u.size {
                        let u = sim.upload.take().unwrap();
                        let still = (u.store == 0).then_some(u.index);
                        if u.store == 0xFFFF {
                            if u.label {
                                sim.labels.insert(u.index, u.data);
                            } else {
                                sim.macros.insert(u.index, u.data);
                            }
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
                        // The pool announces the slot in use.
                        if let Some(slot) = still {
                            let mut mpfe = vec![0u8; 24];
                            mpfe[2..4].copy_from_slice(&slot.to_be_bytes());
                            mpfe[4] = 1;
                            mpfe[23] = 4;
                            mpfe.extend_from_slice(b"pict");
                            self.send(&cmd(b"MPfe", &mpfe)).await;
                        }
                    }
                    Some(Err(id)) => self.budget(id).await,
                    None => {}
                }
            }
            b"FTSU" => {
                let (id, store, index) = (u16_at(b, 0), u16_at(b, 2), u16_at(b, 6));
                let (locked, data) = {
                    let sim = self.sim.lock().unwrap();
                    let data = if store == 0xFFFF {
                        sim.macros.get(&index).cloned()
                    } else {
                        sim.stills.get(&index).cloned()
                    };
                    (sim.holder == Some(Holder::Client), data)
                };
                let idb = [(id >> 8) as u8, id as u8];
                if store != 0xFFFF && !locked {
                    self.send(&cmd(b"FTDE", &[idb[0], idb[1], 5, 0])).await;
                    return;
                }
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
            b"FTUA" => self.sim.lock().unwrap().acks += 1,
            b"CSTL" => {
                // Like the models bmdwire describes, a clear is taken only
                // from the client holding the stills lock; a bare one is
                // ignored.
                let slot = b[0] as u16;
                let taken = {
                    let mut sim = self.sim.lock().unwrap();
                    let locked = sim.holder == Some(Holder::Client);
                    if !locked {
                        sim.bare_clears += 1;
                    }
                    locked && sim.stills.remove(&slot).is_some()
                };
                if taken {
                    let mut mpfe = vec![0u8; 24];
                    mpfe[2..4].copy_from_slice(&slot.to_be_bytes());
                    self.send(&cmd(b"MPfe", &mpfe)).await;
                }
            }
            _ => {}
        }
    }

    async fn budget(&mut self, id: u16) {
        let mut b = vec![0u8; 12];
        b[0..2].copy_from_slice(&id.to_be_bytes());
        b[6..8].copy_from_slice(&1396u16.to_be_bytes());
        b[8..10].copy_from_slice(&64u16.to_be_bytes());
        self.send(&cmd(b"FTCD", &b)).await;
    }

    async fn run(mut self) {
        let mut buf = [0u8; 2048];
        loop {
            let wait = {
                let sim = self.sim.lock().unwrap();
                sim.other_until
                    .map(|t| t.saturating_duration_since(Instant::now()))
            };
            let received = match wait {
                Some(w) => tokio::time::timeout(w, self.socket.recv_from(&mut buf))
                    .await
                    .ok(),
                None => Some(self.socket.recv_from(&mut buf).await),
            };
            let Some(received) = received else {
                // The other client lets go; a queued request is granted with
                // LKOB alone.
                let queued = {
                    let mut sim = self.sim.lock().unwrap();
                    sim.other_until = None;
                    sim.holder = None;
                    std::mem::take(&mut sim.queued)
                };
                if queued {
                    self.sim.lock().unwrap().holder = Some(Holder::Client);
                    self.send(&cmd(b"LKOB", &[0, 0, 0, 0])).await;
                } else {
                    self.send(&cmd(b"LKST", &[0, 0, 0, 0])).await;
                }
                continue;
            };
            let Ok((n, from)) = received else {
                return;
            };
            let data = buf[..n].to_vec();
            let flags = data[0] >> 3;
            if flags & 0x02 != 0 && data.get(12) == Some(&0x04) {
                // Goodbye: the session ends and its locks are freed at once.
                let mut sim = self.sim.lock().unwrap();
                sim.goodbyes += 1;
                sim.upload = None;
                if sim.holder == Some(Holder::Client) {
                    sim.holder = None;
                }
                self.client = None;
                continue;
            }
            if flags & 0x02 != 0 {
                // Hello: a new session. A silent (dead) session's lock is
                // freed, as the switcher reaps it.
                {
                    let mut sim = self.sim.lock().unwrap();
                    if sim.silent {
                        sim.silent = false;
                        sim.drop_after = None;
                        sim.upload = None;
                        if sim.holder == Some(Holder::Client) {
                            sim.holder = None;
                        }
                    }
                }
                self.client = Some(from);
                self.next_id = 1;
                self.last_from_client = 0;
                let reply = packet(0x02, 0x53ab, 0, 0, &[2, 0, 0, 0, 0, 0, 0, 0]);
                self.socket.send_to(&reply, from).await.unwrap();
                continue;
            }
            if self.sim.lock().unwrap().silent {
                continue;
            }
            if flags == 0x10 && self.next_id == 1 && self.client == Some(from) {
                // The handshake's acknowledgement: send the initial state.
                self.dump(from).await;
                continue;
            }
            if flags & 0x01 != 0 {
                // Commands, taken strictly in order: acknowledge the packet,
                // then apply each. A resend of one already taken is only
                // acknowledged again; one past a gap waits for the gap.
                let id = u16_at(&data, 10);
                if id != self.last_from_client.wrapping_add(1) {
                    if id <= self.last_from_client {
                        let ack = packet(0x10, SESSION, self.last_from_client, 0, &[]);
                        self.socket.send_to(&ack, from).await.unwrap();
                    }
                    continue;
                }
                self.last_from_client = id;
                self.socket
                    .send_to(&packet(0x10, SESSION, id, 0, &[]), from)
                    .await
                    .unwrap();
                let mut at = 12;
                while at + 8 <= data.len() {
                    let len = u16_at(&data, at) as usize;
                    let name = data[at + 4..at + 8].to_vec();
                    let body = data[at + 8..at + len].to_vec();
                    self.command(&name, &body).await;
                    if self.sim.lock().unwrap().silent {
                        break;
                    }
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
    let switcher = Switcher {
        socket,
        client: None,
        next_id: 1,
        last_from_client: 0,
        sim,
    };
    tokio::spawn(switcher.run());
    port
}

async fn wait_for(core: &Core, mut pred: impl FnMut(&Event) -> bool) {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if core.next_events(64).await.iter().any(&mut pred) {
                return;
            }
        }
    })
    .await
    .expect("event within 20 s")
}

/// The transfer's finishing message event's arguments.
async fn finished(core: &Core, transfer: u64) -> Value {
    let mut args = Value::Null;
    wait_for(core, |e| match e {
        Event::Message {
            address, args: a, ..
        } if address == "transfer_finished" && a["transfer"] == transfer => {
            args = a.clone();
            true
        }
        _ => false,
    })
    .await;
    args
}

/// Wait for the client's release of the stills lock to reach the switcher:
/// it is sent as the transfer finishes.
async fn released(sim: &Arc<Mutex<Sim>>) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while sim.lock().unwrap().holder.is_some() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the stills lock is released");
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

async fn open(core: &Core, port: u16) -> u64 {
    let id = core
        .open(OpenRequest {
            device: "blackmagic-atem".into(),
            model: "atem-mini".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: Default::default(),
            monitor: true,
        })
        .unwrap();
    wait_for(core, |e| {
        matches!(e, Event::Connection { device, connection: Connection::Connected } if *device == id)
    })
    .await;
    id
}

/// Start a transfer; while the device is reconnecting, try again.
async fn start(core: &Core, id: u64, name: &str, p: Value) -> u64 {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        match core.execute(id, name, params(p.clone())).await {
            Ok(Outcome::Value { value }) => return value["transfer"].as_u64().unwrap(),
            Err(CommandError::NotConnected) if Instant::now() < until => {
                tokio::time::sleep(Duration::from_millis(20)).await
            }
            other => panic!("{name}: {other:?}"),
        }
    }
}

/// A scratch directory for the host files, removed by the test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("meros-atem-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
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
async fn atem_end_to_end() {
    let port = simulated_atem(Arc::default()).await;
    let core = common::core().unwrap();
    let id = open(&core, port).await;
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["device"]["product"], "ATEM Mini");
    assert_eq!(state["sources"]["2"]["long_name"], "Camera 2");
    assert_eq!(state["mes"]["1"]["program"], 1);
    assert_eq!(state["tally"]["1"]["program"], true);

    let outcome = core
        .execute(id, "set_program", params(json!({"me": 1, "source": 2})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    wait_for(
        &core,
        |e| matches!(e, Event::State { patch, .. } if patch["mes"]["1"]["program"] == 2),
    )
    .await;
    assert_eq!(
        core.snapshot(id).unwrap().state["tally"]["2"]["program"],
        true
    );

    // A setting command passes the spec's checks and is acknowledged.
    let outcome = core
        .execute(id, "set_mix_rate", params(json!({"me": 1, "rate": 30})))
        .await;
    assert_eq!(outcome, Ok(Outcome::Ack));
    // The spec's range is checked before anything is sent.
    let outcome = core
        .execute(id, "set_mix_rate", params(json!({"me": 1, "rate": 0})))
        .await;
    assert!(matches!(outcome, Err(CommandError::InvalidParams { .. })));
    // The ATEM Mini model lists Fairlight commands, but this switcher reported
    // no Fairlight mixer, so the module refuses it.
    let outcome = core
        .execute(
            id,
            "set_fairlight_master",
            params(json!({"fader_gain": 0.0})),
        )
        .await;
    assert!(matches!(
        outcome,
        Err(CommandError::UnsupportedForModel { .. })
    ));
    // Commands from the keyer and settings areas pass the spec's checks and
    // are acknowledged.
    for (name, p) in [
        (
            "set_usk_mask",
            json!({"me": 1, "keyer": 1, "enabled": true, "top": 4.5}),
        ),
        ("set_dsk_mask", json!({"dsk": 1, "left": -8.0})),
        ("set_video_mode", json!({"mode": "1080p50"})),
        ("save_startup_state", json!({})),
        ("macro_add_pause", json!({"frames": 25})),
    ] {
        assert_eq!(
            core.execute(id, name, params(p)).await,
            Ok(Outcome::Ack),
            "{name}"
        );
    }
    // A mask edge outside the frame is refused before anything is sent.
    let outcome = core
        .execute(
            id,
            "set_usk_mask",
            params(json!({"me": 1, "keyer": 1, "top": 12.0})),
        )
        .await;
    assert!(matches!(outcome, Err(CommandError::InvalidParams { .. })));
    // Classic audio is not listed for the ATEM Mini at all.
    let outcome = core
        .execute(id, "set_audio_master", params(json!({"gain": 0.0})))
        .await;
    assert!(matches!(
        outcome,
        Err(CommandError::UnsupportedForModel { .. })
    ));

    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn atem_stills_and_macros_go_up_and_come_back() {
    let sim: Arc<Mutex<Sim>> = Arc::default();
    sim.lock()
        .unwrap()
        .macros
        .insert(0, b"\x00\x0c\x00\x00macro bytes!".to_vec());
    let port = simulated_atem(sim.clone()).await;
    let core = common::core().unwrap();
    let id = open(&core, port).await;
    let dir = scratch("roundtrip");

    let rgba = picture();
    let source = dir.join("in.rgba");
    std::fs::write(&source, &rgba).unwrap();
    let t = start(
        &core,
        id,
        "upload_still",
        json!({"still": 3, "path": source.to_str().unwrap(), "format": "rgba", "name": "Lower third"}),
    )
    .await;
    let done = finished(&core, t).await;
    assert_eq!(done["status"], "done", "{done}");
    assert_eq!(done["still"], 3);
    released(&sim).await;
    {
        let sim = sim.lock().unwrap();
        assert!(sim.stills.contains_key(&2));
        assert_eq!(sim.names[&2], "Lower third");
    }

    // The switcher reports the slot in use; downloading it brings back the
    // same picture within a level or two.
    let back = dir.join("out.rgba");
    let t = start(
        &core,
        id,
        "download_still",
        json!({"still": 3, "path": back.to_str().unwrap(), "format": "rgba"}),
    )
    .await;
    let done = finished(&core, t).await;
    assert_eq!(done["status"], "done", "{done}");
    let got = std::fs::read(&back).unwrap();
    assert_eq!(got.len(), rgba.len());
    assert!(got
        .iter()
        .zip(&rgba)
        .all(|(a, b)| (*a as i16 - *b as i16).abs() <= 2));
    assert!(sim.lock().unwrap().acks > 0, "every chunk acknowledged");
    released(&sim).await;
    let state = core.snapshot(id).unwrap().state;
    assert_eq!(state["transfers"][t.to_string()]["status"], "done");
    assert_eq!(state["media_pool"]["locks"]["stills"], false);

    // A macro comes down and goes up again unchanged, with no lock.
    let file = dir.join("m.bin");
    let t = start(
        &core,
        id,
        "download_macro",
        json!({"macro": 1, "path": file.to_str().unwrap()}),
    )
    .await;
    assert_eq!(finished(&core, t).await["status"], "done");
    assert_eq!(
        std::fs::read(&file).unwrap(),
        b"\x00\x0c\x00\x00macro bytes!"
    );
    let t = start(
        &core,
        id,
        "upload_macro",
        json!({"macro": 5, "path": file.to_str().unwrap(), "name": "Copy"}),
    )
    .await;
    assert_eq!(finished(&core, t).await["status"], "done");
    assert_eq!(
        sim.lock().unwrap().macros[&4],
        b"\x00\x0c\x00\x00macro bytes!"
    );

    // A missing file fails the transfer, not the command.
    let t = start(
        &core,
        id,
        "upload_still",
        json!({"still": 4, "path": dir.join("none.png").to_str().unwrap()}),
    )
    .await;
    let failed = finished(&core, t).await;
    assert_eq!(failed["status"], "failed");

    core.close(id).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn atem_transfer_waits_while_another_client_holds_the_stills() {
    let sim: Arc<Mutex<Sim>> = Arc::default();
    sim.lock().unwrap().stills.insert(0, {
        let mut d = RLE_MARKER.to_vec();
        d.extend_from_slice(&((640 * 480 / 2) as u64).to_be_bytes());
        d.extend_from_slice(&[0x04, 0x00, 0x80, 0x40, 0x04, 0x00, 0x80, 0x40]);
        d
    });
    let port = simulated_atem(sim.clone()).await;
    let core = common::core().unwrap();
    let id = open(&core, port).await;
    let dir = scratch("contention");
    {
        let mut s = sim.lock().unwrap();
        s.holder = Some(Holder::Other);
        s.other_until = Some(Instant::now() + Duration::from_millis(1500));
    }
    let asked = Instant::now();
    let file = dir.join("s.png");
    let t = start(
        &core,
        id,
        "download_still",
        json!({"still": 1, "path": file.to_str().unwrap()}),
    )
    .await;
    let done = finished(&core, t).await;
    assert_eq!(done["status"], "done", "{done}");
    assert!(
        asked.elapsed() >= Duration::from_millis(1400),
        "it waited its turn"
    );
    assert!(std::fs::read(&file).unwrap().starts_with(b"\x89PNG"));
    released(&sim).await;

    // Canceling a transfer stuck behind another client lets go of the
    // request; the grant that still comes is released at once.
    {
        let mut s = sim.lock().unwrap();
        s.holder = Some(Holder::Other);
        s.other_until = Some(Instant::now() + Duration::from_millis(800));
    }
    let t = start(
        &core,
        id,
        "download_still",
        json!({"still": 1, "path": file.to_str().unwrap()}),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let canceled = core
        .execute(id, "cancel_transfer", params(json!({"transfer": t})))
        .await;
    assert_eq!(
        canceled,
        Ok(Outcome::Value {
            value: json!({"canceled": [t]})
        })
    );
    tokio::time::sleep(Duration::from_millis(1200)).await;
    released(&sim).await;

    core.close(id).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn atem_clear_still_takes_the_stills_lock_and_waits_its_turn() {
    let sim: Arc<Mutex<Sim>> = Arc::default();
    sim.lock().unwrap().stills.insert(3, vec![0; 8]);
    let port = simulated_atem(sim.clone()).await;
    let core = common::core().unwrap();
    let id = open(&core, port).await;
    assert_eq!(
        core.snapshot(id).unwrap().state["media_pool"]["stills"]["4"]["name"],
        "pict"
    );
    // Another client holds the stills: the clear waits for the lock rather
    // than going out bare, which this switcher would ignore.
    {
        let mut s = sim.lock().unwrap();
        s.holder = Some(Holder::Other);
        s.other_until = Some(Instant::now() + Duration::from_millis(800));
    }
    let asked = Instant::now();
    let t = start(&core, id, "clear_still", json!({"still": 4})).await;
    let done = finished(&core, t).await;
    assert_eq!(
        (&done["status"], &done["kind"], &done["still"]),
        (&json!("done"), &json!("clear_still"), &json!(4)),
        "{done}"
    );
    assert!(asked.elapsed() >= Duration::from_millis(700), "it waited");
    released(&sim).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !core.snapshot(id).unwrap().state["media_pool"]["stills"]["4"].is_null() {
            core.poll_events(256);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the pool reports the slot empty");
    let s = sim.lock().unwrap();
    assert!(!s.stills.contains_key(&3));
    assert_eq!(s.bare_clears, 0);
    drop(s);
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn atem_upload_cut_off_by_a_lost_connection_fails_and_frees_the_lock() {
    let sim: Arc<Mutex<Sim>> = Arc::default();
    sim.lock().unwrap().drop_after = Some(5);
    let port = simulated_atem(sim.clone()).await;
    let core = common::core().unwrap();
    let id = open(&core, port).await;
    let dir = scratch("lost");
    // A picture with no runs, so it takes many chunks.
    let rgba: Vec<u8> = (0..640 * 480 * 4).map(|i| (i * 37 % 251) as u8).collect();
    let source = dir.join("noise.rgba");
    std::fs::write(&source, &rgba).unwrap();
    let t = start(
        &core,
        id,
        "upload_still",
        json!({"still": 2, "path": source.to_str().unwrap(), "format": "rgba"}),
    )
    .await;
    let failed = finished(&core, t).await;
    assert_eq!(failed["status"], "failed");
    assert!(
        failed["error"]
            .as_str()
            .unwrap()
            .starts_with("connection lost"),
        "{failed}"
    );

    // Connected again (start waits for it): the dead session's lock is gone
    // and transfers work.
    let t = start(
        &core,
        id,
        "upload_still",
        json!({"still": 2, "path": source.to_str().unwrap(), "format": "rgba"}),
    )
    .await;
    let done = finished(&core, t).await;
    assert_eq!(done["status"], "done", "{done}");
    released(&sim).await;

    core.close(id).await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn atem_closing_says_goodbye_and_frees_the_lock_at_once() {
    let sim: Arc<Mutex<Sim>> = Arc::default();
    sim.lock().unwrap().stall = true;
    let port = simulated_atem(sim.clone()).await;
    let core = common::core().unwrap();
    let id = open(&core, port).await;
    let dir = scratch("goodbye");
    let source = dir.join("flat.rgba");
    std::fs::write(&source, picture()).unwrap();
    start(
        &core,
        id,
        "upload_still",
        json!({"still": 1, "path": source.to_str().unwrap(), "format": "rgba"}),
    )
    .await;
    // The upload holds the stills lock, waiting for a budget that never
    // comes.
    tokio::time::timeout(Duration::from_secs(10), async {
        while sim.lock().unwrap().upload.is_none() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the upload starts");
    assert_eq!(sim.lock().unwrap().holder, Some(Holder::Client));
    core.close(id).await;
    released(&sim).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while sim.lock().unwrap().goodbyes == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the session says goodbye");
    let goodbyes = sim.lock().unwrap().goodbyes;
    assert_eq!(goodbyes, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn atem_label_drawn_from_text_reaches_an_older_switcher() {
    let sim: Arc<Mutex<Sim>> = Arc::default();
    sim.lock().unwrap().older = true;
    let port = simulated_atem(sim.clone()).await;
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "blackmagic-atem".into(),
            model: "atem-television-studio-pro-hd".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: Default::default(),
            monitor: true,
        })
        .unwrap();
    let t = start(
        &core,
        id,
        "set_multiviewer_label",
        json!({"source": 2, "text": "Camera 2"}),
    )
    .await;
    let done = finished(&core, t).await;
    assert_eq!(done["status"], "done", "{done}");
    assert_eq!(done["kind"], "set_multiviewer_label");
    let label = sim.lock().unwrap().labels.get(&2).cloned().unwrap();
    // A 1080 and a 720 label stacked, 320 wide: 50 and 40 rows.
    assert_eq!(label.len(), 320 * 90);
    assert!(label.iter().all(|b| *b < 254));
    assert!(label.iter().any(|b| *b != 0));
    core.close(id).await;
}
