//! The interface every device module implements.
//!
//! A module is protocol logic only. It never opens a socket or reads a clock:
//! it is handed commands, inbound data and timer expiries, and answers with
//! [`Action`]s that the session carries out. That keeps a module deterministic,
//! so it can be tested by feeding it recorded bytes and synthetic time, and it
//! means no module can differ from another in how it uses the network.

use std::net::SocketAddr;

use serde::Serialize;
use serde_json::Value;

use crate::catalog::Params;

/// Milliseconds since the session started. Monotonic, and synthetic in tests.
pub type Millis = u64;

/// Identifies one pending command within a session.
pub type CommandId = u64;

/// A module-chosen name for a socket, timer, request or stream. Modules use
/// small constants; the session maps them to real resources.
pub type Key = &'static str;

/// How a command ended.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Outcome {
    /// The device confirmed the command.
    Ack,
    /// The device returned a value.
    Value { value: Value },
    /// Sent, but the protocol gives no way to confirm it was applied.
    Unverified,
}

/// Why a command failed. The set is closed and identical in every delivery.
#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum CommandError {
    #[error("invalid parameters: {message}")]
    InvalidParams { message: String },
    #[error("unknown command '{command}'")]
    UnknownCommand { command: String },
    #[error("'{command}' is not supported by model '{model}'")]
    UnsupportedForModel { command: String, model: String },
    #[error("device is not connected")]
    NotConnected,
    #[error("no reply within the timeout")]
    Timeout,
    #[error("device rejected the command: {message}")]
    DeviceError {
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
        message: String,
    },
    #[error("transport error: {message}")]
    Transport { message: String },
    #[error("authentication failed: {message}")]
    Auth { message: String },
    #[error("session closed")]
    Closed,
}

pub type CommandResult = Result<Outcome, CommandError>;

/// Identifies one HTTP request within a module; the module allocates it.
pub type RequestId = u64;

/// An HTTP request a module asks the session to make.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequest {
    pub method: &'static str,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    /// Overall deadline for the response. `None` only for streams.
    pub timeout: Option<Millis>,
    /// Accept a certificate that does not chain to a trusted root, such as a
    /// device's self-signed one. The connection is still encrypted; the peer is
    /// not authenticated.
    pub accept_invalid_certs: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// What happens on a server-sent event stream.
#[derive(Debug, Clone, PartialEq)]
pub enum SseInput {
    /// The server accepted the request and the stream is open.
    Opened,
    Event(crate::sse::SseEvent),
    /// Bytes arrived, whether or not they completed an event. Keepalive
    /// comments are proof of life too.
    Activity,
    /// The stream ended. `status` is set when the server refused to open it.
    Closed {
        status: Option<u16>,
        reason: String,
    },
}

/// What happens on a TCP connection.
#[derive(Debug, Clone, PartialEq)]
pub enum TcpInput {
    Connected,
    Data(Vec<u8>),
    /// The connection failed or ended. It is gone; open it again to retry.
    Closed {
        reason: String,
    },
}

/// A WebSocket a module asks the session to open.
#[derive(Debug, Clone, PartialEq)]
pub struct WsRequest {
    /// `ws://host:port/path`.
    pub url: String,
    /// Extra handshake headers, such as `Sec-WebSocket-Protocol`.
    pub headers: Vec<(String, String)>,
}

/// What happens on a WebSocket.
#[derive(Debug, Clone, PartialEq)]
pub enum WsInput {
    Opened,
    Text(String),
    Binary(Vec<u8>),
    /// A ping or pong arrived: proof of life, nothing to handle.
    Activity,
    /// The connection failed or ended. `code` is the close code the peer sent,
    /// if it sent one. It is gone; open it again to retry.
    Closed {
        code: Option<u16>,
        reason: String,
    },
}

/// Where a UDP socket binds locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bind {
    /// An ephemeral port owned by this session. The device replies to it.
    Ephemeral,
    /// A fixed port shared by every session that asks for it, with inbound
    /// datagrams routed by source address. For protocols whose devices reply to
    /// a fixed port rather than the sender's (Sennheiser MCP on 53212).
    Shared(u16),
}

/// Whether the device is reachable, as the module judges it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Connection {
    Connecting,
    Connected,
    Disconnected {
        reason: String,
    },
    /// Reachable, but the device refused the configured credentials.
    Unauthorized {
        reason: String,
    },
    /// The protocol offers no way to tell whether the device is there: it never
    /// replies. Commands are sent and reported `unverified`.
    Unmonitored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Debug,
    Info,
    Warning,
}

/// Something a module asks the session to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    UdpOpen {
        socket: Key,
        bind: Bind,
    },
    UdpSend {
        socket: Key,
        to: SocketAddr,
        data: Vec<u8>,
    },
    UdpClose {
        socket: Key,
    },
    /// Connect, replacing any connection open under this key.
    TcpOpen {
        socket: Key,
        to: SocketAddr,
    },
    TcpSend {
        socket: Key,
        data: Vec<u8>,
    },
    TcpClose {
        socket: Key,
    },
    /// Accept connections from the device on a local TCP port, shared with
    /// other sessions and routed by peer address. Each accepted connection is
    /// reported on `socket` as `TcpInput::Connected`, replacing any before it.
    TcpListen {
        socket: Key,
        port: u16,
    },
    /// Open a WebSocket, replacing any open under this key.
    WsOpen {
        socket: Key,
        request: WsRequest,
    },
    WsSend {
        socket: Key,
        text: String,
    },
    WsClose {
        socket: Key,
    },
    Http {
        id: RequestId,
        request: HttpRequest,
    },
    /// Open a server-sent event stream, replacing any open under this key.
    SseOpen {
        stream: Key,
        request: HttpRequest,
    },
    SseClose {
        stream: Key,
    },
    /// Fire [`Module::timer`] with this key after `after` ms, replacing any
    /// pending timer with the same key.
    SetTimer {
        key: Key,
        after: Millis,
    },
    CancelTimer {
        key: Key,
    },
    Complete {
        id: CommandId,
        result: CommandResult,
    },
    /// Merge into the device state (RFC 7386 JSON merge patch).
    State(Value),
    Connection(Connection),
    /// Heard from the device, whether or not it carried telemetry.
    Alive,
    Log {
        level: Level,
        message: String,
    },
}

/// Collects a module's actions for one callback.
#[derive(Debug)]
pub struct Cx {
    now: Millis,
    actions: Vec<Action>,
}

impl Cx {
    pub fn new(now: Millis) -> Cx {
        Cx {
            now,
            actions: Vec::new(),
        }
    }

    pub fn now(&self) -> Millis {
        self.now
    }

    pub fn take(self) -> Vec<Action> {
        self.actions
    }

    pub fn push(&mut self, action: Action) {
        self.actions.push(action);
    }

    pub fn udp_open(&mut self, socket: Key, bind: Bind) {
        self.push(Action::UdpOpen { socket, bind });
    }

    pub fn udp_send(&mut self, socket: Key, to: SocketAddr, data: impl Into<Vec<u8>>) {
        self.push(Action::UdpSend {
            socket,
            to,
            data: data.into(),
        });
    }

    pub fn udp_close(&mut self, socket: Key) {
        self.push(Action::UdpClose { socket });
    }

    pub fn tcp_open(&mut self, socket: Key, to: SocketAddr) {
        self.push(Action::TcpOpen { socket, to });
    }

    pub fn tcp_send(&mut self, socket: Key, data: impl Into<Vec<u8>>) {
        self.push(Action::TcpSend {
            socket,
            data: data.into(),
        });
    }

    pub fn tcp_close(&mut self, socket: Key) {
        self.push(Action::TcpClose { socket });
    }

    pub fn tcp_listen(&mut self, socket: Key, port: u16) {
        self.push(Action::TcpListen { socket, port });
    }

    pub fn ws_open(&mut self, socket: Key, request: WsRequest) {
        self.push(Action::WsOpen { socket, request });
    }

    pub fn ws_send(&mut self, socket: Key, text: impl Into<String>) {
        self.push(Action::WsSend {
            socket,
            text: text.into(),
        });
    }

    pub fn ws_close(&mut self, socket: Key) {
        self.push(Action::WsClose { socket });
    }

    pub fn http(&mut self, id: RequestId, request: HttpRequest) {
        self.push(Action::Http { id, request });
    }

    pub fn sse_open(&mut self, stream: Key, request: HttpRequest) {
        self.push(Action::SseOpen { stream, request });
    }

    pub fn sse_close(&mut self, stream: Key) {
        self.push(Action::SseClose { stream });
    }

    pub fn set_timer(&mut self, key: Key, after: Millis) {
        self.push(Action::SetTimer { key, after });
    }

    pub fn cancel_timer(&mut self, key: Key) {
        self.push(Action::CancelTimer { key });
    }

    pub fn complete(&mut self, id: CommandId, result: CommandResult) {
        self.push(Action::Complete { id, result });
    }

    pub fn state(&mut self, patch: Value) {
        self.push(Action::State(patch));
    }

    pub fn connection(&mut self, connection: Connection) {
        self.push(Action::Connection(connection));
    }

    pub fn alive(&mut self) {
        self.push(Action::Alive);
    }

    pub fn log(&mut self, level: Level, message: impl Into<String>) {
        self.push(Action::Log {
            level,
            message: message.into(),
        });
    }
}

/// A device protocol implementation.
///
/// Commands reaching [`Module::command`] have already been checked against the
/// model's `supports` list and validated against the spec's parameters, with
/// defaults applied.
pub trait Module: Send + 'static {
    /// The session has started: open sockets, send subscriptions, arm timers.
    fn start(&mut self, cx: &mut Cx);

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params);

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        let _ = (cx, socket, from, data);
    }

    /// A UDP socket failed; it has been closed.
    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        let _ = (socket, message);
        cx.connection(Connection::Disconnected {
            reason: "socket error".into(),
        });
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        let _ = (cx, socket, input);
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        let _ = (cx, socket, input);
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        let _ = (cx, id, result);
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        let _ = (cx, stream, input);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key);

    /// The session is closing: cancel subscriptions cleanly where the protocol
    /// allows it. Pending commands are failed with `Closed` by the session.
    fn stop(&mut self, cx: &mut Cx) {
        let _ = cx;
    }
}

/// What a module is constructed from when a device is opened.
#[derive(Debug, Clone)]
pub struct OpenContext {
    pub host: std::net::IpAddr,
    /// Overrides the protocol's default port.
    pub port: Option<u16>,
    pub model: String,
    pub channels: Option<u32>,
    pub settings: Params,
}
