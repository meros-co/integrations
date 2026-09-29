//! Fixed UDP ports shared by several sessions.
//!
//! Some devices send to a fixed port rather than back to the sender's port.
//! Sennheiser MCP uses 53212 in both directions (TI 1254, "Protocol basics"), so
//! every G3/G4 session has to share one socket bound to it, with inbound
//! datagrams routed to the session for their source address.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::module::Key;
use crate::session::Inbound;

/// Large buffers because one shared socket carries telemetry from every device
/// on it; at OS defaults a burst can overflow the receive queue and drop live
/// telemetry, which reads as devices going offline.
const RECV_BUFFER: usize = 8 * 1024 * 1024;
const SEND_BUFFER: usize = 4 * 1024 * 1024;

struct Route {
    key: Key,
    inbound: mpsc::Sender<Inbound>,
}

struct Port {
    socket: Arc<UdpSocket>,
    routes: Arc<Mutex<HashMap<IpAddr, Route>>>,
}

#[derive(Default)]
pub(crate) struct SharedUdp {
    ports: Mutex<HashMap<u16, Port>>,
}

impl SharedUdp {
    /// Route datagrams from `host` on `port` to a session, binding the port on
    /// first use. Must be called from within the runtime.
    pub(crate) fn register(
        &self,
        port: u16,
        host: IpAddr,
        key: Key,
        inbound: mpsc::Sender<Inbound>,
    ) -> Result<(), String> {
        let mut ports = self.ports.lock().unwrap();
        let entry = match ports.entry(port) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                let socket = Arc::new(bind_shared(port).map_err(|e| format!("bind :{port}: {e}"))?);
                let routes: Arc<Mutex<HashMap<IpAddr, Route>>> = Default::default();
                spawn_router(socket.clone(), routes.clone());
                e.insert(Port { socket, routes })
            }
        };
        let mut routes = entry.routes.lock().unwrap();
        if routes.contains_key(&host) {
            return Err(format!(
                "{host} already has a session on shared port {port}"
            ));
        }
        routes.insert(host, Route { key, inbound });
        Ok(())
    }

    pub(crate) fn unregister(&self, port: u16, host: IpAddr) {
        if let Some(p) = self.ports.lock().unwrap().get(&port) {
            p.routes.lock().unwrap().remove(&host);
        }
    }

    pub(crate) async fn send(&self, port: u16, to: SocketAddr, data: &[u8]) -> std::io::Result<()> {
        let socket = match self.ports.lock().unwrap().get(&port) {
            Some(p) => p.socket.clone(),
            None => return Err(std::io::Error::other("shared port not open")),
        };
        socket.send_to(data, to).await.map(|_| ())
    }
}

fn bind_shared(port: u16) -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    let _ = socket.set_recv_buffer_size(RECV_BUFFER);
    let _ = socket.set_send_buffer_size(SEND_BUFFER);
    socket.set_nonblocking(true)?;
    socket.bind(&SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port).into())?;
    UdpSocket::from_std(socket.into())
}

fn spawn_router(socket: Arc<UdpSocket>, routes: Arc<Mutex<HashMap<IpAddr, Route>>>) {
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65_536];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((n, from)) => {
                    let target = routes
                        .lock()
                        .unwrap()
                        .get(&from.ip())
                        .map(|r| (r.key, r.inbound.clone()));
                    if let Some((key, inbound)) = target {
                        let _ = inbound.try_send(Inbound::Datagram {
                            socket: key,
                            from,
                            data: buf[..n].to_vec(),
                        });
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => continue,
                Err(_) => return,
            }
        }
    });
}
