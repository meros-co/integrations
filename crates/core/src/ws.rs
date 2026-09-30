//! WebSocket connections for device modules.
//!
//! One task per connection, like TCP: it connects, reports the result,
//! forwards every received message and writes whatever the session hands it.
//! Pings are answered by the library; the module sees text and binary messages
//! and the close.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::{Error, Message};

use crate::module::{Key, WsInput, WsRequest};
use crate::session::Inbound;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) enum Outgoing {
    Text(String),
    Close,
}

pub(crate) struct Connection {
    pub(crate) generation: u64,
    pub(crate) writer: mpsc::UnboundedSender<Outgoing>,
    pub(crate) task: JoinHandle<()>,
}

pub(crate) fn spawn(
    socket: Key,
    generation: u64,
    request: WsRequest,
    inbound: mpsc::Sender<Inbound>,
) -> Connection {
    let (writer, mut outgoing) = mpsc::unbounded_channel::<Outgoing>();
    let task = tokio::spawn(async move {
        let send = |input: WsInput| {
            let inbound = inbound.clone();
            async move {
                inbound
                    .send(Inbound::Ws {
                        socket,
                        generation,
                        input,
                    })
                    .await
                    .is_ok()
            }
        };
        let closed = |reason: String| WsInput::Closed { code: None, reason };

        let mut http = match request.url.as_str().into_client_request() {
            Ok(r) => r,
            Err(e) => {
                send(closed(format!("invalid request: {e}"))).await;
                return;
            }
        };
        for (name, value) in &request.headers {
            let (Ok(name), Ok(value)) = (
                name.parse::<tokio_tungstenite::tungstenite::http::HeaderName>(),
                HeaderValue::from_str(value),
            ) else {
                send(closed(format!("invalid header {name}"))).await;
                return;
            };
            http.headers_mut().insert(name, value);
        }

        let stream =
            match tokio::time::timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(http))
                .await
            {
                Ok(Ok((stream, _))) => stream,
                Ok(Err(Error::Http(response))) => {
                    send(WsInput::Closed {
                        code: None,
                        reason: format!("connect: HTTP {}", response.status()),
                    })
                    .await;
                    return;
                }
                Ok(Err(e)) => {
                    send(closed(format!("connect: {e}"))).await;
                    return;
                }
                Err(_) => {
                    send(closed("connect: timed out".into())).await;
                    return;
                }
            };
        if !send(WsInput::Opened).await {
            return;
        }

        let (mut write, mut read) = stream.split();
        let end = loop {
            tokio::select! {
                message = read.next() => match message {
                    Some(Ok(Message::Text(text))) => {
                        if !send(WsInput::Text(text.to_string())).await {
                            return;
                        }
                    }
                    Some(Ok(Message::Binary(data))) => {
                        if !send(WsInput::Binary(data.to_vec())).await {
                            return;
                        }
                    }
                    Some(Ok(Message::Close(frame))) => {
                        break match frame {
                            Some(f) => WsInput::Closed {
                                code: Some(u16::from(f.code)),
                                reason: f.reason.to_string(),
                            },
                            None => closed("closed by the device".into()),
                        };
                    }
                    Some(Ok(_)) => {
                        // Ping, pong or a raw frame: the library answers pings.
                        if !send(WsInput::Activity).await {
                            return;
                        }
                    }
                    Some(Err(e)) => break closed(e.to_string()),
                    None => break closed("closed by the device".into()),
                },
                data = outgoing.recv() => match data {
                    Some(Outgoing::Text(text)) => {
                        if let Err(e) = write.send(Message::text(text)).await {
                            break closed(e.to_string());
                        }
                    }
                    Some(Outgoing::Close) | None => {
                        let _ = write
                            .send(Message::Close(Some(CloseFrame {
                                code: CloseCode::Normal,
                                reason: "".into(),
                            })))
                            .await;
                        return;
                    }
                },
            }
        };
        send(end).await;
    });
    Connection {
        generation,
        writer,
        task,
    }
}
