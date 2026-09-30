//! The JSON surface every binding exposes.
//!
//! Bindings pass JSON in and out and do nothing else, so the shape of every
//! value a consumer sees is defined here, once. A binding that serialised
//! results itself could differ from the others in a field name or an omitted
//! null; this module is why none can.

use serde_json::{json, Value};

use crate::{CommandResult, Core, CoreOptions, DeviceId, OpenError, OpenRequest, Params};

/// Parse core options. `null` or absent means the defaults.
pub fn core_options(value: &Value) -> Result<CoreOptions, String> {
    match value {
        Value::Null => Ok(CoreOptions::default()),
        other => {
            serde_json::from_value(other.clone()).map_err(|e| format!("invalid core options: {e}"))
        }
    }
}

/// `{"device": <id>}` or `{"error": {...}}`.
pub fn open(core: &Core, request: &Value) -> Value {
    let request: OpenRequest = match serde_json::from_value(request.clone()) {
        Ok(r) => r,
        Err(e) => return json!({"error": {"error": "invalid_request", "message": e.to_string()}}),
    };
    match core.open(request) {
        Ok(device) => json!({ "device": device }),
        Err(e) => json!({ "error": open_error(&e) }),
    }
}

fn open_error(e: &OpenError) -> Value {
    let mut v = serde_json::to_value(e).expect("OpenError serialises");
    v["message"] = json!(e.to_string());
    v
}

/// Parse command parameters. `null` or absent means none.
pub fn params(value: &Value) -> Result<Params, Value> {
    match value {
        Value::Null => Ok(Params::new()),
        Value::Object(map) => Ok(map.clone()),
        _ => Err(
            json!({"error": {"error": "invalid_params", "message": "params must be an object"}}),
        ),
    }
}

/// `{"ok": <outcome>}` or `{"error": <error>}`, each with a readable `message`
/// on errors.
pub fn result(result: &CommandResult) -> Value {
    match result {
        Ok(outcome) => json!({ "ok": outcome }),
        Err(e) => {
            let mut v = serde_json::to_value(e).expect("CommandError serialises");
            v["message"] = json!(e.to_string());
            json!({ "error": v })
        }
    }
}

pub async fn execute(core: &Core, device: DeviceId, command: &str, params: &Value) -> Value {
    match self::params(params) {
        Ok(p) => result(&core.execute(device, command, p).await),
        Err(e) => e,
    }
}

pub fn catalog(core: &Core) -> Value {
    serde_json::to_value(core.catalog()).expect("catalog serialises")
}

/// The snapshot, or `null` for a device that is not open.
pub fn snapshot(core: &Core, device: DeviceId) -> Value {
    core.snapshot(device)
        .map(|s| serde_json::to_value(s).expect("snapshot serialises"))
        .unwrap_or(Value::Null)
}

pub fn events(events: &[crate::Event]) -> Value {
    serde_json::to_value(events).expect("events serialise")
}
