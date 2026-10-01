//! PTP datasets the Sony module reads and writes: PTP's typed values and
//! strings, the standard DeviceInfo, Sony's extension info (the property and
//! control codes a camera supports) and its property description array.
//!
//! All PTP data is little-endian. A PTP string is a one-byte count of UTF-16
//! code units including the terminating NUL (zero for an empty string),
//! followed by those units. An array is a `u32` element count followed by the
//! elements.

use serde_json::{json, Value};

/// PTP datatype codes.
pub(crate) mod dt {
    pub const INT8: u16 = 0x0001;
    pub const UINT8: u16 = 0x0002;
    pub const INT16: u16 = 0x0003;
    pub const UINT16: u16 = 0x0004;
    pub const INT32: u16 = 0x0005;
    pub const UINT32: u16 = 0x0006;
    pub const INT64: u16 = 0x0007;
    pub const UINT64: u16 = 0x0008;
    pub const STR: u16 = 0xFFFF;
    /// Added to a scalar type for an array of it.
    pub const ARRAY: u16 = 0x4000;
}

/// The name a datatype is reported under in the state.
pub(crate) fn type_name(datatype: u16) -> String {
    let scalar = match datatype & !dt::ARRAY {
        dt::INT8 => "int8",
        dt::UINT8 => "uint8",
        dt::INT16 => "int16",
        dt::UINT16 => "uint16",
        dt::INT32 => "int32",
        dt::UINT32 => "uint32",
        dt::INT64 => "int64",
        dt::UINT64 => "uint64",
        _ if datatype == dt::STR => return "string".into(),
        _ => return format!("0x{datatype:04X}"),
    };
    if datatype & dt::ARRAY != 0 && datatype != dt::STR {
        format!("{scalar}[]")
    } else {
        scalar.into()
    }
}

/// The datatype for a name as `type_name` gives it, for scalar types.
pub(crate) fn type_from_name(name: &str) -> Option<u16> {
    Some(match name {
        "int8" => dt::INT8,
        "uint8" => dt::UINT8,
        "int16" => dt::INT16,
        "uint16" => dt::UINT16,
        "int32" => dt::INT32,
        "uint32" => dt::UINT32,
        "int64" => dt::INT64,
        "uint64" => dt::UINT64,
        "string" => dt::STR,
        _ => return None,
    })
}

/// Byte size of a scalar integer type, `None` for anything else.
fn int_size(datatype: u16) -> Option<usize> {
    Some(match datatype {
        dt::INT8 | dt::UINT8 => 1,
        dt::INT16 | dt::UINT16 => 2,
        dt::INT32 | dt::UINT32 => 4,
        dt::INT64 | dt::UINT64 => 8,
        _ => return None,
    })
}

fn is_signed(datatype: u16) -> bool {
    matches!(datatype, dt::INT8 | dt::INT16 | dt::INT32 | dt::INT64)
}

/// The inclusive range of a scalar integer type.
pub(crate) fn int_bounds(datatype: u16) -> Option<(i128, i128)> {
    let bits = int_size(datatype)? as u32 * 8;
    Some(if is_signed(datatype) {
        (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    } else {
        (0, (1i128 << bits) - 1)
    })
}

/// One PTP value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PtpValue {
    /// Any integer type, held wide enough for `u64` and `i64` alike.
    Int(i128),
    Str(String),
    Array(Vec<i128>),
}

impl PtpValue {
    pub(crate) fn as_int(&self) -> Option<i128> {
        match self {
            PtpValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    pub(crate) fn to_json(&self) -> Value {
        fn num(v: i128) -> Value {
            if let Ok(i) = i64::try_from(v) {
                json!(i)
            } else if let Ok(u) = u64::try_from(v) {
                json!(u)
            } else {
                json!(v.to_string())
            }
        }
        match self {
            PtpValue::Int(v) => num(*v),
            PtpValue::Str(s) => json!(s),
            PtpValue::Array(items) => Value::Array(items.iter().map(|v| num(*v)).collect()),
        }
    }
}

/// Reads PTP data front to back.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes, at: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .at
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| format!("dataset ends early at byte {}", self.at))?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, String> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes(b.try_into().unwrap()))
    }

    pub(crate) fn int(&mut self, datatype: u16) -> Result<i128, String> {
        let size =
            int_size(datatype).ok_or_else(|| format!("0x{datatype:04X} is not an integer type"))?;
        let b = self.take(size)?;
        let mut raw = [0u8; 8];
        raw[..size].copy_from_slice(b);
        let unsigned = u64::from_le_bytes(raw);
        Ok(if is_signed(datatype) {
            let shift = 64 - size as u32 * 8;
            (((unsigned << shift) as i64) >> shift) as i128
        } else {
            unsigned as i128
        })
    }

    pub(crate) fn string(&mut self) -> Result<String, String> {
        let count = self.u8()? as usize;
        let mut units = Vec::with_capacity(count);
        for _ in 0..count {
            units.push(self.u16()?);
        }
        while units.last() == Some(&0) {
            units.pop();
        }
        Ok(String::from_utf16_lossy(&units))
    }

    pub(crate) fn u16_array(&mut self) -> Result<Vec<u16>, String> {
        let count = self.u32()? as usize;
        if count > self.remaining() / 2 {
            return Err(format!("array of {count} does not fit the dataset"));
        }
        (0..count).map(|_| self.u16()).collect()
    }

    pub(crate) fn value(&mut self, datatype: u16) -> Result<PtpValue, String> {
        if datatype == dt::STR {
            return Ok(PtpValue::Str(self.string()?));
        }
        if datatype & dt::ARRAY != 0 {
            let element = datatype & !dt::ARRAY;
            let size = int_size(element)
                .ok_or_else(|| format!("unsupported array type 0x{datatype:04X}"))?;
            let count = self.u32()? as usize;
            if count > self.remaining() / size {
                return Err(format!("array of {count} does not fit the dataset"));
            }
            return Ok(PtpValue::Array(
                (0..count)
                    .map(|_| self.int(element))
                    .collect::<Result<_, _>>()?,
            ));
        }
        Ok(PtpValue::Int(self.int(datatype)?))
    }
}

pub(crate) fn write_string(out: &mut Vec<u8>, text: &str) {
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.is_empty() {
        out.push(0);
        return;
    }
    let units = &units[..units.len().min(254)];
    out.push(units.len() as u8 + 1);
    for u in units {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out.extend_from_slice(&[0, 0]);
}

/// Encodes a value as the given datatype. Integers out of the type's range
/// are refused, never wrapped.
pub(crate) fn encode(datatype: u16, value: &PtpValue) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    match (datatype, value) {
        (dt::STR, PtpValue::Str(s)) => write_string(&mut out, s),
        (_, PtpValue::Int(v)) => {
            let size = int_size(datatype)
                .ok_or_else(|| format!("cannot write an integer as type 0x{datatype:04X}"))?;
            let (lo, hi) = int_bounds(datatype).unwrap();
            if *v < lo || *v > hi {
                return Err(format!(
                    "{v} is outside the range of {}",
                    type_name(datatype)
                ));
            }
            out.extend_from_slice(&(*v as u64).to_le_bytes()[..size]);
        }
        _ => {
            return Err(format!(
                "a {} value cannot be written as {}",
                match value {
                    PtpValue::Int(_) => "number",
                    PtpValue::Str(_) => "string",
                    PtpValue::Array(_) => "array",
                },
                type_name(datatype)
            ))
        }
    }
    Ok(out)
}

/// The codes a camera supports, from SDIO_GetExtDeviceInfo.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ExtDeviceInfo {
    /// Extension version, 100 times the real version (0x012C is 3.00).
    pub version: u16,
    pub properties: Vec<u16>,
    pub controls: Vec<u16>,
}

pub(crate) fn parse_ext_device_info(bytes: &[u8]) -> Result<ExtDeviceInfo, String> {
    let mut r = Reader::new(bytes);
    Ok(ExtDeviceInfo {
        version: r.u16()?,
        properties: r.u16_array()?,
        controls: r.u16_array()?,
    })
}

/// The identifying strings of the standard DeviceInfo dataset.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct DeviceInfo {
    pub manufacturer: String,
    pub model: String,
    pub version: String,
    pub serial: String,
}

pub(crate) fn parse_device_info(bytes: &[u8]) -> Result<DeviceInfo, String> {
    let mut r = Reader::new(bytes);
    r.u16()?; // standard version
    r.u32()?; // vendor extension id
    r.u16()?; // vendor extension version
    r.string()?; // vendor extension description
    r.u16()?; // functional mode
    for _ in 0..5 {
        // operations, events, properties, capture formats, image formats
        r.u16_array()?;
    }
    Ok(DeviceInfo {
        manufacturer: r.string()?,
        model: r.string()?,
        version: r.string()?,
        serial: r.string()?,
    })
}

/// What values a property accepts.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Form {
    None,
    Range {
        min: PtpValue,
        max: PtpValue,
        step: PtpValue,
    },
    /// `settable` lists what may be written now; `values` what the camera
    /// may report.
    Enum {
        settable: Vec<PtpValue>,
        values: Vec<PtpValue>,
    },
}

/// Whether the camera offers a property right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Enabled {
    /// Greyed out: the value is not meaningful.
    No,
    Yes,
    /// Shown, but cannot be changed.
    DisplayOnly,
}

impl Enabled {
    fn from_u8(v: u8) -> Enabled {
        match v {
            0 => Enabled::No,
            2 => Enabled::DisplayOnly,
            _ => Enabled::Yes,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Enabled::No => "disabled",
            Enabled::Yes => "enabled",
            Enabled::DisplayOnly => "display_only",
        }
    }
}

/// One property as the camera describes it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PropInfo {
    pub code: u16,
    pub datatype: u16,
    pub writable: bool,
    pub enabled: Enabled,
    pub current: PtpValue,
    pub form: Form,
}

impl PropInfo {
    /// The property as the state reports it under `properties`.
    pub(crate) fn to_json(&self) -> Value {
        let mut v = json!({
            "value": self.current.to_json(),
            "type": type_name(self.datatype),
            "writable": self.writable,
            "enabled": self.enabled.name(),
        });
        match &self.form {
            Form::None => {}
            Form::Range { min, max, step } => {
                v["range"] =
                    json!({"min": min.to_json(), "max": max.to_json(), "step": step.to_json()});
            }
            Form::Enum { settable, .. } => {
                v["options"] = Value::Array(settable.iter().map(PtpValue::to_json).collect());
            }
        }
        v
    }
}

fn parse_prop_info(r: &mut Reader) -> Result<PropInfo, String> {
    let code = r.u16()?;
    let datatype = r.u16()?;
    let writable = r.u8()? == 1;
    let enabled = Enabled::from_u8(r.u8()?);
    let _factory_default = r.value(datatype)?;
    let current = r.value(datatype)?;
    let form = match r.u8()? {
        1 => Form::Range {
            min: r.value(datatype)?,
            max: r.value(datatype)?,
            step: r.value(datatype)?,
        },
        2 => {
            let mut list = || -> Result<Vec<PtpValue>, String> {
                let count = r.u16()? as usize;
                (0..count).map(|_| r.value(datatype)).collect()
            };
            let settable = list()?;
            let values = list()?;
            Form::Enum { settable, values }
        }
        _ => Form::None,
    };
    Ok(PropInfo {
        code,
        datatype,
        writable,
        enabled,
        current,
        form,
    })
}

/// The dataset array SDIO_GetAllExtDevicePropInfo returns.
pub(crate) fn parse_prop_info_array(bytes: &[u8]) -> Result<Vec<PropInfo>, String> {
    let mut r = Reader::new(bytes);
    let count = r.u64()?;
    let mut out = Vec::new();
    for i in 0..count {
        let info = parse_prop_info(&mut r).map_err(|e| format!("property {i} of {count}: {e}"))?;
        out.push(info);
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod build {
    //! Builds datasets for tests, from the layouts above.
    use super::*;

    pub(crate) fn int(datatype: u16, v: i128) -> Vec<u8> {
        encode(datatype, &PtpValue::Int(v)).unwrap()
    }

    /// A property description with an enumeration form.
    pub(crate) fn enum_prop(
        code: u16,
        datatype: u16,
        writable: bool,
        enabled: u8,
        current: i128,
        settable: &[i128],
    ) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&code.to_le_bytes());
        b.extend_from_slice(&datatype.to_le_bytes());
        b.push(writable as u8);
        b.push(enabled);
        b.extend(int(datatype, 0));
        b.extend(int(datatype, current));
        b.push(2);
        for list in [settable, settable] {
            b.extend_from_slice(&(list.len() as u16).to_le_bytes());
            for v in list {
                b.extend(int(datatype, *v));
            }
        }
        b
    }

    /// A property description with a range form.
    pub(crate) fn range_prop(
        code: u16,
        datatype: u16,
        writable: bool,
        current: i128,
        (min, max, step): (i128, i128, i128),
    ) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&code.to_le_bytes());
        b.extend_from_slice(&datatype.to_le_bytes());
        b.push(writable as u8);
        b.push(1);
        b.extend(int(datatype, 0));
        b.extend(int(datatype, current));
        b.push(1);
        for v in [min, max, step] {
            b.extend(int(datatype, v));
        }
        b
    }

    /// A string property without a form.
    pub(crate) fn string_prop(code: u16, current: &str) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&code.to_le_bytes());
        b.extend_from_slice(&dt::STR.to_le_bytes());
        b.push(0);
        b.push(1);
        write_string(&mut b, "");
        write_string(&mut b, current);
        b.push(0);
        b
    }

    pub(crate) fn prop_array(props: &[Vec<u8>]) -> Vec<u8> {
        let mut b = (props.len() as u64).to_le_bytes().to_vec();
        for p in props {
            b.extend_from_slice(p);
        }
        b
    }

    pub(crate) fn ext_device_info(version: u16, props: &[u16], controls: &[u16]) -> Vec<u8> {
        let mut b = version.to_le_bytes().to_vec();
        for list in [props, controls] {
            b.extend_from_slice(&(list.len() as u32).to_le_bytes());
            for c in list {
                b.extend_from_slice(&c.to_le_bytes());
            }
        }
        b
    }

    pub(crate) fn device_info(
        manufacturer: &str,
        model: &str,
        version: &str,
        serial: &str,
    ) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&100u16.to_le_bytes());
        b.extend_from_slice(&0x11u32.to_le_bytes());
        b.extend_from_slice(&100u16.to_le_bytes());
        write_string(&mut b, "");
        b.extend_from_slice(&0u16.to_le_bytes());
        for list in [&[0x1001u16, 0x1002][..], &[], &[], &[], &[]] {
            b.extend_from_slice(&(list.len() as u32).to_le_bytes());
            for c in list {
                b.extend_from_slice(&c.to_le_bytes());
            }
        }
        for s in [manufacturer, model, version, serial] {
            write_string(&mut b, s);
        }
        b
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    #[test]
    fn integers_read_with_their_sign() {
        let mut r = Reader::new(&[0xFF, 0xFE, 0xFF, 0x78, 0xEC]);
        assert_eq!(r.int(dt::INT8).unwrap(), -1);
        assert_eq!(r.int(dt::UINT16).unwrap(), 0xFFFE);
        // 0xEC78 is -5000.
        assert_eq!(r.int(dt::INT16).unwrap(), -5000);
    }

    #[test]
    fn signed_and_unsigned_64_bit_values() {
        let bytes = [0xFFu8; 8];
        assert_eq!(
            Reader::new(&bytes).int(dt::UINT64).unwrap(),
            u64::MAX as i128
        );
        assert_eq!(Reader::new(&bytes).int(dt::INT64).unwrap(), -1);
        assert_eq!(PtpValue::Int(u64::MAX as i128).to_json(), json!(u64::MAX));
    }

    #[test]
    fn strings_round_trip() {
        let mut b = Vec::new();
        write_string(&mut b, "ILCE-1");
        assert_eq!(b[0], 7, "six characters and the NUL");
        assert_eq!(Reader::new(&b).string().unwrap(), "ILCE-1");
        let mut empty = Vec::new();
        write_string(&mut empty, "");
        assert_eq!(empty, [0]);
        assert_eq!(Reader::new(&empty).string().unwrap(), "");
    }

    #[test]
    fn encoding_refuses_out_of_range_values() {
        assert_eq!(
            encode(dt::UINT16, &PtpValue::Int(0x8001)).unwrap(),
            [0x01, 0x80]
        );
        assert_eq!(encode(dt::INT8, &PtpValue::Int(-2)).unwrap(), [0xFE]);
        assert!(encode(dt::UINT8, &PtpValue::Int(256)).is_err());
        assert!(encode(dt::INT8, &PtpValue::Int(-129)).is_err());
        assert!(encode(dt::UINT16, &PtpValue::Str("x".into())).is_err());
    }

    #[test]
    fn ext_device_info_lists_codes() {
        let info =
            parse_ext_device_info(&ext_device_info(0x012C, &[0x5005, 0xD21D], &[0xD2C1])).unwrap();
        assert_eq!(
            info,
            ExtDeviceInfo {
                version: 0x012C,
                properties: vec![0x5005, 0xD21D],
                controls: vec![0xD2C1]
            }
        );
    }

    #[test]
    fn device_info_strings() {
        let info = parse_device_info(&device_info("Sony Corporation", "ILME-FX3", "2.00", "1234"))
            .unwrap();
        assert_eq!(info.model, "ILME-FX3");
        assert_eq!(info.serial, "1234");
    }

    #[test]
    fn property_array_with_each_form() {
        let bytes = prop_array(&[
            enum_prop(0x5005, dt::UINT16, true, 1, 0x0002, &[0x0002, 0x0004]),
            range_prop(0xD00D, dt::INT8, true, -5, (-99, 99, 1)),
            string_prop(0xD040, "1.10"),
        ]);
        let props = parse_prop_info_array(&bytes).unwrap();
        assert_eq!(props.len(), 3);
        assert_eq!(props[0].code, 0x5005);
        assert_eq!(props[0].current, PtpValue::Int(2));
        assert_eq!(
            props[0].form,
            Form::Enum {
                settable: vec![PtpValue::Int(2), PtpValue::Int(4)],
                values: vec![PtpValue::Int(2), PtpValue::Int(4)]
            }
        );
        assert_eq!(props[1].current, PtpValue::Int(-5));
        assert_eq!(
            props[1].to_json(),
            json!({"value": -5, "type": "int8", "writable": true, "enabled": "enabled",
                   "range": {"min": -99, "max": 99, "step": 1}})
        );
        assert_eq!(props[2].current, PtpValue::Str("1.10".into()));
        assert!(!props[2].writable);
    }

    #[test]
    fn a_truncated_array_is_an_error() {
        let mut bytes = prop_array(&[enum_prop(0x5005, dt::UINT16, true, 1, 2, &[2])]);
        bytes.truncate(bytes.len() - 1);
        assert!(parse_prop_info_array(&bytes).is_err());
        // A count larger than the data is not trusted either.
        let mut bad = 5u64.to_le_bytes().to_vec();
        bad.extend(enum_prop(0x5005, dt::UINT16, true, 1, 2, &[2]));
        assert!(parse_prop_info_array(&bad).is_err());
    }

    #[test]
    fn array_values() {
        let mut b = 2u32.to_le_bytes().to_vec();
        b.extend_from_slice(&[1, 0, 2, 0]);
        assert_eq!(
            Reader::new(&b).value(dt::ARRAY | dt::UINT16).unwrap(),
            PtpValue::Array(vec![1, 2])
        );
        assert_eq!(type_name(dt::ARRAY | dt::UINT16), "uint16[]");
    }
}
