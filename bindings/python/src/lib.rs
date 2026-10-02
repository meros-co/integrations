//! Python binding for the integrations core.
//!
//! The native layer passes JSON strings, produced by `meros_integrations::json`
//! like every other delivery; `meros_integrations/__init__.py` turns them into
//! dicts and exceptions. Every call that may wait releases the GIL.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meros_integrations::{json as api, Core, StreamHandle};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use serde_json::{json, Value};

fn parse(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|e| {
        json!({"error": {"error": "invalid_request", "message": e.to_string()}}).to_string()
    })
}

#[pyclass(frozen, name = "NativeCore")]
struct NativeCore {
    core: Core,
    /// Open streams, by the id handed to Python.
    streams: Mutex<HashMap<u64, Arc<StreamHandle>>>,
    next_stream: AtomicU64,
}

/// A frame as Python receives it: (format, data, sequence, dropped).
type PyFrame<'py> = (String, Bound<'py, PyBytes>, u64, u64);

#[pymethods]
impl NativeCore {
    #[new]
    #[pyo3(signature = (options=None))]
    fn new(options: Option<&str>) -> PyResult<Self> {
        let options = match options {
            Some(text) => serde_json::from_str(text)
                .map_err(|e| e.to_string())
                .and_then(|v| api::core_options(&v))
                .map_err(PyRuntimeError::new_err)?,
            None => Default::default(),
        };
        Core::with_options(options)
            .map(|core| NativeCore {
                core,
                streams: Mutex::new(HashMap::new()),
                next_stream: AtomicU64::new(1),
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    fn catalog(&self) -> String {
        api::catalog(&self.core).to_string()
    }

    fn discover(&self, request: &str) -> String {
        match parse(request) {
            Ok(request) => api::discover(&self.core, &request).to_string(),
            Err(e) => e,
        }
    }

    fn open(&self, request: &str) -> String {
        match parse(request) {
            Ok(request) => api::open(&self.core, &request).to_string(),
            Err(e) => e,
        }
    }

    fn execute(&self, py: Python<'_>, device: u64, command: &str, params: &str) -> String {
        let params = match parse(params) {
            Ok(p) => p,
            Err(e) => return e,
        };
        py.detach(|| {
            self.core
                .block_on(api::execute(&self.core, device, command, &params))
                .to_string()
        })
    }

    fn snapshot(&self, device: u64) -> String {
        api::snapshot(&self.core, device).to_string()
    }

    fn poll_events(&self, max: usize) -> String {
        api::events(&self.core.poll_events(max.max(1))).to_string()
    }

    fn wait_events(&self, py: Python<'_>, max: usize, timeout_ms: u64) -> String {
        py.detach(|| {
            let wait = Duration::from_millis(timeout_ms);
            let events = self
                .core
                .block_on(async {
                    tokio::time::timeout(wait, self.core.next_events(max.max(1))).await
                })
                .unwrap_or_default();
            api::events(&events).to_string()
        })
    }

    fn interrupt_events(&self) {
        self.core.interrupt_events();
    }

    /// `{"stream": id}` or `{"error": {...}}`.
    fn open_stream(&self, device: u64, stream: &str) -> String {
        match api::open_stream(&self.core, device, stream) {
            Ok(handle) => {
                let id = self.next_stream.fetch_add(1, Ordering::Relaxed);
                self.streams.lock().unwrap().insert(id, Arc::new(handle));
                json!({ "stream": id }).to_string()
            }
            Err(error) => error.to_string(),
        }
    }

    /// The next frame within `timeout_ms`, or None on timeout or once the
    /// stream has ended (see `stream_ended`).
    fn wait_frame<'py>(
        &self,
        py: Python<'py>,
        stream: u64,
        timeout_ms: u64,
    ) -> Option<PyFrame<'py>> {
        let handle = self.streams.lock().unwrap().get(&stream).cloned()?;
        let frame = py.detach(|| {
            let wait = Duration::from_millis(timeout_ms);
            self.core
                .block_on(async { tokio::time::timeout(wait, handle.next_frame()).await })
                .ok()
                .flatten()
        })?;
        Some((
            frame.format.to_string(),
            PyBytes::new(py, &frame.data),
            frame.sequence,
            frame.dropped,
        ))
    }

    /// True once the stream is closed or its device's session has ended.
    fn stream_ended(&self, stream: u64) -> bool {
        self.streams
            .lock()
            .unwrap()
            .get(&stream)
            .is_none_or(|h| h.is_closed())
    }

    /// Stop watching; a waiting `wait_frame` returns None.
    fn close_stream(&self, stream: u64) {
        if let Some(handle) = self.streams.lock().unwrap().remove(&stream) {
            handle.close();
        }
    }

    fn close(&self, py: Python<'_>, device: u64) {
        py.detach(|| self.core.block_on(self.core.close(device)));
    }
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<NativeCore>()
}
