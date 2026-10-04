//! One implementation of control and telemetry for third-party production
//! devices, shared by every consumer.
//!
//! ```no_run
//! use meros_integrations::{Core, CoreOptions, OpenRequest};
//! use serde_json::json;
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let core = Core::with_options(CoreOptions::new().devices(["sennheiser-ew-g3-g4"]))?;
//! let device = core.open(OpenRequest {
//!     device: "sennheiser-ew-g3-g4".into(),
//!     model: "em-300-500-g4".into(),
//!     host: "192.168.1.40".into(),
//!     port: None,
//!     settings: Default::default(),
//!     monitor: true,
//! })?;
//! let outcome = core
//!     .execute(device, "mute", json!({"muted": true}).as_object().unwrap().clone())
//!     .await?;
//! # Ok(()) }
//! ```
//!
//! Every spec is one integration, and a product chooses exactly the
//! integrations it uses. At build time each is a Cargo feature named after
//! its spec id (`features = ["sennheiser-ew-dx", "shure-wireless"]`); a
//! vendor group (`vendor-sennheiser`) or `all` enables several at once. The
//! default is none. An integration left out has neither its spec, its native
//! module, its discovery protocol nor its own dependencies in the binary. At
//! run time [`CoreOptions::devices`] narrows a core to some of the built
//! integrations. The spec engine and transports are shared and always built.

// Which helpers a build uses depends on the integrations it includes; a build
// without every integration leaves some unused, which is not a defect.
#![cfg_attr(
    not(feature = "all"),
    allow(dead_code, unused_imports, unused_variables)
)]

pub mod catalog;
mod digest;
mod discovery;
mod engine;
pub mod events;
mod files;
mod http;
pub mod json;
#[cfg(feature = "sennheiser-ew-g3-g4")]
mod mcp_discovery;
pub mod module;
mod modules;
#[cfg(feature = "pjlink")]
mod pjlink_discovery;
mod session;
#[cfg(feature = "sony-camera")]
mod ssdp;
pub mod sse;
#[cfg(feature = "sony-camera")]
mod ssh;
#[cfg(not(feature = "sony-camera"))]
#[path = "ssh_disabled.rs"]
mod ssh;
pub mod streams;
mod tcp;
mod tcp_listen;
mod tls;
mod udp;
mod ws;

use std::collections::{BTreeSet, HashMap};
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

pub use catalog::{Catalog, Params};
pub use discovery::{DiscoverAction, DiscoverRequest, PROTOCOLS as DISCOVERY_PROTOCOLS};
pub use events::Event;
pub use module::{CommandError, CommandResult, Connection, Outcome};
pub use session::{DeviceId, DeviceSnapshot};
pub use streams::{Frame, StreamError, StreamHandle};

use events::EventQueue;
use session::{Services, Session, SessionMsg};

/// Events held for a consumer that is not draining them.
const EVENT_CAPACITY: usize = 10_000;

/// How to reach one device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRequest {
    /// Spec id, e.g. `sennheiser-g4`.
    pub device: String,
    /// Model id within the spec.
    pub model: String,
    /// IP address or hostname. A hostname is resolved once, when opened, for
    /// TCP and UDP; the spec engine's HTTP, websocket and event-stream URLs
    /// keep the name, so a TLS certificate is checked against it. May be
    /// empty or left out for an integration on which only the core listens
    /// (every one of its `ports` is `listener: core`), such as `osc-listener`,
    /// which hears any sender; it is then `0.0.0.0`.
    #[serde(default)]
    pub host: String,
    /// The device's port, when it is not the protocol's default: many devices
    /// let an operator change it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Per-installation settings declared by the spec, such as passwords.
    #[serde(default)]
    pub settings: Params,
    /// Whether the core keeps the device's state current: subscribing to its
    /// changes, reading its state on connecting, and polling. `false` opens
    /// it for commands only: the core asks for nothing beyond what commands
    /// and its liveness check need, and so takes none of a device's limited
    /// subscription slots (a WING has one). What the device sends unasked is
    /// still applied to the state. Defaults to `true`.
    #[serde(default = "monitor_default")]
    pub monitor: bool,
}

fn monitor_default() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum OpenError {
    #[error("unknown device '{device}'")]
    UnknownDevice { device: String },
    /// The device is in this build but not among the devices the core was
    /// started with (`CoreOptions::devices`).
    #[error("device '{device}' is not among the devices this core was started with")]
    NotSelected { device: String },
    /// The device's integration was left out of this build; `feature` is
    /// the Cargo feature that builds it in (its spec id).
    #[error("device '{device}' is not in this build: it needs the '{feature}' feature")]
    NotBuilt { device: String, feature: String },
    #[error("device '{device}' has no model '{model}'")]
    UnknownModel { device: String, model: String },
    #[error("invalid settings: {message}")]
    InvalidSettings { message: String },
    #[error("cannot resolve host '{host}': {message}")]
    UnresolvableHost { host: String, message: String },
    #[error("device '{device}' cannot be driven: {reason}")]
    NotImplemented { device: String, reason: String },
    /// The core is shutting down (`Core::close_all`).
    #[error("the core is closing")]
    Closing,
}

struct DeviceEntry {
    spec: String,
    model: String,
    tx: mpsc::Sender<SessionMsg>,
    snapshot: Arc<Mutex<DeviceSnapshot>>,
}

/// The core: every open device session, one runtime, one event queue.
pub struct Core {
    runtime: Option<tokio::runtime::Runtime>,
    catalog: Catalog,
    /// Specs this build has but the core was not started with.
    excluded: BTreeSet<String>,
    devices: Mutex<HashMap<DeviceId, DeviceEntry>>,
    next_device: AtomicU64,
    services: Arc<Services>,
    discovery: discovery::Discovery,
    /// Set by `close_all`: no device is opened and no command started.
    closing: AtomicBool,
    /// Commands started and not yet finished.
    in_flight: Arc<AtomicUsize>,
}

/// Counts a command in flight for as long as it lives.
struct InFlight(Arc<AtomicUsize>);

impl Drop for InFlight {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// How a core is set up. Every option has a default suitable for most hosts.
///
/// Build it with [`CoreOptions::new`] and its setters, so options added later
/// break no one:
///
/// ```
/// use meros_integrations::CoreOptions;
///
/// let options = CoreOptions::new()
///     .devices(["sennheiser-ew-dx", "shure-wireless"])
///     .bind_address("192.168.10.5".parse().unwrap());
/// ```
///
/// Bindings take the same options as JSON (`{"devices": [...]}`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CoreOptions {
    /// The local address UDP sockets bind to: the network interface device
    /// traffic uses. Absent means every interface. A venue with separate
    /// control and Dante networks sets its control interface here, so
    /// devices that answer on both are only heard on one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind_address: Option<IpAddr>,
    /// The integrations this core works with: spec ids, vendor groups
    /// (`vendor-sennheiser`: those of the vendor's integrations in the build)
    /// or `all`. Absent means every integration in the build. With a list,
    /// the catalogue holds only those specs, opening any other device fails
    /// with `not_selected`, discovery runs only the protocols that find them,
    /// and only they are reported as discovered. A name the build does not
    /// include fails construction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<Vec<String>>,
}

impl CoreOptions {
    /// The defaults: every interface, every integration in the build.
    pub fn new() -> CoreOptions {
        CoreOptions::default()
    }

    /// Work only with these integrations (see [`CoreOptions::devices`]).
    pub fn devices<I, S>(mut self, devices: I) -> CoreOptions
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.devices = Some(devices.into_iter().map(Into::into).collect());
        self
    }

    /// Bind UDP sockets to this local address (see
    /// [`CoreOptions::bind_address`]).
    pub fn bind_address(mut self, address: IpAddr) -> CoreOptions {
        self.bind_address = Some(address);
        self
    }
}

impl Core {
    /// Start the core on its own runtime, independent of any runtime the host
    /// may have. Every API call can be made from any thread.
    pub fn new() -> std::io::Result<Core> {
        Core::with_options(CoreOptions::default())
    }

    /// Start the core with options. Fails with `InvalidInput` when
    /// `devices` names an integration this build does not include, or is
    /// empty.
    pub fn with_options(options: CoreOptions) -> std::io::Result<Core> {
        let embedded = Catalog::embedded();
        let catalog = match &options.devices {
            None => embedded.clone(),
            Some(ids) => embedded
                .clone()
                .select(ids)
                .map_err(|m| std::io::Error::new(std::io::ErrorKind::InvalidInput, m))?,
        };
        let excluded = embedded
            .devices
            .into_keys()
            .filter(|id| catalog.device(id).is_none())
            .collect();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("meros-integrations")
            .enable_all()
            .build()?;
        let events = Arc::new(
            EventQueue::new(EVENT_CAPACITY)
                .discovering_only(catalog.devices.keys().cloned().collect()),
        );
        Ok(Core {
            runtime: Some(runtime),
            discovery: discovery::Discovery::new(&catalog),
            catalog,
            excluded,
            devices: Mutex::new(HashMap::new()),
            next_device: AtomicU64::new(1),
            services: Arc::new(Services {
                events: events.clone(),
                bind_address: options
                    .bind_address
                    .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)),
                shared_udp: udp::SharedUdp::new(events),
                shared_tcp: Default::default(),
                http: http::HttpClients::new().map_err(std::io::Error::other)?,
                streams: Default::default(),
            }),
            closing: AtomicBool::new(false),
            in_flight: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// Start or stop listening for devices, or scan for them now. Found
    /// devices arrive as `Discovered` events. When to scan is the host's
    /// decision; how a scan is done, and how much traffic it may cost, is the
    /// core's.
    pub fn discover(&self, request: DiscoverRequest) -> Result<(), String> {
        let _runtime = self.runtime().enter();
        self.discovery.handle(&self.services, request)
    }

    /// The discovery protocols this core runs: those that find a device in
    /// its catalogue ([`DISCOVERY_PROTOCOLS`] says which finds which).
    pub fn discovery_protocols(&self) -> &[&'static str] {
        self.discovery.protocols()
    }

    /// The devices this core works with: every spec in the build, or the
    /// ones named in [`CoreOptions::devices`].
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Start a session. The connection is made in the background; watch
    /// `Connection` events or the snapshot for its progress.
    pub fn open(&self, request: OpenRequest) -> Result<DeviceId, OpenError> {
        if self.closing.load(Ordering::SeqCst) {
            return Err(OpenError::Closing);
        }
        let spec = self.catalog.device(&request.device).ok_or_else(|| {
            let device = request.device.clone();
            if self.excluded.contains(&device) {
                OpenError::NotSelected { device }
            } else if let Some(feature) = catalog::feature(&device) {
                OpenError::NotBuilt {
                    device,
                    feature: feature.into(),
                }
            } else {
                OpenError::UnknownDevice { device }
            }
        })?;
        let model = spec
            .model(&request.model)
            .ok_or_else(|| OpenError::UnknownModel {
                device: request.device.clone(),
                model: request.model.clone(),
            })?;
        let settings = catalog::validate(&spec.settings, &request.settings)
            .map_err(|message| OpenError::InvalidSettings { message })?;
        let host = if request.host.trim().is_empty() && spec.core_listens_only() {
            IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)
        } else {
            resolve(&request.host)?
        };

        let context = module::OpenContext {
            host,
            host_name: host_name(&request.host),
            port: request.port,
            model: model.id.clone(),
            channels: model.channels,
            settings,
            monitor: request.monitor,
        };
        let module =
            modules::construct(spec, context).map_err(|reason| OpenError::NotImplemented {
                device: spec.id.clone(),
                reason,
            })?;

        let id = self.next_device.fetch_add(1, Ordering::Relaxed);
        let snapshot = Arc::new(Mutex::new(DeviceSnapshot {
            connection: Connection::Connecting,
            state: serde_json::Value::Object(Default::default()),
            latency_ms: None,
        }));
        let (tx, rx) = mpsc::channel(256);
        let session = Session::new(id, host, module, self.services.clone(), snapshot.clone());
        self.runtime().spawn(session.run(rx));
        self.devices.lock().unwrap().insert(
            id,
            DeviceEntry {
                spec: spec.id.clone(),
                model: model.id.clone(),
                tx,
                snapshot,
            },
        );
        Ok(id)
    }

    /// Run a command. Parameters are validated against the spec, and the
    /// command against the model's `supports` list, before anything is sent.
    pub async fn execute(&self, device: DeviceId, command: &str, params: Params) -> CommandResult {
        if self.closing.load(Ordering::SeqCst) {
            return Err(CommandError::Closed);
        }
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        let _counted = InFlight(self.in_flight.clone());
        let (tx, name, params) = {
            let devices = self.devices.lock().unwrap();
            let entry = devices.get(&device).ok_or(CommandError::Closed)?;
            let spec = self
                .catalog
                .device(&entry.spec)
                .expect("open device has a spec");
            let command_spec =
                spec.commands
                    .get(command)
                    .ok_or_else(|| CommandError::UnknownCommand {
                        command: command.into(),
                    })?;
            let model = spec.model(&entry.model).expect("open device has a model");
            if !model.supports.iter().any(|c| c == command) {
                return Err(CommandError::UnsupportedForModel {
                    command: command.into(),
                    model: model.id.clone(),
                });
            }
            let params = catalog::validate(&command_spec.params, &params)
                .map_err(|message| CommandError::InvalidParams { message })?;
            (entry.tx.clone(), command.to_string(), params)
        };
        let (reply, result) = oneshot::channel();
        tx.send(SessionMsg::Command {
            name,
            params,
            reply,
        })
        .await
        .map_err(|_| CommandError::Closed)?;
        result.await.map_err(|_| CommandError::Closed)?
    }

    /// Run a future on the core's runtime and wait for it, for bindings whose
    /// host has no async runtime. Must not be called from within an async task.
    pub fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.runtime().block_on(future)
    }

    /// Blocking form of [`Core::execute`], for hosts without an async runtime.
    /// Must not be called from within an async task.
    pub fn execute_blocking(
        &self,
        device: DeviceId,
        command: &str,
        params: Params,
    ) -> CommandResult {
        self.runtime()
            .block_on(self.execute(device, command, params))
    }

    pub fn snapshot(&self, device: DeviceId) -> Option<DeviceSnapshot> {
        let devices = self.devices.lock().unwrap();
        devices
            .get(&device)
            .map(|e| e.snapshot.lock().unwrap().clone())
    }

    /// Queued events, without waiting.
    pub fn poll_events(&self, max: usize) -> Vec<Event> {
        self.services.events.drain(max)
    }

    /// Make a pending [`Core::next_events`] return, empty if nothing is queued.
    pub fn interrupt_events(&self) {
        self.services.events.interrupt();
    }

    /// Wait for at least one event.
    pub async fn next_events(&self, max: usize) -> Vec<Event> {
        self.services.events.next(max).await
    }

    /// Watch one of the device's streams, such as a camera's live view. The
    /// stream must be declared in the spec's `streams` for the device's
    /// model. The device produces frames while at least one handle is open;
    /// each handle receives the newest frame, never a backlog. Closing the
    /// device ends every handle.
    pub fn open_stream(&self, device: DeviceId, stream: &str) -> Result<StreamHandle, StreamError> {
        let format = {
            let devices = self.devices.lock().unwrap();
            let entry = devices.get(&device).ok_or(StreamError::Closed)?;
            let spec = self
                .catalog
                .device(&entry.spec)
                .expect("open device has a spec");
            let declared = spec
                .streams
                .get(stream)
                .ok_or_else(|| StreamError::UnknownStream {
                    stream: stream.into(),
                })?;
            if let Some(models) = &declared.models {
                if !models.iter().any(|m| m == &entry.model) {
                    return Err(StreamError::UnsupportedForModel {
                        stream: stream.into(),
                        model: entry.model.clone(),
                    });
                }
            }
            declared.format.clone()
        };
        self.services
            .streams
            .watch(device, stream, format)
            .ok_or(StreamError::Closed)
    }

    /// Stop watching a stream; the same as dropping the handle.
    pub fn close_stream(&self, handle: StreamHandle) {
        handle.close();
    }

    /// End a session cleanly. Pending commands fail with `Closed`.
    pub async fn close(&self, device: DeviceId) {
        let entry = self.devices.lock().unwrap().remove(&device);
        if let Some(entry) = entry {
            let (done, finished) = oneshot::channel();
            if entry.tx.send(SessionMsg::Close { done }).await.is_ok() {
                let _ = finished.await;
            }
        }
    }

    /// Shut down cleanly, for a host that is about to exit: refuse new
    /// devices and commands, give commands already sent up to `grace` to
    /// finish, then close every device as [`Core::close`] does, so each
    /// module ends what it started on the device (subscriptions, metering,
    /// sessions) and its last messages are written. Commands still waiting
    /// after `grace` fail with `Closed`. Returns once every device is
    /// closed; the core accepts nothing afterwards.
    pub async fn close_all(&self, grace: Duration) {
        self.closing.store(true, Ordering::SeqCst);
        let deadline = tokio::time::Instant::now() + grace;
        while self.in_flight.load(Ordering::SeqCst) > 0 && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let devices: Vec<DeviceId> = self.devices.lock().unwrap().keys().copied().collect();
        futures_util::future::join_all(devices.into_iter().map(|d| self.close(d))).await;
    }

    /// Blocking form of [`Core::close_all`], for hosts without an async
    /// runtime. Must not be called from within an async task.
    pub fn close_all_blocking(&self, grace: Duration) {
        self.runtime().block_on(self.close_all(grace));
    }

    fn runtime(&self) -> &tokio::runtime::Runtime {
        self.runtime.as_ref().expect("runtime present until drop")
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        // Dropping a runtime blocks, which panics inside an async context. The
        // sessions hold nothing that needs a graceful end beyond their sockets.
        if let Some(rt) = self.runtime.take() {
            rt.shutdown_background();
        }
    }
}

/// The host as a name, when it is not an address.
fn host_name(host: &str) -> Option<String> {
    let host = host.trim();
    (!host.is_empty() && host.parse::<IpAddr>().is_err()).then(|| host.to_string())
}

fn resolve(host: &str) -> Result<IpAddr, OpenError> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(ip);
    }
    (host, 0)
        .to_socket_addrs()
        .map_err(|e| OpenError::UnresolvableHost {
            host: host.into(),
            message: e.to_string(),
        })?
        .find(|a| a.is_ipv4())
        .map(|a| a.ip())
        .ok_or_else(|| OpenError::UnresolvableHost {
            host: host.into(),
            message: "no IPv4 address".into(),
        })
}
