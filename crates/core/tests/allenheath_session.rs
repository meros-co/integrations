//! Allen & Heath consoles simulated on real TCP, driven through the public
//! API: a dLive MixRack (SysEx gets answered by Note On, NRPN and SysEx) and
//! an SQ (NRPN gets answered by the set message).
//!
//! Each simulator reads the MIDI stream the core sends, answers the reads it
//! knows, records every byte, and pushes changes of its own as a console does
//! when it is operated: a scene recall and a fader move, using running status.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type Log = Arc<Mutex<Vec<u8>>>;

/// Split the core's stream into messages: SysEx whole, channel messages by
/// their status byte (the core always sends status bytes).
fn messages(buf: &mut Vec<u8>) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    while let Some(&first) = buf.first() {
        let len = match first {
            0xF0 => match buf.iter().position(|&b| b == 0xF7) {
                Some(end) => end + 1,
                None => break,
            },
            0xC0..=0xDF => 2,
            0x80..=0xEF => 3,
            _ => 1,
        };
        if buf.len() < len {
            break;
        }
        out.push(buf.drain(..len).collect());
    }
    out
}

/// Accept one connection; for each message the core sends, write what
/// `answer` returns.
async fn simulate(answer: fn(&[u8]) -> Vec<u8>) -> (u16, Log) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let log: Log = Arc::default();
    let record = log.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let mut pending = Vec::new();
        loop {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 {
                return;
            }
            record.lock().unwrap().extend_from_slice(&buf[..n]);
            pending.extend_from_slice(&buf[..n]);
            let mut reply = Vec::new();
            for m in messages(&mut pending) {
                reply.extend(answer(&m));
            }
            if !reply.is_empty() {
                stream.write_all(&reply).await.unwrap();
            }
        }
    });
    (port, log)
}

const DLIVE: [u8; 8] = [0xF0, 0x00, 0x00, 0x1A, 0x50, 0x10, 0x01, 0x00];

/// A dLive MixRack on base MIDI channel 1 (N = 0).
fn dlive(m: &[u8]) -> Vec<u8> {
    let sx = |body: &[u8]| [&DLIVE[..], body, &[0xF7]].concat();
    if m.starts_with(&DLIVE) {
        return match &m[8..m.len() - 1] {
            // Name of input 1.
            [0x00, 0x01, 0x00] => sx(&[0x00, 0x02, 0x00, b'K', b'i', b'c', b'k']),
            // Fader of input 1: 0 dB.
            [0x00, 0x05, 0x0B, 0x17, 0x00] => {
                vec![0xB0, 0x63, 0x00, 0xB0, 0x62, 0x17, 0xB0, 0x06, 0x6B]
            }
            // Mute of input 2: on, then the Note On's release.
            [0x00, 0x05, 0x09, 0x01] => vec![0x90, 0x01, 0x7F, 0x90, 0x01, 0x00],
            _ => Vec::new(),
        };
    }
    // Someone on the console recalls scene 134 and pushes stereo group 1 to
    // +10 dB, in running status, when the mute arrives.
    if m == [0x90, 0x01, 0x7F] {
        return vec![
            0xB0, 0x00, 0x01, 0xC0, 0x05, 0xB1, 0x63, 0x40, 0x62, 0x17, 0x06, 0x7F,
        ];
    }
    Vec::new()
}

/// An SQ on MIDI channel 1: NRPN state per channel, answering gets.
fn sq(m: &[u8]) -> Vec<u8> {
    static NRPN: Mutex<(u8, u8)> = Mutex::new((0, 0));
    match *m {
        [0xB0, 0x63, msb] => NRPN.lock().unwrap().0 = msb,
        [0xB0, 0x62, lsb] => NRPN.lock().unwrap().1 = lsb,
        [0xB0, 0x60, 0x7F] => {
            let (msb, lsb) = *NRPN.lock().unwrap();
            let value: (u8, u8) = match (msb, lsb) {
                // LR mute: on.
                (0x00, 0x44) => (0x00, 0x01),
                // Input 40 to aux 5: -20 dB on the linear taper.
                (0x44, 0x1C) => (0x64, 0x16),
                // Any other mute: off.
                (0x00..=0x04, _) => (0x00, 0x00),
                _ => return Vec::new(),
            };
            // The answer is the set message, in running status.
            return vec![0xB0, 0x63, msb, 0x62, lsb, 0x06, value.0, 0x26, value.1];
        }
        // Input 1 to LR set: the console pushes a scene recall afterwards.
        [0xB0, 0x26, 0x5C] => return vec![0xB0, 0x00, 0x01, 0xC0, 0x1B],
        _ => {}
    }
    Vec::new()
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

async fn wait_for_state(
    core: &Core,
    id: meros_integrations::DeviceId,
    pred: impl Fn(&Value) -> bool,
) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if pred(&core.snapshot(id).unwrap().state) {
                return;
            }
            core.next_events(64).await;
        }
    })
    .await
    .expect("state within 10 s")
}

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[tokio::test(flavor = "multi_thread")]
async fn dlive_end_to_end() {
    let (port, log) = simulate(dlive).await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "allenheath-dlive".into(),
            model: "dlive-mixrack".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"midi_channel": 1, "sync_preamps": false})),
        })
        .unwrap();

    // The read after connecting asks for every name; the console's answer
    // becomes state.
    wait_for(&core, |e| {
        matches!(e, Event::State { device, patch } if *device == id
            && patch["channels"]["input"]["1"]["name"] == "Kick")
    })
    .await;

    // A get answered by the console's NRPN.
    assert_eq!(
        core.execute(
            id,
            "get_level",
            params(json!({"channel_type": "input", "channel": 1}))
        )
        .await,
        Ok(Outcome::Value {
            value: json!({"level_db": 0.0, "level_raw": 0x6B})
        })
    );

    // A write: unverified, followed by its get, whose answer is state.
    assert_eq!(
        core.execute(
            id,
            "set_mute",
            params(json!({"channel_type": "input", "channel": 2, "muted": true}))
        )
        .await,
        Ok(Outcome::Unverified)
    );
    wait_for_state(&core, id, |s| s["channels"]["input"]["2"]["mute"] == true).await;

    // Changes pushed by the console, in running status.
    wait_for_state(&core, id, |s| {
        s["scene"]["current"] == 134 && s["channels"]["stereo_group"]["1"]["level_db"] == 10.0
    })
    .await;

    // Out of range for the console is refused before sending.
    assert!(core
        .execute(
            id,
            "set_level",
            params(json!({"channel_type": "main", "channel": 7, "level_db": 0.0}))
        )
        .await
        .is_err());

    let log = log.lock().unwrap().clone();
    // Mute on for input 2, then velocity 0 (p.2), and its get.
    assert!(contains(&log, &[0x90, 0x01, 0x7F, 0x90, 0x01, 0x00]));
    assert!(contains(
        &log,
        &[&DLIVE[..], &[0x00, 0x05, 0x09, 0x01, 0xF7]].concat()
    ));
    core.close(id).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn sq_end_to_end() {
    let (port, log) = simulate(sq).await;
    let core = Core::new().unwrap();
    let id = core
        .open(OpenRequest {
            device: "allenheath-sq".into(),
            model: "sq-6".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: params(json!({"midi_channel": 1})),
        })
        .unwrap();

    // The mute reads after connecting are answered.
    wait_for_state(&core, id, |s| {
        s["channels"]["main"]["1"]["mute"] == true && s["channels"]["input"]["48"]["mute"] == false
    })
    .await;

    // A get answered by the set message.
    assert_eq!(
        core.execute(
            id,
            "get_send_level",
            params(json!({"channel_type": "input", "channel": 40,
                "destination_type": "aux", "destination": 5}))
        )
        .await,
        Ok(Outcome::Value {
            value: json!({"level_db": -20.0, "level_raw": (0x64 << 7) | 0x16})
        })
    );

    // SQ p.13: Ip1 to LR 0 dB, linear taper.
    assert_eq!(
        core.execute(
            id,
            "set_level",
            params(json!({"channel_type": "input", "channel": 1, "level_db": 0.0}))
        )
        .await,
        Ok(Outcome::Unverified)
    );
    // The console's pushed scene recall (bank 01, program 1B: scene 156).
    wait_for_state(&core, id, |s| s["scene"]["current"] == 156).await;

    let log = log.lock().unwrap().clone();
    assert!(contains(
        &log,
        &[0xB0, 0x63, 0x40, 0xB0, 0x62, 0x00, 0xB0, 0x06, 0x76, 0xB0, 0x26, 0x5C]
    ));
    // The LR mute read (00 44) went out.
    assert!(contains(
        &log,
        &[0xB0, 0x63, 0x00, 0xB0, 0x62, 0x44, 0xB0, 0x60, 0x7F]
    ));
    core.close(id).await;
}
