//! TSL UMD in both directions on real UDP, through the public API: one core
//! sends V5.0 to a port on which another core listens, as a switcher would.

use std::time::Duration;

use meros_integrations::{Core, Event, OpenRequest, Outcome};
use serde_json::{json, Value};

fn params(v: Value) -> meros_integrations::Params {
    v.as_object().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn tsl_sent_by_one_core_is_received_by_another() {
    // A free port for the listener.
    let port = std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();

    let receiver = Core::new().unwrap();
    let listener = receiver
        .open(OpenRequest {
            device: "tsl-umd-listener".into(),
            model: "tsl-umd".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: Default::default(),
        })
        .unwrap();

    let sender = Core::new().unwrap();
    let display = sender
        .open(OpenRequest {
            device: "tsl-umd-display".into(),
            model: "tsl-5-0-udp".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: Default::default(),
        })
        .unwrap();

    let got = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            // UDP: resend until the listener has bound and heard it.
            let outcome = sender
                .execute(
                    display,
                    "set_display",
                    params(json!({"index": 2, "text": "CAM 3", "lh": "red", "rh": "green"})),
                )
                .await;
            assert_eq!(outcome, Ok(Outcome::Unverified));
            for e in receiver.poll_events(64) {
                if let Event::State { device, patch } = e {
                    if device == listener {
                        return patch;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the listener receives the display update");
    assert_eq!(got["protocol"], "5.0");
    let d = &got["screens"]["0"]["displays"]["2"];
    assert_eq!(d["text"], "CAM 3");
    assert_eq!(
        d["tally"],
        json!({"lh": "red", "text": "off", "rh": "green"})
    );

    // Without a port there is nothing to listen on.
    assert!(receiver
        .open(OpenRequest {
            device: "tsl-umd-listener".into(),
            model: "tsl-umd".into(),
            host: "127.0.0.1".into(),
            port: None,
            settings: Default::default(),
        })
        .is_err());
}
