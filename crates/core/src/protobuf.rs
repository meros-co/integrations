//! Protocol Buffers (proto3) messages as JSON, for native modules that speak
//! gRPC or another protobuf protocol.
//!
//! The codec is driven by a [`Schema`]: static tables of the messages and
//! enums a protocol defines, generated from its `.proto` files (each
//! integration's generator in `tools/` writes them). Nothing is compiled
//! from a `.proto` file at build time, so no code generator or `protoc` is
//! needed.
//!
//! Encoding follows the protobuf wire format (protobuf.dev, "Encoding"):
//! each field a tag of its number and wire type, varints for integers,
//! booleans and enums (zigzag for `sint32`/`sint64`), little-endian 4 and 8
//! bytes for the fixed types, `float` and `double`, and length-delimited
//! strings, bytes, messages and packed repeated scalars. A map is a repeated
//! message of `key` (1) and `value` (2).
//!
//! JSON has one shape both ways. A message is an object whose keys are its
//! field names in snake_case (`cuelistId` is `cuelist_id`); when encoding,
//! the proto's own spelling is taken too. Enums are their value names as
//! the `.proto` file writes them (a number the schema does not name is
//! decoded as that number, and a number is accepted when encoding), bytes
//! are base64, 64-bit integers are JSON numbers (or decimal strings when
//! encoding), maps are objects keyed by the key as text, and repeated fields
//! are arrays. Decoding writes every singular scalar field of proto3's
//! implicit presence, its default when the message leaves it out (0, false,
//! "", the enum's first value), and every repeated and map field, so a
//! decoded message always has the same keys; a message field or a `oneof`
//! member appears only when present. Unknown fields are skipped, as proto3
//! requires. Nothing caps a message's size.

use crate::value::{Map, Value};
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use base64::Engine;

/// Standard base64 (RFC 4648), written padded and read with or without.
const BASE64: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// A field's type. Every proto3 type is here, though a schema may use
/// only some (DMXControl's has none of the fixed types).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Double,
    Float,
    Int32,
    Int64,
    Uint32,
    Uint64,
    Sint32,
    Sint64,
    Fixed32,
    Fixed64,
    Sfixed32,
    Sfixed64,
    Bool,
    String,
    Bytes,
    /// An enum, by its full name.
    Enum(&'static str),
    /// A message, by its full name.
    Message(&'static str),
}

/// How many values a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Label {
    Single,
    Repeated,
    /// `map<key, value>`, with the key's kind; the field's kind is the value's.
    Map(Kind),
}

#[derive(Debug)]
pub(crate) struct Field {
    /// The name as the `.proto` file writes it.
    pub name: &'static str,
    /// The name in JSON: snake_case.
    pub json: &'static str,
    pub number: u32,
    pub kind: Kind,
    pub label: Label,
    /// The `oneof` the field belongs to, if any.
    pub oneof: Option<&'static str>,
}

#[derive(Debug)]
pub(crate) struct MessageDef {
    /// The full name, package included (`LumosProtobuf.GetRequest`).
    pub name: &'static str,
    pub fields: &'static [Field],
}

#[derive(Debug)]
pub(crate) struct EnumDef {
    pub name: &'static str,
    pub values: &'static [(&'static str, i32)],
}

/// A protocol's messages and enums, each table sorted by name.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Schema {
    pub messages: &'static [MessageDef],
    pub enums: &'static [EnumDef],
}

impl Schema {
    pub(crate) fn message(&self, name: &str) -> Option<&'static MessageDef> {
        self.messages
            .binary_search_by(|m| m.name.cmp(name))
            .ok()
            .map(|i| &self.messages[i])
    }

    pub(crate) fn enumeration(&self, name: &str) -> Option<&'static EnumDef> {
        self.enums
            .binary_search_by(|e| e.name.cmp(name))
            .ok()
            .map(|i| &self.enums[i])
    }

    /// Encode a JSON object as the named message.
    pub(crate) fn encode(&self, message: &str, value: &Value) -> Result<Vec<u8>, String> {
        let def = self
            .message(message)
            .ok_or_else(|| format!("no message {message} in the schema"))?;
        let mut out = Vec::new();
        self.encode_message(def, value, &mut out, message)?;
        Ok(out)
    }

    /// Decode bytes as the named message.
    pub(crate) fn decode(&self, message: &str, data: &[u8]) -> Result<Value, String> {
        let def = self
            .message(message)
            .ok_or_else(|| format!("no message {message} in the schema"))?;
        self.decode_message(def, data, 0)
    }

    fn encode_message(
        &self,
        def: &MessageDef,
        value: &Value,
        out: &mut Vec<u8>,
        path: &str,
    ) -> Result<(), String> {
        let object = value
            .as_object()
            .ok_or_else(|| format!("{path} must be an object"))?;
        let mut oneofs: Vec<&str> = Vec::new();
        // Fields are written in the order of their numbers, as protobuf's
        // own serializers write them.
        let mut given = Vec::with_capacity(object.len());
        for (key, v) in object {
            let field = def
                .fields
                .iter()
                .find(|f| f.json == key || f.name == key)
                .ok_or_else(|| format!("{path} has no field '{key}'"))?;
            given.push((field, key, v));
        }
        given.sort_by_key(|(f, _, _)| f.number);
        for (field, key, v) in given {
            let here = format!("{path}.{key}");
            if v.is_null() {
                continue;
            }
            if let Some(group) = field.oneof {
                if oneofs.contains(&group) {
                    return Err(format!(
                        "{path} sets more than one of its '{group}' choices"
                    ));
                }
                oneofs.push(group);
            }
            match field.label {
                Label::Single => self.encode_field(field.number, field.kind, v, out, &here)?,
                Label::Repeated => {
                    let items = v
                        .as_array()
                        .ok_or_else(|| format!("{here} must be an array"))?;
                    if packable(field.kind) {
                        let mut packed = Vec::new();
                        for (i, item) in items.iter().enumerate() {
                            self.encode_scalar(
                                field.kind,
                                item,
                                &mut packed,
                                &format!("{here}[{i}]"),
                            )?;
                        }
                        if !packed.is_empty() {
                            tag(field.number, 2, out);
                            varint(packed.len() as u64, out);
                            out.extend_from_slice(&packed);
                        }
                    } else {
                        for (i, item) in items.iter().enumerate() {
                            self.encode_field(
                                field.number,
                                field.kind,
                                item,
                                out,
                                &format!("{here}[{i}]"),
                            )?;
                        }
                    }
                }
                Label::Map(key_kind) => {
                    let entries = v
                        .as_object()
                        .ok_or_else(|| format!("{here} must be an object"))?;
                    for (k, item) in entries {
                        let mut entry = Vec::new();
                        let key_value = map_key_value(key_kind, k)
                            .ok_or_else(|| format!("{here} key '{k}' is not a {key_kind:?}"))?;
                        self.encode_field(1, key_kind, &key_value, &mut entry, &here)?;
                        self.encode_field(2, field.kind, item, &mut entry, &format!("{here}.{k}"))?;
                        tag(field.number, 2, out);
                        varint(entry.len() as u64, out);
                        out.extend_from_slice(&entry);
                    }
                }
            }
        }
        Ok(())
    }

    fn encode_field(
        &self,
        number: u32,
        kind: Kind,
        v: &Value,
        out: &mut Vec<u8>,
        path: &str,
    ) -> Result<(), String> {
        match kind {
            Kind::String => {
                let s = v
                    .as_str()
                    .ok_or_else(|| format!("{path} must be a string"))?;
                tag(number, 2, out);
                varint(s.len() as u64, out);
                out.extend_from_slice(s.as_bytes());
            }
            Kind::Bytes => {
                let s = v
                    .as_str()
                    .ok_or_else(|| format!("{path} must be base64 text"))?;
                let bytes = BASE64
                    .decode(s)
                    .map_err(|_| format!("{path} is not base64"))?;
                tag(number, 2, out);
                varint(bytes.len() as u64, out);
                out.extend_from_slice(&bytes);
            }
            Kind::Message(name) => {
                let def = self
                    .message(name)
                    .ok_or_else(|| format!("no message {name} in the schema"))?;
                let mut inner = Vec::new();
                self.encode_message(def, v, &mut inner, path)?;
                tag(number, 2, out);
                varint(inner.len() as u64, out);
                out.extend_from_slice(&inner);
            }
            _ => {
                tag(number, wire_type(kind), out);
                self.encode_scalar(kind, v, out, path)?;
            }
        }
        Ok(())
    }

    /// A scalar's value without its tag: what a packed field repeats.
    fn encode_scalar(
        &self,
        kind: Kind,
        v: &Value,
        out: &mut Vec<u8>,
        path: &str,
    ) -> Result<(), String> {
        match kind {
            Kind::Double => out.extend_from_slice(&number(v, path)?.to_le_bytes()),
            Kind::Float => out.extend_from_slice(&(number(v, path)? as f32).to_le_bytes()),
            Kind::Int32 => varint(int(v, path, i32::MIN as i64, i32::MAX as i64)? as u64, out),
            Kind::Int64 => varint(int(v, path, i64::MIN, i64::MAX)? as u64, out),
            Kind::Uint32 => varint(uint(v, path, u32::MAX as u64)?, out),
            Kind::Uint64 => varint(uint(v, path, u64::MAX)?, out),
            Kind::Sint32 => {
                let n = int(v, path, i32::MIN as i64, i32::MAX as i64)?;
                varint(((n << 1) ^ (n >> 63)) as u64, out)
            }
            Kind::Sint64 => {
                let n = int(v, path, i64::MIN, i64::MAX)?;
                varint(((n << 1) ^ (n >> 63)) as u64, out)
            }
            Kind::Fixed32 => {
                out.extend_from_slice(&(uint(v, path, u32::MAX as u64)? as u32).to_le_bytes())
            }
            Kind::Fixed64 => out.extend_from_slice(&uint(v, path, u64::MAX)?.to_le_bytes()),
            Kind::Sfixed32 => out.extend_from_slice(
                &(int(v, path, i32::MIN as i64, i32::MAX as i64)? as i32).to_le_bytes(),
            ),
            Kind::Sfixed64 => {
                out.extend_from_slice(&int(v, path, i64::MIN, i64::MAX)?.to_le_bytes())
            }
            Kind::Bool => {
                let b = v
                    .as_bool()
                    .ok_or_else(|| format!("{path} must be true or false"))?;
                out.push(u8::from(b));
            }
            Kind::Enum(name) => {
                let n = self.enum_number(name, v, path)?;
                varint(n as i64 as u64, out);
            }
            Kind::String | Kind::Bytes | Kind::Message(_) => {
                return Err(format!("{path} cannot be packed"))
            }
        }
        Ok(())
    }

    fn enum_number(&self, name: &str, v: &Value, path: &str) -> Result<i32, String> {
        let def = self
            .enumeration(name)
            .ok_or_else(|| format!("no enum {name} in the schema"))?;
        match v {
            Value::String(s) => def
                .values
                .iter()
                .find(|(n, _)| n == s)
                .map(|(_, number)| *number)
                .ok_or_else(|| {
                    format!(
                        "{path} '{s}' is not one of {}",
                        def.values
                            .iter()
                            .map(|(n, _)| *n)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }),
            Value::Number(_) => Ok(int(v, path, i32::MIN as i64, i32::MAX as i64)? as i32),
            _ => Err(format!("{path} must be an enum value name")),
        }
    }

    fn decode_message(&self, def: &MessageDef, data: &[u8], depth: usize) -> Result<Value, String> {
        if depth > 100 {
            return Err(format!("{} nests too deep", def.name));
        }
        let mut object = Map::new();
        let mut maps: Vec<(usize, Map<String, Value>)> = Vec::new();
        let mut pos = 0;
        while pos < data.len() {
            let (key, n) = read_varint(&data[pos..]).ok_or("truncated tag")?;
            pos += n;
            let number = (key >> 3) as u32;
            let wire = (key & 7) as u8;
            let start = pos;
            pos += skip(wire, &data[pos..])
                .ok_or_else(|| format!("{}: truncated field {number}", def.name))?;
            let raw = &data[start..pos];
            let Some((index, field)) = def
                .fields
                .iter()
                .enumerate()
                .find(|(_, f)| f.number == number)
            else {
                continue;
            };
            match field.label {
                Label::Single => {
                    if let Some(group) = field.oneof {
                        // A later member of the same oneof replaces an earlier one.
                        for other in def.fields.iter().filter(|f| f.oneof == Some(group)) {
                            object.remove(other.json);
                        }
                    }
                    let value = self.decode_value(field.kind, wire, raw, depth)?;
                    match (field.kind, object.get_mut(field.json)) {
                        // Two occurrences of a message field merge.
                        (Kind::Message(_), Some(Value::Object(old))) => {
                            if let Value::Object(new) = value {
                                for (k, v) in new {
                                    old.insert(k, v);
                                }
                            }
                        }
                        _ => {
                            object.insert(field.json.to_string(), value);
                        }
                    }
                }
                Label::Repeated => {
                    let list = object
                        .entry(field.json.to_string())
                        .or_insert_with(|| Value::Array(Vec::new()));
                    let Value::Array(list) = list else {
                        unreachable!()
                    };
                    if wire == 2 && packable(field.kind) {
                        let (len, n) = read_varint(raw).ok_or("truncated length")?;
                        let mut body = &raw[n..n + len as usize];
                        while !body.is_empty() {
                            let size = scalar_size(field.kind, body)
                                .ok_or_else(|| format!("{}: truncated packed field", def.name))?;
                            list.push(self.decode_scalar(field.kind, &body[..size])?);
                            body = &body[size..];
                        }
                    } else {
                        list.push(self.decode_value(field.kind, wire, raw, depth)?);
                    }
                }
                Label::Map(key_kind) => {
                    let (len, n) = read_varint(raw).ok_or("truncated length")?;
                    let entry = &raw[n..n + len as usize];
                    let (k, v) = self.decode_entry(key_kind, field.kind, entry, depth)?;
                    match maps.iter_mut().find(|(i, _)| *i == index) {
                        Some((_, m)) => {
                            m.insert(k, v);
                        }
                        None => {
                            let mut m = Map::new();
                            m.insert(k, v);
                            maps.push((index, m));
                        }
                    }
                }
            }
        }
        for (index, m) in maps {
            object.insert(def.fields[index].json.to_string(), Value::Object(m));
        }
        // proto3's implicit presence: every singular scalar outside a oneof,
        // and every list and map, has a value.
        for field in def.fields {
            if object.contains_key(field.json) || field.oneof.is_some() {
                continue;
            }
            let default = match (field.label, field.kind) {
                (Label::Repeated, _) => Value::Array(Vec::new()),
                (Label::Map(_), _) => Value::Object(Map::new()),
                (Label::Single, Kind::Message(_)) => continue,
                (Label::Single, kind) => self.default_value(kind),
            };
            object.insert(field.json.to_string(), default);
        }
        Ok(Value::Object(object))
    }

    fn default_value(&self, kind: Kind) -> Value {
        match kind {
            Kind::Double | Kind::Float => Value::from(0.0),
            Kind::Bool => Value::Bool(false),
            Kind::String | Kind::Bytes => Value::String(String::new()),
            Kind::Enum(name) => self.enum_value(name, 0),
            Kind::Message(_) => Value::Null,
            _ => Value::from(0),
        }
    }

    fn enum_value(&self, name: &str, number: i64) -> Value {
        self.enumeration(name)
            .and_then(|def| def.values.iter().find(|(_, n)| i64::from(*n) == number))
            .map(|(n, _)| Value::String(n.to_string()))
            .unwrap_or_else(|| Value::from(number))
    }

    fn decode_entry(
        &self,
        key_kind: Kind,
        value_kind: Kind,
        data: &[u8],
        depth: usize,
    ) -> Result<(String, Value), String> {
        let mut key = None;
        let mut value = None;
        let mut pos = 0;
        while pos < data.len() {
            let (tag, n) = read_varint(&data[pos..]).ok_or("truncated map entry")?;
            pos += n;
            let wire = (tag & 7) as u8;
            let start = pos;
            pos += skip(wire, &data[pos..]).ok_or("truncated map entry")?;
            match tag >> 3 {
                1 => key = Some(self.decode_value(key_kind, wire, &data[start..pos], depth)?),
                2 => value = Some(self.decode_value(value_kind, wire, &data[start..pos], depth)?),
                _ => {}
            }
        }
        let key = match key.unwrap_or_else(|| self.default_value(key_kind)) {
            Value::String(s) => s,
            other => other.to_string(),
        };
        let value = match value {
            Some(v) => v,
            None => match value_kind {
                Kind::Message(name) => match self.message(name) {
                    Some(def) => self.decode_message(def, &[], depth + 1)?,
                    None => Value::Object(Map::new()),
                },
                kind => self.default_value(kind),
            },
        };
        Ok((key, value))
    }

    /// One field's value from its raw bytes (tag excluded).
    fn decode_value(
        &self,
        kind: Kind,
        wire: u8,
        raw: &[u8],
        depth: usize,
    ) -> Result<Value, String> {
        match kind {
            Kind::String | Kind::Bytes | Kind::Message(_) => {
                if wire != 2 {
                    return Err(format!("a {kind:?} field with wire type {wire}"));
                }
                let (len, n) = read_varint(raw).ok_or("truncated length")?;
                let body = &raw[n..n + len as usize];
                match kind {
                    Kind::String => Ok(Value::String(String::from_utf8_lossy(body).into_owned())),
                    Kind::Bytes => Ok(Value::String(BASE64.encode(body))),
                    Kind::Message(name) => match self.message(name) {
                        Some(def) => self.decode_message(def, body, depth + 1),
                        None => Err(format!("no message {name} in the schema")),
                    },
                    _ => unreachable!(),
                }
            }
            _ => {
                if wire != wire_type(kind) {
                    return Err(format!("a {kind:?} field with wire type {wire}"));
                }
                self.decode_scalar(kind, raw)
            }
        }
    }

    fn decode_scalar(&self, kind: Kind, raw: &[u8]) -> Result<Value, String> {
        let v = || read_varint(raw).map(|(v, _)| v).ok_or("truncated varint");
        let fixed4 = || -> Result<[u8; 4], String> {
            raw.get(..4)
                .and_then(|b| b.try_into().ok())
                .ok_or_else(|| "truncated fixed32".to_string())
        };
        let fixed8 = || -> Result<[u8; 8], String> {
            raw.get(..8)
                .and_then(|b| b.try_into().ok())
                .ok_or_else(|| "truncated fixed64".to_string())
        };
        Ok(match kind {
            Kind::Double => float_value(f64::from_le_bytes(fixed8()?)),
            Kind::Float => float_value(f64::from(f32::from_le_bytes(fixed4()?))),
            Kind::Int32 => Value::from(v()? as i64 as i32),
            Kind::Int64 => Value::from(v()? as i64),
            Kind::Uint32 => Value::from(v()? as u32),
            Kind::Uint64 => Value::from(v()?),
            Kind::Sint32 | Kind::Sint64 => {
                let n = v()?;
                Value::from(((n >> 1) as i64) ^ -((n & 1) as i64))
            }
            Kind::Fixed32 => Value::from(u32::from_le_bytes(fixed4()?)),
            Kind::Fixed64 => Value::from(u64::from_le_bytes(fixed8()?)),
            Kind::Sfixed32 => Value::from(i32::from_le_bytes(fixed4()?)),
            Kind::Sfixed64 => Value::from(i64::from_le_bytes(fixed8()?)),
            Kind::Bool => Value::Bool(v()? != 0),
            Kind::Enum(name) => self.enum_value(name, v()? as i64 as i32 as i64),
            Kind::String | Kind::Bytes | Kind::Message(_) => {
                return Err(format!("{kind:?} is not a scalar"))
            }
        })
    }
}

/// A float as JSON: NaN and the infinities, which JSON has no number for,
/// as the text protobuf's JSON mapping uses.
fn float_value(f: f64) -> Value {
    if f.is_finite() {
        Value::from(f)
    } else if f.is_nan() {
        Value::String("NaN".into())
    } else if f > 0.0 {
        Value::String("Infinity".into())
    } else {
        Value::String("-Infinity".into())
    }
}

fn packable(kind: Kind) -> bool {
    !matches!(kind, Kind::String | Kind::Bytes | Kind::Message(_))
}

fn wire_type(kind: Kind) -> u8 {
    match kind {
        Kind::Double | Kind::Fixed64 | Kind::Sfixed64 => 1,
        Kind::Float | Kind::Fixed32 | Kind::Sfixed32 => 5,
        Kind::String | Kind::Bytes | Kind::Message(_) => 2,
        _ => 0,
    }
}

/// The length of one packed scalar at the start of `data`.
fn scalar_size(kind: Kind, data: &[u8]) -> Option<usize> {
    match wire_type(kind) {
        0 => read_varint(data).map(|(_, n)| n),
        1 => (data.len() >= 8).then_some(8),
        5 => (data.len() >= 4).then_some(4),
        _ => None,
    }
}

/// How far a field's value runs, from just after its tag.
fn skip(wire: u8, data: &[u8]) -> Option<usize> {
    match wire {
        0 => read_varint(data).map(|(_, n)| n),
        1 => (data.len() >= 8).then_some(8),
        2 => {
            let (len, n) = read_varint(data)?;
            let end = n.checked_add(usize::try_from(len).ok()?)?;
            (end <= data.len()).then_some(end)
        }
        5 => (data.len() >= 4).then_some(4),
        _ => None,
    }
}

fn tag(number: u32, wire: u8, out: &mut Vec<u8>) {
    varint((u64::from(number) << 3) | u64::from(wire), out);
}

pub(crate) fn varint(mut n: u64, out: &mut Vec<u8>) {
    while n >= 0x80 {
        out.push((n as u8) | 0x80);
        n >>= 7;
    }
    out.push(n as u8);
}

pub(crate) fn read_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut n = 0u64;
    for (i, b) in data.iter().enumerate().take(10) {
        n |= u64::from(b & 0x7f) << (7 * i);
        if b & 0x80 == 0 {
            return Some((n, i + 1));
        }
    }
    None
}

fn number(v: &Value, path: &str) -> Result<f64, String> {
    match v {
        Value::Number(n) => n.as_f64().ok_or_else(|| format!("{path} is not a number")),
        Value::String(s) => match s.as_str() {
            "NaN" => Ok(f64::NAN),
            "Infinity" => Ok(f64::INFINITY),
            "-Infinity" => Ok(f64::NEG_INFINITY),
            _ => s.parse().map_err(|_| format!("{path} is not a number")),
        },
        _ => Err(format!("{path} must be a number")),
    }
}

fn int(v: &Value, path: &str, min: i64, max: i64) -> Result<i64, String> {
    let n = match v {
        Value::Number(n) => n.as_i64().or_else(|| {
            n.as_f64()
                .filter(|f| f.fract() == 0.0 && f.abs() < 9.2e18)
                .map(|f| f as i64)
        }),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
    .ok_or_else(|| format!("{path} must be a whole number"))?;
    if n < min || n > max {
        return Err(format!("{path} {n} is out of range"));
    }
    Ok(n)
}

fn uint(v: &Value, path: &str, max: u64) -> Result<u64, String> {
    let n = match v {
        Value::Number(n) => n.as_u64().or_else(|| {
            n.as_f64()
                .filter(|f| f.fract() == 0.0 && *f >= 0.0 && *f < 1.8e19)
                .map(|f| f as u64)
        }),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
    .ok_or_else(|| format!("{path} must be a whole number of 0 or more"))?;
    if n > max {
        return Err(format!("{path} {n} is out of range"));
    }
    Ok(n)
}

/// A map key, given as object text, as the JSON value its kind encodes.
fn map_key_value(kind: Kind, key: &str) -> Option<Value> {
    match kind {
        Kind::String => Some(Value::String(key.to_string())),
        Kind::Bool => match key {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        Kind::Uint32 | Kind::Uint64 | Kind::Fixed32 | Kind::Fixed64 => {
            key.parse::<u64>().ok().map(Value::from)
        }
        _ => key.parse::<i64>().ok().map(Value::from),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json;

    static FIELDS_INNER: &[Field] = &[
        Field {
            name: "r",
            json: "r",
            number: 1,
            kind: Kind::Int32,
            label: Label::Single,
            oneof: None,
        },
        Field {
            name: "g",
            json: "g",
            number: 2,
            kind: Kind::Int32,
            label: Label::Single,
            oneof: None,
        },
    ];
    static FIELDS_OUTER: &[Field] = &[
        Field {
            name: "requestId",
            json: "request_id",
            number: 2,
            kind: Kind::String,
            label: Label::Single,
            oneof: None,
        },
        Field {
            name: "cuelistId",
            json: "cuelist_id",
            number: 3,
            kind: Kind::String,
            label: Label::Single,
            oneof: None,
        },
        Field {
            name: "intensity",
            json: "intensity",
            number: 11,
            kind: Kind::Double,
            label: Label::Single,
            oneof: Some("value"),
        },
        Field {
            name: "fadeFactor",
            json: "fade_factor",
            number: 12,
            kind: Kind::Double,
            label: Label::Single,
            oneof: Some("value"),
        },
        Field {
            name: "state",
            json: "state",
            number: 4,
            kind: Kind::Enum("T.State"),
            label: Label::Single,
            oneof: None,
        },
        Field {
            name: "numbers",
            json: "numbers",
            number: 5,
            kind: Kind::Sint32,
            label: Label::Repeated,
            oneof: None,
        },
        Field {
            name: "names",
            json: "names",
            number: 6,
            kind: Kind::String,
            label: Label::Repeated,
            oneof: None,
        },
        Field {
            name: "color",
            json: "color",
            number: 7,
            kind: Kind::Message("T.Color"),
            label: Label::Single,
            oneof: None,
        },
        Field {
            name: "byKey",
            json: "by_key",
            number: 8,
            kind: Kind::Double,
            label: Label::Map(Kind::Int32),
            oneof: None,
        },
        Field {
            name: "raw",
            json: "raw",
            number: 9,
            kind: Kind::Bytes,
            label: Label::Single,
            oneof: None,
        },
        Field {
            name: "big",
            json: "big",
            number: 10,
            kind: Kind::Uint64,
            label: Label::Single,
            oneof: None,
        },
        Field {
            name: "f",
            json: "f",
            number: 13,
            kind: Kind::Float,
            label: Label::Single,
            oneof: None,
        },
        Field {
            name: "flag",
            json: "flag",
            number: 14,
            kind: Kind::Bool,
            label: Label::Single,
            oneof: None,
        },
    ];
    static MESSAGES: &[MessageDef] = &[
        MessageDef {
            name: "T.Color",
            fields: FIELDS_INNER,
        },
        MessageDef {
            name: "T.Outer",
            fields: FIELDS_OUTER,
        },
    ];
    static ENUMS: &[EnumDef] = &[EnumDef {
        name: "T.State",
        values: &[("STOPPED", 0), ("PAUSED", 1), ("RUNNING", 2)],
    }];
    const SCHEMA: Schema = Schema {
        messages: MESSAGES,
        enums: ENUMS,
    };

    #[test]
    fn a_message_encodes_as_the_wire_format_states() {
        // Stated by hand from the encoding rules, in field number order
        // whatever the order given: field 3 string "c1" (1a 02 63 31), field
        // 4 enum 2 (20 02), field 5 packed sint32 [-1, 1] as zigzag 1 and 2
        // (2a 02 01 02), field 7 message {r: 255} (3a 03 08 ff 01), field 12
        // double 0.5 (61 then 8 little-endian bytes).
        let bytes = SCHEMA
            .encode(
                "T.Outer",
                &json!({"cuelist_id": "c1", "fadeFactor": 0.5, "state": "RUNNING",
                        "numbers": [-1, 1], "color": {"r": 255}}),
            )
            .unwrap();
        let mut expected = vec![
            0x1a, 0x02, b'c', b'1', 0x20, 0x02, 0x2a, 0x02, 0x01, 0x02, 0x3a, 0x03, 0x08, 0xff,
            0x01, 0x61,
        ];
        expected.extend_from_slice(&0.5f64.to_le_bytes());
        assert_eq!(bytes, expected);
    }

    #[test]
    fn decoding_fills_proto3_defaults_and_round_trips() {
        let value = json!({"request_id": "r", "cuelist_id": "c1", "intensity": 0.25, "state": "PAUSED",
            "numbers": [-5, 0, 70000], "names": ["a", "b"], "color": {"r": 1, "g": -2},
            "by_key": {"-3": 1.5, "7": 2.0}, "raw": "AAEC", "big": 18446744073709551615u64, "f": 0.5, "flag": true});
        let bytes = SCHEMA.encode("T.Outer", &value).unwrap();
        assert_eq!(SCHEMA.decode("T.Outer", &bytes).unwrap(), value);
        // An empty message has every scalar, list and map at its default,
        // and no message field or oneof member.
        assert_eq!(
            SCHEMA.decode("T.Outer", &[]).unwrap(),
            json!({"request_id": "", "cuelist_id": "", "state": "STOPPED", "numbers": [], "names": [],
                   "by_key": {}, "raw": "", "big": 0, "f": 0.0, "flag": false})
        );
    }

    #[test]
    fn unpacked_lists_unknown_fields_and_unknown_enum_numbers_are_read() {
        // numbers unpacked (28 02 28 03: -1 and +2... zigzag 2 = 1, 3 = -2),
        // an unknown field 99 (varint), and state 9, which no value names.
        let bytes = [0x28, 0x02, 0x28, 0x03, 0x98, 0x06, 0x01, 0x20, 0x09];
        let v = SCHEMA.decode("T.Outer", &bytes).unwrap();
        assert_eq!(v["numbers"], json!([1, -2]));
        assert_eq!(v["state"], json!(9));
    }

    #[test]
    fn bad_values_are_refused_before_sending() {
        assert!(SCHEMA.encode("T.Outer", &json!({"nope": 1})).is_err());
        assert!(SCHEMA.encode("T.Outer", &json!({"state": "GONE"})).is_err());
        assert!(
            SCHEMA
                .encode("T.Outer", &json!({"intensity": 1.0, "fade_factor": 2.0}))
                .is_err(),
            "two members of a oneof"
        );
        assert!(SCHEMA
            .encode("T.Outer", &json!({"color": {"r": 3000000000u64}}))
            .is_err());
        assert!(SCHEMA.encode("T.Outer", &json!({"raw": "*"})).is_err());
        assert!(
            SCHEMA.decode("T.Outer", &[0x1a, 0x05, b'a']).is_err(),
            "truncated"
        );
    }

    #[test]
    fn a_later_oneof_member_replaces_an_earlier_one() {
        let mut bytes = SCHEMA
            .encode("T.Outer", &json!({"intensity": 1.0}))
            .unwrap();
        bytes.extend(
            SCHEMA
                .encode("T.Outer", &json!({"fade_factor": 2.0}))
                .unwrap(),
        );
        let v = SCHEMA.decode("T.Outer", &bytes).unwrap();
        assert_eq!(v.get("intensity"), None);
        assert_eq!(v["fade_factor"], json!(2.0));
    }
}
