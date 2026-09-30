//! Python binding for the integrations core.
//!
//! The native layer passes JSON strings, produced by `meros_integrations::json`
//! like every other delivery; `meros_integrations/__init__.py` turns them into
//! dicts and exceptions. Every call that may wait releases the GIL.

use std::time::Duration;

use meros_integrations::{json as api, Core};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde_json::{json, Value};

fn parse(text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|e| {
        json!({"error": {"error": "invalid_request", "message": e.to_string()}}).to_string()
    })
}

#[pyclass(frozen, name = "NativeCore")]
struct NativeCore {
    core: Core,
}

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
            .map(|core| NativeCore { core })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    fn catalog(&self) -> String {
        api::catalog(&self.core).to_string()
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

    fn close(&self, py: Python<'_>, device: u64) {
        py.detach(|| self.core.block_on(self.core.close(device)));
    }
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<NativeCore>()
}
