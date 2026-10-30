//! The transfer folder: files a remote client hands to device commands that
//! read a file on the host (an ATEM still, a Sony camera's LUT) and files
//! those that write one leave for it to fetch (a downloaded still or clip).
//!
//! A client never names a real path. Each file is known by an opaque id, 32
//! hex digits from a random number, and lives at `<folder>/<id>/<name>`,
//! where `<name>` is the client's file name made safe (so a command that
//! takes a default name or checks an extension from the file's name sees the
//! client's). In a command's params, a line `transfer:<id>` is replaced by
//! that real path before the command reaches the core (in a string param,
//! or in a `binary` param's host file form, `{"file": "transfer:<id>"}`, at
//! any depth), and every real path
//! of the folder in a command's result, an event or a snapshot is shown as
//! `transfer:<id>` again. Only ids this service handed out resolve; anything
//! else in the folder is never served, listed or resolved.
//!
//! The service puts no limit on a file's size, on the folder's total or on
//! how long a file is kept: bodies are streamed to and from disk, and what
//! to keep is the client's call. A file is kept until the client deletes it
//! (or fetches it with `?delete=true`); the folder's ids are cleared when
//! the service starts and when it stops cleanly.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures_util::StreamExt;
use meros_integrations::{json, value::Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The prefix that marks a transfer-folder file in a command's params.
pub const SCHEME: &str = "transfer:";
/// Most outputs one `reserve` hands out: as many frames as a clip transfer
/// can name.
pub const MAX_RESERVE: u64 = 65_535;
/// The longest file name kept, in bytes.
const MAX_NAME: usize = 128;
const READ_CHUNK: usize = 64 * 1024;

/// How the transfer folder is set up (`serve --transfer-dir`).
#[derive(Default)]
pub struct Options {
    /// The folder; `<temp>/meros-integrations-transfer-<port>` when absent.
    pub dir: Option<PathBuf>,
}

struct Entry {
    name: String,
    /// The file's length as last seen (while uploading, as received).
    size: u64,
    created: SystemTime,
    /// Being uploaded: not yet listed, served or resolved.
    uploading: bool,
    /// Downloads of it in progress.
    readers: u32,
}

#[derive(Default)]
struct Index {
    entries: HashMap<String, Entry>,
    /// The sum of every entry's `size`.
    total: u64,
}

impl Index {
    fn resize(&mut self, id: &str, size: u64) {
        if let Some(entry) = self.entries.get_mut(id) {
            self.total = self.total - entry.size + size;
            entry.size = size;
        }
    }

    fn remove(&mut self, id: &str) -> Option<Entry> {
        let entry = self.entries.remove(id)?;
        self.total -= entry.size;
        Some(entry)
    }
}

pub struct Transfers {
    root: PathBuf,
    /// `root` as given to the core, to find it again in results.
    root_text: String,
    /// The folder is the default one, which the service removes on a clean
    /// stop once it is empty.
    default_dir: bool,
    index: Mutex<Index>,
}

fn error(status: StatusCode, code: &str, message: impl Into<String>) -> Response {
    (
        status,
        Json(json!({"error": {"error": code, "message": message.into()}})),
    )
        .into_response()
}

/// An id this service hands out: 32 lowercase hex digits. Nothing else is
/// ever joined to the folder's path.
pub fn is_id(text: &str) -> bool {
    text.len() == 32
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn new_id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("random id");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A client's file name made safe as one path component on every system:
/// the part after its last separator, with only letters, digits, space and
/// `._()-` kept (anything else becomes `_`), no leading or trailing dots or
/// spaces, not a Windows device name, at most 128 bytes; `file` when nothing
/// is left.
pub fn safe_name(given: Option<&str>) -> String {
    let base = given.unwrap_or("").rsplit(['/', '\\']).next().unwrap_or("");
    let mut name: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || " ._()-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    name.truncate(MAX_NAME);
    let name = name.trim_matches(|c| c == '.' || c == ' ').to_string();
    if name.is_empty() {
        return "file".into();
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let stem = stem.trim_end();
    let device = matches!(stem, "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    if device {
        format!("_{name}")
    } else {
        name
    }
}

/// `<temp>/meros-integrations-transfer-<port>`: the folder when none is given.
fn default_folder(port: u16) -> PathBuf {
    std::env::temp_dir().join(format!("meros-integrations-transfer-{port}"))
}

impl Transfers {
    /// Set up the folder (created if absent) and clear what an earlier run
    /// left in it.
    pub fn start(options: Options, port: u16) -> std::io::Result<Arc<Self>> {
        let default_dir = options.dir.is_none();
        let dir = options.dir.unwrap_or_else(|| default_folder(port));
        std::fs::create_dir_all(&dir)?;
        let root = std::path::absolute(&dir)?;
        let transfers = Arc::new(Self {
            root_text: root.to_string_lossy().into_owned(),
            root,
            default_dir,
            index: Mutex::default(),
        });
        transfers.clear();
        Ok(transfers)
    }

    #[cfg(test)]
    fn root(&self) -> &Path {
        &self.root
    }

    /// On a clean stop: remove every id's file, then the default folder
    /// itself if nothing else is left in it (a folder given with
    /// `--transfer-dir` stays, as does anything the service did not put there).
    pub fn stop(&self) {
        self.clear();
        if self.default_dir {
            let _ = std::fs::remove_dir(&self.root);
        }
    }

    /// Remove every id's file from the folder, and nothing else there.
    pub fn clear(&self) {
        let mut index = self.index.lock().unwrap();
        *index = Index::default();
        let Ok(dir) = std::fs::read_dir(&self.root) else {
            return;
        };
        for item in dir.flatten() {
            if !item.file_name().to_str().is_some_and(is_id) {
                continue;
            }
            let path = item.path();
            // A link is removed itself, never followed.
            match std::fs::symlink_metadata(&path) {
                Ok(meta) if meta.is_dir() => {
                    let _ = std::fs::remove_dir_all(&path);
                }
                Ok(_) => {
                    let _ = std::fs::remove_file(&path);
                }
                Err(_) => {}
            }
        }
    }

    fn file_path(&self, id: &str, name: &str) -> PathBuf {
        self.root.join(id).join(name)
    }

    /// The file's length and modification time now, if it is a regular file
    /// (a link is never followed).
    fn stat(&self, id: &str, name: &str) -> Option<(u64, SystemTime)> {
        let dir = std::fs::symlink_metadata(self.root.join(id)).ok()?;
        if !dir.is_dir() {
            return None;
        }
        let meta = std::fs::symlink_metadata(self.file_path(id, name)).ok()?;
        meta.is_file().then(|| {
            (
                meta.len(),
                meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            )
        })
    }

    /// Re-read the length of every file a device command may have written.
    fn refresh(&self, index: &mut Index) -> HashMap<String, Option<SystemTime>> {
        let ids: Vec<(String, String)> = index
            .entries
            .iter()
            .filter(|(_, e)| !e.uploading)
            .map(|(id, e)| (id.clone(), e.name.clone()))
            .collect();
        let mut written = HashMap::new();
        for (id, name) in ids {
            let found = self.stat(&id, &name);
            index.resize(&id, found.map_or(0, |(len, _)| len));
            written.insert(id, found.map(|(_, modified)| modified));
        }
        written
    }

    fn remove_files(&self, id: &str) -> std::io::Result<()> {
        let dir = self.root.join(id);
        match std::fs::symlink_metadata(&dir) {
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&dir),
            Ok(_) => std::fs::remove_file(&dir),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn listing(&self, id: &str, entry: &Entry, written: bool) -> Value {
        json!({
            "file": id,
            "name": entry.name,
            "path": format!("{SCHEME}{id}"),
            "size": entry.size,
            "written": written,
        })
    }

    /// `POST /v1/files?name=`: the request body, streamed to a new file
    /// whatever its length.
    pub async fn upload(self: &Arc<Self>, name: Option<&str>, body: Body) -> Response {
        let id = new_id();
        let name = safe_name(name);
        if let Err(e) = std::fs::create_dir(self.root.join(&id)) {
            return error(StatusCode::INTERNAL_SERVER_ERROR, "io", e.to_string());
        }
        let path = self.file_path(&id, &name);
        self.index.lock().unwrap().entries.insert(
            id.clone(),
            Entry {
                name: name.clone(),
                size: 0,
                created: SystemTime::now(),
                uploading: true,
                readers: 0,
            },
        );
        let result = self.receive(&id, &path, body).await;
        let mut index = self.index.lock().unwrap();
        match result {
            Ok(()) => {
                let entry = index.entries.get_mut(&id).expect("uploading entry");
                entry.uploading = false;
                let size = entry.size;
                drop(index);
                (
                    StatusCode::CREATED,
                    Json(json!({"file": id, "name": name,
                        "path": format!("{SCHEME}{id}"), "size": size})),
                )
                    .into_response()
            }
            Err(refusal) => {
                index.remove(&id);
                drop(index);
                let _ = self.remove_files(&id);
                *refusal
            }
        }
    }

    async fn receive(&self, id: &str, path: &Path, body: Body) -> Result<(), Box<Response>> {
        let io = |e: std::io::Error| {
            Box::new(error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "io",
                e.to_string(),
            ))
        };
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .await
            .map_err(io)?;
        let mut chunks = body.into_data_stream();
        while let Some(chunk) = chunks.next().await {
            let chunk = chunk.map_err(|e| {
                Box::new(error(
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    format!("the upload was cut off: {e}"),
                ))
            })?;
            {
                let mut index = self.index.lock().unwrap();
                let size = index.entries[id].size + chunk.len() as u64;
                index.resize(id, size);
            }
            file.write_all(&chunk).await.map_err(io)?;
        }
        file.flush().await.map_err(io)?;
        Ok(())
    }

    /// `POST /v1/files/reserve`: ids for files a device command will write.
    pub fn reserve(&self, count: u64, name: Option<&str>) -> Response {
        if !(1..=MAX_RESERVE).contains(&count) {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                format!("count must be 1 to {MAX_RESERVE}"),
            );
        }
        let name = safe_name(name);
        let mut index = self.index.lock().unwrap();
        let mut files = Vec::new();
        for _ in 0..count {
            let id = new_id();
            if let Err(e) = std::fs::create_dir(self.root.join(&id)) {
                return error(StatusCode::INTERNAL_SERVER_ERROR, "io", e.to_string());
            }
            let entry = Entry {
                name: name.clone(),
                size: 0,
                created: SystemTime::now(),
                uploading: false,
                readers: 0,
            };
            files.push(self.listing(&id, &entry, false));
            index.entries.insert(id, entry);
        }
        (StatusCode::CREATED, Json(json!({"files": files}))).into_response()
    }

    /// `GET /v1/files`.
    pub fn list(&self) -> Response {
        let mut index = self.index.lock().unwrap();
        let written = self.refresh(&mut index);
        let mut entries: Vec<(&String, &Entry)> =
            index.entries.iter().filter(|(_, e)| !e.uploading).collect();
        entries.sort_by(|a, b| a.1.created.cmp(&b.1.created).then(a.0.cmp(b.0)));
        let files: Vec<Value> = entries
            .into_iter()
            .map(|(id, e)| self.listing(id, e, written.get(id).is_some_and(Option::is_some)))
            .collect();
        Json(json!({
            "files": files,
            "used": index.total,
        }))
        .into_response()
    }

    /// The entry an id from a URL names, or the refusal.
    fn known(&self, index: &Index, id: &str) -> Result<String, Box<Response>> {
        if !is_id(id) {
            return Err(Box::new(error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "a file is named by the id its upload or reservation returned",
            )));
        }
        match index.entries.get(id) {
            Some(entry) if !entry.uploading => Ok(entry.name.clone()),
            _ => Err(Box::new(error(
                StatusCode::NOT_FOUND,
                "not_found",
                format!("no file {id} in the transfer folder"),
            ))),
        }
    }

    /// `GET /v1/files/{id}`: the file, streamed; removed once sent in full
    /// when `delete` is set.
    pub async fn download(self: &Arc<Self>, id: &str, delete: bool) -> Response {
        let (name, file, len) = {
            let mut index = self.index.lock().unwrap();
            let name = match self.known(&index, id) {
                Ok(name) => name,
                Err(refusal) => return *refusal,
            };
            let Some((len, _)) = self.stat(id, &name) else {
                return error(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    format!("nothing has been written to {id} yet"),
                );
            };
            let file = match std::fs::File::open(self.file_path(id, &name)) {
                Ok(file) => file,
                Err(e) => return error(StatusCode::NOT_FOUND, "not_found", e.to_string()),
            };
            index.resize(id, len);
            let entry = index.entries.get_mut(id).expect("known entry");
            entry.readers += 1;
            (name, file, len)
        };
        let reading = Reading {
            transfers: self.clone(),
            id: id.to_string(),
            delete,
            complete: len == 0,
        };
        let file = tokio::fs::File::from_std(file);
        let chunks = futures_util::stream::unfold(
            (file, reading, len),
            |(mut file, mut reading, left)| async move {
                if left == 0 {
                    // Closed before `reading` removes it: Windows does not
                    // remove an open file.
                    drop(file);
                    return None;
                }
                let mut buf = vec![0u8; left.min(READ_CHUNK as u64) as usize];
                match file.read(&mut buf).await {
                    Ok(0) => None,
                    Ok(n) => {
                        buf.truncate(n);
                        let left = left - n as u64;
                        if left == 0 {
                            reading.complete = true;
                        }
                        Some((Ok(buf), (file, reading, left)))
                    }
                    Err(e) => Some((Err(e), (file, reading, 0))),
                }
            },
        );
        (
            [
                (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                (header::CONTENT_LENGTH, len.to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{name}\""),
                ),
                (header::CACHE_CONTROL, "no-store".to_string()),
            ],
            Body::from_stream(chunks),
        )
            .into_response()
    }

    /// `DELETE /v1/files/{id}`.
    pub fn delete(&self, id: &str) -> Response {
        let mut index = self.index.lock().unwrap();
        if let Err(refusal) = self.known(&index, id) {
            return *refusal;
        }
        if index.entries[id].readers > 0 {
            return error(
                StatusCode::CONFLICT,
                "in_use",
                format!("{id} is being downloaded"),
            );
        }
        if let Err(e) = self.remove_files(id) {
            return error(StatusCode::CONFLICT, "in_use", e.to_string());
        }
        index.remove(id);
        Json(json!({"deleted": true})).into_response()
    }

    /// Replace each `transfer:<id>` line in a command's string params with
    /// the file's real path, strings inside objects and lists included, so
    /// a `binary` param's `{"file": "transfer:<id>"}` names the real file
    /// too. An id this service did not hand out (or one
    /// still uploading) refuses the command before it reaches the core.
    pub fn resolve(&self, params: &Value) -> Result<Value, Value> {
        if !mentions_scheme(params) {
            return Ok(params.clone());
        }
        let index = self.index.lock().unwrap();
        self.resolve_in(&index, params)
    }

    fn resolve_in(&self, index: &Index, value: &Value) -> Result<Value, Value> {
        Ok(match value {
            Value::String(text) if text.contains(SCHEME) => {
                let mut lines = Vec::new();
                for line in text.split('\n') {
                    let (body, cr) = match line.strip_suffix('\r') {
                        Some(body) => (body, "\r"),
                        None => (line, ""),
                    };
                    // `transfer:` then text with no space after it: a name
                    // that merely starts with the word is left alone.
                    let reference = body
                        .trim()
                        .strip_prefix(SCHEME)
                        .filter(|id| id.chars().next().is_some_and(|c| !c.is_whitespace()));
                    let Some(id) = reference else {
                        lines.push(line.to_string());
                        continue;
                    };
                    let entry = index
                        .entries
                        .get(id)
                        .filter(|e| is_id(id) && !e.uploading)
                        .ok_or_else(|| {
                            json!({"error": {"error": "invalid_params",
                                "message": format!("{SCHEME}{id} is not a file in the transfer folder; upload or reserve one first")}})
                        })?;
                    let path = self.file_path(id, &entry.name);
                    lines.push(format!("{}{cr}", path.to_string_lossy()));
                }
                Value::String(lines.join("\n"))
            }
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|v| self.resolve_in(index, v))
                    .collect::<Result<_, _>>()?,
            ),
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(k, v)| Ok((k.clone(), self.resolve_in(index, v)?)))
                    .collect::<Result<_, Value>>()?,
            ),
            other => other.clone(),
        })
    }

    /// Show every real path of a transfer-folder file in a command's result,
    /// an event or a snapshot as `transfer:<id>`, so a client never sees the
    /// host's paths.
    pub fn hide(&self, value: &mut Value) {
        match value {
            Value::String(text) if text.contains(&self.root_text) => {
                let index = self.index.lock().unwrap();
                *text = self.hide_text(&index, text);
            }
            Value::Array(items) => items.iter_mut().for_each(|v| self.hide(v)),
            Value::Object(map) => map.values_mut().for_each(|v| self.hide(v)),
            _ => {}
        }
    }

    fn hide_text(&self, index: &Index, text: &str) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(at) = rest.find(&self.root_text) {
            out.push_str(&rest[..at]);
            let after = &rest[at + self.root_text.len()..];
            let replaced = (|| {
                let after = after.strip_prefix(['/', '\\'])?;
                let id = after.get(..32)?;
                let entry = index.entries.get(id)?;
                let after = after[32..].strip_prefix(['/', '\\'])?;
                let after = after.strip_prefix(entry.name.as_str())?;
                Some((format!("{SCHEME}{id}"), after))
            })();
            match replaced {
                Some((shown, after)) => {
                    out.push_str(&shown);
                    rest = after;
                }
                None => {
                    out.push_str(&self.root_text);
                    rest = after;
                }
            }
        }
        out.push_str(rest);
        out
    }
}

fn mentions_scheme(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains(SCHEME),
        Value::Array(items) => items.iter().any(mentions_scheme),
        Value::Object(map) => map.values().any(mentions_scheme),
        _ => false,
    }
}

/// A download in progress: the file is not deleted meanwhile.
struct Reading {
    transfers: Arc<Transfers>,
    id: String,
    delete: bool,
    complete: bool,
}

impl Drop for Reading {
    fn drop(&mut self) {
        let mut index = self.transfers.index.lock().unwrap();
        let Some(entry) = index.entries.get_mut(&self.id) else {
            return;
        };
        entry.readers -= 1;
        if self.delete
            && self.complete
            && entry.readers == 0
            && self.transfers.remove_files(&self.id).is_ok()
        {
            index.remove(&self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("meros-transfer-unit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn transfers(dir: &Path) -> Arc<Transfers> {
        Transfers::start(
            Options {
                dir: Some(dir.to_path_buf()),
            },
            0,
        )
        .unwrap()
    }

    #[test]
    fn ids_are_32_lowercase_hex_digits_only() {
        assert!(is_id("0123456789abcdef0123456789abcdef"));
        assert!(!is_id("0123456789ABCDEF0123456789abcdef"));
        assert!(!is_id("../../../../../../../../etc/passw"));
        assert!(!is_id("0123456789abcdef0123456789abcde"));
        assert!(!is_id(".."));
        assert!(is_id(&new_id()));
    }

    #[test]
    fn names_are_made_safe() {
        assert_eq!(safe_name(Some("still.png")), "still.png");
        assert_eq!(safe_name(Some("../../etc/passwd")), "passwd");
        assert_eq!(safe_name(Some("..\\..\\boot.ini")), "boot.ini");
        assert_eq!(safe_name(Some("..")), "file");
        assert_eq!(safe_name(Some("a:b*c?.cube")), "a_b_c_.cube");
        assert_eq!(safe_name(Some("CON.txt")), "_CON.txt");
        assert_eq!(safe_name(Some("com1")), "_com1");
        assert_eq!(safe_name(Some("Lower third.png ")), "Lower third.png");
        assert_eq!(safe_name(Some("é.wav")), "_.wav");
        assert_eq!(safe_name(None), "file");
        assert_eq!(safe_name(Some(&"x".repeat(300))).len(), MAX_NAME);
    }

    #[test]
    fn transfer_lines_resolve_and_paths_are_hidden_again() {
        let dir = folder("resolve");
        let t = transfers(&dir);
        let out = t.reserve(2, Some("frame.png"));
        assert_eq!(out.status(), StatusCode::CREATED);
        let ids: Vec<String> = t.index.lock().unwrap().entries.keys().cloned().collect();
        let (a, b) = (&ids[0], &ids[1]);

        let params = json!({"clip": 1, "paths": format!("transfer:{a}\r\n  transfer:{b}\n/kept/as/is.png"),
            "name": "not transfer", "scene": "transfer: wide"});
        let resolved = t.resolve(&params).unwrap();
        let pa = t.file_path(a, "frame.png").to_string_lossy().into_owned();
        let pb = t.file_path(b, "frame.png").to_string_lossy().into_owned();
        assert_eq!(resolved["paths"], format!("{pa}\r\n{pb}\n/kept/as/is.png"));
        assert_eq!(resolved["name"], "not transfer");
        assert_eq!(resolved["scene"], "transfer: wide");
        // A binary parameter's host file form, and base64 beside it.
        let resolved = t
            .resolve(&json!({"file": {"file": format!("transfer:{a}")}, "image": "AAEC"}))
            .unwrap();
        assert_eq!(resolved, json!({"file": {"file": pa}, "image": "AAEC"}));
        let refused = t
            .resolve(&json!({"file": {"file": "transfer:0123456789abcdef0123456789abcdef"}}))
            .unwrap_err();
        assert_eq!(refused["error"]["error"], "invalid_params");

        // A fake command writes the first and reports both paths.
        std::fs::write(&pa, b"frame one").unwrap();
        let mut result = json!({"ok": {"path": pa, "message": format!("wrote {pa}; {pb} next"),
            "other": format!("{}/unknown/x", t.root_text)}});
        t.hide(&mut result);
        assert_eq!(result["ok"]["path"], format!("transfer:{a}"));
        assert_eq!(
            result["ok"]["message"],
            format!("wrote transfer:{a}; transfer:{b} next")
        );
        assert_eq!(result["ok"]["other"], format!("{}/unknown/x", t.root_text));

        // Unknown, malformed or traversing ids are refused.
        for bad in [
            "transfer:0123456789abcdef0123456789abcdef".to_string(),
            "transfer:../../etc/passwd".to_string(),
            format!("transfer:{a}/../{b}"),
        ] {
            let refused = t.resolve(&json!({"path": bad})).unwrap_err();
            assert_eq!(refused["error"]["error"], "invalid_params", "{bad}");
        }
        drop(t);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_fake_command_reads_an_upload_and_writes_an_output() {
        let dir = folder("fake");
        let t = transfers(&dir);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let up = runtime.block_on(t.upload(Some("in.cube"), Body::from("LUT_3D_SIZE 2")));
        assert_eq!(up.status(), StatusCode::CREATED);
        t.reserve(1, Some("out.bin"));
        let (input, output) = {
            let index = t.index.lock().unwrap();
            let input = index
                .entries
                .iter()
                .find(|e| e.1.name == "in.cube")
                .unwrap()
                .0
                .clone();
            let output = index
                .entries
                .iter()
                .find(|e| e.1.name == "out.bin")
                .unwrap()
                .0
                .clone();
            (input, output)
        };
        let params = t
            .resolve(
                &json!({"path": format!("transfer:{input}"), "out": format!("transfer:{output}")}),
            )
            .unwrap();
        // The command: read one file, write its bytes reversed to the other.
        let read = std::fs::read(params["path"].as_str().unwrap()).unwrap();
        assert!(params["path"].as_str().unwrap().ends_with("in.cube"));
        let reversed: Vec<u8> = read.iter().rev().copied().collect();
        std::fs::write(params["out"].as_str().unwrap(), &reversed).unwrap();
        let mut result = json!({"path": params["out"], "bytes": reversed.len()});
        t.hide(&mut result);
        assert_eq!(result["path"], format!("transfer:{output}"));

        let mut index = t.index.lock().unwrap();
        t.refresh(&mut index);
        assert_eq!(index.entries[&output].size, 13);
        assert_eq!(index.total, 26);
        drop(index);
        drop(t);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn startup_clears_only_ids() {
        let dir = folder("clear");
        std::fs::create_dir_all(dir.join("0123456789abcdef0123456789abcdef")).unwrap();
        std::fs::write(dir.join("0123456789abcdef0123456789abcdef/x.png"), b"x").unwrap();
        std::fs::write(dir.join("fedcba9876543210fedcba9876543210"), b"x").unwrap();
        std::fs::write(dir.join("keep.txt"), b"not ours").unwrap();
        let t = transfers(&dir);
        let left: Vec<String> = std::fs::read_dir(t.root())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(left, ["keep.txt"]);
        drop(t);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_clean_stop_removes_the_default_folder_only_when_empty() {
        // A port no other test or service of this run uses for its folder.
        let port = 40_000 + (std::process::id() % 20_000) as u16;
        let t = Transfers::start(Options::default(), port).unwrap();
        let dir = t.root().to_path_buf();
        assert_eq!(dir, std::path::absolute(default_folder(port)).unwrap());
        std::fs::create_dir_all(dir.join("0123456789abcdef0123456789abcdef")).unwrap();
        t.stop();
        assert!(!dir.exists(), "the default folder is removed");

        // Something the service did not put there keeps it.
        let t = Transfers::start(Options::default(), port).unwrap();
        std::fs::write(dir.join("keep.txt"), b"not ours").unwrap();
        t.stop();
        assert!(dir.join("keep.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);

        // A folder given is never removed.
        let given = folder("given");
        let t = transfers(&given);
        t.stop();
        assert!(given.exists());
        let _ = std::fs::remove_dir_all(&given);
    }
}
