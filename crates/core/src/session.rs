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
    Action, Bind, CommandError, CommandId, CommandResult, Connection, Cx, HttpResponse, Key, Level,
    Module, RequestId, SseInput, TcpInput, WsInput,
};
use crate::udp::{Buffers, SharedUdp, RECV_BUFFER, SEND_BUFFER};

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
    Tcp {
        socket: Key,
        generation: u64,
        input: TcpInput,
    },
    Ws {
        socket: Key,
        generation: u64,
        input: WsInput,
    },
    /// A file this session writes was closed, or failed.
    File {
        file: Key,
        generation: u64,
        input: crate::module::FileInput,
    },
    /// A connection the device opened to a port this session listens on.
    Accepted {
        socket: Key,
        stream: tokio::net::TcpStream,
    },
}

/// What every session shares.
pub(crate) struct Services {
    pub(crate) events: Arc<EventQueue>,
    /// Local address every UDP socket binds to.
    pub(crate) bind_address: IpAddr,
    pub(crate) shared_udp: SharedUdp,
    pub(crate) shared_tcp: crate::tcp_listen::SharedTcp,
    pub(crate) http: HttpClients,
    /// Frames of every device's streams, outside the event queue.
    pub(crate) streams: Arc<crate::streams::Streams>,
}

/// What any consumer can read about a device without asking it.
///
/// `state` is the last known state and is kept across disconnections, so a
/// consumer can still show a device's configuration while it is offline.
/// Whether it is current is `connection`'s answer, not the state's.
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
    tcp: HashMap<Key, crate::tcp::Connection>,
    /// SSH sessions shared by this device's tunnelled streams.
    ssh: crate::ssh::Sessions,
    /// Files being written, by key.
    files: HashMap<Key, crate::files::Writer>,
    /// Files being read, by key, with the generation of the read.
    reads: HashMap<Key, u64>,
    ws: HashMap<Key, crate::ws::Connection>,
    /// TCP ports this session listens on, by key.
    listening: HashMap<Key, u16>,
    next_generation: u64,
    pending: HashMap<CommandId, Pending>,
    next_command: CommandId,
    inbound_tx: mpsc::Sender<Inbound>,
    inbound_rx: mpsc::Receiver<Inbound>,
    services: Arc<Services>,
    snapshot: Arc<Mutex<DeviceSnapshot>>,
    last_alive: Option<Instant>,
    /// Fires when one of this device's streams gains its first watcher or
    /// loses its last.
    stream_wake: Arc<tokio::sync::Notify>,
    /// Streams the module has been told are watched.
    watched: Vec<String>,
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
        let stream_wake = services.streams.add_device(device);
        Session {
            device,
            host,
            module,
            started: Instant::now(),
            timers: HashMap::new(),
            sockets: HashMap::new(),
            streams: HashMap::new(),
            tcp: HashMap::new(),
            ssh: crate::ssh::Sessions::default(),
            files: HashMap::new(),
            reads: HashMap::new(),
            ws: HashMap::new(),
            listening: HashMap::new(),
            next_generation: 1,
            pending: HashMap::new(),
            next_command: 1,
            inbound_tx,
            inbound_rx,
            services,
            snapshot,
            last_alive: None,
            stream_wake,
            watched: Vec::new(),
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
                        Inbound::Tcp { socket, generation, input } => {
                            let current = self.tcp.get(socket).map(|c| c.generation);
                            if current != Some(generation) {
                                continue;
                            }
                            if matches!(input, TcpInput::Closed { .. }) {
                                self.tcp.remove(socket);
                            }
                            self.module.tcp(&mut cx, socket, input);
                        }
                        Inbound::File { file, generation, input } => {
                            if self.reads.get(file) == Some(&generation) {
                                self.reads.remove(file);
                            } else if self.files.get(file).map(|w| w.generation) == Some(generation) {
                                self.files.remove(file);
                            } else {
                                continue;
                            }
                            self.module.file(&mut cx, file, input);
                        }
                        Inbound::Accepted { socket, stream } => {
                            // The newest connection from the device replaces
                            // any before it.
                            self.close_tcp(socket);
                            let generation = self.next_generation;
                            self.next_generation += 1;
                            let connection = crate::tcp::adopt(
                                socket,
                                generation,
                                stream,
                                self.inbound_tx.clone(),
                            );
                            self.tcp.insert(socket, connection);
                        }
                        Inbound::Ws { socket, generation, input } => {
                            let current = self.ws.get(socket).map(|c| c.generation);
                            if current != Some(generation) {
                                continue;
                            }
                            if matches!(input, WsInput::Closed { .. }) {
                                self.ws.remove(socket);
                            }
                            self.module.ws(&mut cx, socket, input);
                        }
                    }
                    self.apply(cx.take()).await;
                }
                _ = sleep_until(wake) => self.fire_due().await,
                _ = self.stream_wake.notified() => self.sync_watched().await,
            }
        }
    }

    /// Tell the module which streams gained their first watcher or lost
    /// their last since it was last told.
    async fn sync_watched(&mut self) {
        let now = self.services.streams.watched(self.device);
        let stopped: Vec<String> = self
            .watched
            .iter()
            .filter(|s| !now.contains(s))
            .cloned()
            .collect();
        let started: Vec<String> = now
            .iter()
            .filter(|s| !self.watched.contains(s))
            .cloned()
            .collect();
        self.watched = now;
        for (stream, watching) in stopped
            .into_iter()
            .map(|s| (s, false))
            .chain(started.into_iter().map(|s| (s, true)))
        {
            let mut cx = self.cx();
            self.module.stream_watch(&mut cx, &stream, watching);
            self.apply(cx.take()).await;
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
                Action::TcpOpen { socket, to } => {
                    self.close_tcp(socket);
                    let generation = self.next_generation;
                    self.next_generation += 1;
                    let connection =
                        crate::tcp::spawn(socket, generation, to, self.inbound_tx.clone());
                    self.tcp.insert(socket, connection);
                }
                Action::TcpSend { socket, data } => {
                    if let Some(c) = self.tcp.get(socket) {
                        let _ = c.writer.send(data);
                    }
                }
                Action::TcpOpenSsh { socket, tunnel } => {
                    self.close_tcp(socket);
                    let generation = self.next_generation;
                    self.next_generation += 1;
                    let connection = crate::ssh::spawn(
                        socket,
                        generation,
                        tunnel,
                        self.ssh.clone(),
                        self.inbound_tx.clone(),
                    );
                    self.tcp.insert(socket, connection);
                }
                Action::TcpClose { socket } => self.close_tcp(socket),
                Action::FileOpen { file, path } => {
                    let generation = self.next_generation;
                    self.next_generation += 1;
                    let writer =
                        crate::files::open(file, generation, path.into(), self.inbound_tx.clone());
                    // A file reopened under the same key replaces the old one;
                    // the old writer stops when its queue is dropped.
                    self.files.insert(file, writer);
                }
                Action::FileWrite { file, data } => {
                    if let Some(w) = self.files.get(file) {
                        w.write(data);
                    }
                }
                Action::FileClose { file } => {
                    if let Some(w) = self.files.get(file) {
                        w.close();
                    }
                }
                Action::FileRead {
                    file,
                    path,
                    max_bytes,
                } => {
                    let generation = self.next_generation;
                    self.next_generation += 1;
                    self.reads.insert(file, generation);
                    crate::files::read(
                        file,
                        generation,
                        path.into(),
                        max_bytes,
                        self.inbound_tx.clone(),
                    );
                }
                Action::TcpListen { socket, port } => {
                    let result = self.services.shared_tcp.register(
                        self.services.bind_address,
                        port,
                        self.host,
                        socket,
                        self.inbound_tx.clone(),
                    );
                    match result {
                        Ok(()) => {
                            self.listening.insert(socket, port);
                        }
                        Err(message) => {
                            let mut cx = self.cx();
                            self.module.socket_error(&mut cx, socket, &message);
                            Box::pin(self.apply(cx.take())).await;
                        }
                    }
                }
                Action::WsOpen { socket, request } => {
                    self.close_ws(socket);
                    let generation = self.next_generation;
                    self.next_generation += 1;
                    let connection =
                        crate::ws::spawn(socket, generation, request, self.inbound_tx.clone());
                    self.ws.insert(socket, connection);
                }
                Action::WsSend { socket, text } => {
                    if let Some(c) = self.ws.get(socket) {
                        let _ = c.writer.send(crate::ws::Outgoing::Text(text));
                    }
                }
                Action::WsClose { socket } => self.close_ws(socket),
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
                Action::Frame {
                    stream,
                    format,
                    data,
                } => {
                    self.services
                        .streams
                        .publish(self.device, stream, format, data);
                }
            }
        }
    }

    /// Say what the OS granted for a shared port's buffers, since a short
    /// receive buffer drops telemetry silently.
    fn report_buffers(&self, port: u16, granted: Buffers) {
        let short = granted.receive < RECV_BUFFER;
        let mut message = format!(
            "shared UDP port {port}: receive buffer {} bytes granted of {RECV_BUFFER} requested, \
             send buffer {} of {SEND_BUFFER}",
            granted.receive, granted.send
        );
        if short {
            message.push_str(
                "; bursts of telemetry may be dropped. On Linux, raise net.core.rmem_max",
            );
        }
        self.services.events.push(Event::Log {
            device: self.device,
            level: if short { Level::Warning } else { Level::Info },
            message,
        });
    }

    async fn open_socket(&mut self, key: Key, bind: Bind) -> Result<(), String> {
        self.drop_socket(key);
        match bind {
            Bind::Ephemeral => {
                let socket = UdpSocket::bind((self.services.bind_address, 0))
                    .await
                    .map_err(|e| format!("bind: {e}"))?;
                let socket = Arc::new(socket);
                let reader = spawn_reader(key, socket.clone(), self.host, self.inbound_tx.clone());
                self.sockets.insert(key, Socket::Own { socket, reader });
            }
            Bind::Shared(port) => {
                let bound = self.services.shared_udp.register(
                    self.services.bind_address,
                    port,
                    self.host,
                    key,
                    self.device,
                    self.inbound_tx.clone(),
                )?;
                if let Some(buffers) = bound {
                    self.report_buffers(port, buffers);
                }
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

    fn close_tcp(&mut self, key: Key) {
        if let Some(c) = self.tcp.remove(key) {
            c.task.abort();
        }
    }

    /// Send a close frame and let the connection's task end by itself.
    fn close_ws(&mut self, key: Key) {
        if let Some(c) = self.ws.remove(key) {
            if c.writer.send(crate::ws::Outgoing::Close).is_err() {
                c.task.abort();
            }
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
            match action {
                Action::UdpSend { socket, to, data } => self.send(socket, to, data).await,
                Action::TcpSend { socket, data } => {
                    if let Some(c) = self.tcp.get(socket) {
                        let _ = c.writer.send(data);
                    }
                }
                Action::WsSend { socket, text } => {
                    if let Some(c) = self.ws.get(socket) {
                        let _ = c.writer.send(crate::ws::Outgoing::Text(text));
                    }
                }
                // Sent, with nobody waiting for the reply.
                Action::Http { id, request } => {
                    let (tx, _rx) = tokio::sync::mpsc::channel(1);
                    self.services.http.spawn_request(id, &request, tx);
                }
                _ => {}
            }
        }
        for (_, p) in self.pending.drain() {
            let _ = p.reply.send(Err(CommandError::Closed));
        }
        let keys: Vec<Key> = self.sockets.keys().copied().collect();
        for key in keys {
            self.drop_socket(key);
        }
        // Let each connection write what the module queued (a subscription
        // cancelled, a QUIT) before it closes, within a second.
        for (_, c) in self.tcp.drain() {
            let crate::tcp::Connection { writer, task, .. } = c;
            drop(writer);
            tokio::spawn(async move {
                let abort = task.abort_handle();
                if tokio::time::timeout(Duration::from_secs(1), task)
                    .await
                    .is_err()
                {
                    abort.abort();
                }
            });
        }
        let streams: Vec<Key> = self.streams.keys().copied().collect();
        for key in streams {
            self.close_stream(key);
        }
        let ws: Vec<Key> = self.ws.keys().copied().collect();
        for key in ws {
            self.close_ws(key);
        }
        for (_, port) in self.listening.drain() {
            self.services.shared_tcp.unregister(port, self.host);
        }
        self.services.streams.remove_device(self.device);
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
