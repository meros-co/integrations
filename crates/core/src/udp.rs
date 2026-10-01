//! Fixed UDP ports shared by several sessions.
//!
//! Some devices send to a fixed port rather than back to the sender's port.
//! Sennheiser MCP uses 53212 in both directions (TI 1254, "Protocol basics"), so
//! every G3/G4 session has to share one socket bound to it, with inbound
//! datagrams routed to the session for their source address.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};

use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use crate::events::{Event, EventQueue};
use crate::module::{Key, Level};
use crate::session::{DeviceId, Inbound};

/// Large buffers because one shared socket carries telemetry from every device
/// on it; at OS defaults a burst can overflow the receive queue and drop live
/// telemetry, which reads as devices going offline.
pub(crate) const RECV_BUFFER: usize = 8 * 1024 * 1024;
pub(crate) const SEND_BUFFER: usize = 4 * 1024 * 1024;
/// Smallest size still worth asking for when the OS refuses a larger one.
const BUFFER_FLOOR: usize = 256 * 1024;

/// Buffer sizes the OS actually granted, read back after asking. Linux clamps
/// the request to net.core.rmem_max (about 208 KB by default) unless the
/// process holds CAP_NET_ADMIN, so the granted size is not the requested one.
/// Linux also reports double what it will use, for bookkeeping overhead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Buffers {
    pub receive: usize,
    pub send: usize,
}

struct Route {
    key: Key,
    device: DeviceId,
    inbound: mpsc::Sender<Inbound>,
    /// Datagrams dropped because the session's queue was full, not yet
    /// reported.
    dropped: u64,
    reported: Option<std::time::Instant>,
}

/// Where datagrams from an address no session claims go: discovery.
pub(crate) type Fallback = mpsc::Sender<(SocketAddr, Vec<u8>)>;

struct Port {
    socket: Arc<UdpSocket>,
    routes: Arc<Mutex<HashMap<IpAddr, Route>>>,
    fallback: Arc<Mutex<Option<Fallback>>>,
}

pub(crate) struct SharedUdp {
    ports: Mutex<HashMap<u16, Port>>,
    events: Arc<EventQueue>,
}

impl SharedUdp {
    pub(crate) fn new(events: Arc<EventQueue>) -> SharedUdp {
        SharedUdp {
            ports: Mutex::new(HashMap::new()),
            events,
        }
    }

    /// The port's socket, binding it on first use. Returns the granted buffer
    /// sizes when this call bound it.
    fn port<'a>(
        &self,
        ports: &'a mut HashMap<u16, Port>,
        bind_address: IpAddr,
        port: u16,
    ) -> Result<(&'a mut Port, Option<Buffers>), String> {
        let mut bound = None;
        let entry = match ports.entry(port) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                let (socket, buffers) =
                    bind_shared(bind_address, port).map_err(|e| format!("bind :{port}: {e}"))?;
                let socket = Arc::new(socket);
                let routes: Arc<Mutex<HashMap<IpAddr, Route>>> = Default::default();
                let fallback: Arc<Mutex<Option<Fallback>>> = Default::default();
                spawn_router(
                    socket.clone(),
                    routes.clone(),
                    fallback.clone(),
                    self.events.clone(),
                );
                bound = Some(buffers);
                e.insert(Port {
                    socket,
                    routes,
                    fallback,
                })
            }
        };
        Ok((entry, bound))
    }

    /// Send datagrams from addresses no session claims to `fallback`, binding
    /// the port if no session has. Discovery listens this way, on the same
    /// socket as the devices it finds.
    pub(crate) fn set_fallback(
        &self,
        bind_address: IpAddr,
        port: u16,
        fallback: Option<Fallback>,
    ) -> Result<(), String> {
        let mut ports = self.ports.lock().unwrap();
        let (entry, _) = self.port(&mut ports, bind_address, port)?;
        *entry.fallback.lock().unwrap() = fallback;
        Ok(())
    }

    /// Route datagrams from `host` on `port` to a session, binding the port on
    /// first use. Every session on a port shares its one socket: with
    /// SO_REUSEADDR and no SO_REUSEPORT, a datagram reaches exactly one socket
    /// bound to the port, and which one is undefined. Returns the granted
    /// buffer sizes when this call bound the port. Must be called from within
    /// the runtime.
    pub(crate) fn register(
        &self,
        bind_address: IpAddr,
        port: u16,
        host: IpAddr,
        key: Key,
        device: DeviceId,
        inbound: mpsc::Sender<Inbound>,
    ) -> Result<Option<Buffers>, String> {
        let mut ports = self.ports.lock().unwrap();
        let (entry, bound) = self.port(&mut ports, bind_address, port)?;
        let mut routes = entry.routes.lock().unwrap();
        if routes.contains_key(&host) {
            return Err(format!(
                "{host} already has a session on shared port {port}"
            ));
        }
        routes.insert(
            host,
            Route {
                key,
                device,
                inbound,
                dropped: 0,
                reported: None,
            },
        );
        Ok(bound)
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

fn bind_shared(address: IpAddr, port: u16) -> std::io::Result<(UdpSocket, Buffers)> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    let buffers = size_buffers(&socket)?;
    // Discovery broadcasts its probe on the same socket.
    socket.set_broadcast(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&SocketAddr::new(address, port).into())?;
    Ok((UdpSocket::from_std(socket.into())?, buffers))
}

/// Ask for the large buffers, halving on refusal (macOS rejects a request
/// above kern.ipc.maxsockbuf outright rather than clamping), then read back
/// what was granted.
fn size_buffers(socket: &Socket) -> std::io::Result<Buffers> {
    let mut ask = RECV_BUFFER;
    while socket.set_recv_buffer_size(ask).is_err() && ask > BUFFER_FLOOR {
        ask /= 2;
    }
    let mut ask = SEND_BUFFER;
    while socket.set_send_buffer_size(ask).is_err() && ask > BUFFER_FLOOR {
        ask /= 2;
    }
    Ok(Buffers {
        receive: socket.recv_buffer_size()?,
        send: socket.send_buffer_size()?,
    })
}

fn spawn_router(
    socket: Arc<UdpSocket>,
    routes: Arc<Mutex<HashMap<IpAddr, Route>>>,
    fallback: Arc<Mutex<Option<Fallback>>>,
    events: Arc<EventQueue>,
) {
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65_536];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((n, from)) => {
                    let data = buf[..n].to_vec();
                    let mut routes = routes.lock().unwrap();
                    match routes.get_mut(&from.ip()) {
                        Some(route) => {
                            let sent = route.inbound.try_send(Inbound::Datagram {
                                socket: route.key,
                                from,
                                data,
                            });
                            if sent.is_err() {
                                route.dropped += 1;
                            }
                            report_drops(route, from.ip(), &events);
                        }
                        None => {
                            drop(routes);
                            if let Some(f) = fallback.lock().unwrap().as_ref() {
                                let _ = f.try_send((from, data));
                            }
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => continue,
                Err(_) => return,
            }
        }
    });
}

/// Datagrams dropped because a session fell behind are lost telemetry with no
/// other symptom, so they are reported: at most once a second per device.
fn report_drops(route: &mut Route, from: IpAddr, events: &EventQueue) {
    if route.dropped == 0 {
        return;
    }
    let now = std::time::Instant::now();
    if route
        .reported
        .is_some_and(|at| now.duration_since(at) < std::time::Duration::from_secs(1))
    {
        return;
    }
    events.push(Event::Log {
        device: route.device,
        level: Level::Warning,
        message: format!(
            "{} datagram(s) from {from} dropped: the session fell behind",
            route.dropped
        ),
    });
    route.dropped = 0;
    route.reported = Some(now);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const LOCAL: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    /// At the OS default a sweep overflows the receive queue and telemetry from
    /// devices already connected is dropped (RFDeck review item N.1).
    #[tokio::test]
    async fn buffers_are_enlarged_and_the_granted_size_is_read_back() {
        let plain = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
        let default = Buffers {
            receive: plain.recv_buffer_size().unwrap(),
            send: plain.send_buffer_size().unwrap(),
        };
        let (socket, granted) = bind_shared(LOCAL, 0).unwrap();
        assert!(
            granted.receive > default.receive,
            "granted {granted:?}, default {default:?}"
        );
        let raw = socket2::SockRef::from(&socket);
        assert_eq!(granted.receive, raw.recv_buffer_size().unwrap());
        assert_eq!(granted.send, raw.send_buffer_size().unwrap());
    }

    /// A second socket on the port would receive an undefined share of the
    /// datagrams (RFDeck review item N.3).
    #[tokio::test]
    async fn every_session_on_a_port_shares_one_socket() {
        let shared = SharedUdp::new(Arc::new(EventQueue::new(16)));
        let (tx, _rx) = mpsc::channel(8);
        let a = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let b = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        let first = shared.register(LOCAL, 0, a, "mcp", 1, tx.clone()).unwrap();
        assert!(
            first.is_some(),
            "the first session binds and reports buffers"
        );
        let second = shared.register(LOCAL, 0, b, "mcp", 2, tx.clone()).unwrap();
        assert!(second.is_none(), "the second session binds nothing");
        assert_eq!(shared.ports.lock().unwrap().len(), 1);
        assert!(shared.register(LOCAL, 0, a, "mcp", 1, tx).is_err());
    }
}
