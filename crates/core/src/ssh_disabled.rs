//! Stands in for `ssh.rs` in a build without the `sony` feature, the only
//! family whose devices tunnel over SSH; it leaves out russh and its
//! dependencies. A module that asks for a tunnel anyway gets a stream that
//! closes at once, saying why.

use tokio::sync::mpsc;

use crate::module::{Key, SshTunnel, TcpInput};
use crate::session::Inbound;

#[derive(Clone, Default)]
pub(crate) struct Sessions {}

pub(crate) fn spawn(
    socket: Key,
    generation: u64,
    _tunnel: SshTunnel,
    _sessions: Sessions,
    inbound: mpsc::Sender<Inbound>,
) -> crate::tcp::Connection {
    let (writer, _outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
    let task = tokio::spawn(async move {
        let _ = inbound
            .send(Inbound::Tcp {
                socket,
                generation,
                input: TcpInput::Closed {
                    reason:
                        "ssh: this build has no SSH tunnels (they come with the `sony` feature)"
                            .into(),
                },
            })
            .await;
    });
    crate::tcp::Connection {
        generation,
        writer,
        task,
    }
}
