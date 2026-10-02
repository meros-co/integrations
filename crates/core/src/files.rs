//! Files on the host, for downloads too large to return as a command's value
//! (a camera's video clips). A module opens a file, hands over chunks as they
//! arrive and closes it; a writer thread per file keeps the writes in order
//! and off the session's task.

use std::fs::{File, OpenOptions};
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

/// How a file is opened for writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Create it, or truncate it if it exists.
    Create,
    /// Append to it; it must exist. Its length is reported as `Opened`.
    Append,
}

/// Open `path` and start its writer. The module hears `Opened { bytes }`
/// first when appending, then `Closed { bytes }` with the file's length after
/// `close`, or `Failed` once, at the first error, after which later writes
/// are dropped.
pub(crate) fn open(
    file: Key,
    generation: u64,
    path: PathBuf,
    mode: Mode,
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
        let opened = match mode {
            Mode::Create => File::create(&path).map(|f| (f, 0)),
            Mode::Append => OpenOptions::new()
                .append(true)
                .open(&path)
                .and_then(|f| f.metadata().map(|m| (f, m.len()))),
        };
        let (mut out, mut bytes) = match opened {
            Ok(f) => f,
            Err(e) => {
                report(FileInput::Failed {
                    message: format!("{}: {e}", path.display()),
                });
                return;
            }
        };
        if mode == Mode::Append {
            report(FileInput::Opened { bytes });
        }
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

/// Read `path` whole on a thread and report it as `Read`, or `Failed` when it
/// is missing, unreadable or larger than `max_bytes`.
pub(crate) fn read(
    file: Key,
    generation: u64,
    path: PathBuf,
    max_bytes: u64,
    inbound: mpsc::Sender<Inbound>,
) {
    std::thread::spawn(move || {
        let failed = |e: String| FileInput::Failed {
            message: format!("{}: {e}", path.display()),
        };
        let input = match std::fs::metadata(&path) {
            Err(e) => failed(e.to_string()),
            Ok(m) if !m.is_file() => failed("not a file".into()),
            Ok(m) if m.len() > max_bytes => {
                failed(format!("{} bytes, more than {max_bytes}", m.len()))
            }
            Ok(_) => match std::fs::read(&path) {
                Ok(data) => FileInput::Read { data },
                Err(e) => failed(e.to_string()),
            },
        };
        let _ = inbound.blocking_send(Inbound::File {
            file,
            generation,
            input,
        });
    });
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
        let w = open("clip", 1, path.clone(), Mode::Create, tx);
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
    async fn appending_continues_an_existing_file_from_its_length() {
        let dir = std::env::temp_dir().join(format!("meros-append-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clip.bin");
        std::fs::write(&path, b"0123").unwrap();
        let (tx, mut rx) = mpsc::channel(8);
        let w = open("clip", 1, path.clone(), Mode::Append, tx.clone());
        w.write(b"456".to_vec());
        w.close();
        assert!(matches!(
            rx.recv().await,
            Some(Inbound::File {
                input: FileInput::Opened { bytes: 4 },
                ..
            })
        ));
        assert!(matches!(
            rx.recv().await,
            Some(Inbound::File {
                input: FileInput::Closed { bytes: 7 },
                ..
            })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"0123456");

        // A missing file is not created.
        let missing = dir.join("none.bin");
        let w = open("clip", 2, missing.clone(), Mode::Append, tx);
        w.close();
        assert!(matches!(
            rx.recv().await,
            Some(Inbound::File {
                input: FileInput::Failed { .. },
                ..
            })
        ));
        assert!(!missing.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn files_are_read_whole_up_to_a_limit() {
        let dir = std::env::temp_dir().join(format!("meros-read-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("look.cube");
        std::fs::write(&path, b"LUT_3D_SIZE 2").unwrap();
        let (tx, mut rx) = mpsc::channel(8);
        read("lut", 1, path.clone(), 1024, tx.clone());
        match rx.recv().await {
            Some(Inbound::File {
                input: FileInput::Read { data },
                ..
            }) => assert_eq!(data, b"LUT_3D_SIZE 2"),
            _ => panic!("expected the file"),
        }
        read("lut", 2, path, 4, tx);
        assert!(matches!(
            rx.recv().await,
            Some(Inbound::File {
                input: FileInput::Failed { .. },
                ..
            })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_unwritable_path_fails_once() {
        let (tx, mut rx) = mpsc::channel(8);
        let missing = std::env::temp_dir()
            .join("meros-no-such-dir")
            .join("x")
            .join("clip.bin");
        let w = open("clip", 1, missing, Mode::Create, tx);
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
