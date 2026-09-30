//! TCP connections for device modules.
//!
//! One task per connection: it connects, reports the result, forwards every
//! received chunk and writes whatever the session hands it. Framing is the
//! module's job; this only moves bytes.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::module::{Key, TcpInput};
use crate::session::Inbound;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct Connection {
    pub(crate) generation: u64,
    pub(crate) writer: mpsc::UnboundedSender<Vec<u8>>,
    pub(crate) task: JoinHandle<()>,
}

pub(crate) fn spawn(
    socket: Key,
    generation: u64,
    to: SocketAddr,
    inbound: mpsc::Sender<Inbound>,
) -> Connection {
    let (writer, mut outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
    let task = tokio::spawn(async move {
        let send = |input: TcpInput| {
            let inbound = inbound.clone();
            async move {
                inbound
                    .send(Inbound::Tcp {
                        socket,
                        generation,
                        input,
                    })
                    .await
                    .is_ok()
            }
        };

        let stream = match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(to)).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                send(TcpInput::Closed {
                    reason: format!("connect: {e}"),
                })
                .await;
                return;
            }
            Err(_) => {
                send(TcpInput::Closed {
                    reason: "connect: timed out".into(),
                })
                .await;
                return;
            }
        };
        let _ = stream.set_nodelay(true);
        if !send(TcpInput::Connected).await {
            return;
        }

        let (mut read, mut write) = stream.into_split();
        let mut buf = vec![0u8; 16 * 1024];
        let reason = loop {
            tokio::select! {
                result = read.read(&mut buf) => match result {
                    Ok(0) => break "closed by the device".to_string(),
                    Ok(n) => {
                        if !send(TcpInput::Data(buf[..n].to_vec())).await {
                            return;
                        }
                    }
                    Err(e) => break e.to_string(),
                },
                data = outgoing.recv() => match data {
                    Some(data) => {
                        if let Err(e) = write.write_all(&data).await {
                            break e.to_string();
                        }
                    }
                    None => return,
                },
            }
        };
        send(TcpInput::Closed { reason }).await;
    });
    Connection {
        generation,
        writer,
        task,
    }
}
