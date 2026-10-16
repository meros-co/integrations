//! TCP ports the core listens on, for devices that connect to us rather than
//! the other way round (a switcher sending TSL UMD V5.0 over TCP).
//!
//! Like the shared UDP ports: one listener per port, shared by every session
//! that asks for it, with each accepted connection handed to the session for
//! its peer's address. A connection from an address no session claims is
//! closed at once.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::module::Key;
use crate::session::Inbound;

struct Route {
    key: Key,
    inbound: mpsc::Sender<Inbound>,
}

struct Port {
    routes: Arc<Mutex<HashMap<IpAddr, Route>>>,
    task: JoinHandle<()>,
}

type Ports = Arc<Mutex<HashMap<u16, Port>>>;

#[derive(Default)]
pub(crate) struct SharedTcp {
    ports: Ports,
}

impl SharedTcp {
    /// Hand connections from `host` on `port` to a session, listening on the
    /// port on first use. Must be called from within the runtime.
    pub(crate) fn register(
        &self,
        bind_address: IpAddr,
        port: u16,
        host: IpAddr,
        key: Key,
        inbound: mpsc::Sender<Inbound>,
    ) -> Result<(), String> {
        let mut ports = self.ports.lock().unwrap();
        let entry = match ports.entry(port) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                let listener =
                    bind(bind_address, port).map_err(|e| format!("listen :{port}: {e}"))?;
                let routes: Arc<Mutex<HashMap<IpAddr, Route>>> = Default::default();
                let task = spawn_acceptor(listener, routes.clone(), (self.ports.clone(), port));
                e.insert(Port { routes, task })
            }
        };
        let mut routes = entry.routes.lock().unwrap();
        if routes.contains_key(&host) {
            return Err(format!(
                "{host} already has a session listening on TCP port {port}"
            ));
        }
        routes.insert(host, Route { key, inbound });
        Ok(())
    }

    /// Stop routing `host`; the last session on a port closes its listener.
    pub(crate) fn unregister(&self, port: u16, host: IpAddr) {
        let mut ports = self.ports.lock().unwrap();
        let empty = match ports.get(&port) {
            Some(p) => {
                let mut routes = p.routes.lock().unwrap();
                routes.remove(&host);
                routes.is_empty()
            }
            None => false,
        };
        if empty {
            if let Some(p) = ports.remove(&port) {
                p.task.abort();
            }
        }
    }
}

fn bind(address: IpAddr, port: u16) -> std::io::Result<TcpListener> {
    let listener = std::net::TcpListener::bind(SocketAddr::new(address, port))?;
    listener.set_nonblocking(true)?;
    TcpListener::from_std(listener)
}

/// Consecutive accept errors, 100 ms apart, after which the listener is
/// taken to have failed for good rather than to be short of descriptors.
const FAILED_AFTER: u32 = 20;

fn spawn_acceptor(
    listener: TcpListener,
    routes: Arc<Mutex<HashMap<IpAddr, Route>>>,
    (ports, port): (Ports, u16),
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut errors = 0;
        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(accepted) => {
                    errors = 0;
                    accepted
                }
                // A connection that went away before it was accepted.
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionAborted => continue,
                // Out of descriptors and the like: wait rather than spin. A
                // listener that keeps failing (a suspended mobile app's
                // sockets can be reclaimed by the OS) is closed, and every
                // session on it told, so each can listen again.
                Err(e) => {
                    errors += 1;
                    if errors >= FAILED_AFTER {
                        close_failed(&ports, port, &routes, &e.to_string()).await;
                        return;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    continue;
                }
            };
            let target = routes
                .lock()
                .unwrap()
                .get(&peer.ip())
                .map(|r| (r.key, r.inbound.clone()));
            // Unclaimed: dropping the stream closes it.
            if let Some((key, inbound)) = target {
                let _ = inbound
                    .send(Inbound::Accepted {
                        socket: key,
                        stream,
                    })
                    .await;
            }
        }
    })
}

async fn close_failed(
    ports: &Ports,
    port: u16,
    routes: &Arc<Mutex<HashMap<IpAddr, Route>>>,
    error: &str,
) {
    {
        let mut ports = ports.lock().unwrap();
        if ports
            .get(&port)
            .is_some_and(|p| Arc::ptr_eq(&p.routes, routes))
        {
            ports.remove(&port);
        }
    }
    let targets: Vec<(Key, mpsc::Sender<Inbound>)> = routes
        .lock()
        .unwrap()
        .values()
        .map(|r| (r.key, r.inbound.clone()))
        .collect();
    for (socket, inbound) in targets {
        let _ = inbound
            .send(Inbound::SocketError {
                socket,
                message: format!("listening on TCP port {port} failed: {error}"),
            })
            .await;
    }
}
