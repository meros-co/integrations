//! Node binding for the integrations core.
//!
//! Every method passes JSON through `meros_integrations::json` and nothing
//! else, so this binding cannot differ from the others in what it returns. The
//! idiomatic JavaScript surface (promises that reject, an EventEmitter) is in
//! `index.js`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use meros_integrations::{json, Core, StreamHandle};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value;

#[napi(js_name = "NativeCore")]
pub struct NativeCore {
    core: Arc<Core>,
    /// Open streams, by the id handed to JavaScript.
    streams: Arc<Mutex<HashMap<u32, Arc<StreamHandle>>>>,
    next_stream: AtomicU32,
}

/// One frame of a stream: the encoded bytes and what the core says about them.
#[napi(object)]
pub struct NativeFrame {
    /// `jpeg`.
    pub format: String,
    pub data: Buffer,
    /// Rises by one per frame the device published.
    pub sequence: i64,
    /// Frames replaced unseen since the last one taken.
    pub dropped: i64,
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
    /// `options`: CoreOptions JSON (`bind_address`, `devices`), or null.
    #[napi(constructor)]
    pub fn new(options: Option<Value>) -> Result<Self> {
        let options =
            json::core_options(&options.unwrap_or(Value::Null)).map_err(Error::from_reason)?;
        let core = Core::with_options(options).map_err(|e| Error::from_reason(e.to_string()))?;
        Ok(NativeCore {
            core: Arc::new(core),
            streams: Default::default(),
            next_stream: AtomicU32::new(1),
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

    /// `{ok: true}` or `{error}`: merge settings into an open device's.
    #[napi]
    pub async fn update_settings(&self, device: f64, settings: Value) -> Result<Value> {
        let device = device_id(device)?;
        Ok(json::update_settings(&self.core, device, &settings).await)
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

    /// `{stream: <id>}` or `{error}`. The device produces frames while the
    /// stream is open.
    #[napi]
    pub fn open_stream(&self, device: f64, stream: String) -> Result<Value> {
        match json::open_stream(&self.core, device_id(device)?, &stream) {
            Ok(handle) => {
                let id = self.next_stream.fetch_add(1, Ordering::Relaxed);
                self.streams.lock().unwrap().insert(id, Arc::new(handle));
                Ok(serde_json::json!({ "stream": id }))
            }
            Err(error) => Ok(error),
        }
    }

    /// The next frame, waiting for it; `null` once the stream is closed or
    /// its device's session has ended. Only the newest frame is kept.
    #[napi]
    pub async fn next_frame(&self, stream: u32) -> Option<NativeFrame> {
        let handle = self.streams.lock().unwrap().get(&stream).cloned()?;
        let frame = handle.next_frame().await?;
        Some(NativeFrame {
            format: frame.format.to_string(),
            data: frame.data.to_vec().into(),
            sequence: frame.sequence as i64,
            dropped: frame.dropped as i64,
        })
    }

    /// Stop watching; a pending `nextFrame` resolves `null`.
    #[napi]
    pub fn close_stream(&self, stream: u32) {
        if let Some(handle) = self.streams.lock().unwrap().remove(&stream) {
            handle.close();
        }
    }

    #[napi]
    pub async fn close(&self, device: f64) -> Result<()> {
        self.core.close(device_id(device)?).await;
        Ok(())
    }

    #[napi]
    pub async fn close_all(&self, grace_ms: u32) -> Result<()> {
        self.core
            .close_all(std::time::Duration::from_millis(grace_ms as u64))
            .await;
        Ok(())
    }
}
