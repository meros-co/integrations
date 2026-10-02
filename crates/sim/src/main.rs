//! Simulated devices for the shared binding test.
//!
//! Starts each simulated device on a free local port, prints one JSON line
//! naming the ports, and runs until its standard input closes. Every binding's
//! test starts this, then plays `tests/bindings/script.json` through its own
//! delivery, so all of them are driven against identical devices.
//!
//! Devices:
//! - `kramer`: Kramer Protocol 3000 over TCP. Answers #ROUTE with OK, and with
//!   ERR for input 9, and #MODEL? with a model name.
//! - `d6000`: Sennheiser Digital 6000 SSC over UDP. Answers subscriptions with
//!   the channel tree and echoes mute writes.
//! - `snapshot`: a camera's HTTP/1.1 snapshot URL. GET /snapshot.jpg answers a
//!   small JPEG whose third byte counts requests; anything else is a 404.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, UdpSocket};

async fn kramer(listener: TcpListener) {
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        tokio::spawn(async move {
            let (read, mut write) = stream.into_split();
            let mut read = BufReader::new(read);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                match read.read_until(b'\r', &mut buf).await {
                    Ok(0) | Err(_) => return,
                    Ok(_) => {}
                }
                let line = String::from_utf8_lossy(&buf).trim().to_string();
                let reply = if let Some(args) = line.strip_prefix("#ROUTE ") {
                    let status = if args.ends_with(",9") {
                        "ERR 003"
                    } else {
                        "OK"
                    };
                    format!("~01@ROUTE {args} {status}\r\n")
                } else if line == "#MODEL?" {
                    "~01@MODEL VS-88UT\r\n".to_string()
                } else {
                    format!("~01@{line} ERR 002\r\n")
                };
                if write.write_all(reply.as_bytes()).await.is_err() {
                    return;
                }
            }
        });
    }
}

async fn d6000(socket: UdpSocket) {
    let mut buf = [0u8; 4096];
    let mut mute = [false, false];
    loop {
        let Ok((n, from)) = socket.recv_from(&mut buf).await else {
            return;
        };
        let Ok(msg) = serde_json::from_slice::<Value>(&buf[..n]) else {
            continue;
        };
        let mut replies = Vec::new();
        if msg.pointer("/osc/state/subscribe").is_some() {
            for ch in 1..=2 {
                replies.push(json!({ format!("rx{ch}"): {
                    "name": format!("Mic {ch}"),
                    "carrier": 606000 + ch * 400,
                    "audio_mute": mute[ch - 1],
                    "active_warnings": [],
                }}));
            }
        } else {
            for ch in 1..=2usize {
                if let Some(m) = msg
                    .pointer(&format!("/rx{ch}/audio_mute"))
                    .and_then(Value::as_bool)
                {
                    mute[ch - 1] = m;
                    replies.push(json!({ format!("rx{ch}"): {"audio_mute": m} }));
                }
            }
        }
        for reply in replies {
            let _ = socket.send_to(reply.to_string().as_bytes(), from).await;
        }
    }
}

async fn snapshot(listener: TcpListener) {
    let count = Arc::new(AtomicU8::new(0));
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let count = count.clone();
        tokio::spawn(async move {
            let (read, mut write) = stream.into_split();
            let mut read = BufReader::new(read);
            // Kept alive across requests.
            loop {
                let mut request_line = String::new();
                match read.read_line(&mut request_line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                loop {
                    let mut header = String::new();
                    match read.read_line(&mut header).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) if header == "\r\n" => break,
                        Ok(_) => {}
                    }
                }
                let reply = if request_line.starts_with("GET /snapshot.jpg ") {
                    let n = count.fetch_add(1, Ordering::Relaxed).wrapping_add(1);
                    let body = [0xFF, 0xD8, n, 0xFF, 0xD9];
                    let mut reply = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    reply.extend_from_slice(&body);
                    reply
                } else {
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec()
                };
                if write.write_all(&reply).await.is_err() {
                    break;
                }
            }
        });
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let kramer_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind kramer");
    let d6000_socket = UdpSocket::bind("127.0.0.1:0").await.expect("bind d6000");
    let snapshot_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind snapshot");
    let ports = json!({
        "kramer": kramer_listener.local_addr().unwrap().port(),
        "d6000": d6000_socket.local_addr().unwrap().port(),
        "snapshot": snapshot_listener.local_addr().unwrap().port(),
    });
    println!("{ports}");

    tokio::spawn(kramer(kramer_listener));
    tokio::spawn(d6000(d6000_socket));
    tokio::spawn(snapshot(snapshot_listener));

    // Run until the test that started us closes our stdin.
    let mut stdin = tokio::io::stdin();
    let mut sink = [0u8; 64];
    while let Ok(n) = stdin.read(&mut sink).await {
        if n == 0 {
            break;
        }
    }
}
