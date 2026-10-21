//! Blackmagic HyperDeck media over the deck's FTP server, added to the
//! spec-driven deck (`specs/blackmagic-hyperdeck.yaml`).
//!
//! Every transport, clip, configuration and setup command is the spec's,
//! run by the spec engine on TCP 9993 and the configuration API. This adds
//! the deck's FTP server (TCP 21), which the HyperDeck manuals offer for
//! moving files to and from its media (HyperDeck Operation Manual, February
//! 2026, "Transferring Files over a Network", p.52-55; the Extreme and
//! Shuttle manuals the same): any FTP client logs in anonymously, the top
//! folders are the media by slot (`sd1`, `ssd2`, `cfast1`, `usb/...`), files
//! go both ways while the deck records, and only files in the deck's codecs
//! play. bmdwire's hyperdeckwire (MIT) adds what it measured: some decks
//! name the slot folders by number (`1`, `2`), and STOR at the root is
//! refused.
//!
//! Listing, deleting, renaming and creating folders answer when done, on a
//! connection of their own. Downloads and uploads can outlast a command's
//! timeout, so each answers at once with the transfer's number and runs in
//! the background, one at a time in the order asked, on another
//! connection: `transfers.<number>` in the state follows it (queued,
//! connecting, transferring, writing, then done, failed or canceled) with
//! its progress, and `transfer_started` and `transfer_finished` message
//! events mark its ends, as the ATEM's file transfers do. These commands
//! work whether or not the device is monitored, since they add commands
//! rather than state.

use std::collections::VecDeque;
use std::net::SocketAddr;

use serde_json::{json, Map, Value};

use super::ftp::{Event, Failure, Keys, Lane, Login, Op};
use crate::catalog::Params;
use crate::engine::SpecEngine;
use crate::module::{
    CommandError, CommandId, Cx, FileInput, HttpResponse, Key, Level, Millis, Module, OpenContext,
    Outcome, RequestId, SettingsUpdate, SseInput, TcpInput, WsInput,
};

const OPS: Keys = Keys {
    control: "hyperdeck-ftp",
    data: "hyperdeck-ftp-data",
    file: "hyperdeck-ftp-file",
    timer: "hyperdeck-ftp-timer",
};
const TRANSFERS: Keys = Keys {
    control: "hyperdeck-ftp-transfer",
    data: "hyperdeck-ftp-transfer-data",
    file: "hyperdeck-ftp-transfer-file",
    timer: "hyperdeck-ftp-transfer-timer",
};

const DEFAULT_PORT: u16 = 21;
/// Finished transfers kept in the state.
const KEEP_FINISHED: usize = 32;
/// How often a transfer's progress reaches the state.
const PROGRESS_EVERY: Millis = 250;
/// The longest path taken on the deck.
const MAX_REMOTE: usize = 1024;

/// What the extension answers itself; every other command is the engine's.
const COMMANDS: [&str; 7] = [
    "list_files",
    "download_file",
    "upload_file",
    "delete_file",
    "rename_file",
    "create_folder",
    "cancel_transfer",
];

#[derive(Debug)]
struct Transfer {
    number: u64,
    kind: &'static str,
    remote: String,
    path: String,
    op: Op,
    /// When progress last reached the state.
    reported: Millis,
    bytes: u64,
}

impl Transfer {
    fn target(&self) -> Map<String, Value> {
        let mut args = Map::new();
        args.insert("transfer".into(), json!(self.number));
        args.insert("kind".into(), json!(self.kind));
        args.insert("remote".into(), json!(self.remote));
        args.insert("path".into(), json!(self.path));
        args
    }
}

pub(crate) struct HyperdeckFtp {
    engine: SpecEngine,
    /// The deck's FTP server: where both lanes connect, and the source of
    /// the transfer events.
    to: SocketAddr,
    ops: Lane,
    transfers: Lane,
    /// Listing, deleting, renaming and folders waiting their turn.
    waiting: VecDeque<(CommandId, Op)>,
    /// The command the operation on `ops` answers.
    running: Option<CommandId>,
    queue: VecDeque<Transfer>,
    active: Option<Transfer>,
    finished: VecDeque<u64>,
    next: u64,
}

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::InvalidParams {
        message: message.into(),
    }
}

fn target_of(host: std::net::IpAddr, settings: &Params) -> SocketAddr {
    let port = settings
        .get("ftp_port")
        .and_then(Value::as_u64)
        .and_then(|p| u16::try_from(p).ok())
        .filter(|&p| p != 0)
        .unwrap_or(DEFAULT_PORT);
    SocketAddr::new(host, port)
}

fn login_of(settings: &Params) -> Login {
    let text = |name: &str| {
        settings
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    Login {
        user: text("ftp_username"),
        password: text("ftp_password"),
    }
}

fn param<'a>(params: &'a Params, name: &str) -> &'a str {
    params.get(name).and_then(Value::as_str).unwrap_or("")
}

/// A path on the deck, from its root, without the leading slash. `empty`
/// allows the root itself.
fn remote(params: &Params, name: &str, empty: bool) -> Result<String, CommandError> {
    let raw = param(params, name).trim();
    if raw.chars().any(|c| matches!(c, '\r' | '\n' | '\0')) {
        return Err(invalid(format!("'{name}' holds a line break")));
    }
    let path = raw.trim_matches('/').to_string();
    if path.len() > MAX_REMOTE {
        return Err(invalid(format!(
            "'{name}' is longer than {MAX_REMOTE} bytes"
        )));
    }
    if path.split('/').any(|s| s == "." || s == "..") {
        return Err(invalid(format!("'{name}' holds . or ..")));
    }
    if path.split('/').any(str::is_empty) && !path.is_empty() {
        return Err(invalid(format!("'{name}' holds an empty folder name")));
    }
    if path.is_empty() && !empty {
        return Err(invalid(format!("'{name}' is empty")));
    }
    Ok(path)
}

/// A host file's own name, the last part of its path.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn command_error(failure: Failure) -> CommandError {
    match failure {
        Failure::Transport(message) => CommandError::Transport { message },
        Failure::Login(message) => CommandError::Auth {
            message: format!("the FTP server refused the login: {message}"),
        },
        Failure::Refused { code, message } => CommandError::DeviceError {
            code: Some(code.to_string()),
            message,
        },
        Failure::Invalid(message) => invalid(message),
        Failure::Timeout(_) => CommandError::Timeout,
        Failure::Canceled => CommandError::Closed,
    }
}

impl HyperdeckFtp {
    pub(crate) fn new(engine: SpecEngine, ctx: &OpenContext) -> HyperdeckFtp {
        let to = target_of(ctx.host, &ctx.settings);
        let login = login_of(&ctx.settings);
        HyperdeckFtp {
            engine,
            to,
            ops: Lane::new(OPS, to, login.clone()),
            transfers: Lane::new(TRANSFERS, to, login),
            waiting: VecDeque::new(),
            running: None,
            queue: VecDeque::new(),
            active: None,
            finished: VecDeque::new(),
            next: 1,
        }
    }

    fn command_own(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        let op = match name {
            "list_files" => remote(params, "path", true).map(|path| Op::List { path }),
            "delete_file" => remote(params, "remote", false).map(|path| Op::Delete { path }),
            "create_folder" => remote(params, "remote", false).map(|path| Op::MakeDir { path }),
            "rename_file" => remote(params, "remote", false).and_then(|from| {
                Ok(Op::Rename {
                    from,
                    to: remote(params, "to", false)?,
                    overwrite: params.get("overwrite").and_then(Value::as_bool) == Some(true),
                })
            }),
            "download_file" | "upload_file" => return self.transfer(cx, id, name, params),
            _ => {
                let outcome = self.cancel(cx, params);
                return cx.complete(id, Ok(outcome));
            }
        };
        match op {
            Ok(op) => {
                self.waiting.push_back((id, op));
                self.pump_ops(cx);
            }
            Err(e) => cx.complete(id, Err(e)),
        }
    }

    fn pump_ops(&mut self, cx: &mut Cx) {
        if !self.ops.idle() || self.running.is_some() {
            return;
        }
        let Some((id, op)) = self.waiting.pop_front() else {
            return;
        };
        self.running = Some(id);
        self.ops.start(cx, op);
        self.ops_events(cx);
    }

    fn ops_events(&mut self, cx: &mut Cx) {
        for event in self.ops.take_events() {
            if let Event::Done(result) = event {
                if let Some(id) = self.running.take() {
                    let outcome = match result {
                        Ok(Value::Null) => Ok(Outcome::Ack),
                        Ok(value) => Ok(Outcome::Value { value }),
                        Err(failure) => Err(command_error(failure)),
                    };
                    cx.complete(id, outcome);
                }
            }
        }
        self.pump_ops(cx);
    }

    fn transfer(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        let path = param(params, "path").to_string();
        if path.trim().is_empty() {
            return cx.complete(id, Err(invalid("'path' is empty")));
        }
        let planned = if name == "download_file" {
            remote(params, "remote", false).map(|remote| {
                let op = Op::Download {
                    remote: remote.clone(),
                    host: path.clone(),
                };
                ("download_file", remote, op)
            })
        } else {
            remote(params, "folder", false).and_then(|folder| {
                let given = param(params, "name").trim();
                let own = if given.is_empty() {
                    file_name(&path)
                } else {
                    given
                };
                if own.is_empty() || own.contains(['/', '\\', '\r', '\n', '\0']) {
                    return Err(invalid(
                        "'name' must be a file name, without folders or line breaks",
                    ));
                }
                let remote = format!("{folder}/{own}");
                if remote.len() > MAX_REMOTE {
                    return Err(invalid(format!(
                        "the path on the deck is longer than {MAX_REMOTE} bytes"
                    )));
                }
                let op = Op::Upload {
                    host: path.clone(),
                    remote: remote.clone(),
                    overwrite: params.get("overwrite").and_then(Value::as_bool) == Some(true),
                };
                Ok(("upload_file", remote, op))
            })
        };
        let (kind, remote, op) = match planned {
            Ok(p) => p,
            Err(e) => return cx.complete(id, Err(e)),
        };
        let number = self.next;
        self.next += 1;
        let transfer = Transfer {
            number,
            kind,
            remote,
            path,
            op,
            reported: 0,
            bytes: 0,
        };
        let mut entry = transfer.target();
        entry.remove("transfer");
        entry.insert("status".into(), json!("queued"));
        entry.insert("progress".into(), json!(0.0));
        entry.insert("bytes".into(), json!(0));
        entry.insert("size".into(), Value::Null);
        entry.insert("error".into(), Value::Null);
        cx.state(json!({"transfers": {number.to_string(): entry}}));
        let ahead = self.queue.len() + usize::from(self.active.is_some());
        self.queue.push_back(transfer);
        cx.complete(
            id,
            Ok(Outcome::Value {
                value: json!({"transfer": number, "ahead": ahead}),
            }),
        );
        self.pump_transfers(cx);
    }

    fn pump_transfers(&mut self, cx: &mut Cx) {
        if self.active.is_some() || !self.transfers.idle() {
            return;
        }
        let Some(transfer) = self.queue.pop_front() else {
            return;
        };
        cx.message(
            "transfer_started",
            None,
            Value::Object(transfer.target()),
            self.to,
        );
        let op = transfer.op.clone();
        self.active = Some(transfer);
        self.transfers.start(cx, op);
        self.transfer_events(cx);
    }

    fn transfer_events(&mut self, cx: &mut Cx) {
        for event in self.transfers.take_events() {
            let Some(t) = self.active.as_mut() else {
                continue;
            };
            let key = t.number.to_string();
            match event {
                Event::Status(status) => {
                    cx.state(json!({"transfers": {key: {"status": status}}}));
                }
                Event::Progress { bytes, total } => {
                    t.bytes = bytes;
                    let now = cx.now();
                    let whole = total == Some(bytes);
                    if !whole && t.reported != 0 && now < t.reported + PROGRESS_EVERY {
                        continue;
                    }
                    t.reported = now.max(1);
                    let progress = total
                        .filter(|&t| t > 0)
                        .map(|t| ((bytes as f64 / t as f64) * 1000.0).round() / 1000.0)
                        .or(total.map(|_| 1.0));
                    let mut entry = json!({"bytes": bytes, "size": total});
                    if let Some(p) = progress {
                        entry["progress"] = json!(p);
                    }
                    cx.state(json!({"transfers": {key: entry}}));
                }
                Event::Done(result) => {
                    let transfer = self.active.take().expect("active");
                    let (status, error, bytes) = match result {
                        Ok(value) => (
                            "done",
                            None,
                            value["bytes"].as_u64().unwrap_or(transfer.bytes),
                        ),
                        Err(Failure::Canceled) => ("canceled", None, transfer.bytes),
                        Err(f) => ("failed", Some(f.message()), transfer.bytes),
                    };
                    self.finish(cx, transfer, status, error, bytes);
                }
            }
        }
        self.pump_transfers(cx);
    }

    fn finish(
        &mut self,
        cx: &mut Cx,
        transfer: Transfer,
        status: &str,
        error: Option<String>,
        bytes: u64,
    ) {
        if let Some(e) = &error {
            cx.log(
                Level::Warning,
                format!(
                    "transfer {} ({} {}) failed: {e}",
                    transfer.number, transfer.kind, transfer.remote
                ),
            );
        }
        let mut entry = json!({"status": status, "error": error, "bytes": bytes});
        if status == "done" {
            entry["progress"] = json!(1.0);
        }
        cx.state(json!({"transfers": {transfer.number.to_string(): entry}}));
        let mut args = transfer.target();
        args.insert("status".into(), json!(status));
        args.insert("error".into(), json!(error));
        args.insert("bytes".into(), json!(bytes));
        cx.message("transfer_finished", None, Value::Object(args), self.to);
        self.finished.push_back(transfer.number);
        while self.finished.len() > KEEP_FINISHED {
            let old = self.finished.pop_front().unwrap();
            cx.state(json!({"transfers": {old.to_string(): null}}));
        }
    }

    /// Cancel one transfer, or every one queued or running.
    fn cancel(&mut self, cx: &mut Cx, params: &Params) -> Outcome {
        let only = params.get("transfer").and_then(Value::as_u64);
        let mut canceled = Vec::new();
        let queued: VecDeque<Transfer> = {
            let (gone, kept) = std::mem::take(&mut self.queue)
                .into_iter()
                .partition(|t| only.is_none_or(|n| n == t.number));
            self.queue = kept;
            gone
        };
        for t in queued {
            canceled.push(t.number);
            let bytes = t.bytes;
            self.finish(cx, t, "canceled", None, bytes);
        }
        if self
            .active
            .as_ref()
            .is_some_and(|t| only.is_none_or(|n| n == t.number))
        {
            canceled.push(self.active.as_ref().unwrap().number);
            self.transfers.cancel(cx);
            self.transfer_events(cx);
        }
        canceled.sort_unstable();
        Outcome::Value {
            value: json!({"canceled": canceled}),
        }
    }
}

impl Module for HyperdeckFtp {
    fn start(&mut self, cx: &mut Cx) {
        self.engine.start(cx);
    }

    fn command(&mut self, cx: &mut Cx, id: CommandId, name: &str, params: &Params) {
        if COMMANDS.contains(&name) {
            self.command_own(cx, id, name, params);
        } else {
            self.engine.command(cx, id, name, params);
        }
    }

    fn datagram(&mut self, cx: &mut Cx, socket: Key, from: SocketAddr, data: &[u8]) {
        self.engine.datagram(cx, socket, from, data);
    }

    fn socket_error(&mut self, cx: &mut Cx, socket: Key, message: &str) {
        self.engine.socket_error(cx, socket, message);
    }

    fn tcp(&mut self, cx: &mut Cx, socket: Key, input: TcpInput) {
        if self.ops.owns(socket) {
            self.ops.tcp(cx, socket, input);
            self.ops_events(cx);
        } else if self.transfers.owns(socket) {
            self.transfers.tcp(cx, socket, input);
            self.transfer_events(cx);
        } else {
            self.engine.tcp(cx, socket, input);
        }
    }

    fn ws(&mut self, cx: &mut Cx, socket: Key, input: WsInput) {
        self.engine.ws(cx, socket, input);
    }

    fn http_response(&mut self, cx: &mut Cx, id: RequestId, result: Result<HttpResponse, String>) {
        self.engine.http_response(cx, id, result);
    }

    fn sse(&mut self, cx: &mut Cx, stream: Key, input: SseInput) {
        self.engine.sse(cx, stream, input);
    }

    fn file(&mut self, cx: &mut Cx, file: Key, input: FileInput) {
        if self.ops.owns(file) {
            self.ops.file(cx, input);
            self.ops_events(cx);
        } else if self.transfers.owns(file) {
            self.transfers.file(cx, input);
            self.transfer_events(cx);
        } else {
            self.engine.file(cx, file, input);
        }
    }

    fn computed(&mut self, cx: &mut Cx, key: Key, result: Result<Vec<u8>, String>) {
        self.engine.computed(cx, key, result);
    }

    fn timer(&mut self, cx: &mut Cx, key: Key) {
        if self.ops.owns(key) {
            self.ops.timer(cx);
            self.ops_events(cx);
        } else if self.transfers.owns(key) {
            self.transfers.timer(cx);
            self.transfer_events(cx);
        } else {
            self.engine.timer(cx, key);
        }
    }

    fn stop(&mut self, cx: &mut Cx) {
        self.ops.stop(cx);
        self.transfers.stop(cx);
        // Pending commands are failed by the session; transfers end here.
        let _ = self.ops.take_events();
        self.running = None;
        self.waiting.clear();
        let _ = self.transfers.take_events();
        let mut ending: Vec<Transfer> = self.active.take().into_iter().collect();
        ending.extend(self.queue.drain(..));
        for t in ending {
            let bytes = t.bytes;
            self.finish(
                cx,
                t,
                "canceled",
                Some("the device was closed".into()),
                bytes,
            );
        }
        self.engine.stop(cx);
    }

    fn update_settings(&mut self, cx: &mut Cx, settings: &Params) -> SettingsUpdate {
        let update = self.engine.update_settings(cx, settings);
        if update == SettingsUpdate::Applied {
            // Taken by the next connection; one running keeps its own.
            self.to = target_of(self.to.ip(), settings);
            let login = login_of(settings);
            self.ops.configure(self.to, login.clone());
            self.transfers.configure(self.to, login);
        }
        update
    }

    fn reconnect(&mut self, cx: &mut Cx) -> bool {
        self.engine.reconnect(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(v: Value) -> Params {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn deck_paths_are_checked() {
        assert_eq!(
            remote(&p(json!({"r": "/sd1/Clip 1.mov"})), "r", false),
            Ok("sd1/Clip 1.mov".into())
        );
        assert_eq!(remote(&p(json!({"r": ""})), "r", true), Ok(String::new()));
        assert!(remote(&p(json!({"r": ""})), "r", false).is_err());
        assert!(remote(&p(json!({"r": "sd1/../x"})), "r", false).is_err());
        assert!(remote(&p(json!({"r": "sd1//x"})), "r", false).is_err());
        assert!(remote(&p(json!({"r": "sd1/x\r\nDELE y"})), "r", false).is_err());
        assert_eq!(file_name("C:\\clips\\intro.mov"), "intro.mov");
        assert_eq!(file_name("/tmp/transfer/ab/intro.mov"), "intro.mov");
    }

    #[test]
    fn failures_map_to_command_errors() {
        assert!(matches!(
            command_error(Failure::Login("530 no".into())),
            CommandError::Auth { .. }
        ));
        assert_eq!(
            command_error(Failure::Refused {
                code: 550,
                message: "nope".into()
            }),
            CommandError::DeviceError {
                code: Some("550".into()),
                message: "nope".into()
            }
        );
    }

    #[test]
    fn the_default_port_is_21() {
        let host = "192.0.2.1".parse().unwrap();
        assert_eq!(target_of(host, &Params::new()).port(), 21);
        assert_eq!(target_of(host, &p(json!({"ftp_port": 2121}))).port(), 2121);
    }
}
