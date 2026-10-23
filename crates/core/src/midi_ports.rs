//! MIDI ports of the host for device modules: USB and other class-compliant
//! MIDI interfaces and instruments, and other programs' virtual ports, as the
//! operating system lists them.
//!
//! An open port is moved like a serial port (`crate::serial`): one task
//! reports it open, forwards every MIDI message received on its input and
//! writes what the session hands it to its output, and a port that goes away
//! (a USB device unplugged) is reported closed, with a reason starting
//! `midi:`, after which the module opens it again as it would reconnect. A
//! port has no message boundaries for the module either: it receives bytes
//! and parses them as MIDI (`crate::midi`).
//!
//! Ports are opened through [`MidiPorts`], so a test hands the session an
//! in-memory port instead and never opens a real one. On the host, in a
//! build with a MIDI integration (`generic-midi`, `midi-show-control`), they
//! are opened with the `midir` crate (MIT) on Windows (Windows Multimedia,
//! WinMM), macOS and iOS (CoreMIDI), and on Linux with the ALSA sequencer,
//! whose library is loaded only when a port is first listed or opened
//! ([`alsa`]): a Linux build needs no ALSA to build or to start, and a host
//! without `libasound.so.2` lists no port. Android is left out: its MIDI API
//! is reached only through Java, which a library linked into an app cannot
//! start on its own, so there no port is listed and opening one fails with a
//! reason saying so (docs/MOBILE.md).

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::module::{Key, MidiPort, TcpInput};
use crate::session::Inbound;
use crate::tcp::{Connection, Outgoing, Report};

/// A MIDI port of the host ([`crate::Core::midi_ports`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiPortInfo {
    /// What the `midi_input` and `midi_output` settings take: the name the
    /// operating system gives the port.
    pub name: String,
    /// Whether the port can be received from (`midi_input`).
    pub input: bool,
    /// Whether the port can be sent to (`midi_output`).
    pub output: bool,
}

/// Where an open port's messages go.
pub(crate) trait MidiSink: Send {
    /// Send whole or partial messages; what does not complete a message is
    /// kept until it does.
    fn send(&mut self, bytes: &[u8]) -> Result<(), String>;
    /// Whether the ports it holds are still there.
    fn present(&mut self) -> bool;
}

/// Opens MIDI ports: the host's own ([`HostMidi`]), or a test's.
pub(crate) trait MidiPorts: Send + Sync {
    /// Open the named input and output; what the input receives goes to
    /// `received`, a message or more at a time.
    fn open(
        &self,
        port: &MidiPort,
        received: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Result<Box<dyn MidiSink>, String>;

    /// The ports there are now, sorted by name.
    fn list(&self) -> Result<Vec<MidiPortInfo>, String>;
}

/// The host's MIDI ports.
pub(crate) struct HostMidi;

#[cfg(all(
    any(feature = "generic-midi", feature = "midi-show-control"),
    any(target_os = "linux", test)
))]
mod alsa;

#[cfg(all(
    any(feature = "generic-midi", feature = "midi-show-control"),
    not(any(target_os = "android", target_os = "linux"))
))]
mod host {
    use std::collections::BTreeMap;

    use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
    use tokio::sync::mpsc;

    use super::{MidiPortInfo, MidiSink};
    use crate::midi::Parser;
    use crate::module::MidiPort;

    /// The client name the operating system shows for the core's connections.
    const CLIENT: &str = "Meros";

    fn input_names(input: &MidiInput) -> Vec<String> {
        input
            .ports()
            .iter()
            .filter_map(|p| input.port_name(p).ok())
            .collect()
    }

    fn output_names(output: &MidiOutput) -> Vec<String> {
        output
            .ports()
            .iter()
            .filter_map(|p| output.port_name(p).ok())
            .collect()
    }

    /// Every port either way. A host whose MIDI system cannot be reached
    /// has none to list.
    pub(super) fn list() -> Vec<MidiPortInfo> {
        let mut ports: BTreeMap<String, (bool, bool)> = BTreeMap::new();
        if let Ok(input) = MidiInput::new(CLIENT) {
            for name in input_names(&input) {
                ports.entry(name).or_default().0 = true;
            }
        }
        if let Ok(output) = MidiOutput::new(CLIENT) {
            for name in output_names(&output) {
                ports.entry(name).or_default().1 = true;
            }
        }
        ports
            .into_iter()
            .map(|(name, (input, output))| MidiPortInfo {
                name,
                input,
                output,
            })
            .collect()
    }

    pub(super) struct Sink {
        output: Option<MidiOutputConnection>,
        _input: Option<MidiInputConnection<()>>,
        parser: Parser,
        /// Clients kept to see whether the ports are still listed.
        watch_input: Option<(MidiInput, String)>,
        watch_output: Option<(MidiOutput, String)>,
    }

    pub(super) fn open(
        port: &MidiPort,
        received: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Result<Box<dyn MidiSink>, String> {
        let mut sink = Sink {
            output: None,
            _input: None,
            parser: Parser::default(),
            watch_input: None,
            watch_output: None,
        };
        if let Some(name) = &port.input {
            let mut input = MidiInput::new(CLIENT).map_err(|e| e.to_string())?;
            // Everything, SysEx, timing and active sensing included.
            input.ignore(Ignore::None);
            let found = input
                .ports()
                .into_iter()
                .find(|p| input.port_name(p).ok().as_deref() == Some(name))
                .ok_or_else(|| format!("no MIDI input named '{name}'"))?;
            let connection = input
                .connect(
                    &found,
                    "meros-in",
                    move |_, bytes, _| {
                        let _ = received.send(bytes.to_vec());
                    },
                    (),
                )
                .map_err(|e| format!("MIDI input '{name}': {e}"))?;
            sink._input = Some(connection);
            sink.watch_input = Some((
                MidiInput::new(CLIENT).map_err(|e| e.to_string())?,
                name.clone(),
            ));
        }
        if let Some(name) = &port.output {
            let output = MidiOutput::new(CLIENT).map_err(|e| e.to_string())?;
            let found = output
                .ports()
                .into_iter()
                .find(|p| output.port_name(p).ok().as_deref() == Some(name))
                .ok_or_else(|| format!("no MIDI output named '{name}'"))?;
            let connection = output
                .connect(&found, "meros-out")
                .map_err(|e| format!("MIDI output '{name}': {e}"))?;
            sink.output = Some(connection);
            sink.watch_output = Some((
                MidiOutput::new(CLIENT).map_err(|e| e.to_string())?,
                name.clone(),
            ));
        }
        Ok(Box::new(sink))
    }

    impl MidiSink for Sink {
        fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
            let Some(output) = self.output.as_mut() else {
                return Ok(());
            };
            // The port takes one whole message at a time.
            for message in crate::midi::split(&mut self.parser, bytes) {
                output.send(&message).map_err(|e| e.to_string())?;
            }
            Ok(())
        }

        fn present(&mut self) -> bool {
            let input = self
                .watch_input
                .as_ref()
                .is_none_or(|(client, name)| input_names(client).contains(name));
            let output = self
                .watch_output
                .as_ref()
                .is_none_or(|(client, name)| output_names(client).contains(name));
            input && output
        }
    }
}

#[cfg(all(
    any(feature = "generic-midi", feature = "midi-show-control"),
    target_os = "linux"
))]
use alsa as platform;
#[cfg(all(
    any(feature = "generic-midi", feature = "midi-show-control"),
    not(any(target_os = "android", target_os = "linux"))
))]
use host as platform;

/// A build without a MIDI integration, or Android: no port to list or open.
#[cfg(not(all(
    any(feature = "generic-midi", feature = "midi-show-control"),
    not(target_os = "android")
)))]
mod platform {
    use tokio::sync::mpsc;

    use super::{MidiPortInfo, MidiSink};
    use crate::module::MidiPort;

    pub(super) fn open(
        _port: &MidiPort,
        _received: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Result<Box<dyn MidiSink>, String> {
        Err(if cfg!(target_os = "android") {
            "MIDI ports are not available on this platform".into()
        } else {
            "this build has no MIDI integration, so it opens no MIDI port".into()
        })
    }

    pub(super) fn list() -> Vec<MidiPortInfo> {
        Vec::new()
    }
}

impl MidiPorts for HostMidi {
    fn open(
        &self,
        port: &MidiPort,
        received: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Result<Box<dyn MidiSink>, String> {
        platform::open(port, received)
    }

    fn list(&self) -> Result<Vec<MidiPortInfo>, String> {
        Ok(platform::list())
    }
}

/// How often an open port is checked for still being there.
const PRESENCE_CHECK: Duration = Duration::from_secs(1);

/// Open `port` and move messages both ways until it goes or is closed.
pub(crate) fn spawn(
    socket: Key,
    generation: u64,
    port: MidiPort,
    ports: Arc<dyn MidiPorts>,
    inbound: mpsc::Sender<Inbound>,
) -> Connection {
    let (writer, mut outgoing) = mpsc::unbounded_channel::<Outgoing>();
    let task = tokio::spawn(async move {
        let report = Report {
            socket,
            generation,
            inbound,
        };
        let (received, mut incoming) = mpsc::unbounded_channel();
        // Kept so a port with no input never looks closed.
        let _keep = received.clone();
        let mut sink = match ports.open(&port, received) {
            Ok(sink) => sink,
            Err(e) => {
                report
                    .send(TcpInput::Closed {
                        reason: format!("midi: {e}"),
                    })
                    .await;
                return;
            }
        };
        if !report.send(TcpInput::Connected).await {
            return;
        }
        let mut check = tokio::time::interval(PRESENCE_CHECK);
        check.tick().await;
        let reason = loop {
            tokio::select! {
                out = outgoing.recv() => match out {
                    Some(Outgoing::Data(data)) => {
                        if let Err(e) = sink.send(&data) {
                            break e;
                        }
                    }
                    // A MIDI port has no files and no half-close.
                    Some(_) => {}
                    None => return,
                },
                Some(bytes) = incoming.recv() => {
                    if !report.send(TcpInput::Data(bytes)).await {
                        return;
                    }
                }
                _ = check.tick() => {
                    if !sink.present() {
                        break "the port is no longer there".into();
                    }
                }
            }
        };
        drop(sink);
        report
            .send(TcpInput::Closed {
                reason: format!("midi: {reason}"),
            })
            .await;
    });
    Connection {
        generation,
        writer,
        task,
    }
}

/// In-memory MIDI ports for tests: each port a test plugs in plays a
/// device, and nothing real is opened.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct MemoryMidi {
    plugged: std::sync::Mutex<std::collections::HashMap<String, MemoryPort>>,
}

#[cfg(test)]
struct MemoryPort {
    /// Where what the core sends goes: the test's end.
    sent: mpsc::UnboundedSender<Vec<u8>>,
    /// Where the device's messages go once the port is open.
    received: Option<mpsc::UnboundedSender<Vec<u8>>>,
    present: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(test)]
impl MemoryMidi {
    /// Plug in a device whose input and output are both named `name`; the
    /// returned receiver gets what the core sends it.
    pub(crate) fn plug(&self, name: &str) -> mpsc::UnboundedReceiver<Vec<u8>> {
        let (sent, rx) = mpsc::unbounded_channel();
        self.plugged.lock().unwrap().insert(
            name.to_string(),
            MemoryPort {
                sent,
                received: None,
                present: Arc::new(true.into()),
            },
        );
        rx
    }

    /// The device sends `bytes`; false when its port is not open.
    pub(crate) fn play(&self, name: &str, bytes: &[u8]) -> bool {
        let plugged = self.plugged.lock().unwrap();
        plugged
            .get(name)
            .and_then(|p| p.received.as_ref())
            .is_some_and(|tx| tx.send(bytes.to_vec()).is_ok())
    }

    pub(crate) fn unplug(&self, name: &str) {
        if let Some(port) = self.plugged.lock().unwrap().remove(name) {
            port.present
                .store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
struct MemorySink {
    sent: mpsc::UnboundedSender<Vec<u8>>,
    present: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(test)]
impl MidiSink for MemorySink {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.sent
            .send(bytes.to_vec())
            .map_err(|_| "the device is gone".to_string())
    }

    fn present(&mut self) -> bool {
        self.present.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
impl MidiPorts for MemoryMidi {
    fn open(
        &self,
        port: &MidiPort,
        received: mpsc::UnboundedSender<Vec<u8>>,
    ) -> Result<Box<dyn MidiSink>, String> {
        let mut plugged = self.plugged.lock().unwrap();
        let name = port
            .input
            .as_ref()
            .or(port.output.as_ref())
            .ok_or("no port named")?;
        let found = plugged
            .get_mut(name)
            .ok_or_else(|| format!("no MIDI port named '{name}'"))?;
        found.received = Some(received);
        Ok(Box::new(MemorySink {
            sent: found.sent.clone(),
            present: found.present.clone(),
        }))
    }

    fn list(&self) -> Result<Vec<MidiPortInfo>, String> {
        let mut names: Vec<String> = self.plugged.lock().unwrap().keys().cloned().collect();
        names.sort();
        Ok(names
            .into_iter()
            .map(|name| MidiPortInfo {
                name,
                input: true,
                output: true,
            })
            .collect())
    }
}
