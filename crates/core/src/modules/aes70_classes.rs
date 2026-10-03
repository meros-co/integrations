//! The AES70 classes the module knows by name, and the properties it reads,
//! keeps current and sets: their ids, getter and setter methods and types.
//!
//! Class ids, method ids and property ids are those of AES70-2 as listed in
//! tschiemer/ocac `docs/occ_classes.md` (AES70-2-2018 Annex A, MIT) and in
//! PADL/SwiftOCA's control classes (Apache-2.0), which agree wherever both
//! list an item. A class id is a path from OcaRoot (1): a subclass's id
//! starts with its parent's, so a proprietary subclass of OcaGain
//! (1.1.1.5.65535...) is handled as an OcaGain.

use serde_json::{json, Value};

use super::codec::{Decoded, Id, Reader, Writer};

/// Class names by id, for state.
pub(crate) const CLASSES: &[(&[u16], &str)] = &[
    (&[1], "OcaRoot"),
    (&[1, 1], "OcaWorker"),
    (&[1, 1, 1], "OcaActuator"),
    (&[1, 1, 1, 1], "OcaBasicActuator"),
    (&[1, 1, 1, 1, 1], "OcaBooleanActuator"),
    (&[1, 1, 1, 1, 2], "OcaInt8Actuator"),
    (&[1, 1, 1, 1, 3], "OcaInt16Actuator"),
    (&[1, 1, 1, 1, 4], "OcaInt32Actuator"),
    (&[1, 1, 1, 1, 5], "OcaInt64Actuator"),
    (&[1, 1, 1, 1, 6], "OcaUint8Actuator"),
    (&[1, 1, 1, 1, 7], "OcaUint16Actuator"),
    (&[1, 1, 1, 1, 8], "OcaUint32Actuator"),
    (&[1, 1, 1, 1, 9], "OcaUint64Actuator"),
    (&[1, 1, 1, 1, 10], "OcaFloat32Actuator"),
    (&[1, 1, 1, 1, 11], "OcaFloat64Actuator"),
    (&[1, 1, 1, 1, 12], "OcaStringActuator"),
    (&[1, 1, 1, 1, 13], "OcaBitstringActuator"),
    (&[1, 1, 1, 2], "OcaMute"),
    (&[1, 1, 1, 3], "OcaPolarity"),
    (&[1, 1, 1, 4], "OcaSwitch"),
    (&[1, 1, 1, 5], "OcaGain"),
    (&[1, 1, 1, 6], "OcaPanBalance"),
    (&[1, 1, 1, 7], "OcaDelay"),
    (&[1, 1, 1, 7, 1], "OcaDelayExtended"),
    (&[1, 1, 1, 8], "OcaFrequencyActuator"),
    (&[1, 1, 1, 9], "OcaFilterClassical"),
    (&[1, 1, 1, 10], "OcaFilterParametric"),
    (&[1, 1, 1, 11], "OcaFilterPolynomial"),
    (&[1, 1, 1, 12], "OcaFilterFIR"),
    (&[1, 1, 1, 13], "OcaFilterArbitraryCurve"),
    (&[1, 1, 1, 14], "OcaDynamics"),
    (&[1, 1, 1, 15], "OcaDynamicsDetector"),
    (&[1, 1, 1, 16], "OcaDynamicsCurve"),
    (&[1, 1, 1, 17], "OcaSignalGenerator"),
    (&[1, 1, 1, 18], "OcaSignalInput"),
    (&[1, 1, 1, 19], "OcaSignalOutput"),
    (&[1, 1, 1, 20], "OcaTemperatureActuator"),
    (&[1, 1, 1, 21], "OcaIdentificationActuator"),
    (&[1, 1, 1, 22], "OcaSummingPoint"),
    (&[1, 1, 1, 23], "OcaSamplingRateConverter"),
    (&[1, 1, 2], "OcaSensor"),
    (&[1, 1, 2, 1], "OcaBasicSensor"),
    (&[1, 1, 2, 1, 1], "OcaBooleanSensor"),
    (&[1, 1, 2, 1, 2], "OcaInt8Sensor"),
    (&[1, 1, 2, 1, 3], "OcaInt16Sensor"),
    (&[1, 1, 2, 1, 4], "OcaInt32Sensor"),
    (&[1, 1, 2, 1, 5], "OcaInt64Sensor"),
    (&[1, 1, 2, 1, 6], "OcaUint8Sensor"),
    (&[1, 1, 2, 1, 7], "OcaUint16Sensor"),
    (&[1, 1, 2, 1, 8], "OcaUint32Sensor"),
    (&[1, 1, 2, 1, 9], "OcaUint64Sensor"),
    (&[1, 1, 2, 1, 10], "OcaFloat32Sensor"),
    (&[1, 1, 2, 1, 11], "OcaFloat64Sensor"),
    (&[1, 1, 2, 1, 12], "OcaStringSensor"),
    (&[1, 1, 2, 2], "OcaLevelSensor"),
    (&[1, 1, 2, 2, 1], "OcaAudioLevelSensor"),
    (&[1, 1, 2, 3], "OcaTimeIntervalSensor"),
    (&[1, 1, 2, 4], "OcaFrequencySensor"),
    (&[1, 1, 2, 5], "OcaTemperatureSensor"),
    (&[1, 1, 2, 6], "OcaIdentificationSensor"),
    (&[1, 1, 2, 7], "OcaVoltageSensor"),
    (&[1, 1, 2, 8], "OcaCurrentSensor"),
    (&[1, 1, 2, 9], "OcaImpedanceSensor"),
    (&[1, 1, 2, 10], "OcaGainSensor"),
    (&[1, 1, 2, 11], "OcaPowerSensor"),
    (&[1, 1, 2, 12], "OcaStateSensor"),
    (&[1, 1, 3], "OcaBlock"),
    (&[1, 1, 4], "OcaBlockFactory"),
    (&[1, 1, 5], "OcaMatrix"),
    (&[1, 2], "OcaAgent"),
    (&[1, 2, 5], "OcaEventHandler"),
    (&[1, 2, 7], "OcaPowerSupply"),
    (&[1, 2, 15], "OcaMediaClock3"),
    (&[1, 2, 16], "OcaTimeSource"),
    (&[1, 2, 17], "OcaPhysicalPosition"),
    (&[1, 2, 18], "OcaCounterNotifier"),
    (&[1, 2, 19], "OcaCounterSetAgent"),
    (&[1, 2, 20], "OcaMediaTransportSessionAgent"),
    (&[1, 2, 22], "OcaGroup"),
    (&[1, 3], "OcaManager"),
    (&[1, 3, 1], "OcaDeviceManager"),
    (&[1, 3, 2], "OcaSecurityManager"),
    (&[1, 3, 3], "OcaFirmwareManager"),
    (&[1, 3, 4], "OcaSubscriptionManager"),
    (&[1, 3, 5], "OcaPowerManager"),
    (&[1, 3, 6], "OcaNetworkManager"),
    (&[1, 3, 7], "OcaMediaClockManager"),
    (&[1, 3, 8], "OcaLibraryManager"),
    (&[1, 3, 9], "OcaAudioProcessingManager"),
    (&[1, 3, 10], "OcaDeviceTimeManager"),
    (&[1, 3, 11], "OcaTaskManager"),
    (&[1, 3, 12], "OcaCodingManager"),
    (&[1, 3, 13], "OcaDiagnosticManager"),
    (&[1, 3, 14], "OcaLockManager"),
    (&[1, 4], "OcaApplicationNetwork"),
    (&[1, 4, 1], "OcaControlNetwork"),
    (&[1, 5], "OcaDataset"),
];

pub(crate) const BLOCK: &[u16] = &[1, 1, 3];
pub(crate) const GAIN: &[u16] = &[1, 1, 1, 5];
pub(crate) const MUTE: &[u16] = &[1, 1, 1, 2];
pub(crate) const POLARITY: &[u16] = &[1, 1, 1, 3];
pub(crate) const SWITCH: &[u16] = &[1, 1, 1, 4];
pub(crate) const DELAY: &[u16] = &[1, 1, 1, 7];
pub(crate) const IDENTIFICATION: &[u16] = &[1, 1, 1, 21];
pub(crate) const BOOLEAN_ACTUATOR: &[u16] = &[1, 1, 1, 1, 1];
pub(crate) const INT_ACTUATORS: &[&[u16]] = &[
    &[1, 1, 1, 1, 2],
    &[1, 1, 1, 1, 3],
    &[1, 1, 1, 1, 4],
    &[1, 1, 1, 1, 5],
    &[1, 1, 1, 1, 6],
    &[1, 1, 1, 1, 7],
    &[1, 1, 1, 1, 8],
    &[1, 1, 1, 1, 9],
];
pub(crate) const FLOAT_ACTUATORS: &[&[u16]] = &[&[1, 1, 1, 1, 10], &[1, 1, 1, 1, 11]];
pub(crate) const STRING_ACTUATOR: &[u16] = &[1, 1, 1, 1, 12];
pub(crate) const LEVEL_SENSOR: &[u16] = &[1, 1, 2, 2];
pub(crate) const DEVICE_MANAGER: &[u16] = &[1, 3, 1];

/// `class` is `parent` or one of its subclasses.
pub(crate) fn is_a(class: &[u16], parent: &[u16]) -> bool {
    class.starts_with(parent)
}

/// The name of exactly this class, if known.
pub(crate) fn exact_name(class: &[u16]) -> Option<&'static str> {
    CLASSES.iter().find(|(id, _)| *id == class).map(|(_, n)| *n)
}

/// The name of the nearest known class this one is, or derives from.
pub(crate) fn kind(class: &[u16]) -> Option<&'static str> {
    CLASSES
        .iter()
        .filter(|(id, _)| is_a(class, id))
        .max_by_key(|(id, _)| id.len())
        .map(|(_, n)| *n)
}

/// How a property's value is marshalled, and shown in state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ty {
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Str,
    /// OcaTimeInterval, seconds: OcaFloat32 in AES70-2018 (ocac), OcaFloat64
    /// in SwiftOCA. Read by its length; written as wide as the device sent.
    TimeInterval,
    /// OcaMuteState: 1 muted, 2 unmuted; a boolean `muted` in state.
    Mute,
    /// OcaPolarityState: 1 non-inverted, 2 inverted; a boolean `inverted`.
    Polarity,
    /// OcaSensorReadingState (u8): unknown, valid, underrange, overrange,
    /// error.
    SensorState,
    /// OcaDeviceState (bit set 16): operational, disabled, error,
    /// initializing, updating; the names of the bits set.
    DeviceState,
    /// OcaModelGUID: reserved byte, manufacturer code (3), model code (4); hex.
    ModelGuid,
    /// OcaModelDescription: manufacturer, name and version strings.
    ModelDescription,
    StrList,
    BoolList,
}

/// One property the module knows.
#[derive(Debug)]
pub(crate) struct Prop {
    /// The class that declares it; subclasses have it too.
    pub class: &'static [u16],
    pub id: Id,
    /// Its key in state.
    pub key: &'static str,
    pub get: Option<Id>,
    pub set: Option<Id>,
    pub ty: Ty,
    /// The getter returns the value, its minimum and its maximum.
    pub bounded: bool,
    /// Read on connecting and kept current.
    pub read: bool,
}

#[allow(clippy::too_many_arguments)]
const fn p(
    class: &'static [u16],
    id: (u16, u16),
    key: &'static str,
    get: Option<(u16, u16)>,
    set: Option<(u16, u16)>,
    ty: Ty,
    bounded: bool,
    read: bool,
) -> Prop {
    Prop {
        class,
        id: Id(id.0, id.1),
        key,
        get: match get {
            Some((l, i)) => Some(Id(l, i)),
            None => None,
        },
        set: match set {
            Some((l, i)) => Some(Id(l, i)),
            None => None,
        },
        ty,
        bounded,
        read,
    }
}

#[rustfmt::skip]
pub(crate) const PROPS: &[Prop] = &[
    // OcaRoot
    p(&[1], (1, 5), "role", Some((1, 5)), None, Ty::Str, false, false),
    // OcaWorker
    p(&[1, 1], (2, 1), "enabled", Some((2, 1)), Some((2, 2)), Ty::Bool, false, false),
    p(&[1, 1], (2, 3), "label", Some((2, 8)), Some((2, 9)), Ty::Str, false, false),
    // Actuators
    p(MUTE, (4, 1), "muted", Some((4, 1)), Some((4, 2)), Ty::Mute, false, true),
    p(POLARITY, (4, 1), "inverted", Some((4, 1)), Some((4, 2)), Ty::Polarity, false, true),
    p(SWITCH, (4, 1), "position", Some((4, 1)), Some((4, 2)), Ty::U16, true, true),
    p(SWITCH, (4, 2), "position_names", Some((4, 5)), Some((4, 6)), Ty::StrList, false, true),
    p(SWITCH, (4, 3), "position_enabled", Some((4, 9)), Some((4, 10)), Ty::BoolList, false, false),
    p(GAIN, (4, 1), "gain_db", Some((4, 1)), Some((4, 2)), Ty::F32, true, true),
    p(DELAY, (4, 1), "delay_s", Some((4, 1)), Some((4, 2)), Ty::TimeInterval, true, true),
    p(IDENTIFICATION, (4, 1), "active", Some((4, 1)), Some((4, 2)), Ty::Bool, false, true),
    p(BOOLEAN_ACTUATOR, (5, 1), "setting", Some((5, 1)), Some((5, 2)), Ty::Bool, false, true),
    p(&[1, 1, 1, 1, 2], (5, 1), "setting", Some((5, 1)), Some((5, 2)), Ty::I8, true, true),
    p(&[1, 1, 1, 1, 3], (5, 1), "setting", Some((5, 1)), Some((5, 2)), Ty::I16, true, true),
    p(&[1, 1, 1, 1, 4], (5, 1), "setting", Some((5, 1)), Some((5, 2)), Ty::I32, true, true),
    p(&[1, 1, 1, 1, 5], (5, 1), "setting", Some((5, 1)), Some((5, 2)), Ty::I64, true, true),
    p(&[1, 1, 1, 1, 6], (5, 1), "setting", Some((5, 1)), Some((5, 2)), Ty::U8, true, true),
    p(&[1, 1, 1, 1, 7], (5, 1), "setting", Some((5, 1)), Some((5, 2)), Ty::U16, true, true),
    p(&[1, 1, 1, 1, 8], (5, 1), "setting", Some((5, 1)), Some((5, 2)), Ty::U32, true, true),
    p(&[1, 1, 1, 1, 9], (5, 1), "setting", Some((5, 1)), Some((5, 2)), Ty::U64, true, true),
    p(&[1, 1, 1, 1, 10], (5, 1), "setting", Some((5, 1)), Some((5, 2)), Ty::F32, true, true),
    p(&[1, 1, 1, 1, 11], (5, 1), "setting", Some((5, 1)), Some((5, 2)), Ty::F64, true, true),
    p(STRING_ACTUATOR, (5, 1), "setting", Some((5, 1)), Some((5, 2)), Ty::Str, false, true),
    // Sensors
    p(&[1, 1, 2], (3, 1), "reading_state", Some((3, 1)), None, Ty::SensorState, false, false),
    p(LEVEL_SENSOR, (4, 1), "reading_db", Some((4, 1)), None, Ty::F32, true, true),
    // OcaDeviceManager (object number 1)
    p(DEVICE_MANAGER, (3, 1), "model_guid", Some((3, 2)), None, Ty::ModelGuid, false, true),
    p(DEVICE_MANAGER, (3, 2), "serial_number", Some((3, 3)), None, Ty::Str, false, true),
    p(DEVICE_MANAGER, (3, 3), "model", Some((3, 6)), None, Ty::ModelDescription, false, true),
    p(DEVICE_MANAGER, (3, 4), "name", Some((3, 4)), Some((3, 5)), Ty::Str, false, true),
    p(DEVICE_MANAGER, (3, 5), "oca_version", Some((3, 1)), None, Ty::U16, false, true),
    p(DEVICE_MANAGER, (3, 6), "role", Some((3, 7)), Some((3, 8)), Ty::Str, false, false),
    p(DEVICE_MANAGER, (3, 7), "user_inventory_code", Some((3, 9)), Some((3, 10)), Ty::Str, false, false),
    p(DEVICE_MANAGER, (3, 8), "enabled", Some((3, 11)), Some((3, 12)), Ty::Bool, false, true),
    p(DEVICE_MANAGER, (3, 9), "state", Some((3, 13)), None, Ty::DeviceState, false, true),
    p(DEVICE_MANAGER, (3, 12), "message", Some((3, 17)), Some((3, 18)), Ty::Str, false, false),
    p(DEVICE_MANAGER, (3, 14), "revision_id", Some((3, 20)), None, Ty::Str, false, true),
];

/// Every property an object of `class` has that the module knows, most
/// specific class first.
pub(crate) fn props_of(class: &[u16]) -> Vec<&'static Prop> {
    let mut props: Vec<&Prop> = PROPS.iter().filter(|p| is_a(class, p.class)).collect();
    props.sort_by_key(|p| std::cmp::Reverse(p.class.len()));
    props
}

/// The property `id` of an object of `class`.
pub(crate) fn prop(class: &[u16], id: Id) -> Option<&'static Prop> {
    props_of(class).into_iter().find(|p| p.id == id)
}

/// The class has a property worth reading and following.
pub(crate) fn is_known(class: &[u16]) -> bool {
    props_of(class).iter().any(|p| p.read)
}

const SENSOR_STATES: &[&str] = &["unknown", "valid", "underrange", "overrange", "error"];
const DEVICE_STATES: &[&str] = &[
    "operational",
    "disabled",
    "error",
    "initializing",
    "updating",
];

/// One value of `ty` from the wire, as JSON. `wide` is set when an
/// OcaTimeInterval was a 64-bit float.
pub(crate) fn decode(ty: Ty, r: &mut Reader, wide: bool) -> Decoded<Value> {
    Ok(match ty {
        Ty::Bool => json!(r.bool()?),
        Ty::I8 => json!(r.i8()?),
        Ty::I16 => json!(r.i16()?),
        Ty::I32 => json!(r.i32()?),
        Ty::I64 => json!(r.i64()?),
        Ty::U8 => json!(r.u8()?),
        Ty::U16 => json!(r.u16()?),
        Ty::U32 => json!(r.u32()?),
        Ty::U64 => json!(r.u64()?),
        Ty::F32 => float32(r.f32()?),
        Ty::F64 => float(r.f64()?),
        Ty::TimeInterval => {
            if wide {
                float(r.f64()?)
            } else {
                float32(r.f32()?)
            }
        }
        Ty::Str => json!(r.string()?),
        Ty::Mute => match r.u8()? {
            1 => json!(true),
            2 => json!(false),
            n => return Err(format!("mute state {n}")),
        },
        Ty::Polarity => match r.u8()? {
            1 => json!(false),
            2 => json!(true),
            n => return Err(format!("polarity state {n}")),
        },
        Ty::SensorState => {
            let n = r.u8()?;
            match SENSOR_STATES.get(n as usize) {
                Some(name) => json!(name),
                None => json!(n),
            }
        }
        Ty::DeviceState => {
            let bits = r.u16()?;
            let names: Vec<&str> = DEVICE_STATES
                .iter()
                .enumerate()
                .filter(|(i, _)| bits & (1 << i) != 0)
                .map(|(_, n)| *n)
                .collect();
            json!(names)
        }
        Ty::ModelGuid => json!(super::codec::hex(r.take(8)?)),
        Ty::ModelDescription => json!({
            "manufacturer": r.string()?,
            "name": r.string()?,
            "version": r.string()?,
        }),
        Ty::StrList => {
            let n = r.u16()?;
            json!((0..n).map(|_| r.string()).collect::<Decoded<Vec<_>>>()?)
        }
        Ty::BoolList => {
            let n = r.u16()?;
            json!((0..n).map(|_| r.bool()).collect::<Decoded<Vec<_>>>()?)
        }
    })
}

/// A 32-bit float as the shortest decimal that reads back as the same
/// float, so -10.2 is not shown as -10.199999809265137.
fn float32(v: f32) -> Value {
    float(v.to_string().parse().unwrap_or(f64::from(v)))
}

/// A float as JSON; NaN and infinities, which JSON cannot hold, as text.
fn float(v: f64) -> Value {
    if v.is_finite() {
        json!(v)
    } else if v.is_nan() {
        json!("nan")
    } else if v > 0.0 {
        json!("inf")
    } else {
        json!("-inf")
    }
}

/// What a getter returned.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reply {
    pub value: Value,
    /// A bounded property's minimum and maximum.
    pub min: Option<Value>,
    pub max: Option<Value>,
    /// An OcaTimeInterval was a 64-bit float.
    pub wide: bool,
}

/// The value a getter's bytes hold for `prop`, with the minimum and maximum
/// of a bounded property.
pub(crate) fn decode_reply(prop: &Prop, bytes: &[u8]) -> Decoded<Reply> {
    let wide = prop.ty == Ty::TimeInterval
        && if prop.bounded {
            bytes.len() == 24
        } else {
            bytes.len() == 8
        };
    let mut r = Reader::new(bytes);
    let value = decode(prop.ty, &mut r, wide)?;
    let (min, max) = if prop.bounded && r.remaining() > 0 {
        (
            Some(decode(prop.ty, &mut r, wide)?),
            Some(decode(prop.ty, &mut r, wide)?),
        )
    } else {
        (None, None)
    };
    Ok(Reply {
        value,
        min,
        max,
        wide,
    })
}

/// An event's new value of `prop` (one value, not bounded).
pub(crate) fn decode_event_value(prop: &Prop, bytes: &[u8]) -> Decoded<(Value, bool)> {
    let wide = prop.ty == Ty::TimeInterval && bytes.len() == 8;
    let mut r = Reader::new(bytes);
    Ok((decode(prop.ty, &mut r, wide)?, wide))
}

fn int<T: TryFrom<i64>>(v: &Value, what: &str) -> Result<T, String> {
    let n = v
        .as_i64()
        .ok_or_else(|| format!("{what} must be an integer"))?;
    T::try_from(n).map_err(|_| format!("{n} is out of range for {what}"))
}

/// A JSON value as `ty` on the wire. `wide` writes an OcaTimeInterval as a
/// 64-bit float.
pub(crate) fn encode(ty: Ty, v: &Value, wide: bool) -> Result<Vec<u8>, String> {
    let mut w = Writer::new();
    let bool_of = |v: &Value| v.as_bool().ok_or_else(|| "a boolean is wanted".to_string());
    let float_of = |v: &Value| {
        v.as_f64()
            .filter(|f| f.is_finite())
            .ok_or_else(|| "a finite number is wanted".to_string())
    };
    match ty {
        Ty::Bool => {
            w.bool(bool_of(v)?);
        }
        Ty::I8 => {
            w.i8(int(v, "an Int8")?);
        }
        Ty::I16 => {
            w.i16(int(v, "an Int16")?);
        }
        Ty::I32 => {
            w.i32(int(v, "an Int32")?);
        }
        Ty::I64 => {
            w.i64(int(v, "an Int64")?);
        }
        Ty::U8 => {
            w.u8(int(v, "a Uint8")?);
        }
        Ty::U16 => {
            w.u16(int(v, "a Uint16")?);
        }
        Ty::U32 => {
            w.u32(int(v, "a Uint32")?);
        }
        Ty::U64 => {
            let n = v
                .as_u64()
                .ok_or_else(|| "a Uint64 must be a non-negative integer".to_string())?;
            w.u64(n);
        }
        Ty::F32 => {
            let f = float_of(v)?;
            if f.abs() > f64::from(f32::MAX) {
                return Err(format!("{f} is out of range for a Float32"));
            }
            w.f32(f as f32);
        }
        Ty::F64 => {
            w.f64(float_of(v)?);
        }
        Ty::TimeInterval => {
            let f = float_of(v)?;
            if wide {
                w.f64(f);
            } else {
                w.f32(f as f32);
            }
        }
        Ty::Str => {
            w.string(v.as_str().ok_or("a string is wanted")?)?;
        }
        Ty::Mute => {
            w.u8(if bool_of(v)? { 1 } else { 2 });
        }
        Ty::Polarity => {
            w.u8(if bool_of(v)? { 2 } else { 1 });
        }
        Ty::StrList => {
            let list = v.as_array().ok_or("a list of strings is wanted")?;
            w.u16(u16::try_from(list.len()).map_err(|_| "too many items")?);
            for item in list {
                w.string(item.as_str().ok_or("a list of strings is wanted")?)?;
            }
        }
        Ty::BoolList => {
            let list = v.as_array().ok_or("a list of booleans is wanted")?;
            w.u16(u16::try_from(list.len()).map_err(|_| "too many items")?);
            for item in list {
                w.bool(item.as_bool().ok_or("a list of booleans is wanted")?);
            }
        }
        Ty::SensorState | Ty::DeviceState | Ty::ModelGuid | Ty::ModelDescription => {
            return Err("the module does not write this type".into())
        }
    }
    Ok(w.buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_are_named_by_their_nearest_known_ancestor() {
        assert_eq!(exact_name(&[1, 1, 1, 5]), Some("OcaGain"));
        assert_eq!(kind(&[1, 1, 1, 5]), Some("OcaGain"));
        // A proprietary subclass of OcaGain.
        let custom = [1, 1, 1, 5, 65535, 0, 0x1234, 1];
        assert_eq!(exact_name(&custom), None);
        assert_eq!(kind(&custom), Some("OcaGain"));
        assert_eq!(kind(&[2, 1]), None);
        assert!(is_a(&custom, GAIN));
        assert!(!is_a(&[1, 1, 1], GAIN));
    }

    #[test]
    fn properties_are_found_through_the_class_tree() {
        let gain = prop(&[1, 1, 1, 5, 65535, 0, 1, 1], Id(4, 1)).unwrap();
        assert_eq!(gain.key, "gain_db");
        assert_eq!(gain.set, Some(Id(4, 2)));
        // Inherited from OcaWorker and OcaRoot.
        assert_eq!(prop(GAIN, Id(2, 1)).unwrap().key, "enabled");
        assert_eq!(prop(GAIN, Id(1, 5)).unwrap().key, "role");
        // The device manager's enabled is its own 3.8, not the worker's.
        assert_eq!(prop(DEVICE_MANAGER, Id(3, 8)).unwrap().get, Some(Id(3, 11)));
        assert!(prop(DEVICE_MANAGER, Id(2, 1)).is_none());
        // The int actuators each have their own width.
        assert_eq!(prop(&[1, 1, 1, 1, 3], Id(5, 1)).unwrap().ty, Ty::I16);
        assert!(is_known(GAIN));
        assert!(!is_known(BLOCK));
        assert!(!is_known(&[1, 1, 1, 14]));
    }

    #[test]
    fn values_decode_to_json() {
        let gain = prop(GAIN, Id(4, 1)).unwrap();
        let mut w = Writer::new();
        w.f32(-6.0).f32(-120.0).f32(12.0);
        let Reply {
            value: v, min, max, ..
        } = decode_reply(gain, &w.buf).unwrap();
        assert_eq!(
            (v, min, max),
            (json!(-6.0), Some(json!(-120.0)), Some(json!(12.0)))
        );

        let mute = prop(MUTE, Id(4, 1)).unwrap();
        assert_eq!(decode_reply(mute, &[1]).unwrap().value, json!(true));
        assert_eq!(decode_reply(mute, &[2]).unwrap().value, json!(false));
        assert!(decode_reply(mute, &[3]).is_err());

        let polarity = prop(POLARITY, Id(4, 1)).unwrap();
        assert_eq!(decode_reply(polarity, &[2]).unwrap().value, json!(true));

        let names = prop(SWITCH, Id(4, 2)).unwrap();
        let mut w = Writer::new();
        w.u16(2);
        w.string("A").unwrap();
        w.string("Bé").unwrap();
        assert_eq!(
            decode_reply(names, &w.buf).unwrap().value,
            json!(["A", "Bé"])
        );

        let state = prop(DEVICE_MANAGER, Id(3, 9)).unwrap();
        assert_eq!(
            decode_reply(state, &[0, 0b101]).unwrap().value,
            json!(["operational", "error"])
        );
        let guid = prop(DEVICE_MANAGER, Id(3, 1)).unwrap();
        assert_eq!(
            decode_reply(guid, &[0, 0, 0x0B, 0x5E, 1, 2, 3, 4])
                .unwrap()
                .value,
            json!("00000b5e01020304")
        );
        let model = prop(DEVICE_MANAGER, Id(3, 3)).unwrap();
        let mut w = Writer::new();
        w.string("Maker").unwrap();
        w.string("Amp").unwrap();
        w.string("1.0").unwrap();
        assert_eq!(
            decode_reply(model, &w.buf).unwrap().value,
            json!({"manufacturer": "Maker", "name": "Amp", "version": "1.0"})
        );
        let reading_state = prop(LEVEL_SENSOR, Id(3, 1)).unwrap();
        assert_eq!(
            decode_reply(reading_state, &[3]).unwrap().value,
            json!("overrange")
        );
        assert_eq!(decode_reply(reading_state, &[9]).unwrap().value, json!(9));
        // A 32-bit float is shown as the decimal it was written as.
        let v = decode_reply(gain, &(-10.2f32).to_be_bytes()).unwrap().value;
        assert_eq!(v, json!(-10.2));
        // Not finite.
        let mut w = Writer::new();
        w.f32(f32::NEG_INFINITY).f32(f32::NAN).f32(0.0);
        let level = prop(LEVEL_SENSOR, Id(4, 1)).unwrap();
        let Reply { value: v, min, .. } = decode_reply(level, &w.buf).unwrap();
        assert_eq!((v, min), (json!("-inf"), Some(json!("nan"))));
    }

    #[test]
    fn time_intervals_are_read_at_either_width() {
        let delay = prop(DELAY, Id(4, 1)).unwrap();
        let mut narrow = Writer::new();
        narrow.f32(0.5).f32(0.0).f32(1.0);
        let Reply {
            value: v,
            max,
            wide,
            ..
        } = decode_reply(delay, &narrow.buf).unwrap();
        assert_eq!((v, max, wide), (json!(0.5), Some(json!(1.0)), false));
        let mut broad = Writer::new();
        broad.f64(0.25).f64(0.0).f64(2.0);
        let Reply {
            value: v,
            max,
            wide,
            ..
        } = decode_reply(delay, &broad.buf).unwrap();
        assert_eq!((v, max, wide), (json!(0.25), Some(json!(2.0)), true));
        assert_eq!(
            decode_event_value(delay, &0.75f64.to_be_bytes()).unwrap(),
            (json!(0.75), true)
        );
        assert_eq!(
            encode(Ty::TimeInterval, &json!(0.5), false).unwrap(),
            0.5f32.to_be_bytes()
        );
        assert_eq!(
            encode(Ty::TimeInterval, &json!(0.5), true).unwrap(),
            0.5f64.to_be_bytes()
        );
    }

    #[test]
    fn values_encode_with_range_checks() {
        assert_eq!(
            encode(Ty::F32, &json!(-6.5), false).unwrap(),
            (-6.5f32).to_be_bytes()
        );
        assert!(encode(Ty::F32, &json!(1e300), false).is_err());
        assert_eq!(encode(Ty::Mute, &json!(true), false).unwrap(), vec![1]);
        assert_eq!(encode(Ty::Mute, &json!(false), false).unwrap(), vec![2]);
        assert_eq!(encode(Ty::Polarity, &json!(true), false).unwrap(), vec![2]);
        assert_eq!(encode(Ty::I8, &json!(-128), false).unwrap(), vec![0x80]);
        assert!(encode(Ty::I8, &json!(128), false).is_err());
        assert!(encode(Ty::U16, &json!(-1), false).is_err());
        assert_eq!(encode(Ty::U16, &json!(258), false).unwrap(), vec![1, 2]);
        assert_eq!(
            encode(Ty::U64, &json!(u64::MAX), false).unwrap(),
            u64::MAX.to_be_bytes()
        );
        assert!(encode(Ty::I32, &json!(1.5), false).is_err());
        assert_eq!(
            encode(Ty::Str, &json!("ab"), false).unwrap(),
            vec![0, 2, b'a', b'b']
        );
        assert_eq!(
            encode(Ty::StrList, &json!(["a"]), false).unwrap(),
            vec![0, 1, 0, 1, b'a']
        );
        assert_eq!(
            encode(Ty::BoolList, &json!([true, false]), false).unwrap(),
            vec![0, 2, 1, 0]
        );
        assert!(encode(Ty::Bool, &json!(1), false).is_err());
        assert!(encode(Ty::DeviceState, &json!([]), false).is_err());
    }
}
