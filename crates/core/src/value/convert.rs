//! Conversions to and from plain `serde_json` values, for consumers whose own
//! code works with those. Object keys keep their order going in; coming out,
//! they follow whatever order `serde_json::Map` keeps in the consumer's build.

use super::{Map, Value};

impl From<serde_json::Value> for Value {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(b),
            serde_json::Value::Number(n) => Value::Number(n),
            serde_json::Value::String(s) => Value::String(s),
            serde_json::Value::Array(a) => Value::Array(a.into_iter().map(Value::from).collect()),
            serde_json::Value::Object(o) => Value::Object(o.into()),
        }
    }
}

impl From<Value> for serde_json::Value {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(b),
            Value::Number(n) => serde_json::Value::Number(n),
            Value::String(s) => serde_json::Value::String(s),
            Value::Array(a) => serde_json::Value::Array(a.into_iter().map(Into::into).collect()),
            Value::Object(o) => serde_json::Value::Object(o.into()),
        }
    }
}

impl From<serde_json::Map<String, serde_json::Value>> for Map<String, Value> {
    fn from(map: serde_json::Map<String, serde_json::Value>) -> Self {
        map.into_iter().map(|(k, v)| (k, v.into())).collect()
    }
}

impl From<Map<String, Value>> for serde_json::Map<String, serde_json::Value> {
    fn from(map: Map<String, Value>) -> Self {
        map.into_iter().map(|(k, v)| (k, v.into())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_keep_their_key_order() {
        let v: Value = serde_json::from_str(r#"{"b":1,"a":{"z":[true,null],"y":"s"}}"#).unwrap();
        assert_eq!(v.to_string(), r#"{"b":1,"a":{"z":[true,null],"y":"s"}}"#);
        let made = crate::json!({"b": 1, "a": 2});
        assert_eq!(made.to_string(), r#"{"b":1,"a":2}"#);
        let plain: serde_json::Value = v.clone().into();
        assert_eq!(Value::from(plain)["a"]["y"], "s");
    }
}
