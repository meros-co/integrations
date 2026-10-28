//! DMXControl 3's Umbra simulated on loopback, driven through the public
//! API. The simulator speaks HTTP/2 (RFC 9113) and gRPC by hand, written
//! here from the RFC and gRPC's protocol document, sharing nothing with the
//! core: it answers the preface with its settings, PING with its ack, Login
//! with a session ID, ReportReadyToWork and CuelistAction with an ok
//! ConfirmedResponse, GetExecutors with one executor, and every other
//! method with UNIMPLEMENTED. Messages are protobuf stated byte by byte from
//! the .proto field numbers.

#![cfg(feature = "dmxcontrol3")]

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{
    Connection, Core, CoreOptions, DiscoverAction, DiscoverRequest, Event, OpenRequest, Outcome,
};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// One call the simulator received: path, its sessionid header, and the
/// first message's bytes.
#[derive(Debug, Clone)]
struct Call {
    path: String,
    session: Option<String>,
    message: Vec<u8>,
}

type Calls = Arc<Mutex<Vec<Call>>>;

fn frame(kind: u8, flags: u8, stream: u32, payload: &[u8]) -> Vec<u8> {
    let len = payload.len();
    let mut out = vec![(len >> 16) as u8, (len >> 8) as u8, len as u8, kind, flags];
    out.extend_from_slice(&stream.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

/// A string literal, not Huffman-coded (RFC 7541 §5.2), its length under 127.
fn literal(s: &str, out: &mut Vec<u8>) {
    out.push(s.len() as u8);
    out.extend_from_slice(s.as_bytes());
}

/// A header block of literals without indexing, new names (RFC 7541 §6.2.2).
fn block(fields: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (n, v) in fields {
        out.push(0x00);
        literal(n, &mut out);
        literal(v, &mut out);
    }
    out
}

/// RFC 7541 Appendix A's names for the indexes the client uses.
fn static_name(index: usize) -> &'static str {
    match index {
        1 => ":authority",
        2 | 3 => ":method",
        4 | 5 => ":path",
        6 | 7 => ":scheme",
        31 => "content-type",
        _ => "?",
    }
}

/// The header fields of a block the core's encoder wrote: literals without
/// indexing, with indexed or new names, values not Huffman-coded.
fn decode_block(mut b: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    while !b.is_empty() {
        let first = b[0];
        assert_eq!(first & 0xf0, 0, "a literal without indexing");
        let mut index = usize::from(first & 0x0f);
        b = &b[1..];
        if index == 15 {
            index += usize::from(b[0]);
            b = &b[1..];
        }
        let name = if index == 0 {
            let len = usize::from(b[0]);
            let n = String::from_utf8(b[1..1 + len].to_vec()).unwrap();
            b = &b[1 + len..];
            n
        } else {
            static_name(index).to_string()
        };
        let len = usize::from(b[0]);
        assert!(len < 127 && b[0] & 0x80 == 0);
        let value = String::from_utf8(b[1..1 + len].to_vec()).unwrap();
        b = &b[1 + len..];
        out.push((name, value));
    }
    out
}

fn grpc_message(m: &[u8]) -> Vec<u8> {
    let mut out = vec![0];
    out.extend_from_slice(&(m.len() as u32).to_be_bytes());
    out.extend_from_slice(m);
    out
}

/// The answer to a call, or None for UNIMPLEMENTED.
fn answer(path: &str) -> Option<Vec<u8>> {
    match path {
        // UmbraLoginResponse: sessionId (5) "S1"; returnCode (7) NoError (0).
        "/LumosProtobufCommon.ClientService/Login" => Some(vec![0x2a, 2, b'S', b'1']),
        // ConfirmedResponse: ok (1) true.
        "/LumosProtobufCommon.ConnectedClientService/ReportReadyToWork"
        | "/LumosProtobufClient.CuelistClient/CuelistAction" => Some(vec![0x08, 1]),
        // GetExecutorsResponse: executors (3) [ExecutorDescriptor: id (2)
        // "e1", name (3) "Front"].
        "/LumosProtobufClient.ExecutorClient/GetExecutors" => {
            let executor = [0x12, 2, b'e', b'1', 0x1a, 5, b'F', b'r', b'o', b'n', b't'];
            let mut out = vec![0x1a, executor.len() as u8];
            out.extend_from_slice(&executor);
            Some(out)
        }
        _ => None,
    }
}

async fn serve(calls: Calls) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let calls = calls.clone();
            tokio::spawn(async move {
                let mut data = Vec::new();
                let mut buf = vec![0u8; 65_536];
                let mut preface = false;
                let mut headers: HashMap<u32, Vec<(String, String)>> = HashMap::new();
                let mut answered: std::collections::HashSet<u32> = Default::default();
                // The server's settings, empty: the defaults.
                socket.write_all(&frame(4, 0, 0, &[])).await.unwrap();
                loop {
                    let n = match socket.read(&mut buf).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => n,
                    };
                    data.extend_from_slice(&buf[..n]);
                    if !preface {
                        if data.len() < 24 {
                            continue;
                        }
                        assert_eq!(&data[..24], b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
                        data.drain(..24);
                        preface = true;
                    }
                    let mut out = Vec::new();
                    while data.len() >= 9 {
                        let len = (usize::from(data[0]) << 16)
                            | (usize::from(data[1]) << 8)
                            | usize::from(data[2]);
                        if data.len() < 9 + len {
                            break;
                        }
                        let (kind, flags) = (data[3], data[4]);
                        let stream =
                            u32::from_be_bytes([data[5], data[6], data[7], data[8]]) & 0x7fff_ffff;
                        let payload = data[9..9 + len].to_vec();
                        data.drain(..9 + len);
                        match kind {
                            // SETTINGS: ack the client's.
                            4 if flags & 1 == 0 => out.extend(frame(4, 1, 0, &[])),
                            // PING: ack it.
                            6 if flags & 1 == 0 => out.extend(frame(6, 1, 0, &payload)),
                            // HEADERS: a call starts (END_HEADERS always set by the core here).
                            1 => {
                                headers.insert(stream, decode_block(&payload));
                            }
                            // DATA: the first message is the request.
                            0 if !answered.contains(&stream) && payload.len() >= 5 => {
                                answered.insert(stream);
                                let fields = headers.get(&stream).cloned().unwrap_or_default();
                                let path = fields
                                    .iter()
                                    .find(|(k, _)| k == ":path")
                                    .map(|(_, v)| v.clone())
                                    .unwrap_or_default();
                                let session = fields
                                    .iter()
                                    .find(|(k, _)| k == "sessionid")
                                    .map(|(_, v)| v.clone());
                                let mlen = u32::from_be_bytes([
                                    payload[1], payload[2], payload[3], payload[4],
                                ]) as usize;
                                let message = payload[5..5 + mlen].to_vec();
                                calls.lock().unwrap().push(Call {
                                    path: path.clone(),
                                    session,
                                    message,
                                });
                                let ping =
                                    path.ends_with("/Ping") || path.ends_with("/SendBroadcast");
                                match answer(&path) {
                                    Some(m) => {
                                        out.extend(frame(
                                            1,
                                            4,
                                            stream,
                                            &block(&[
                                                (":status", "200"),
                                                ("content-type", "application/grpc"),
                                            ]),
                                        ));
                                        out.extend(frame(0, 0, stream, &grpc_message(&m)));
                                        out.extend(frame(
                                            1,
                                            5,
                                            stream,
                                            &block(&[("grpc-status", "0")]),
                                        ));
                                    }
                                    // The ping call stays open.
                                    None if ping => {
                                        out.extend(frame(
                                            1,
                                            4,
                                            stream,
                                            &block(&[
                                                (":status", "200"),
                                                ("content-type", "application/grpc"),
                                            ]),
                                        ));
                                    }
                                    None => out.extend(frame(
                                        1,
                                        5,
                                        stream,
                                        &block(&[
                                            (":status", "200"),
                                            ("content-type", "application/grpc"),
                                            ("grpc-status", "12"),
                                        ]),
                                    )),
                                }
                            }
                            _ => {}
                        }
                    }
                    if !out.is_empty() && socket.write_all(&out).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    port
}

async fn wait_for(what: &str, mut check: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(10), async {
        while !check() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what} within 10 s"));
}

fn open(core: &Core, port: u16, monitor: bool) -> u64 {
    core.open(OpenRequest {
        device: "dmxcontrol3".into(),
        model: "dmxcontrol3".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: json!({"network_id": "LIGHT-Net"})
            .as_object()
            .unwrap()
            .clone(),
        monitor,
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_session_logs_in_and_runs_a_command() {
    let calls: Calls = Arc::default();
    let port = serve(calls.clone()).await;
    let core = common::core_with(CoreOptions::new().devices(["dmxcontrol3"])).unwrap();
    let device = open(&core, port, false);
    wait_for("connected", || {
        matches!(
            core.snapshot(device).unwrap().connection,
            Connection::Connected
        )
    })
    .await;
    wait_for("ready and ping", || calls.lock().unwrap().len() >= 3).await;
    {
        let calls = calls.lock().unwrap();
        assert_eq!(calls[0].path, "/LumosProtobufCommon.ClientService/Login");
        assert_eq!(calls[0].session, None, "the login carries no session");
        assert_eq!(
            calls[1].path,
            "/LumosProtobufCommon.ConnectedClientService/ReportReadyToWork"
        );
        assert_eq!(calls[1].session.as_deref(), Some("S1"));
        assert_eq!(
            calls[2].path,
            "/LumosProtobufCommon.ConnectedClientService/Ping"
        );
    }
    let outcome = core
        .execute(
            device,
            "cuelist_action",
            json!({"cuelist_id": "c1", "action": "GO"})
                .as_object()
                .unwrap()
                .clone(),
        )
        .await
        .unwrap();
    assert_eq!(outcome, Outcome::Ack);
    let call = calls
        .lock()
        .unwrap()
        .iter()
        .find(|c| c.path.ends_with("/CuelistAction"))
        .cloned()
        .unwrap();
    // CuelistActionRequest: cuelistId (3) "c1", action (5) GO (7).
    assert_eq!(call.message, [0x1a, 2, b'c', b'1', 0x28, 7]);
    assert_eq!(call.session.as_deref(), Some("S1"));
    let paths: Vec<String> = calls
        .lock()
        .unwrap()
        .iter()
        .map(|c| c.path.clone())
        .collect();
    assert!(
        !paths.iter().any(|p| p.contains("/Receive")),
        "commands only: no stream: {paths:?}"
    );
    core.close(device).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_monitored_session_reads_its_lists_into_state() {
    let calls: Calls = Arc::default();
    let port = serve(calls.clone()).await;
    let core = common::core_with(CoreOptions::new().devices(["dmxcontrol3"])).unwrap();
    let device = open(&core, port, true);
    wait_for("the executors", || {
        core.snapshot(device)
            .unwrap()
            .state
            .pointer("/executors/e1/name")
            == Some(&Value::from("Front"))
    })
    .await;
    let paths: Vec<String> = calls
        .lock()
        .unwrap()
        .iter()
        .map(|c| c.path.clone())
        .collect();
    assert!(paths
        .iter()
        .any(|p| p == "/LumosProtobufClient.ExecutorClient/ReceiveExecutorChanges"));
    // A method this simulator does not offer is not called again.
    tokio::time::sleep(Duration::from_millis(1_500)).await;
    let again = calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.path == "/LumosProtobufClient.ExecutorClient/ReceiveExecutorChanges")
        .count();
    assert_eq!(again, 1);
    core.close(device).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_umbra_announcing_itself_is_discovered() {
    let core = common::core_with(CoreOptions::new().devices(["dmxcontrol3"])).unwrap();
    core.discover(DiscoverRequest {
        action: DiscoverAction::Listen,
        protocols: vec!["umbra".into()],
        hints: vec![],
    })
    .unwrap();
    // UmbraUdpBroadcast: umbraServer (2) { clientInfo (3) { clientname (4)
    // "U1", runtimeid (21) "r1", umbraPort (30) 17475 } }.
    let info = [
        0x22, 2, b'U', b'1', 0xaa, 0x01, 2, b'r', b'1', 0xf0, 0x01, 0xc3, 0x88, 0x01,
    ];
    let mut program = vec![0x1a, info.len() as u8];
    program.extend_from_slice(&info);
    let mut datagram = vec![0x12, program.len() as u8];
    datagram.extend(program);
    let umbra = tokio::net::UdpSocket::bind("127.0.0.2:0").await.unwrap();
    let found = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            umbra.send_to(&datagram, "127.0.0.1:17474").await.unwrap();
            for e in core.next_events(64).await {
                if let Event::Discovered {
                    address,
                    port,
                    device,
                    name,
                    ..
                } = e
                {
                    return (address, port, device, name);
                }
            }
        }
    })
    .await
    .expect("discovered within 10 s");
    assert_eq!(
        found,
        (
            "127.0.0.2".to_string(),
            17475,
            "dmxcontrol3".to_string(),
            Some("U1".to_string())
        )
    );
}
