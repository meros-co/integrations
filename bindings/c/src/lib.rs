//! C interface to the integrations core. The declarations are in
//! `include/meros_integrations.h`.
//!
//! JSON strings in, JSON strings out, produced by `meros_integrations::json` like
//! every other delivery. Every returned string is freed with
//! `mi_string_free`. Calls block; make them from a worker thread. No call ever
//! unwinds into C: a panic becomes an `internal` error.

use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Duration;

use meros_integrations::{json as api, Core};
use serde_json::{json, Value};

/// Opaque to C.
pub struct MiCore {
    core: Core,
}

fn to_c(value: Value) -> *mut c_char {
    // JSON never contains an interior NUL: serde_json escapes it.
    CString::new(value.to_string())
        .expect("JSON has no NUL")
        .into_raw()
}

fn internal(message: &str) -> *mut c_char {
    to_c(json!({"error": {"error": "internal", "message": message}}))
}

/// # Safety
/// `s` must be null or a valid NUL-terminated string.
unsafe fn read_json(s: *const c_char) -> Result<Value, *mut c_char> {
    if s.is_null() {
        return Ok(Value::Null);
    }
    let text = CStr::from_ptr(s)
        .to_str()
        .map_err(|_| internal("argument is not UTF-8"))?;
    serde_json::from_str(text)
        .map_err(|e| to_c(json!({"error": {"error": "invalid_request", "message": e.to_string()}})))
}

fn guard(f: impl FnOnce() -> *mut c_char) -> *mut c_char {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| internal("the core panicked"))
}

/// Start a core. Returns NULL if it cannot start.
#[no_mangle]
pub extern "C" fn mi_core_new() -> *mut MiCore {
    catch_unwind(|| match Core::new() {
        Ok(core) => Box::into_raw(Box::new(MiCore { core })),
        Err(_) => std::ptr::null_mut(),
    })
    .unwrap_or(std::ptr::null_mut())
}

/// Start a core with options, as CoreOptions JSON (`{"bind_address":"..."}`),
/// or NULL for the defaults. Returns NULL if the options are invalid or the
/// core cannot start.
///
/// # Safety
/// `options` must be NULL or a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn mi_core_new_with_options(options: *const c_char) -> *mut MiCore {
    catch_unwind(AssertUnwindSafe(|| {
        let Ok(value) = read_json(options) else {
            return std::ptr::null_mut();
        };
        let Ok(options) = api::core_options(&value) else {
            return std::ptr::null_mut();
        };
        match Core::with_options(options) {
            Ok(core) => Box::into_raw(Box::new(MiCore { core })),
            Err(_) => std::ptr::null_mut(),
        }
    }))
    .unwrap_or(std::ptr::null_mut())
}

/// Start a core with options, as `mi_core_new_with_options` does, and say why
/// when it cannot: on failure returns NULL and, if `error` is not NULL, sets
/// `*error` to `{"error":{"error":"invalid_options"|"internal","message":...}}`,
/// which the caller frees with `mi_string_free`. An unknown id in `devices`
/// is `invalid_options`.
///
/// # Safety
/// `options` must be NULL or a NUL-terminated string; `error` must be NULL or
/// point to writable storage for a pointer.
#[no_mangle]
pub unsafe extern "C" fn mi_core_create(
    options: *const c_char,
    error: *mut *mut c_char,
) -> *mut MiCore {
    let fail = |e: *mut c_char| {
        if error.is_null() {
            mi_string_free(e);
        } else {
            *error = e;
        }
        std::ptr::null_mut()
    };
    let invalid =
        |message: String| to_c(json!({"error": {"error": "invalid_options", "message": message}}));
    let started = catch_unwind(AssertUnwindSafe(|| {
        let value = read_json(options).map_err(|_| invalid("options are not JSON".into()))?;
        let options = api::core_options(&value).map_err(invalid)?;
        Core::with_options(options).map_err(|e| match e.kind() {
            std::io::ErrorKind::InvalidInput => invalid(e.to_string()),
            _ => internal(&format!("cannot start: {e}")),
        })
    }));
    match started {
        Ok(Ok(core)) => Box::into_raw(Box::new(MiCore { core })),
        Ok(Err(e)) => fail(e),
        Err(_) => fail(internal("the core panicked")),
    }
}

/// # Safety
/// `core` must come from `mi_core_new` and not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn mi_core_free(core: *mut MiCore) {
    if !core.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(Box::from_raw(core))));
    }
}

/// # Safety
/// `s` must come from this library, or be NULL.
#[no_mangle]
pub unsafe extern "C" fn mi_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// The catalogue as JSON.
///
/// # Safety
/// `core` must be a live core.
#[no_mangle]
pub unsafe extern "C" fn mi_catalog(core: *const MiCore) -> *mut c_char {
    guard(|| to_c(api::catalog(&(*core).core)))
}

/// Open a device: `request` is an OpenRequest as JSON. Returns `{"device":id}`
/// or `{"error":{...}}`.
///
/// # Safety
/// `core` must be a live core; `request` a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn mi_open(core: *const MiCore, request: *const c_char) -> *mut c_char {
    guard(|| match read_json(request) {
        Ok(request) => to_c(api::open(&(*core).core, &request)),
        Err(e) => e,
    })
}

/// Listen for devices, scan for them now, or stop:
/// `{"action":"listen"|"scan"|"stop","protocols":[...],"hints":[...]}`.
/// Returns `{"ok":true}` or `{"error":...}`; found devices arrive as events.
///
/// # Safety
/// `core` must be a live core and `request` a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn mi_discover(core: *const MiCore, request: *const c_char) -> *mut c_char {
    guard(|| match read_json(request) {
        Ok(request) => to_c(api::discover(&(*core).core, &request)),
        Err(e) => e,
    })
}

/// Run a command and wait for its result: `{"ok":...}` or `{"error":...}`.
/// `params` may be NULL for none.
///
/// # Safety
/// `core` must be a live core; `command` a NUL-terminated string; `params`
/// NULL or a NUL-terminated string. Do not call from inside an async runtime.
#[no_mangle]
pub unsafe extern "C" fn mi_execute(
    core: *const MiCore,
    device: u64,
    command: *const c_char,
    params: *const c_char,
) -> *mut c_char {
    guard(|| {
        let Ok(command) = CStr::from_ptr(command).to_str() else {
            return internal("command is not UTF-8");
        };
        let params = match read_json(params) {
            Ok(p) => p,
            Err(e) => return e,
        };
        let core = &(*core).core;
        to_c(core.block_on(api::execute(core, device, command, &params)))
    })
}

/// The device's snapshot as JSON, or `null` if it is not open.
///
/// # Safety
/// `core` must be a live core.
#[no_mangle]
pub unsafe extern "C" fn mi_snapshot(core: *const MiCore, device: u64) -> *mut c_char {
    guard(|| to_c(api::snapshot(&(*core).core, device)))
}

/// Queued events as a JSON array, without waiting.
///
/// # Safety
/// `core` must be a live core.
#[no_mangle]
pub unsafe extern "C" fn mi_poll_events(core: *const MiCore, max: u32) -> *mut c_char {
    guard(|| to_c(api::events(&(*core).core.poll_events(max.max(1) as usize))))
}

/// Wait up to `timeout_ms` for events; an empty array on timeout or after
/// `mi_interrupt_events`.
///
/// # Safety
/// `core` must be a live core. Do not call from inside an async runtime.
#[no_mangle]
pub unsafe extern "C" fn mi_wait_events(
    core: *const MiCore,
    max: u32,
    timeout_ms: u32,
) -> *mut c_char {
    guard(|| {
        let core = &(*core).core;
        let wait = Duration::from_millis(timeout_ms as u64);
        let events = core
            .block_on(async {
                tokio::time::timeout(wait, core.next_events(max.max(1) as usize)).await
            })
            .unwrap_or_default();
        to_c(api::events(&events))
    })
}

/// Make a waiting `mi_wait_events` return.
///
/// # Safety
/// `core` must be a live core.
#[no_mangle]
pub unsafe extern "C" fn mi_interrupt_events(core: *const MiCore) {
    let _ = catch_unwind(AssertUnwindSafe(|| (*core).core.interrupt_events()));
}

/// Shut down cleanly before the host exits: refuse new devices and
/// commands, give commands in flight up to `grace_ms` to finish, then close
/// every device so each ends what it started on the device. Returns once all
/// are closed; the core accepts nothing afterwards, but must still be freed.
///
/// # Safety
/// `core` must be a live core. Do not call from inside an async runtime.
#[no_mangle]
pub unsafe extern "C" fn mi_close_all(core: *const MiCore, grace_ms: u32) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        (*core)
            .core
            .close_all_blocking(std::time::Duration::from_millis(grace_ms as u64));
    }));
}

/// End a device's session and wait for it to close.
///
/// # Safety
/// `core` must be a live core. Do not call from inside an async runtime.
#[no_mangle]
pub unsafe extern "C" fn mi_close(core: *const MiCore, device: u64) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let core = &(*core).core;
        core.block_on(core.close(device));
    }));
}

/// One watcher of a device's stream. Opaque to C.
pub struct MiStream {
    handle: meros_integrations::StreamHandle,
}

/// One frame, owned by the caller until `mi_frame_free`. `data` points at
/// `len` bytes; `format` is a NUL-terminated string such as `jpeg`.
#[repr(C)]
pub struct MiFrame {
    pub data: *const u8,
    pub len: usize,
    pub format: *const c_char,
    pub sequence: u64,
    pub dropped: u64,
}

/// What `MiFrame` points into. `frame` comes first so a `*mut MiFrame` is
/// also a pointer to this.
#[repr(C)]
struct OwnedFrame {
    frame: MiFrame,
    _data: std::sync::Arc<[u8]>,
    _format: CString,
}

fn to_frame(frame: meros_integrations::Frame) -> *mut MiFrame {
    let format = CString::new(frame.format).unwrap_or_default();
    let owned = Box::new(OwnedFrame {
        frame: MiFrame {
            data: frame.data.as_ptr(),
            len: frame.data.len(),
            format: format.as_ptr(),
            sequence: frame.sequence,
            dropped: frame.dropped,
        },
        _data: frame.data,
        _format: format,
    });
    Box::into_raw(owned).cast::<MiFrame>()
}

/// Watch a device's stream, such as a camera's `live` preview. On success
/// returns `{"ok":true}` and sets `*out`; otherwise `{"error":{...}}` and sets
/// `*out` to NULL. The device produces frames while any stream is open.
///
/// # Safety
/// `core` must be a live core; `stream` a NUL-terminated string; `out` a
/// valid pointer.
#[no_mangle]
pub unsafe extern "C" fn mi_stream_open(
    core: *const MiCore,
    device: u64,
    stream: *const c_char,
    out: *mut *mut MiStream,
) -> *mut c_char {
    if !out.is_null() {
        *out = std::ptr::null_mut();
    }
    guard(|| {
        if out.is_null() {
            return internal("out is NULL");
        }
        let Ok(stream) = CStr::from_ptr(stream).to_str() else {
            return internal("stream is not UTF-8");
        };
        match api::open_stream(&(*core).core, device, stream) {
            Ok(handle) => {
                *out = Box::into_raw(Box::new(MiStream { handle }));
                to_c(json!({"ok": true}))
            }
            Err(e) => to_c(e),
        }
    })
}

/// The newest frame, without waiting; NULL if none is waiting.
///
/// # Safety
/// `stream` must come from `mi_stream_open` and not be freed.
#[no_mangle]
pub unsafe extern "C" fn mi_stream_poll(stream: *const MiStream) -> *mut MiFrame {
    catch_unwind(AssertUnwindSafe(|| match (*stream).handle.try_frame() {
        Some(frame) => to_frame(frame),
        None => std::ptr::null_mut(),
    }))
    .unwrap_or(std::ptr::null_mut())
}

/// Wait up to `timeout_ms` for the next frame. NULL on timeout, or once the
/// stream is closed or its device's session has ended (`mi_stream_ended`
/// tells which). Only the newest frame is kept: frames replaced before they
/// were taken are counted in `dropped`.
///
/// # Safety
/// `stream` must come from `mi_stream_open` and not be freed. Do not call
/// from inside an async runtime.
#[no_mangle]
pub unsafe extern "C" fn mi_stream_wait(
    core: *const MiCore,
    stream: *const MiStream,
    timeout_ms: u32,
) -> *mut MiFrame {
    catch_unwind(AssertUnwindSafe(|| {
        let core = &(*core).core;
        let handle = &(*stream).handle;
        let wait = Duration::from_millis(timeout_ms as u64);
        match core.block_on(async { tokio::time::timeout(wait, handle.next_frame()).await }) {
            Ok(Some(frame)) => to_frame(frame),
            _ => std::ptr::null_mut(),
        }
    }))
    .unwrap_or(std::ptr::null_mut())
}

/// 1 once the stream is closed or its device's session has ended and no
/// frame is left, else 0.
///
/// # Safety
/// `stream` must come from `mi_stream_open` and not be freed.
#[no_mangle]
pub unsafe extern "C" fn mi_stream_ended(stream: *const MiStream) -> i32 {
    catch_unwind(AssertUnwindSafe(|| (*stream).handle.is_closed() as i32)).unwrap_or(1)
}

/// Stop watching. A waiting `mi_stream_wait` returns NULL. Safe from any
/// thread while another waits; the stream must still be freed.
///
/// # Safety
/// `stream` must come from `mi_stream_open` and not be freed.
#[no_mangle]
pub unsafe extern "C" fn mi_stream_close(stream: *const MiStream) {
    let _ = catch_unwind(AssertUnwindSafe(|| (*stream).handle.close()));
}

/// Free a stream, closing it if open. No other call may be using it.
///
/// # Safety
/// `stream` must come from `mi_stream_open`, or be NULL, and not be used
/// afterwards.
#[no_mangle]
pub unsafe extern "C" fn mi_stream_free(stream: *mut MiStream) {
    if !stream.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(Box::from_raw(stream))));
    }
}

/// # Safety
/// `frame` must come from `mi_stream_poll` or `mi_stream_wait`, or be NULL,
/// and not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn mi_frame_free(frame: *mut MiFrame) {
    if !frame.is_null() {
        drop(Box::from_raw(frame.cast::<OwnedFrame>()));
    }
}
