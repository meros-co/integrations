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
