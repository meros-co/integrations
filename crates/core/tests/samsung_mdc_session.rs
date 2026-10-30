//! A Samsung display simulated on real TCP (and in its secured TLS mode),
//! driven through the public API.
//!
//! The simulated display checks each request's frame and checksum as MDC
//! Ver. 15.0 §1.2.1 has them, answers only its own ID, and keeps power,
//! volume, mute and input: status (0x00), power (0x11), volume (0x12), mute
//! (0x13), input (0x14), the error status (0x0D) and the model name (0x8A).
//! Every other command is answered NAK. In secured mode it greets with
//! MDCSTART<<TLS>>, takes a TLS handshake on the same connection, then the
//! PIN, and answers MDCAUTH<<PASS>> or MDCAUTH<<FAIL:0x01>>.

#![cfg(feature = "samsung-mdc")]

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{json, value::Value};
use meros_integrations::{Connection, Core, Event, OpenRequest, Outcome};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpListener;

const ID: u8 = 3;

#[derive(Default)]
struct Display {
    power: u8,
    volume: u8,
    mute: u8,
    input: u8,
    /// Every request received: (command, ID, data).
    received: Vec<(u8, u8, Vec<u8>)>,
}

fn reply(id: u8, ack: bool, cmd: u8, values: &[u8]) -> Vec<u8> {
    let mut data = vec![if ack { b'A' } else { b'N' }, cmd];
    data.extend_from_slice(values);
    let mut f = vec![0xAA, 0xFF, id, data.len() as u8];
    f.extend_from_slice(&data);
    let sum = f[1..].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    f.push(sum);
    f
}

impl Display {
    fn answer(&mut self, cmd: u8, id: u8, data: &[u8]) -> Option<Vec<u8>> {
        self.received.push((cmd, id, data.to_vec()));
        if id != ID {
            return None;
        }
        let set = data.first().copied();
        let value = |field: &mut u8| {
            if let Some(v) = set {
                *field = v;
            }
            *field
        };
        Some(match cmd {
            0x00 => reply(
                id,
                true,
                0,
                &[self.power, self.volume, self.mute, self.input, 0x01, 0, 0],
            ),
            0x11 => reply(id, true, 0x11, &[value(&mut self.power)]),
            0x12 => reply(id, true, 0x12, &[value(&mut self.volume)]),
            0x13 => reply(id, true, 0x13, &[value(&mut self.mute)]),
            0x14 => reply(id, true, 0x14, &[value(&mut self.input)]),
            0x0D => reply(id, true, 0x0D, &[0, 0, 2, 0, 41, 0]),
            0x8A => reply(id, true, 0x8A, b"QM55B"),
            other => reply(id, false, other, &[0x01]),
        })
    }
}

async fn serve<S: AsyncRead + AsyncWrite + Unpin>(mut stream: S, display: Arc<Mutex<Display>>) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 512];
    loop {
        let n = stream.read(&mut chunk).await.unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        while buf.len() >= 4 && buf.len() >= 5 + buf[3] as usize {
            assert_eq!(buf[0], 0xAA, "header");
            let total = 5 + buf[3] as usize;
            let frame: Vec<u8> = buf.drain(..total).collect();
            let sum = frame[1..total - 1]
                .iter()
                .fold(0u8, |a, b| a.wrapping_add(*b));
            assert_eq!(sum, frame[total - 1], "checksum");
            let answer = display
                .lock()
                .unwrap()
                .answer(frame[1], frame[2], &frame[4..total - 1]);
            if let Some(answer) = answer {
                stream.write_all(&answer).await.unwrap();
                stream.flush().await.unwrap();
            }
        }
    }
}

fn tls_acceptor() -> tokio_rustls::TlsAcceptor {
    let rcgen::CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(vec!["display".into()]).unwrap();
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(key_pair.serialize_der().into());
    let config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![cert.der().clone()], key)
    .unwrap();
    tokio_rustls::TlsAcceptor::from(Arc::new(config))
}

/// A display on loopback; with a PIN, in secured mode.
async fn simulate(pin: Option<&'static str>) -> (u16, Arc<Mutex<Display>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let display = Arc::new(Mutex::new(Display {
        power: 1,
        volume: 20,
        input: 0x21,
        ..Default::default()
    }));
    let d = display.clone();
    tokio::spawn(async move {
        while let Ok((mut tcp, _)) = listener.accept().await {
            let d = d.clone();
            tokio::spawn(async move {
                let Some(pin) = pin else {
                    serve(tcp, d).await;
                    return;
                };
                tcp.write_all(b"MDCSTART<<TLS>>").await.unwrap();
                let Ok(mut tls) = tls_acceptor().accept(tcp).await else {
                    return;
                };
                let mut got = [0u8; 4];
                if tls.read_exact(&mut got).await.is_err() {
                    return;
                }
                if got == pin.as_bytes() {
                    tls.write_all(b"MDCAUTH<<PASS>>").await.unwrap();
                    tls.flush().await.unwrap();
                    serve(tls, d).await;
                } else {
                    tls.write_all(b"MDCAUTH<<FAIL:0x01>>").await.unwrap();
                    tls.flush().await.unwrap();
                }
            });
        }
    });
    (port, display)
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core, port: u16, monitor: bool, settings: Value) -> meros_integrations::DeviceId {
    core.open(OpenRequest {
        device: "samsung-mdc".into(),
        model: "lcd-signage".into(),
        host: "127.0.0.1".into(),
        port: Some(port),
        settings: params(settings),
        monitor,
    })
    .unwrap()
}

async fn wait_for_state(
    core: &Core,
    id: meros_integrations::DeviceId,
    pred: impl Fn(&Value) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if pred(&core.snapshot(id).unwrap().state) {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 20 s")
}

#[tokio::test(flavor = "multi_thread")]
async fn status_is_read_and_commands_are_read_back() {
    let (port, display) = simulate(None).await;
    let core = common::core().unwrap();
    let id = open(
        &core,
        port,
        true,
        json!({"display_id": ID, "poll_interval_s": 1}),
    );
    wait_for_state(&core, id, |s| {
        let d = &s["displays"]["3"];
        d["power"] == "on"
            && d["volume"] == 20
            && d["input"] == "hdmi1"
            && d["model_name"] == "QM55B"
            && d["display_status"]["temperature_c"] == 41
    })
    .await;
    assert_eq!(
        core.execute(id, "set_volume", params(json!({"volume": 35})))
            .await,
        Ok(Outcome::Ack)
    );
    assert_eq!(
        core.snapshot(id).unwrap().state["displays"]["3"]["volume"],
        json!(35)
    );
    assert_eq!(
        core.execute(id, "get_mute", params(json!({}))).await,
        Ok(Outcome::Value {
            value: json!("off")
        })
    );
    // A command the display lacks fails with its NAK.
    assert!(matches!(
        core.execute(id, "set_still", params(json!({"still": "on"})))
            .await,
        Err(meros_integrations::CommandError::DeviceError { .. })
    ));
    // A change made elsewhere is seen at the next poll.
    display.lock().unwrap().input = 0x23;
    wait_for_state(&core, id, |s| s["displays"]["3"]["input"] == "hdmi2").await;
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn opened_for_commands_only_only_power_is_read() {
    let (port, display) = simulate(None).await;
    let core = common::core().unwrap();
    let id = open(&core, port, false, json!({"display_id": ID}));
    assert_eq!(
        core.execute(id, "set_input", params(json!({"input": "displayport1"})))
            .await,
        Ok(Outcome::Ack)
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    let received: Vec<u8> = display
        .lock()
        .unwrap()
        .received
        .iter()
        .map(|r| r.0)
        .collect();
    assert!(
        received.iter().all(|c| *c == 0x11 || *c == 0x14),
        "{received:?}"
    );
    assert!(received.contains(&0x11), "{received:?}");
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_secured_mode_takes_the_pin_and_refuses_a_wrong_one() {
    let (port, _display) = simulate(Some("4321")).await;
    let core = common::core().unwrap();
    let id = open(&core, port, false, json!({"display_id": ID, "pin": "4321"}));
    assert_eq!(
        core.execute(id, "get_volume", params(json!({}))).await,
        Ok(Outcome::Value { value: json!(20) })
    );
    core.close(id).await;

    let id = open(&core, port, false, json!({"display_id": ID, "pin": "0000"}));
    let refused = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            for event in core.next_events(64).await {
                if let Event::Connection {
                    device,
                    connection: Connection::Unauthorized { .. },
                } = event
                {
                    if device == id {
                        return;
                    }
                }
            }
        }
    })
    .await;
    assert!(refused.is_ok(), "a wrong PIN is reported unauthorized");
    core.close(id).await;
}
