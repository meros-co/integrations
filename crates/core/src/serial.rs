//! Serial ports for device modules: RS-232, RS-422 and USB serial adapters on
//! the host running the core.
//!
//! An open port is moved like a TCP connection (`crate::tcp::pump`): one task
//! reports it open, forwards every received chunk and writes what the session
//! hands it, and a port that fails, such as a USB adapter unplugged, is
//! reported closed, after which the module opens it again as it would
//! reconnect. Framing is the module's job.
//!
//! The ports are opened through [`SerialPorts`], so a test hands the session
//! an in-memory pair instead and never opens a real port. On the host, ports
//! are opened with the `serial2-tokio` crate on Windows, macOS, Linux and the
//! other Unix systems it supports. Phones have no serial port an app can open
//! (Android exposes USB serial adapters only through its Java USB host API,
//! iOS not at all), so there opening one fails with a reason saying so
//! (docs/MOBILE.md).

use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use crate::module::{Key, SerialPort, TcpInput};
use crate::session::Inbound;
use crate::tcp::{Connection, Outgoing, Report};

/// What an open port reads and writes through.
pub(crate) trait SerialStream: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> SerialStream for T {}

/// Opens serial ports: the host's own ([`HostPorts`]), or a test's.
pub(crate) trait SerialPorts: Send + Sync {
    fn open(&self, port: &SerialPort) -> std::io::Result<Box<dyn SerialStream>>;
}

/// The host's serial ports.
pub(crate) struct HostPorts;

impl SerialPorts for HostPorts {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn open(&self, port: &SerialPort) -> std::io::Result<Box<dyn SerialStream>> {
        use crate::module::{FlowControl, Parity};
        use serial2_tokio::{CharSize, Settings, StopBits};
        let line = port.clone();
        let opened = serial2_tokio::SerialPort::open(&port.path, move |mut s: Settings| {
            s.set_raw();
            s.set_baud_rate(line.baud_rate)?;
            s.set_char_size(match line.data_bits {
                5 => CharSize::Bits5,
                6 => CharSize::Bits6,
                7 => CharSize::Bits7,
                _ => CharSize::Bits8,
            });
            s.set_stop_bits(if line.stop_bits == 2 {
                StopBits::Two
            } else {
                StopBits::One
            });
            s.set_parity(match line.parity {
                Parity::None => serial2_tokio::Parity::None,
                Parity::Odd => serial2_tokio::Parity::Odd,
                Parity::Even => serial2_tokio::Parity::Even,
            });
            s.set_flow_control(match line.flow_control {
                FlowControl::None => serial2_tokio::FlowControl::None,
                FlowControl::XonXoff => serial2_tokio::FlowControl::XonXoff,
                FlowControl::RtsCts => serial2_tokio::FlowControl::RtsCts,
            });
            Ok(s)
        })?;
        Ok(Box::new(opened))
    }

    #[cfg(any(target_os = "android", target_os = "ios"))]
    fn open(&self, _port: &SerialPort) -> std::io::Result<Box<dyn SerialStream>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "serial ports are not available on this platform",
        ))
    }
}

/// Open `port` and move bytes both ways until it fails or is closed.
pub(crate) fn spawn(
    socket: Key,
    generation: u64,
    port: SerialPort,
    ports: Arc<dyn SerialPorts>,
    inbound: mpsc::Sender<Inbound>,
) -> Connection {
    let (writer, outgoing) = mpsc::unbounded_channel::<Outgoing>();
    let task = tokio::spawn(async move {
        let report = Report {
            socket,
            generation,
            inbound,
        };
        match ports.open(&port) {
            Ok(stream) => crate::tcp::pump(stream, outgoing, &report).await,
            Err(e) => {
                let reason = format!("serial: {}: {e}", port.path);
                report.send(TcpInput::Closed { reason }).await;
            }
        }
    });
    Connection {
        generation,
        writer,
        task,
    }
}

/// In-memory serial ports for tests: each port a test plugs in is one end of
/// a pipe whose other end plays the device. Nothing real is opened.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct MemoryPorts {
    /// Per port name, the ends waiting to be opened, in order; a port with
    /// none is absent, as an unplugged adapter is.
    plugged: std::sync::Mutex<std::collections::HashMap<String, Vec<tokio::io::DuplexStream>>>,
    /// Every port opened, with its settings.
    pub(crate) opened: std::sync::Mutex<Vec<SerialPort>>,
}

#[cfg(test)]
impl MemoryPorts {
    /// Plug in a device on `path`: the next open of the port gets one end,
    /// and the returned end is the device's. Dropping the device's end
    /// unplugs it.
    pub(crate) fn plug(&self, path: &str) -> tokio::io::DuplexStream {
        let (ours, device) = tokio::io::duplex(64 * 1024);
        self.plugged
            .lock()
            .unwrap()
            .entry(path.to_string())
            .or_default()
            .push(ours);
        device
    }
}

#[cfg(test)]
impl SerialPorts for MemoryPorts {
    fn open(&self, port: &SerialPort) -> std::io::Result<Box<dyn SerialStream>> {
        self.opened.lock().unwrap().push(port.clone());
        let mut plugged = self.plugged.lock().unwrap();
        match plugged.get_mut(&port.path).filter(|ends| !ends.is_empty()) {
            Some(ends) => Ok(Box::new(ends.remove(0))),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no such port",
            )),
        }
    }
}
