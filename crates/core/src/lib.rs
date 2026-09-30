//! One implementation of control and telemetry for third-party production
//! devices, shared by every consumer.
//!
//! ```no_run
//! use meros_integrations::{Core, OpenRequest};
//! use serde_json::json;
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let core = Core::new()?;
//! let device = core.open(OpenRequest {
//!     device: "sennheiser-ew-g3-g4".into(),
//!     model: "em-300-500-g4".into(),
//!     host: "192.168.1.40".into(),
//!     port: None,
//!     settings: Default::default(),
//! })?;
//! let outcome = core
//!     .execute(device, "mute", json!({"muted": true}).as_object().unwrap().clone())
//!     .await?;
//! # Ok(()) }
//! ```

pub mod catalog;
mod engine;
pub mod events;
mod http;
pub mod json;
pub mod module;
mod modules;
mod session;
pub mod sse;
mod tcp;
mod udp;

use std::collections::HashMap;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

pub use catalog::{Catalog, Params};
pub use events::Event;
pub use module::{CommandError, CommandResult, Connection, Outcome};
pub use session::{DeviceId, DeviceSnapshot};

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
    /// IP address or hostname. A hostname is resolved once, when opened.
    pub host: String,
    /// The device's port, when it is not the protocol's default: many devices
    /// let an operator change it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Per-installation settings declared by the spec, such as passwords.
    #[serde(default)]
    pub settings: Params,
}

#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum OpenError {
    #[error("unknown device '{device}'")]
    UnknownDevice { device: String },
    #[error("device '{device}' has no model '{model}'")]
    UnknownModel { device: String, model: String },
    #[error("invalid settings: {message}")]
    InvalidSettings { message: String },
    #[error("cannot resolve host '{host}': {message}")]
    UnresolvableHost { host: String, message: String },
    #[error("device '{device}' cannot be driven: {reason}")]
    NotImplemented { device: String, reason: String },
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
    devices: Mutex<HashMap<DeviceId, DeviceEntry>>,
    next_device: AtomicU64,
    services: Arc<Services>,
}

impl Core {
    /// Start the core on its own runtime, independent of any runtime the host
    /// may have. Every API call can be made from any thread.
    pub fn new() -> std::io::Result<Core> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("meros-integrations")
            .enable_all()
            .build()?;
        Ok(Core {
            runtime: Some(runtime),
            catalog: Catalog::embedded(),
            devices: Mutex::new(HashMap::new()),
            next_device: AtomicU64::new(1),
            services: Arc::new(Services {
                events: Arc::new(EventQueue::new(EVENT_CAPACITY)),
                shared_udp: Default::default(),
                http: http::HttpClients::new().map_err(std::io::Error::other)?,
            }),
        })
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Start a session. The connection is made in the background; watch
    /// `Connection` events or the snapshot for its progress.
    pub fn open(&self, request: OpenRequest) -> Result<DeviceId, OpenError> {
        let spec =
            self.catalog
                .device(&request.device)
                .ok_or_else(|| OpenError::UnknownDevice {
                    device: request.device.clone(),
                })?;
        let model = spec
            .model(&request.model)
            .ok_or_else(|| OpenError::UnknownModel {
                device: request.device.clone(),
                model: request.model.clone(),
            })?;
        let settings = catalog::validate(&spec.settings, &request.settings)
            .map_err(|message| OpenError::InvalidSettings { message })?;
        let host = resolve(&request.host)?;

        let context = module::OpenContext {
            host,
            port: request.port,
            model: model.id.clone(),
            channels: model.channels,
            settings,
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
