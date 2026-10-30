//! The JSON value the core works with: serde_json's `Value`, with objects that
//! always keep their keys in insertion order.
//!
//! Devices read query strings, JSON bodies and signed fields in the order a
//! spec writes them, so the core cannot use sorted maps. serde_json keeps
//! order only behind its `preserve_order` feature, and Cargo would turn that
//! feature on for every crate built with the core. This module is serde_json's
//! value code (1.0.151, MIT, third-party/LICENSE-serde_json.txt) with
//! that feature always on, so the core keeps order without changing serde_json
//! for anyone else. Parsing and printing still go through serde_json.
//!
//! Consumers whose own code uses plain `serde_json` values convert with
//! `From` either way (convert.rs).

macro_rules! tri {
    ($e:expr $(,)?) => {
        match $e {
            Ok(val) => val,
            Err(err) => return Err(err),
        }
    };
}

use serde::de::DeserializeOwned;
use serde::ser::Serialize;
use serde_json::Error;
use std::fmt::{self, Debug, Display};
use std::io;
use std::mem;
use std::str;
use std::string::String;
use std::vec::Vec;

pub use self::index::Index;
pub use self::map::Map;
pub use self::ser::Serializer;
pub use serde_json::Number;

#[derive(Clone, Eq, PartialEq, Hash, Default)]
pub enum Value {
    #[default]
    Null,

    Bool(bool),

    Number(Number),

    String(String),

    Array(Vec<Value>),

    Object(Map<String, Value>),
}

impl Debug for Value {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Null => formatter.write_str("Null"),
            Value::Bool(boolean) => write!(formatter, "Bool({})", boolean),
            Value::Number(number) => Debug::fmt(number, formatter),
            Value::String(string) => write!(formatter, "String({:?})", string),
            Value::Array(vec) => {
                tri!(formatter.write_str("Array "));
                Debug::fmt(vec, formatter)
            }
            Value::Object(map) => {
                tri!(formatter.write_str("Object "));
                Debug::fmt(map, formatter)
            }
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct WriterFormatter<'a, 'b: 'a> {
            inner: &'a mut fmt::Formatter<'b>,
        }

        impl<'a, 'b> io::Write for WriterFormatter<'a, 'b> {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                // Safety: the serializer below only emits valid utf8 when using
                // the default formatter.
                let s = unsafe { str::from_utf8_unchecked(buf) };
                tri!(self.inner.write_str(s).map_err(io_error));
                Ok(buf.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        fn io_error(_: fmt::Error) -> io::Error {
            // Error value does not matter because Display impl just maps it
            // back to fmt::Error.
            io::Error::other("fmt error")
        }

        let alternate = f.alternate();
        let mut wr = WriterFormatter { inner: f };
        if alternate {
            // {:#}
            serde_json::to_writer_pretty(&mut wr, self).map_err(|_| fmt::Error)
        } else {
            // {}
            serde_json::to_writer(&mut wr, self).map_err(|_| fmt::Error)
        }
    }
}

fn parse_index(s: &str) -> Option<usize> {
    if s.starts_with('+') || (s.starts_with('0') && s.len() != 1) {
        return None;
    }
    s.parse().ok()
}

impl Value {
    pub fn get<I: Index>(&self, index: I) -> Option<&Value> {
        index.index_into(self)
    }

    pub fn get_mut<I: Index>(&mut self, index: I) -> Option<&mut Value> {
        index.index_into_mut(self)
    }

    pub fn is_object(&self) -> bool {
        self.as_object().is_some()
    }

    pub fn as_object(&self) -> Option<&Map<String, Value>> {
        match self {
            Value::Object(map) => Some(map),
            _ => None,
        }
    }

    pub fn as_object_mut(&mut self) -> Option<&mut Map<String, Value>> {
        match self {
            Value::Object(map) => Some(map),
            _ => None,
        }
    }

    pub fn is_array(&self) -> bool {
        self.as_array().is_some()
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(array) => Some(array),
            _ => None,
        }
    }

    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Value::Array(list) => Some(list),
            _ => None,
        }
    }

    pub fn is_string(&self) -> bool {
        self.as_str().is_some()
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn is_number(&self) -> bool {
        matches!(*self, Value::Number(_))
    }

    pub fn as_number(&self) -> Option<&Number> {
        match self {
            Value::Number(number) => Some(number),
            _ => None,
        }
    }

    pub fn is_i64(&self) -> bool {
        match self {
            Value::Number(n) => n.is_i64(),
            _ => false,
        }
    }

    pub fn is_u64(&self) -> bool {
        match self {
            Value::Number(n) => n.is_u64(),
            _ => false,
        }
    }

    pub fn is_f64(&self) -> bool {
        match self {
            Value::Number(n) => n.is_f64(),
            _ => false,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Number(n) => n.as_i64(),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::Number(n) => n.as_u64(),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => n.as_f64(),
            _ => None,
        }
    }

    pub fn is_boolean(&self) -> bool {
        self.as_bool().is_some()
    }

    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Value::Bool(b) => Some(b),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        self.as_null().is_some()
    }

    pub fn as_null(&self) -> Option<()> {
        match *self {
            Value::Null => Some(()),
            _ => None,
        }
    }

    pub fn pointer(&self, pointer: &str) -> Option<&Value> {
        if pointer.is_empty() {
            return Some(self);
        }
        if !pointer.starts_with('/') {
            return None;
        }
        pointer
            .split('/')
            .skip(1)
            .map(|x| x.replace("~1", "/").replace("~0", "~"))
            .try_fold(self, |target, token| match target {
                Value::Object(map) => map.get(&token),
                Value::Array(list) => parse_index(&token).and_then(|x| list.get(x)),
                _ => None,
            })
    }

    pub fn pointer_mut(&mut self, pointer: &str) -> Option<&mut Value> {
        if pointer.is_empty() {
            return Some(self);
        }
        if !pointer.starts_with('/') {
            return None;
        }
        pointer
            .split('/')
            .skip(1)
            .map(|x| x.replace("~1", "/").replace("~0", "~"))
            .try_fold(self, |target, token| match target {
                Value::Object(map) => map.get_mut(&token),
                Value::Array(list) => parse_index(&token).and_then(move |x| list.get_mut(x)),
                _ => None,
            })
    }

    pub fn take(&mut self) -> Value {
        mem::replace(self, Value::Null)
    }

    pub fn sort_all_objects(&mut self) {
        {
            match self {
                Value::Object(map) => {
                    map.sort_keys();
                    map.values_mut().for_each(Value::sort_all_objects);
                }
                Value::Array(list) => {
                    list.iter_mut().for_each(Value::sort_all_objects);
                }
                _ => {}
            }
        }
    }
}

impl Default for &Value {
    fn default() -> Self {
        const DEFAULT: Value = Value::Null;
        &DEFAULT
    }
}

mod convert;
mod de;
mod from;
#[macro_use]
mod macros;
mod index;
mod map;
mod partial_eq;
mod ser;

// Taking by value is more friendly to iterator adapters, option and result
// consumers, etc. See https://github.com/serde-rs/json/pull/149.
pub fn to_value<T>(value: T) -> Result<Value, Error>
where
    T: Serialize,
{
    value.serialize(Serializer)
}

pub fn from_value<T>(value: Value) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    T::deserialize(value)
}
