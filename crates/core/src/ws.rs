//! WebSocket connections for device modules.
//!
//! One task per connection, like TCP: it connects, reports the result,
//! forwards every received message and writes whatever the session hands it.
//! Pings are answered by the library; the module sees text and binary messages
//! and the close. `wss` uses the same TLS as TCP streams (see `crate::tls`).

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

        let connector = tokio_tungstenite::Connector::Rustls(crate::tls::client_config(
            request.accept_invalid_certs,
        ));
        let connecting =
            tokio_tungstenite::connect_async_tls_with_config(http, None, false, Some(connector));
        let stream = match tokio::time::timeout(CONNECT_TIMEOUT, connecting).await {
            Ok(Ok((stream, _))) => stream,
            Ok(Err(Error::Http(response))) => {
                send(WsInput::Closed {
                    code: None,
                    reason: format!("connect: HTTP {}", response.status()),
                })
                .await;
                return;
            }
            Ok(Err(Error::Io(e))) if is_tls(&e) => {
                send(closed(crate::tls::describe(&e))).await;
                return;
            }
            Ok(Err(Error::Tls(e))) => {
                send(closed(format!("{} {e}", crate::tls::PREFIX))).await;
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

/// An I/O error that is a TLS failure, as tokio-rustls reports one.
fn is_tls(e: &std::io::Error) -> bool {
    e.get_ref()
        .is_some_and(|inner| inner.is::<tokio_rustls::rustls::Error>())
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    /// A wss server with a self-signed certificate that echoes text.
    async fn wss_echo() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let acceptor = tokio_rustls::TlsAcceptor::from(crate::tls::testing::server_config());
        tokio::spawn(async move {
            loop {
                let (tcp, _) = listener.accept().await.unwrap();
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    let Ok(tls) = acceptor.accept(tcp).await else {
                        return;
                    };
                    let Ok(mut ws) = tokio_tungstenite::accept_async(tls).await else {
                        return;
                    };
                    while let Some(Ok(message)) = ws.next().await {
                        if message.is_text() && ws.send(message).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        port
    }

    async fn next(rx: &mut mpsc::Receiver<Inbound>) -> WsInput {
        match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
            Ok(Some(Inbound::Ws { input, .. })) => input,
            _ => panic!("expected a websocket event"),
        }
    }

    fn request(port: u16, accept_invalid_certs: bool) -> WsRequest {
        WsRequest {
            url: format!("wss://127.0.0.1:{port}/"),
            headers: Vec::new(),
            accept_invalid_certs,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn wss_to_a_self_signed_device_when_invalid_certs_are_accepted() {
        let port = wss_echo().await;
        let (tx, mut rx) = mpsc::channel(16);
        let c = spawn("ws", 1, request(port, true), tx);
        assert_eq!(next(&mut rx).await, WsInput::Opened);
        c.writer.send(Outgoing::Text("hello".into())).unwrap();
        assert_eq!(next(&mut rx).await, WsInput::Text("hello".into()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn wss_refuses_a_self_signed_certificate_otherwise() {
        let port = wss_echo().await;
        let (tx, mut rx) = mpsc::channel(16);
        let _c = spawn("ws", 1, request(port, false), tx);
        match next(&mut rx).await {
            WsInput::Closed { reason, .. } => {
                assert!(reason.starts_with("tls:"), "{reason}");
                assert!(reason.contains("accept_invalid_certs"), "{reason}");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
}
