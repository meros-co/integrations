//! What a host that is suspended, or whose network changes, sees: a device
//! that goes silent without closing its connection (a mobile app's sockets
//! while it is in the background), and the host asking for every device to
//! reconnect when it resumes. On real loopback sockets, through the public
//! API.

#![cfg(all(feature = "vmix", feature = "tsl-umd-listener"))]

mod common;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meros_integrations::{Connection, Core, Event, OpenRequest};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// Program is input 1 on the first connection and input 2 on every later one,
/// so a test can tell a fresh connection's state from the old one's.
fn xml(program: u8) -> String {
    let preview = 3 - program;
    format!(
        "<vmix><version>27.0.0.49</version><inputs>\
         <input key=\"a\" number=\"1\" type=\"Capture\" title=\"Cam 1\" state=\"Running\">Cam 1</input>\
         <input key=\"b\" number=\"2\" type=\"Capture\" title=\"Cam 2\" state=\"Running\">Cam 2</input>\
         </inputs><preview>{preview}</preview><active>{program}</active></vmix>"
    )
}

struct Simulated {
    port: u16,
    /// Connections accepted so far.
    connections: Arc<AtomicUsize>,
    /// While set, the simulator reads what it is sent and answers nothing,
    /// keeping the connection open: a peer that is suspended, or gone behind
    /// a network change, rather than closed.
    paused: Arc<AtomicBool>,
}

async fn simulated_vmix() -> Simulated {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let connections = Arc::new(AtomicUsize::new(0));
    let paused = Arc::new(AtomicBool::new(false));
    let (count, pause) = (connections.clone(), paused.clone());
    tokio::spawn(async move {
        loop {
            let Ok((tcp, _)) = listener.accept().await else {
                return;
            };
            let n = count.fetch_add(1, Ordering::SeqCst) + 1;
            let pause = pause.clone();
            tokio::spawn(async move {
                let (program, tally) = if n == 1 { (1, "12") } else { (2, "21") };
                let (read, mut write) = tcp.into_split();
                let mut lines = BufReader::new(read).lines();
                if write.write_all(b"VERSION OK 27.0.0.49\r\n").await.is_err() {
                    return;
                }
                while let Ok(Some(line)) = lines.next_line().await {
                    if pause.load(Ordering::SeqCst) {
                        continue;
                    }
                    let reply = if line == "SUBSCRIBE TALLY" {
                        format!("SUBSCRIBE OK TALLY\r\nTALLY OK {tally}\r\n")
                    } else if let Some(what) = line.strip_prefix("SUBSCRIBE ") {
                        format!("SUBSCRIBE OK {what}\r\n")
                    } else if line == "XML" {
                        let x = xml(program);
                        format!("XML {}\r\n{x}\r\n", x.len() + 2)
                    } else if let Some(query) = line.strip_prefix("ACTS ") {
                        format!("ACTS OK {query} 0\r\n")
                    } else {
                        continue;
                    };
                    if write.write_all(reply.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    Simulated {
        port,
        connections,
        paused,
    }
}

async fn wait_for(core: &Core, within: Duration, mut pred: impl FnMut(&Event) -> bool) {
    tokio::time::timeout(within, async {
        loop {
            if core.next_events(64).await.iter().any(&mut pred) {
                return;
            }
        }
    })
    .await
    .expect("event in time")
}

fn open_vmix(core: &Core, port: u16) -> u64 {
    core.open(OpenRequest {
        device: "vmix".into(),
        model: "vmix".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: json!({"state_poll_ms": 250}).as_object().unwrap().clone(),
        monitor: true,
    })
    .unwrap()
}

fn program_tally(state: &Value, input: &str) -> bool {
    state["tally"][input]["program"] == true
}

/// A device that stops answering without closing is found out by the
/// module's own liveness check, and the core reconnects by itself once the
/// device answers again, with the tally of the new connection.
#[tokio::test(flavor = "multi_thread")]
async fn a_silent_connection_is_dropped_and_reopened() {
    let sim = simulated_vmix().await;
    let core = common::core().unwrap();
    let id = open_vmix(&core, sim.port);
    wait_for(&core, Duration::from_secs(10), |e| {
        matches!(e, Event::State { device, .. } if *device == id)
            && program_tally(&core.snapshot(id).unwrap().state, "1")
    })
    .await;

    // Suspended: the connection stays open and nothing answers.
    sim.paused.store(true, Ordering::SeqCst);
    wait_for(&core, Duration::from_secs(15), |e| {
        matches!(e, Event::Connection { device, connection: Connection::Disconnected { .. } }
            if *device == id)
    })
    .await;

    // Resumed: the next attempt is a new connection, with the new tally.
    sim.paused.store(false, Ordering::SeqCst);
    wait_for(&core, Duration::from_secs(15), |_| {
        program_tally(&core.snapshot(id).unwrap().state, "2")
    })
    .await;
    assert!(sim.connections.load(Ordering::SeqCst) >= 2);
    assert_eq!(core.snapshot(id).unwrap().connection, Connection::Connected);
    core.close(id).await;
}

/// A host that resumes asks every device to reconnect: the core opens a new
/// connection at once, without waiting for the old one to time out, and the
/// device keeps its id.
#[tokio::test(flavor = "multi_thread")]
async fn reconnect_replaces_a_stale_connection_at_once() {
    let sim = simulated_vmix().await;
    let core = common::core().unwrap();
    let id = open_vmix(&core, sim.port);
    wait_for(&core, Duration::from_secs(10), |_| {
        program_tally(&core.snapshot(id).unwrap().state, "1")
    })
    .await;

    // The old connection is stale: it would take the module's reply timeout
    // to notice. The new one answers.
    sim.paused.store(true, Ordering::SeqCst);
    let asked = Instant::now();
    assert!(core.reconnect(id).await);
    sim.paused.store(false, Ordering::SeqCst);
    wait_for(&core, Duration::from_secs(10), |_| {
        program_tally(&core.snapshot(id).unwrap().state, "2")
    })
    .await;
    assert!(
        asked.elapsed() < Duration::from_secs(4),
        "reconnected after {:?}",
        asked.elapsed()
    );
    assert_eq!(sim.connections.load(Ordering::SeqCst), 2);
    assert_eq!(core.snapshot(id).unwrap().connection, Connection::Connected);

    // Every device, and none that is closed.
    core.reconnect_all().await;
    wait_for(&core, Duration::from_secs(10), |_| {
        sim.connections.load(Ordering::SeqCst) == 3
            && core.snapshot(id).unwrap().connection == Connection::Connected
    })
    .await;
    core.close(id).await;
    assert!(!core.reconnect(id).await);
}

/// A listener asked to reconnect listens on its port again and goes on
/// hearing the switcher.
#[tokio::test(flavor = "multi_thread")]
async fn a_tsl_listener_hears_tally_after_reconnecting() {
    let port = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let core = common::core().unwrap();
    let id = core
        .open(OpenRequest {
            device: "tsl-umd-listener".into(),
            model: "tsl-umd".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: Default::default(),
            monitor: true,
        })
        .unwrap();
    let switcher = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();

    // TSL UMD V3.1: address 0x80 + 5, control (program tally 1), 16 bytes
    // of text.
    let packet = |text: &str| {
        let mut p = vec![0x80 + 5, 0b0000_0001];
        p.extend(format!("{text:<16}").bytes());
        p
    };
    let heard = |core: &Core, text: &str| {
        let state = core.snapshot(id).unwrap().state;
        state["screens"]
            .as_object()
            .into_iter()
            .flat_map(|s| s.values())
            .any(|s| s["displays"]["5"]["text"].as_str().map(str::trim) == Some(text))
    };
    let send_until_heard = |text: &'static str| {
        let core = &core;
        let switcher = &switcher;
        async move {
            tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    switcher
                        .send_to(&packet(text), ("127.0.0.1", port))
                        .await
                        .unwrap();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    if heard(core, text) {
                        return;
                    }
                }
            })
            .await
            .expect("the listener hears the switcher")
        }
    };

    send_until_heard("CAM 1").await;
    assert!(core.reconnect(id).await);
    send_until_heard("CAM 2").await;
    core.close(id).await;
}
