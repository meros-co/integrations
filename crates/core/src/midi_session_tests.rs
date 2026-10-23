//! A MIDI integration on a MIDI port of the host, through the public API,
//! with the device played on an in-memory port: messages both ways, state
//! and events, the port listing, and a device unplugged and plugged in
//! again. No real MIDI port is opened.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use crate::catalog::{Catalog, Params};
use crate::midi_ports::MemoryMidi;
use crate::{CommandError, Connection, Core, CoreOptions, Event, OpenRequest, Outcome};

fn core(ports: Arc<MemoryMidi>) -> Core {
    let catalog = Catalog::source_tree();
    let spec = catalog.device("generic-midi").unwrap().clone();
    let catalog = Catalog {
        devices: [(spec.id.clone(), spec)].into(),
    };
    Core::with_catalog(
        catalog,
        CoreOptions::new().bind_address([127, 0, 0, 1].into()),
    )
    .unwrap()
    .with_midi_ports(ports)
}

fn params(v: Value) -> Params {
    v.as_object().unwrap().clone()
}

fn open(core: &Core) -> u64 {
    core.open(OpenRequest {
        device: "generic-midi".into(),
        model: "generic".into(),
        // A device on a MIDI port needs no host.
        host: String::new(),
        port: None,
        settings: params(json!({"link": "port", "midi_input": "Keys", "midi_output": "Keys"})),
        monitor: true,
    })
    .unwrap()
}

async fn wait_connection(core: &Core, device: u64, want: fn(&Connection) -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !want(&core.snapshot(device).unwrap().connection) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("connection within 5 s")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_on_a_midi_port_is_played_both_ways_and_found_again() {
    let ports = Arc::new(MemoryMidi::default());
    let mut sent = ports.plug("Keys");
    let core = core(ports.clone());
    assert_eq!(
        core.midi_ports().unwrap(),
        [crate::MidiPortInfo {
            name: "Keys".into(),
            input: true,
            output: true
        }]
    );
    let id = open(&core);
    wait_connection(&core, id, |c| *c == Connection::Connected).await;

    let result = core
        .execute(
            id,
            "note_on",
            params(json!({"channel": 10, "note": 36, "velocity": 127})),
        )
        .await;
    assert_eq!(result, Ok(Outcome::Unverified));
    let got = tokio::time::timeout(Duration::from_secs(5), sent.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, [0x99, 36, 127]);

    // The device sends a controller and an MMC play.
    assert!(ports.play("Keys", &[0xB0, 74, 33]));
    assert!(ports.play("Keys", &[0xF0, 0x7F, 0x7F, 0x06, 0x02, 0xF7]));
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let state = core.snapshot(id).unwrap().state;
            if state["mmc"]["command"] == "play" {
                assert_eq!(state["channels"]["1"]["cc"]["74"], 33);
                assert_eq!(state["link"], "port");
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("state within 5 s");
    let mut addresses = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while addresses.len() < 2 {
            for e in core.next_events(64).await {
                if let Event::Message { address, .. } = e {
                    addresses.push(address);
                }
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(addresses, ["control_change", "mmc"]);

    // Unplugged: disconnected, and commands fail until it is back.
    ports.unplug("Keys");
    wait_connection(
        &core,
        id,
        |c| matches!(c, Connection::Disconnected { reason } if reason.starts_with("midi:")),
    )
    .await;
    assert_eq!(
        core.execute(id, "tune_request", Params::new()).await,
        Err(CommandError::NotConnected)
    );
    let mut sent = ports.plug("Keys");
    wait_connection(&core, id, |c| *c == Connection::Connected).await;
    assert_eq!(
        core.execute(id, "tune_request", Params::new()).await,
        Ok(Outcome::Unverified)
    );
    let got = tokio::time::timeout(Duration::from_secs(5), sent.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got, [0xF6]);
}
