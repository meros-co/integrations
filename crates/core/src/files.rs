//! Files on the host, for downloads too large to return as a command's value
//! (a camera's video clips). A module opens a file, hands over chunks as they
//! arrive and closes it; a writer thread per file keeps the writes in order
//! and off the session's task.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;

use tokio::sync::mpsc;

use crate::module::{FileInput, Key};
use crate::session::Inbound;

enum Op {
    Write(Vec<u8>),
    Close,
}

/// An open file: chunks go to its writer thread.
pub(crate) struct Writer {
    pub(crate) generation: u64,
    ops: std_mpsc::Sender<Op>,
}

impl Writer {
    pub(crate) fn write(&self, data: Vec<u8>) {
        let _ = self.ops.send(Op::Write(data));
    }

    pub(crate) fn close(&self) {
        let _ = self.ops.send(Op::Close);
    }
}

/// Create (or truncate) `path` and start its writer. The module hears
/// `Closed { bytes }` after `close`, or `Failed` once, at the first error,
/// after which later writes are dropped.
pub(crate) fn open(
    file: Key,
    generation: u64,
    path: PathBuf,
    inbound: mpsc::Sender<Inbound>,
) -> Writer {
    let (ops, queue) = std_mpsc::channel::<Op>();
    std::thread::spawn(move || {
        let report = |input: FileInput| {
            let _ = inbound.blocking_send(Inbound::File {
                file,
                generation,
                input,
            });
        };
        let mut out = match File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                report(FileInput::Failed {
                    message: format!("{}: {e}", path.display()),
                });
                return;
            }
        };
        let mut bytes: u64 = 0;
        while let Ok(op) = queue.recv() {
            match op {
                Op::Write(data) => {
                    if let Err(e) = out.write_all(&data) {
                        report(FileInput::Failed {
                            message: format!("{}: {e}", path.display()),
                        });
                        return;
                    }
                    bytes += data.len() as u64;
                }
                Op::Close => {
                    match out.flush().and_then(|()| out.sync_all()) {
                        Ok(()) => report(FileInput::Closed { bytes }),
                        Err(e) => report(FileInput::Failed {
                            message: format!("{}: {e}", path.display()),
                        }),
                    }
                    return;
                }
            }
        }
        // The session dropped the file without closing it.
    });
    Writer { generation, ops }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn chunks_are_written_in_order_and_counted() {
        let dir = std::env::temp_dir().join(format!("meros-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clip.bin");
        let (tx, mut rx) = mpsc::channel(8);
        let w = open("clip", 1, path.clone(), tx);
        w.write(b"abc".to_vec());
        w.write(b"def".to_vec());
        w.close();
        match rx.recv().await {
            Some(Inbound::File {
                file: "clip",
                input: FileInput::Closed { bytes },
                ..
            }) => assert_eq!(bytes, 6),
            _ => panic!("expected the file to close"),
        }
        assert_eq!(std::fs::read(&path).unwrap(), b"abcdef");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_unwritable_path_fails_once() {
        let (tx, mut rx) = mpsc::channel(8);
        let missing = std::env::temp_dir()
            .join("meros-no-such-dir")
            .join("x")
            .join("clip.bin");
        let w = open("clip", 1, missing, tx);
        w.write(b"abc".to_vec());
        w.close();
        assert!(matches!(
            rx.recv().await,
            Some(Inbound::File {
                input: FileInput::Failed { .. },
                ..
            })
        ));
    }
}
