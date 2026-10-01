//! Node binding for the integrations core.
//!
//! Every method passes JSON through `meros_integrations::json` and nothing
//! else, so this binding cannot differ from the others in what it returns. The
//! idiomatic JavaScript surface (promises that reject, an EventEmitter) is in
//! `index.js`.

use std::sync::Arc;

use meros_integrations::{json, Core};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value;

#[napi(js_name = "NativeCore")]
pub struct NativeCore {
    core: Arc<Core>,
}

fn device_id(device: f64) -> Result<u64> {
    if device.fract() == 0.0 && device >= 0.0 && device <= u64::MAX as f64 {
        Ok(device as u64)
    } else {
        Err(Error::new(
            Status::InvalidArg,
            "device must be a non-negative integer",
        ))
    }
}

#[napi]
impl NativeCore {
    /// `options`: `{bindAddress?}` as CoreOptions JSON (`bind_address`).
    #[napi(constructor)]
    pub fn new(options: Option<Value>) -> Result<Self> {
        let options =
            json::core_options(&options.unwrap_or(Value::Null)).map_err(Error::from_reason)?;
        let core = Core::with_options(options).map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(NativeCore {
            core: Arc::new(core),
        })
    }

    #[napi]
    pub fn catalog(&self) -> Value {
        json::catalog(&self.core)
    }

    /// `{device}` or `{error}`.
    #[napi]
    pub fn open(&self, request: Value) -> Value {
        json::open(&self.core, &request)
    }

    /// `{ok: true}` or `{error}`. Found devices arrive as events.
    #[napi]
    pub fn discover(&self, request: Value) -> Value {
        json::discover(&self.core, &request)
    }

    /// `{ok}` or `{error}`. Never rejects for a device or validation error.
    #[napi]
    pub async fn execute(
        &self,
        device: f64,
        command: String,
        params: Option<Value>,
    ) -> Result<Value> {
        let device = device_id(device)?;
        let params = params.unwrap_or(Value::Null);
        Ok(json::execute(&self.core, device, &command, &params).await)
    }

    #[napi]
    pub fn snapshot(&self, device: f64) -> Result<Value> {
        Ok(json::snapshot(&self.core, device_id(device)?))
    }

    /// Queued events, without waiting.
    #[napi]
    pub fn poll_events(&self, max: Option<u32>) -> Value {
        json::events(&self.core.poll_events(max.unwrap_or(256) as usize))
    }

    /// Wait for at least one event.
    #[napi]
    pub async fn next_events(&self, max: Option<u32>) -> Value {
        json::events(&self.core.next_events(max.unwrap_or(256) as usize).await)
    }

    /// Make a pending `nextEvents` return, empty if nothing is queued.
    #[napi]
    pub fn interrupt_events(&self) {
        self.core.interrupt_events();
    }

    #[napi]
    pub async fn close(&self, device: f64) -> Result<()> {
        self.core.close(device_id(device)?).await;
        Ok(())
    }
}
