//! The `line-serial` transport through the public API, with the device played
//! on an in-memory serial port: framing both ways, the line settings the port
//! is opened with, latency, commands-only opening, and a device unplugged and
//! plugged in again. No real serial port is opened.

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

use crate::catalog::{Catalog, DeviceSpec, Params};
use crate::module::{FlowControl, Parity, SerialPort};
use crate::serial::MemoryPorts;
use crate::{CommandError, Connection, Core, CoreOptions, OpenRequest, Outcome};

const SPEC: &str = r#"
spec: 1
id: serial-test
name: Serial test
vendor: Test
category: switcher
source: [{ title: test }]
transport:
  type: line-serial
  framing: delimited
  open: "\u0002"
  close: ";"
  timeout_ms: 1000
  reply_framing: terminated
  reply_terminators: [";", "\u0006"]
  reply_match: '^(\x06|ERR:[0-9]+;|VER:)'
  probe: "ACS"
settings:
  serial_port: { type: string, required: true, label: Serial port, description: "The port." }
  baud_rate: { type: enum, values: ["9600", "38400"], default: "38400", label: Baud rate, description: "Bits per second." }
  data_bits: { type: int, min: 8, max: 8, default: 8, label: Data bits, description: "Bits per character." }
  parity: { type: enum, values: [none], default: none, label: Parity, description: "Parity." }
  stop_bits: { type: int, min: 1, max: 1, default: 1, label: Stop bits, description: "Stop bits." }
  flow_control: { type: enum, values: [none, xon-xoff], default: xon-xoff, label: Flow control, description: "Flow control." }
ports:
  - { port: null, protocol: serial, role: control, setting: serial_port, note: "RS-232" }
models:
  - { id: m, name: M, supports: [cut, get_version], verification: none }
commands:
  cut:
    send: "CUT"
    expect: { contains: "\u0006" }
    returns: ack
  get_version:
    send: "VER"
    expect: { matches: "VER:([^;]*);" }
    returns: value
state:
  version: { type: string, description: "x" }
telemetry:
  poll: { send: ["VER"] }
  updates:
    - { match: "^VER:([^;]*);", state: { version: "{1}" } }
"#;

fn core(ports: Arc<MemoryPorts>) -> Core {
    let spec: DeviceSpec = crate::yaml::from_str(SPEC).unwrap();
    let catalog = Catalog {
        devices: [(spec.id.clone(), spec)].into(),
    };
    Core::with_catalog(
        catalog,
        CoreOptions::new().bind_address([127, 0, 0, 1].into()),
    )
    .unwrap()
    .with_serial_ports(ports)
}

fn open(core: &Core, monitor: bool) -> u64 {
    let settings: Params = json!({"serial_port": "COM7"}).as_object().unwrap().clone();
    core.open(OpenRequest {
        device: "serial-test".into(),
        model: "m".into(),
        host: String::new(),
        port: None,
        settings,
        monitor,
    })
    .unwrap()
}

/// The next message the core sent the device: STX up to ";".
async fn received(device: &mut DuplexStream) -> String {
    let mut out = Vec::new();
    let mut byte = [0u8; 1];
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            device.read_exact(&mut byte).await.unwrap();
            out.push(byte[0]);
            if byte[0] == b';' {
                break;
            }
        }
    })
    .await
    .expect("a message within 5 s");
    String::from_utf8(out).unwrap()
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
async fn a_serial_device_is_framed_both_ways_and_reports_its_latency() {
    let ports = Arc::new(MemoryPorts::default());
    let mut device = ports.plug("COM7");
    let core = core(ports.clone());
    let id = open(&core, true);

    // Opened with the settings' line settings, defaults filled in.
    assert_eq!(received(&mut device).await, "\u{2}VER;");
    assert_eq!(
        ports.opened.lock().unwrap()[0],
        SerialPort {
            path: "COM7".into(),
            baud_rate: 38400,
            data_bits: 8,
            parity: Parity::None,
            stop_bits: 1,
            flow_control: FlowControl::XonXoff,
        }
    );
    // A reply split across reads, with no line ending, is one message.
    device.write_all(b"VER:V-1").await.unwrap();
    device.write_all(b"200HD,1.00;").await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while core.snapshot(id).unwrap().state.get("version").is_none() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(core.snapshot(id).unwrap().state["version"], "V-1200HD,1.00");
    wait_connection(&core, id, |c| *c == Connection::Connected).await;

    let command = core.execute(id, "cut", Params::new());
    let answer = async {
        assert_eq!(received(&mut device).await, "\u{2}CUT;");
        tokio::time::sleep(Duration::from_millis(30)).await;
        device.write_all(b"\x06").await.unwrap();
    };
    let (result, ()) = tokio::join!(command, answer);
    assert_eq!(result, Ok(Outcome::Ack));
    let latency = core.snapshot(id).unwrap().latency_ms.unwrap();
    assert!((30..2000).contains(&latency), "{latency}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unplugged_serial_device_is_disconnected_and_found_again() {
    let ports = Arc::new(MemoryPorts::default());
    let mut device = ports.plug("COM7");
    let core = core(ports.clone());
    let id = open(&core, false);
    wait_connection(&core, id, |c| *c == Connection::Connected).await;
    // Opened for commands only: nothing is read on opening, so the first
    // message is the command's.
    let command = core.execute(id, "get_version", Params::new());
    let answer = async {
        assert_eq!(received(&mut device).await, "\u{2}VER;");
        device.write_all(b"VER:X,1;").await.unwrap();
    };
    let (result, ()) = tokio::join!(command, answer);
    assert_eq!(
        result,
        Ok(Outcome::Value {
            value: json!("X,1")
        })
    );

    // Unplugged: the port fails, and so do commands, until it is back.
    drop(device);
    wait_connection(&core, id, |c| matches!(c, Connection::Disconnected { .. })).await;
    assert_eq!(
        core.execute(id, "cut", Params::new()).await,
        Err(CommandError::NotConnected)
    );
    // While it is away the port cannot be opened; plugged in again, it is
    // opened again with backoff and works as before.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let mut device = ports.plug("COM7");
    wait_connection(&core, id, |c| *c == Connection::Connected).await;
    let command = core.execute(id, "cut", Params::new());
    let answer = async {
        assert_eq!(received(&mut device).await, "\u{2}CUT;");
        device.write_all(b"\x06").await.unwrap();
    };
    let (result, ()) = tokio::join!(command, answer);
    assert_eq!(result, Ok(Outcome::Ack));
    assert!(ports.opened.lock().unwrap().len() >= 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_serial_device_needs_its_port_and_no_host() {
    let core = core(Arc::new(MemoryPorts::default()));
    let opened = core.open(OpenRequest {
        device: "serial-test".into(),
        model: "m".into(),
        host: String::new(),
        port: None,
        settings: Params::new(),
        monitor: true,
    });
    assert!(
        matches!(&opened, Err(crate::OpenError::InvalidSettings { message }) if message.contains("serial_port")),
        "{opened:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn another_serial_port_in_the_settings_is_opened_at_once() {
    let ports = Arc::new(MemoryPorts::default());
    let _first = ports.plug("COM7");
    let mut second = ports.plug("/dev/ttyUSB1");
    let core = core(ports.clone());
    let id = open(&core, false);
    wait_connection(&core, id, |c| *c == Connection::Connected).await;
    let change: Params = json!({"serial_port": "/dev/ttyUSB1", "baud_rate": "9600"})
        .as_object()
        .unwrap()
        .clone();
    core.update_settings(id, change).await.unwrap();
    // The device is built afresh on the new port; commands go once it is open.
    tokio::time::timeout(Duration::from_secs(5), async {
        while ports
            .opened
            .lock()
            .unwrap()
            .last()
            .is_none_or(|p| p.path != "/dev/ttyUSB1")
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    wait_connection(&core, id, |c| *c == Connection::Connected).await;
    let command = core.execute(id, "cut", Params::new());
    let answer = async {
        assert_eq!(received(&mut second).await, "\u{2}CUT;");
        second.write_all(b"\x06").await.unwrap();
    };
    let (result, ()) = tokio::join!(command, answer);
    assert_eq!(result, Ok(Outcome::Ack));
    let opened = ports.opened.lock().unwrap().clone();
    assert_eq!(opened.last().unwrap().path, "/dev/ttyUSB1");
    assert_eq!(opened.last().unwrap().baud_rate, 9600);
}

#[test]
fn line_settings_come_from_the_settings_with_defaults() {
    let settings = |v: serde_json::Value| v.as_object().unwrap().clone();
    let port = SerialPort::from_settings(&settings(json!({"serial_port": "COM3"}))).unwrap();
    assert_eq!(
        port,
        SerialPort {
            path: "COM3".into(),
            baud_rate: 9600,
            data_bits: 8,
            parity: Parity::None,
            stop_bits: 1,
            flow_control: FlowControl::None,
        }
    );
    let port = SerialPort::from_settings(&settings(json!({
        "serial_port": "/dev/ttyS0", "baud_rate": 38400, "data_bits": 7,
        "parity": "even", "stop_bits": 2, "flow_control": "rts-cts"
    })))
    .unwrap();
    assert_eq!(
        (
            port.baud_rate,
            port.data_bits,
            port.parity,
            port.stop_bits,
            port.flow_control
        ),
        (38400, 7, Parity::Even, 2, FlowControl::RtsCts)
    );
    assert!(SerialPort::from_settings(&settings(json!({"serial_port": " "}))).is_err());
    assert!(
        SerialPort::from_settings(&settings(json!({"serial_port": "COM1", "parity": "mark"})))
            .is_err()
    );
    assert!(
        SerialPort::from_settings(&settings(json!({"serial_port": "COM1", "data_bits": 9})))
            .is_err()
    );
}
