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

// ── One spec over TCP or a serial port (`transport.serial`) ──────────────

const DUAL: &str = r#"
spec: 1
id: dual-test
name: Dual test
vendor: Test
category: switcher
source: [{ title: test }]
transport:
  type: line-tcp
  port: 23
  framing: terminated
  terminator: cr
  timeout_ms: 1000
  reply_match: '^(Ok|Ver)'
  serial:
    serial_only: [rs232-only]
    models: [networked, rs232-only]
    baud_rate: { default: 9600, values: [9600, 19200, 115200] }
    data_bits: 8
    parity: { default: none, values: [none, even] }
    stop_bits: 1
    flow_control: none
settings:
  password: { type: string, default: "", label: Password, description: "The password." }
on_connect:
  - { when_set: password, link: tcp, send: "PW {settings.password}" }
  - { link: serial, send: "SERIAL" }
  - "HELLO"
ports:
  - { port: 23, protocol: tcp, role: control, when: "link tcp" }
  - { port: null, protocol: serial, role: control, setting: serial_port, when: "link serial", note: "RS-232" }
models:
  - { id: networked, name: N, supports: [cut], verification: none }
  - { id: rs232-only, name: R, supports: [cut], verification: none }
  - { id: no-serial, name: X, supports: [cut], verification: none }
commands:
  cut:
    send: "CUT"
    expect: { contains: "Ok" }
    returns: ack
"#;

fn dual_core(ports: Arc<MemoryPorts>) -> Core {
    let spec = DeviceSpec::from_yaml(DUAL).unwrap();
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

fn dual_open(
    core: &Core,
    model: &str,
    host: &str,
    settings: serde_json::Value,
) -> Result<u64, crate::OpenError> {
    core.open(OpenRequest {
        device: "dual-test".into(),
        model: model.into(),
        host: host.into(),
        port: None,
        settings: settings.as_object().unwrap().clone(),
        monitor: true,
    })
}

/// The next CR-terminated line the core sent the device.
async fn line(device: &mut (impl tokio::io::AsyncRead + Unpin)) -> String {
    let mut out = Vec::new();
    let mut byte = [0u8; 1];
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            device.read_exact(&mut byte).await.unwrap();
            if byte[0] == b'\r' {
                break;
            }
            out.push(byte[0]);
        }
    })
    .await
    .expect("a line within 5 s");
    String::from_utf8(out).unwrap()
}

#[test]
fn a_serial_block_adds_the_link_and_the_line_settings() {
    let spec = DeviceSpec::from_yaml(DUAL).unwrap();
    let s = &spec.settings;
    assert_eq!(
        s["link"].values.as_deref().unwrap(),
        ["tcp".to_string(), "serial".to_string()]
    );
    // serial_only: no one default, the model decides.
    assert_eq!(s["link"].default, None);
    assert!(!s["serial_port"].required);
    assert_eq!(
        s["baud_rate"].values.as_deref().unwrap(),
        ["9600", "19200", "115200"]
    );
    assert_eq!(s["baud_rate"].default, Some(json!("9600")));
    assert_eq!(
        (s["data_bits"].min, s["data_bits"].max),
        (Some(8.0), Some(8.0))
    );
    assert_eq!(s["parity"].values.as_deref().unwrap(), ["none", "even"]);
    assert_eq!(s["flow_control"].default, Some(json!("none")));
    for (name, p) in s {
        assert!(p.label.is_some() && p.description.is_some(), "{name}");
    }
    // The other way round: tcp by default, or serial when the block says so.
    let plain = DUAL.replace("    serial_only: [rs232-only]\n", "");
    let spec = DeviceSpec::from_yaml(&plain).unwrap();
    assert_eq!(spec.settings["link"].default, Some(json!("tcp")));
    let serial = plain.replace("    models:", "    link: serial\n    models:");
    let spec = DeviceSpec::from_yaml(&serial).unwrap();
    assert_eq!(spec.settings["link"].default, Some(json!("serial")));
    let defaults = crate::catalog::validate(&spec.settings, &Params::new()).unwrap();
    assert_eq!(spec.serial_link("networked", &defaults), Some(true));

    // What a block may not be.
    for (from, to, says) in [
        ("    stop_bits: 1\n", "", "stop_bits"),
        ("stop_bits: 1", "stop_bits: 3", "stop_bits"),
        (
            "data_bits: 8",
            "data_bits: { default: 8, values: [6, 8] }",
            "no gaps",
        ),
        (
            "parity: { default: none, values: [none, even] }",
            "parity: mark",
            "parity",
        ),
        ("default: 9600, values", "default: 4800, values", "default"),
        ("serial_only: [rs232-only]", "serial_only: [nope]", "nope"),
        (
            "    flow_control: none\n",
            "    flow_control: none\n    speed: 1\n",
            "speed",
        ),
        ("  type: line-tcp", "  type: line-udp", "line-tcp"),
        (
            "settings:\n",
            "settings:\n  baud_rate: { type: int, default: 1, label: B, description: B }\n",
            "leave it out",
        ),
    ] {
        assert!(DUAL.contains(from), "{from}");
        let e = DeviceSpec::from_yaml(&DUAL.replace(from, to)).unwrap_err();
        assert!(e.contains(says), "{from}: {e}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn link_serial_speaks_the_same_protocol_on_the_port() {
    let ports = Arc::new(MemoryPorts::default());
    let mut device = ports.plug("COM4");
    let core = dual_core(ports.clone());
    // No host: the port is the device's address. The network-only login is
    // left out, the serial-only step sent.
    let id = dual_open(
        &core,
        "networked",
        "",
        json!({"link": "serial", "serial_port": "COM4", "baud_rate": "115200", "password": "x"}),
    )
    .unwrap();
    assert_eq!(line(&mut device).await, "SERIAL");
    assert_eq!(line(&mut device).await, "HELLO");
    assert_eq!(
        ports.opened.lock().unwrap()[0],
        SerialPort {
            path: "COM4".into(),
            baud_rate: 115200,
            data_bits: 8,
            parity: Parity::None,
            stop_bits: 1,
            flow_control: FlowControl::None,
        }
    );
    wait_connection(&core, id, |c| *c == Connection::Connected).await;
    let command = core.execute(id, "cut", Params::new());
    let answer = async {
        assert_eq!(line(&mut device).await, "CUT");
        device.write_all(b"Ok\r\n").await.unwrap();
    };
    let (result, ()) = tokio::join!(command, answer);
    assert_eq!(result, Ok(Outcome::Ack));
}

#[tokio::test(flavor = "multi_thread")]
async fn link_tcp_uses_the_network_and_a_serial_only_model_defaults_to_its_port() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let ports = Arc::new(MemoryPorts::default());
    let mut rs232 = ports.plug("COM5");
    let core = dual_core(ports.clone());
    let id = core
        .open(OpenRequest {
            device: "dual-test".into(),
            model: "networked".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: json!({"password": "x"}).as_object().unwrap().clone(),
            monitor: true,
        })
        .unwrap();
    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(line(&mut socket).await, "PW x");
    assert_eq!(line(&mut socket).await, "HELLO");
    wait_connection(&core, id, |c| *c == Connection::Connected).await;
    let command = core.execute(id, "cut", Params::new());
    let answer = async {
        assert_eq!(line(&mut socket).await, "CUT");
        socket.write_all(b"Ok\r\n").await.unwrap();
    };
    let (result, ()) = tokio::join!(command, answer);
    assert_eq!(result, Ok(Outcome::Ack));

    // Switched to its serial port while open: the port is opened at once.
    core.update_settings(
        id,
        json!({"link": "serial", "serial_port": "COM5"})
            .as_object()
            .unwrap()
            .clone(),
    )
    .await
    .unwrap();
    assert_eq!(line(&mut rs232).await, "SERIAL");

    // A model reached only on its serial port uses it unless told otherwise.
    let _second = ports.plug("COM6");
    let only = dual_open(&core, "rs232-only", "", json!({"serial_port": "COM6"})).unwrap();
    wait_connection(&core, only, |c| *c == Connection::Connected).await;
    assert!(ports
        .opened
        .lock()
        .unwrap()
        .iter()
        .any(|p| p.path == "COM6"));
}

/// DUAL with framing the vendor documents differently on the serial port:
/// STX before each message and no CR, replies ended by ";" or an ACK byte,
/// and one model with no STX.
const OVERRIDES: &str = r#"    overrides:
      - { framing: delimited, open: "\u0002", close: "", reply_framing: terminated,
          reply_terminators: [";", "\u0006"], reply_match: '^\x06|^Ver' }
      - { models: [rs232-only], open: "" }
"#;

fn overridden() -> String {
    DUAL.replace(
        "    flow_control: none\n",
        &format!("    flow_control: none\n{OVERRIDES}"),
    )
    .replace(r#"send: "CUT""#, r#"send: "CUT;""#)
    .replace(
        r#"expect: { contains: "Ok" }"#,
        r#"expect: { matches: "Ok|\u0006" }"#,
    )
}

/// Bytes the core sent the device, up to and including the next ";".
async fn upto_semicolon(device: &mut (impl tokio::io::AsyncRead + Unpin)) -> String {
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

#[tokio::test(flavor = "multi_thread")]
async fn overrides_change_the_framing_on_the_serial_port_only() {
    let spec = DeviceSpec::from_yaml(&overridden()).unwrap();
    assert_eq!(spec.serial_overrides("networked")["open"], json!("\u{2}"));
    assert_eq!(spec.serial_overrides("rs232-only")["open"], json!(""));
    assert_eq!(
        spec.serial_overrides("rs232-only")["reply_framing"],
        json!("terminated")
    );
    let catalog = Catalog {
        devices: [(spec.id.clone(), spec)].into(),
    };
    let ports = Arc::new(MemoryPorts::default());
    let mut stx = ports.plug("COM8");
    let mut plain = ports.plug("COM9");
    let core = Core::with_catalog(
        catalog,
        CoreOptions::new().bind_address([127, 0, 0, 1].into()),
    )
    .unwrap()
    .with_serial_ports(ports.clone());

    // On the serial port: STX before each message, nothing after it, and an
    // ACK byte with no line ending is the reply.
    let id = dual_open(
        &core,
        "networked",
        "",
        json!({"link": "serial", "serial_port": "COM8"}),
    )
    .unwrap();
    wait_connection(&core, id, |c| *c == Connection::Connected).await;
    let command = core.execute(id, "cut", Params::new());
    let answer = async {
        assert_eq!(
            upto_semicolon(&mut stx).await,
            "\u{2}SERIAL\u{2}HELLO\u{2}CUT;"
        );
        stx.write_all(b"\x06").await.unwrap();
    };
    let (result, ()) = tokio::join!(command, answer);
    assert_eq!(result, Ok(Outcome::Ack));

    // A model whose entry drops the STX.
    let only = dual_open(&core, "rs232-only", "", json!({"serial_port": "COM9"})).unwrap();
    wait_connection(&core, only, |c| *c == Connection::Connected).await;
    let command = core.execute(only, "cut", Params::new());
    let answer = async {
        assert_eq!(upto_semicolon(&mut plain).await, "SERIALHELLOCUT;");
        plain.write_all(b"\x06").await.unwrap();
    };
    let (result, ()) = tokio::join!(command, answer);
    assert_eq!(result, Ok(Outcome::Ack));

    // Over TCP the transport's own framing stays: CR after each line.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let tcp = core
        .open(OpenRequest {
            device: "dual-test".into(),
            model: "networked".into(),
            host: "127.0.0.1".into(),
            port: Some(port),
            settings: Params::new(),
            monitor: true,
        })
        .unwrap();
    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(line(&mut socket).await, "HELLO");
    wait_connection(&core, tcp, |c| *c == Connection::Connected).await;
    let command = core.execute(tcp, "cut", Params::new());
    let answer = async {
        assert_eq!(line(&mut socket).await, "CUT;");
        socket.write_all(b"Ok\r\n").await.unwrap();
    };
    let (result, ()) = tokio::join!(command, answer);
    assert_eq!(result, Ok(Outcome::Ack));
}

#[test]
fn overrides_name_only_framing_and_reply_keys_and_known_models() {
    let good = overridden();
    for (from, to, says) in [
        (
            r#"{ models: [rs232-only], open: "" }"#,
            "{ port: 24 }",
            "port",
        ),
        ("models: [rs232-only], open", "models: [nope], open", "nope"),
        (OVERRIDES, "    overrides: 5\n", "overrides"),
    ] {
        assert!(good.contains(from), "{from}");
        let e = DeviceSpec::from_yaml(&good.replace(from, to)).unwrap_err();
        assert!(e.contains(says), "{from}: {e}");
    }
    // Delimited with an empty close needs replies framed some other way.
    let bare = good.replace(
        "reply_framing: terminated,\n          reply_terminators: [\";\", \"\\u0006\"], ",
        "",
    );
    assert_ne!(bare, good);
    let spec = DeviceSpec::from_yaml(&bare).unwrap();
    let core = Core::with_catalog(
        Catalog {
            devices: [(spec.id.clone(), spec)].into(),
        },
        CoreOptions::new().bind_address([127, 0, 0, 1].into()),
    )
    .unwrap()
    .with_serial_ports(Arc::new(MemoryPorts::default()));
    assert!(dual_open(
        &core,
        "networked",
        "",
        json!({"link": "serial", "serial_port": "COM1"})
    )
    .is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_link_without_what_it_needs_is_refused_on_opening() {
    let core = dual_core(Arc::new(MemoryPorts::default()));
    let invalid = |r: Result<u64, crate::OpenError>, says: &str| match r {
        Err(crate::OpenError::InvalidSettings { message }) => {
            assert!(message.contains(says), "{message}")
        }
        other => panic!("{other:?}"),
    };
    invalid(
        dual_open(&core, "networked", "", json!({"link": "serial"})),
        "serial_port",
    );
    invalid(dual_open(&core, "rs232-only", "", json!({})), "serial_port");
    invalid(
        dual_open(
            &core,
            "no-serial",
            "",
            json!({"link": "serial", "serial_port": "COM1"}),
        ),
        "no serial port",
    );
    // Link tcp needs a host, as any network device does.
    assert!(matches!(
        dual_open(&core, "networked", "", json!({})),
        Err(crate::OpenError::UnresolvableHost { .. })
    ));
}

#[test]
fn the_host_serial_ports_are_listed() {
    let ports = Arc::new(MemoryPorts::default());
    let core = core(ports.clone());
    assert_eq!(core.serial_ports(), Ok(vec![]));
    let _a = ports.plug("COM9");
    let _b = ports.plug("/dev/ttyUSB0");
    assert_eq!(
        crate::json::serial_ports(&core),
        json!({"ok": [{"name": "/dev/ttyUSB0"}, {"name": "COM9"}]})
    );
}
