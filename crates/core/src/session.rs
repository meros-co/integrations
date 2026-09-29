//! Runs one device: feeds its module, and carries out the module's actions.
//!
//! This is the only code in the core that touches sockets and clocks for a
//! device. Everything protocol-specific is in the module.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::catalog::Params;
use crate::events::{Event, EventQueue};
use crate::http::HttpClients;
use crate::module::{
    Action, Bind, CommandError, CommandId, CommandResult, Connection, Cx, HttpResponse, Key,
    Module, RequestId, SseInput,
};
use crate::udp::SharedUdp;

pub type DeviceId = u64;

/// A module that never completes a command is a module bug, not a slow device;
/// modules enforce their own protocol timeouts well inside this.
const COMMAND_SAFETY_TIMEOUT: Duration = Duration::from_secs(30);

/// Alive events are rate-limited so a device streaming telemetry does not flood
/// consumers with liveness.
const ALIVE_EVERY: Duration = Duration::from_secs(1);

pub(crate) enum SessionMsg {
    Command {
        name: String,
        params: Params,
        reply: oneshot::Sender<CommandResult>,
    },
    Close {
        done: oneshot::Sender<()>,
    },
}

pub(crate) enum Inbound {
    Datagram {
        socket: Key,
        from: SocketAddr,
        data: Vec<u8>,
    },
    SocketError {
        socket: Key,
        message: String,
    },
    Http {
        id: RequestId,
        result: Result<HttpResponse, String>,
    },
    Sse {
        stream: Key,
        generation: u64,
        input: SseInput,
    },
}

/// What every session shares.
pub(crate) struct Services {
    pub(crate) events: Arc<EventQueue>,
    pub(crate) shared_udp: SharedUdp,
    pub(crate) http: HttpClients,
}

/// What any consumer can read about a device without asking it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceSnapshot {
    pub connection: Connection,
    pub state: Value,
}

enum Socket {
    Own {
        socket: Arc<UdpSocket>,
        reader: JoinHandle<()>,
    },
    Shared {
        port: u16,
    },
}

struct Pending {
    reply: oneshot::Sender<CommandResult>,
    deadline: Instant,
}

struct Stream {
    generation: u64,
    task: JoinHandle<()>,
}

pub(crate) struct Session {
    device: DeviceId,
    host: IpAddr,
    module: Box<dyn Module>,
    started: Instant,
    timers: HashMap<Key, Instant>,
    sockets: HashMap<Key, Socket>,
    streams: HashMap<Key, Stream>,
    next_generation: u64,
    pending: HashMap<CommandId, Pending>,
    next_command: CommandId,
    inbound_tx: mpsc::Sender<Inbound>,
    inbound_rx: mpsc::Receiver<Inbound>,
    services: Arc<Services>,
    snapshot: Arc<Mutex<DeviceSnapshot>>,
    last_alive: Option<Instant>,
}

impl Session {
    pub(crate) fn new(
        device: DeviceId,
        host: IpAddr,
        module: Box<dyn Module>,
        services: Arc<Services>,
        snapshot: Arc<Mutex<DeviceSnapshot>>,
    ) -> Session {
        let (inbound_tx, inbound_rx) = mpsc::channel(4096);
        Session {
            device,
            host,
            module,
            started: Instant::now(),
            timers: HashMap::new(),
            sockets: HashMap::new(),
            streams: HashMap::new(),
            next_generation: 1,
            pending: HashMap::new(),
            next_command: 1,
            inbound_tx,
            inbound_rx,
            services,
            snapshot,
            last_alive: None,
        }
    }

    fn cx(&self) -> Cx {
        Cx::new(self.started.elapsed().as_millis() as u64)
    }

    pub(crate) async fn run(mut self, mut messages: mpsc::Receiver<SessionMsg>) {
        let mut cx = self.cx();
        self.module.start(&mut cx);
        self.apply(cx.take()).await;

        loop {
            let wake = self.next_wake();
            tokio::select! {
                msg = messages.recv() => match msg {
                    Some(SessionMsg::Command { name, params, reply }) => {
                        let id = self.next_command;
                        self.next_command += 1;
                        self.pending.insert(id, Pending {
                            reply,
                            deadline: Instant::now() + COMMAND_SAFETY_TIMEOUT,
                        });
                        let mut cx = self.cx();
                        self.module.command(&mut cx, id, &name, &params);
                        self.apply(cx.take()).await;
                    }
                    Some(SessionMsg::Close { done }) => {
                        self.shutdown().await;
                        let _ = done.send(());
                        return;
                    }
                    None => {
                        self.shutdown().await;
                        return;
                    }
                },
                Some(inbound) = self.inbound_rx.recv() => {
                    let mut cx = self.cx();
                    match inbound {
                        Inbound::Datagram { socket, from, data } => {
                            self.module.datagram(&mut cx, socket, from, &data);
                        }
                        Inbound::SocketError { socket, message } => {
                            self.drop_socket(socket);
                            self.module.socket_error(&mut cx, socket, &message);
                        }
                        Inbound::Http { id, result } => {
                            self.module.http_response(&mut cx, id, result);
                        }
                        Inbound::Sse { stream, generation, input } => {
                            // Inputs from a stream since replaced or closed are
                            // not this stream's.
                            let current = self.streams.get(stream).map(|s| s.generation);
                            if current != Some(generation) {
                                continue;
                            }
                            if matches!(input, SseInput::Closed { .. }) {
                                self.streams.remove(stream);
                            }
                            self.module.sse(&mut cx, stream, input);
                        }
                    }
                    self.apply(cx.take()).await;
                }
                _ = sleep_until(wake) => self.fire_due().await,
            }
        }
    }

    fn next_wake(&self) -> Option<Instant> {
        let timer = self.timers.values().min().copied();
        let deadline = self.pending.values().map(|p| p.deadline).min();
        match (timer, deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    async fn fire_due(&mut self) {
        let now = Instant::now();

        let expired: Vec<CommandId> = self
            .pending
            .iter()
            .filter(|(_, p)| p.deadline <= now)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            if let Some(p) = self.pending.remove(&id) {
                let _ = p.reply.send(Err(CommandError::Timeout));
            }
        }

        // Earliest first, one at a time: a timer callback may reschedule or
        // cancel another that is also due.
        loop {
            let due = self
                .timers
                .iter()
                .filter(|(_, at)| **at <= now)
                .min_by_key(|(_, at)| **at)
                .map(|(key, _)| *key);
            let Some(key) = due else { break };
            self.timers.remove(key);
            let mut cx = self.cx();
            self.module.timer(&mut cx, key);
            self.apply(cx.take()).await;
        }
    }

    async fn apply(&mut self, actions: Vec<Action>) {
        for action in actions {
            match action {
                Action::UdpOpen { socket, bind } => {
                    if let Err(message) = self.open_socket(socket, bind).await {
                        let _ = self
                            .inbound_tx
                            .try_send(Inbound::SocketError { socket, message });
                    }
                }
                Action::UdpSend { socket, to, data } => self.send(socket, to, data).await,
                Action::UdpClose { socket } => self.drop_socket(socket),
                Action::Http { id, request } => {
                    self.services
                        .http
                        .spawn_request(id, &request, self.inbound_tx.clone());
                }
                Action::SseOpen { stream, request } => {
                    self.close_stream(stream);
                    let generation = self.next_generation;
                    self.next_generation += 1;
                    let task = self.services.http.spawn_stream(
                        stream,
                        generation,
                        &request,
                        self.inbound_tx.clone(),
                    );
                    self.streams.insert(stream, Stream { generation, task });
                }
                Action::SseClose { stream } => self.close_stream(stream),
                Action::SetTimer { key, after } => {
                    self.timers
                        .insert(key, Instant::now() + Duration::from_millis(after));
                }
                Action::CancelTimer { key } => {
                    self.timers.remove(key);
                }
                Action::Complete { id, result } => {
                    if let Some(p) = self.pending.remove(&id) {
                        let _ = p.reply.send(result);
                    }
                }
                Action::State(patch) => {
                    {
                        let mut snap = self.snapshot.lock().unwrap();
                        merge_patch(&mut snap.state, &patch);
                    }
                    self.services.events.push(Event::State {
                        device: self.device,
                        patch,
                    });
                }
                Action::Connection(connection) => {
                    let changed = {
                        let mut snap = self.snapshot.lock().unwrap();
                        let changed = snap.connection != connection;
                        snap.connection = connection.clone();
                        changed
                    };
                    if changed {
                        self.services.events.push(Event::Connection {
                            device: self.device,
                            connection,
                        });
                    }
                }
                Action::Alive => {
                    let now = Instant::now();
                    if self.last_alive.is_none_or(|t| now - t >= ALIVE_EVERY) {
                        self.last_alive = Some(now);
                        self.services.events.push(Event::Alive {
                            device: self.device,
                        });
                    }
                }
                Action::Log { level, message } => {
                    self.services.events.push(Event::Log {
                        device: self.device,
                        level,
                        message,
                    });
                }
            }
        }
    }

    async fn open_socket(&mut self, key: Key, bind: Bind) -> Result<(), String> {
        self.drop_socket(key);
        match bind {
            Bind::Ephemeral => {
                let socket = UdpSocket::bind(("0.0.0.0", 0))
                    .await
                    .map_err(|e| format!("bind: {e}"))?;
                let socket = Arc::new(socket);
                let reader = spawn_reader(key, socket.clone(), self.host, self.inbound_tx.clone());
                self.sockets.insert(key, Socket::Own { socket, reader });
            }
            Bind::Shared(port) => {
                self.services
                    .shared_udp
                    .register(port, self.host, key, self.inbound_tx.clone())?;
                self.sockets.insert(key, Socket::Shared { port });
            }
        }
        Ok(())
    }

    async fn send(&mut self, key: Key, to: SocketAddr, data: Vec<u8>) {
        let result = match self.sockets.get(&key) {
            Some(Socket::Own { socket, .. }) => socket.send_to(&data, to).await.map(|_| ()),
            Some(Socket::Shared { port }) => self.services.shared_udp.send(*port, to, &data).await,
            None => return,
        };
        if let Err(e) = result {
            let mut cx = self.cx();
            cx.log(
                crate::module::Level::Debug,
                format!("send to {to} failed: {e}"),
            );
            self.module.socket_error(&mut cx, key, &e.to_string());
            self.drop_socket(key);
            Box::pin(self.apply(cx.take())).await;
        }
    }

    fn close_stream(&mut self, key: Key) {
        if let Some(stream) = self.streams.remove(key) {
            stream.task.abort();
        }
    }

    fn drop_socket(&mut self, key: Key) {
        match self.sockets.remove(key) {
            Some(Socket::Own { reader, .. }) => reader.abort(),
            Some(Socket::Shared { port }) => self.services.shared_udp.unregister(port, self.host),
            None => {}
        }
    }

    async fn shutdown(&mut self) {
        let mut cx = self.cx();
        self.module.stop(&mut cx);
        // Sends only: a stopping module may cancel subscriptions, but nothing
        // else it asks for can matter any more.
        for action in cx.take() {
            if let Action::UdpSend { socket, to, data } = action {
                self.send(socket, to, data).await;
            }
        }
        for (_, p) in self.pending.drain() {
            let _ = p.reply.send(Err(CommandError::Closed));
        }
        let keys: Vec<Key> = self.sockets.keys().copied().collect();
        for key in keys {
            self.drop_socket(key);
        }
        let streams: Vec<Key> = self.streams.keys().copied().collect();
        for key in streams {
            self.close_stream(key);
        }
        self.services.events.push(Event::Closed {
            device: self.device,
        });
    }
}

async fn sleep_until(at: Option<Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

/// Reads an owned socket, passing on only datagrams from the device's own
/// address: anything else arriving on the port is not this device's.
fn spawn_reader(
    key: Key,
    socket: Arc<UdpSocket>,
    host: IpAddr,
    inbound: mpsc::Sender<Inbound>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65_536];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((n, from)) => {
                    if from.ip() != host {
                        continue;
                    }
                    let msg = Inbound::Datagram {
                        socket: key,
                        from,
                        data: buf[..n].to_vec(),
                    };
                    if inbound.send(msg).await.is_err() {
                        return;
                    }
                }
                // Windows reports an ICMP port-unreachable from a previous
                // send as a receive error (WSAECONNRESET). It says nothing
                // about this socket, so it is not fatal.
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => continue,
                Err(e) => {
                    let _ = inbound
                        .send(Inbound::SocketError {
                            socket: key,
                            message: e.to_string(),
                        })
                        .await;
                    return;
                }
            }
        }
    })
}

/// RFC 7386 JSON merge patch.
pub(crate) fn merge_patch(target: &mut Value, patch: &Value) {
    match patch {
        Value::Object(patch_map) => {
            if !target.is_object() {
                *target = Value::Object(Default::default());
            }
            let map = target.as_object_mut().unwrap();
            for (k, v) in patch_map {
                if v.is_null() {
                    map.remove(k);
                } else {
                    merge_patch(map.entry(k.clone()).or_insert(Value::Null), v);
                }
            }
        }
        other => *target = other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::merge_patch;
    use serde_json::json;

    #[test]
    fn merge_patch_follows_rfc_7386() {
        let mut v = json!({"a": {"b": 1, "c": 2}, "d": 3});
        merge_patch(&mut v, &json!({"a": {"b": null, "e": 4}, "d": [1]}));
        assert_eq!(v, json!({"a": {"c": 2, "e": 4}, "d": [1]}));
    }
}
