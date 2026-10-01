//! Glow, the Ember+ object schema, written from the "Glow DTD ASN.1 Notation"
//! chapter of Lawo's Ember+ Specification 2.50 (Glow DTD 2.50) and the
//! chapter's prose descriptions of each type.
//!
//! The DTD is `DEFINITIONS EXPLICIT TAGS`: every `[n]` field is a context tag
//! wrapped around the value's own encoding, and every `[APPLICATION n]
//! IMPLICIT` type replaces the SEQUENCE or SET tag of its body. Application
//! numbers: Root 0, Parameter 1, Command 2, Node 3, ElementCollection 4,
//! StreamEntry 5, StreamCollection 6, StringIntegerPair 7,
//! StringIntegerCollection 8, QualifiedParameter 9, QualifiedNode 10,
//! RootElementCollection 11, StreamDescription 12, Matrix 13, Target 14,
//! Source 15, Connection 16, QualifiedMatrix 17, Label 18, Function 19,
//! QualifiedFunction 20, TupleItemDescription 21, Invocation 22,
//! InvocationResult 23, Template 24, QualifiedTemplate 25.
//!
//! Decoding is lenient: a field of an unexpected type, or an unknown field,
//! is skipped, so a newer provider's additions do not lose the message.

use super::ber::{self, Tag, Tlv};

pub(crate) const ROOT: u32 = 0;
pub(crate) const PARAMETER: u32 = 1;
pub(crate) const COMMAND: u32 = 2;
pub(crate) const NODE: u32 = 3;
pub(crate) const ELEMENT_COLLECTION: u32 = 4;
pub(crate) const STREAM_ENTRY: u32 = 5;
pub(crate) const STREAM_COLLECTION: u32 = 6;
pub(crate) const STRING_INTEGER_PAIR: u32 = 7;
pub(crate) const STRING_INTEGER_COLLECTION: u32 = 8;
pub(crate) const QUALIFIED_PARAMETER: u32 = 9;
pub(crate) const QUALIFIED_NODE: u32 = 10;
pub(crate) const ROOT_ELEMENT_COLLECTION: u32 = 11;
pub(crate) const STREAM_DESCRIPTION: u32 = 12;
pub(crate) const MATRIX: u32 = 13;
pub(crate) const TARGET: u32 = 14;
pub(crate) const SOURCE: u32 = 15;
pub(crate) const CONNECTION: u32 = 16;
pub(crate) const QUALIFIED_MATRIX: u32 = 17;
pub(crate) const LABEL: u32 = 18;
pub(crate) const FUNCTION: u32 = 19;
pub(crate) const QUALIFIED_FUNCTION: u32 = 20;
pub(crate) const TUPLE_ITEM_DESCRIPTION: u32 = 21;
pub(crate) const INVOCATION: u32 = 22;
pub(crate) const INVOCATION_RESULT: u32 = 23;
pub(crate) const TEMPLATE: u32 = 24;
pub(crate) const QUALIFIED_TEMPLATE: u32 = 25;

/// CommandType.
pub(crate) const SUBSCRIBE: i64 = 30;
pub(crate) const UNSUBSCRIBE: i64 = 31;
pub(crate) const GET_DIRECTORY: i64 = 32;
pub(crate) const INVOKE: i64 = 33;

/// FieldFlags, the GetDirectory dirFieldMask.
pub(crate) const FIELD_FLAGS: &[(&str, i64)] = &[
    ("sparse", -2),
    ("all", -1),
    ("default", 0),
    ("identifier", 1),
    ("description", 2),
    ("tree", 3),
    ("value", 4),
    ("connections", 5),
];

/// ParameterType.
pub(crate) const PARAMETER_TYPES: &[(i64, &str)] = &[
    (0, "null"),
    (1, "integer"),
    (2, "real"),
    (3, "string"),
    (4, "boolean"),
    (5, "trigger"),
    (6, "enum"),
    (7, "octets"),
];
pub(crate) const TYPE_INTEGER: i64 = 1;
pub(crate) const TYPE_REAL: i64 = 2;
pub(crate) const TYPE_STRING: i64 = 3;
pub(crate) const TYPE_BOOLEAN: i64 = 4;
pub(crate) const TYPE_TRIGGER: i64 = 5;
pub(crate) const TYPE_ENUM: i64 = 6;
pub(crate) const TYPE_OCTETS: i64 = 7;

/// ParameterAccess; "read" is the default.
pub(crate) const ACCESS: &[(i64, &str)] =
    &[(0, "none"), (1, "read"), (2, "write"), (3, "read_write")];
pub(crate) const MATRIX_TYPES: &[(i64, &str)] =
    &[(0, "one_to_n"), (1, "one_to_one"), (2, "n_to_n")];
pub(crate) const ADDRESSING_MODES: &[(i64, &str)] = &[(0, "linear"), (1, "non_linear")];
pub(crate) const OPERATION_ABSOLUTE: i64 = 0;
pub(crate) const OPERATION_CONNECT: i64 = 1;
pub(crate) const OPERATION_DISCONNECT: i64 = 2;
pub(crate) const DISPOSITIONS: &[(i64, &str)] =
    &[(0, "tally"), (1, "modified"), (2, "pending"), (3, "locked")];
pub(crate) const DISPOSITION_LOCKED: i64 = 3;
/// StreamFormat: type in bits 3-7 (0 unsigned, 1 signed, 2 IEEE float), size
/// in bits 1-2 (1, 2, 4, 8 bytes), endianness in bit 0 (0 big, 1 little).
pub(crate) const STREAM_FORMATS: &[(i64, &str)] = &[
    (0, "unsigned_int8"),
    (2, "unsigned_int16_big_endian"),
    (3, "unsigned_int16_little_endian"),
    (4, "unsigned_int32_big_endian"),
    (5, "unsigned_int32_little_endian"),
    (6, "unsigned_int64_big_endian"),
    (7, "unsigned_int64_little_endian"),
    (8, "signed_int8"),
    (10, "signed_int16_big_endian"),
    (11, "signed_int16_little_endian"),
    (12, "signed_int32_big_endian"),
    (13, "signed_int32_little_endian"),
    (14, "signed_int64_big_endian"),
    (15, "signed_int64_little_endian"),
    (20, "ieee_float32_big_endian"),
    (21, "ieee_float32_little_endian"),
    (22, "ieee_float64_big_endian"),
    (23, "ieee_float64_little_endian"),
];

pub(crate) fn name_of(table: &[(i64, &'static str)], v: i64) -> Option<&'static str> {
    table.iter().find(|(n, _)| *n == v).map(|(_, s)| *s)
}

/// Value, and MinMax (its integer, real and null choices).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Integer(i64),
    Real(f64),
    String(String),
    Boolean(bool),
    Octets(Vec<u8>),
    Null,
}

/// How an element is addressed: by its number under the enclosing element,
/// or (the Qualified types) by its full path from the root.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Id {
    Number(u32),
    Path(Vec<u32>),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct NodeContents {
    pub identifier: Option<String>,
    pub description: Option<String>,
    pub is_root: Option<bool>,
    pub is_online: Option<bool>,
    pub schema_identifiers: Option<String>,
    pub template_reference: Option<Vec<u32>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct StreamDescription {
    pub format: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct ParameterContents {
    pub identifier: Option<String>,
    pub description: Option<String>,
    pub value: Option<Value>,
    pub minimum: Option<Value>,
    pub maximum: Option<Value>,
    pub access: Option<i64>,
    pub format: Option<String>,
    pub enumeration: Option<String>,
    pub factor: Option<i64>,
    pub is_online: Option<bool>,
    pub formula: Option<String>,
    pub step: Option<i64>,
    pub default: Option<Value>,
    pub kind: Option<i64>,
    pub stream_identifier: Option<i64>,
    pub enum_map: Option<Vec<(String, i64)>>,
    pub stream_descriptor: Option<StreamDescription>,
    pub schema_identifiers: Option<String>,
    pub template_reference: Option<Vec<u32>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Invocation {
    pub id: Option<i64>,
    pub arguments: Option<Vec<Value>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Command {
    pub number: i64,
    pub dir_field_mask: Option<i64>,
    pub invocation: Option<Invocation>,
}

impl Command {
    pub(crate) fn new(number: i64) -> Command {
        Command {
            number,
            dir_field_mask: None,
            invocation: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ParametersLocation {
    BasePath(Vec<u32>),
    Inline(i64),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Label {
    pub base_path: Vec<u32>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct MatrixContents {
    pub identifier: Option<String>,
    pub description: Option<String>,
    pub kind: Option<i64>,
    pub addressing_mode: Option<i64>,
    pub target_count: Option<i64>,
    pub source_count: Option<i64>,
    pub maximum_total_connects: Option<i64>,
    pub maximum_connects_per_target: Option<i64>,
    pub parameters_location: Option<ParametersLocation>,
    pub gain_parameter_number: Option<i64>,
    pub labels: Option<Vec<Label>>,
    pub schema_identifiers: Option<String>,
    pub template_reference: Option<Vec<u32>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Connection {
    pub target: u32,
    pub sources: Option<Vec<u32>>,
    pub operation: Option<i64>,
    pub disposition: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TupleItem {
    pub kind: i64,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct FunctionContents {
    pub identifier: Option<String>,
    pub description: Option<String>,
    pub arguments: Option<Vec<TupleItem>>,
    pub result: Option<Vec<TupleItem>>,
    pub template_reference: Option<Vec<u32>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Node {
    pub id: Id,
    pub contents: Option<NodeContents>,
    pub children: Option<Vec<Element>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Parameter {
    pub id: Id,
    pub contents: Option<ParameterContents>,
    pub children: Option<Vec<Element>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Matrix {
    pub id: Id,
    pub contents: Option<MatrixContents>,
    pub children: Option<Vec<Element>>,
    pub targets: Option<Vec<u32>>,
    pub sources: Option<Vec<u32>>,
    pub connections: Option<Vec<Connection>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Function {
    pub id: Id,
    pub contents: Option<FunctionContents>,
    pub children: Option<Vec<Element>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InvocationResult {
    pub invocation_id: i64,
    pub success: Option<bool>,
    pub result: Option<Vec<Value>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StreamEntry {
    pub identifier: i64,
    pub value: Value,
}

/// Element and RootElement: the Qualified types are the same elements with
/// an [`Id::Path`]. Templates are recognised and not decoded (a consumer may
/// "discard all Template definitions or references").
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Element {
    Node(Node),
    Parameter(Parameter),
    Matrix(Matrix),
    Function(Function),
    Command(Command),
    Template,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Root {
    Elements(Vec<Element>),
    Streams(Vec<StreamEntry>),
    InvocationResult(InvocationResult),
}

// --- Decoding ----------------------------------------------------------------

fn value(t: &Tlv) -> Option<Value> {
    if t.tag.class != ber::UNIVERSAL || t.constructed {
        return None;
    }
    Some(match t.tag.number {
        ber::INTEGER => Value::Integer(t.as_i64()?),
        ber::REAL => Value::Real(t.as_f64()?),
        ber::UTF8_STRING => Value::String(t.as_string()?),
        ber::BOOLEAN => Value::Boolean(t.as_bool()?),
        ber::OCTET_STRING => Value::Octets(t.content.clone()),
        ber::NULL => Value::Null,
        _ => return None,
    })
}

fn int(t: &Tlv, n: u32) -> Option<i64> {
    t.field(n).and_then(Tlv::as_i64)
}

fn string(t: &Tlv, n: u32) -> Option<String> {
    t.field(n).and_then(Tlv::as_string)
}

fn flag(t: &Tlv, n: u32) -> Option<bool> {
    t.field(n).and_then(Tlv::as_bool)
}

fn path(t: &Tlv, n: u32) -> Option<Vec<u32>> {
    t.field(n).and_then(Tlv::as_roid)
}

fn number(t: &Tlv, n: u32) -> Option<u32> {
    int(t, n).and_then(|v| u32::try_from(v).ok())
}

/// The items of a `SEQUENCE OF [0] X`, unwrapped from their `[0]`. An item
/// sent without the wrapper is taken as it is.
fn items(seq: &Tlv) -> impl Iterator<Item = &Tlv> {
    seq.children.iter().filter_map(|c| {
        if c.tag == Tag::ctx(0) {
            c.inner()
        } else {
            Some(c)
        }
    })
}

/// The id of an element: `number [0]`, or `path [0]` for a Qualified type.
fn id(t: &Tlv, qualified: bool) -> Option<Id> {
    if qualified {
        path(t, 0).map(Id::Path)
    } else {
        number(t, 0).map(Id::Number)
    }
}

fn children(t: &Tlv) -> Option<Vec<Element>> {
    let c = t.field(2)?;
    Some(items(c).filter_map(element).collect())
}

fn node_contents(s: &Tlv) -> NodeContents {
    NodeContents {
        identifier: string(s, 0),
        description: string(s, 1),
        is_root: flag(s, 2),
        is_online: flag(s, 3),
        schema_identifiers: string(s, 4),
        template_reference: path(s, 5),
    }
}

fn parameter_contents(s: &Tlv) -> ParameterContents {
    ParameterContents {
        identifier: string(s, 0),
        description: string(s, 1),
        value: s.field(2).and_then(value),
        minimum: s.field(3).and_then(value),
        maximum: s.field(4).and_then(value),
        access: int(s, 5),
        format: string(s, 6),
        enumeration: string(s, 7),
        factor: int(s, 8),
        is_online: flag(s, 9),
        formula: string(s, 10),
        step: int(s, 11),
        default: s.field(12).and_then(value),
        kind: int(s, 13),
        stream_identifier: int(s, 14),
        enum_map: s.field(15).map(|c| {
            items(c)
                .filter_map(|p| Some((string(p, 0)?, int(p, 1)?)))
                .collect()
        }),
        stream_descriptor: s.field(16).and_then(|d| {
            Some(StreamDescription {
                format: int(d, 0)?,
                offset: int(d, 1)?,
            })
        }),
        schema_identifiers: string(s, 17),
        template_reference: path(s, 18),
    }
}

fn matrix_contents(s: &Tlv) -> MatrixContents {
    MatrixContents {
        identifier: string(s, 0),
        description: string(s, 1),
        kind: int(s, 2),
        addressing_mode: int(s, 3),
        target_count: int(s, 4),
        source_count: int(s, 5),
        maximum_total_connects: int(s, 6),
        maximum_connects_per_target: int(s, 7),
        parameters_location: s.field(8).and_then(|l| {
            l.as_roid()
                .map(ParametersLocation::BasePath)
                .or_else(|| l.as_i64().map(ParametersLocation::Inline))
        }),
        gain_parameter_number: int(s, 9),
        labels: s.field(10).map(|c| {
            items(c)
                .filter_map(|l| {
                    Some(Label {
                        base_path: path(l, 0)?,
                        description: string(l, 1).unwrap_or_default(),
                    })
                })
                .collect()
        }),
        schema_identifiers: string(s, 11),
        template_reference: path(s, 12),
    }
}

fn tuple_description(c: &Tlv) -> Vec<TupleItem> {
    items(c)
        .filter_map(|i| {
            Some(TupleItem {
                kind: int(i, 0)?,
                name: string(i, 1),
            })
        })
        .collect()
}

fn tuple(c: &Tlv) -> Vec<Value> {
    items(c).filter_map(value).collect()
}

fn function_contents(s: &Tlv) -> FunctionContents {
    FunctionContents {
        identifier: string(s, 0),
        description: string(s, 1),
        arguments: s.field(2).map(tuple_description),
        result: s.field(3).map(tuple_description),
        template_reference: path(s, 4),
    }
}

fn signals(c: &Tlv) -> Vec<u32> {
    items(c).filter_map(|s| number(s, 0)).collect()
}

fn connection(c: &Tlv) -> Option<Connection> {
    Some(Connection {
        target: number(c, 0)?,
        sources: path(c, 1),
        operation: int(c, 2),
        disposition: int(c, 3),
    })
}

fn command(t: &Tlv) -> Option<Command> {
    Some(Command {
        number: int(t, 0)?,
        dir_field_mask: int(t, 1),
        invocation: t.field(2).map(|i| Invocation {
            id: int(i, 0),
            arguments: i.field(1).map(tuple),
        }),
    })
}

/// One Element or RootElement.
pub(crate) fn element(t: &Tlv) -> Option<Element> {
    if t.tag.class != ber::APPLICATION {
        return None;
    }
    let qualified = matches!(
        t.tag.number,
        QUALIFIED_NODE | QUALIFIED_PARAMETER | QUALIFIED_MATRIX | QUALIFIED_FUNCTION
    );
    Some(match t.tag.number {
        NODE | QUALIFIED_NODE => Element::Node(Node {
            id: id(t, qualified)?,
            contents: t.field(1).map(node_contents),
            children: children(t),
        }),
        PARAMETER | QUALIFIED_PARAMETER => Element::Parameter(Parameter {
            id: id(t, qualified)?,
            contents: t.field(1).map(parameter_contents),
            children: children(t),
        }),
        MATRIX | QUALIFIED_MATRIX => Element::Matrix(Matrix {
            id: id(t, qualified)?,
            contents: t.field(1).map(matrix_contents),
            children: children(t),
            targets: t.field(3).map(signals),
            sources: t.field(4).map(signals),
            connections: t
                .field(5)
                .map(|c| items(c).filter_map(connection).collect()),
        }),
        FUNCTION | QUALIFIED_FUNCTION => Element::Function(Function {
            id: id(t, qualified)?,
            contents: t.field(1).map(function_contents),
            children: children(t),
        }),
        COMMAND => Element::Command(command(t)?),
        TEMPLATE | QUALIFIED_TEMPLATE => Element::Template,
        _ => return None,
    })
}

fn stream_entry(t: &Tlv) -> Option<StreamEntry> {
    Some(StreamEntry {
        identifier: int(t, 0)?,
        value: t.field(1).and_then(value)?,
    })
}

/// Every Root in a BER message.
pub(crate) fn decode(payload: &[u8]) -> Result<Vec<Root>, String> {
    let mut roots = Vec::new();
    for top in ber::parse_all(payload)? {
        if top.tag != Tag::app(ROOT) {
            return Err(format!(
                "a message that does not start with Root (tag class 0x{:02X}, number {})",
                top.tag.class, top.tag.number
            ));
        }
        let Some(inner) = top.inner() else {
            continue;
        };
        if inner.tag.class != ber::APPLICATION {
            return Err("a Root holding no Glow type".into());
        }
        roots.push(match inner.tag.number {
            ROOT_ELEMENT_COLLECTION => Root::Elements(items(inner).filter_map(element).collect()),
            // The DTD makes StreamCollection APPLICATION 6 and StreamEntry 5;
            // the prose of "Application defined types" swaps them. A
            // collection is recognised by its place under Root either way.
            STREAM_COLLECTION | STREAM_ENTRY => {
                Root::Streams(items(inner).filter_map(stream_entry).collect())
            }
            INVOCATION_RESULT => Root::InvocationResult(InvocationResult {
                invocation_id: int(inner, 0).ok_or("an InvocationResult without its id")?,
                success: flag(inner, 1),
                result: inner.field(2).map(tuple),
            }),
            other => return Err(format!("a Root holding APPLICATION {other}")),
        });
    }
    Ok(roots)
}

// --- Encoding ----------------------------------------------------------------

fn app(n: u32, fields: Vec<Vec<u8>>) -> Vec<u8> {
    ber::constructed(Tag::app(n), &fields)
}

fn encode_value(v: &Value) -> Vec<u8> {
    match v {
        Value::Integer(i) => ber::integer(*i),
        Value::Real(r) => ber::real(*r),
        Value::String(s) => ber::utf8(s),
        Value::Boolean(b) => ber::boolean(*b),
        Value::Octets(o) => ber::octets(o),
        Value::Null => ber::null(),
    }
}

/// Pushes `[n] value` when the value is there.
struct Fields(Vec<Vec<u8>>);

impl Fields {
    fn new() -> Fields {
        Fields(Vec::new())
    }
    fn put(&mut self, n: u32, inner: Option<Vec<u8>>) -> &mut Fields {
        if let Some(inner) = inner {
            self.0.push(ber::ctx(n, inner));
        }
        self
    }
    fn take(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.0)
    }
}

fn opt_str(s: &Option<String>) -> Option<Vec<u8>> {
    s.as_deref().map(ber::utf8)
}
fn opt_int(i: &Option<i64>) -> Option<Vec<u8>> {
    i.map(ber::integer)
}
fn opt_bool(b: &Option<bool>) -> Option<Vec<u8>> {
    b.map(ber::boolean)
}
fn opt_path(p: &Option<Vec<u32>>) -> Option<Vec<u8>> {
    p.as_deref().map(ber::roid)
}
fn opt_value(v: &Option<Value>) -> Option<Vec<u8>> {
    v.as_ref().map(encode_value)
}

fn sequence_of(items: impl IntoIterator<Item = Vec<u8>>) -> Vec<Vec<u8>> {
    items.into_iter().map(|i| ber::ctx(0, i)).collect()
}

fn encode_children(c: &Option<Vec<Element>>) -> Option<Vec<u8>> {
    c.as_ref().map(|c| {
        app(
            ELEMENT_COLLECTION,
            sequence_of(c.iter().map(encode_element)),
        )
    })
}

fn encode_id(id: &Id) -> Vec<u8> {
    match id {
        Id::Number(n) => ber::integer(i64::from(*n)),
        Id::Path(p) => ber::roid(p),
    }
}

fn tag_for(id: &Id, plain: u32, qualified: u32) -> u32 {
    match id {
        Id::Number(_) => plain,
        Id::Path(_) => qualified,
    }
}

fn encode_node_contents(c: &NodeContents) -> Vec<u8> {
    ber::set(
        &Fields::new()
            .put(0, opt_str(&c.identifier))
            .put(1, opt_str(&c.description))
            .put(2, opt_bool(&c.is_root))
            .put(3, opt_bool(&c.is_online))
            .put(4, opt_str(&c.schema_identifiers))
            .put(5, opt_path(&c.template_reference))
            .take(),
    )
}

fn encode_parameter_contents(c: &ParameterContents) -> Vec<u8> {
    let enum_map = c.enum_map.as_ref().map(|m| {
        app(
            STRING_INTEGER_COLLECTION,
            sequence_of(m.iter().map(|(s, i)| {
                app(
                    STRING_INTEGER_PAIR,
                    vec![ber::ctx(0, ber::utf8(s)), ber::ctx(1, ber::integer(*i))],
                )
            })),
        )
    });
    let descriptor = c.stream_descriptor.as_ref().map(|d| {
        app(
            STREAM_DESCRIPTION,
            vec![
                ber::ctx(0, ber::integer(d.format)),
                ber::ctx(1, ber::integer(d.offset)),
            ],
        )
    });
    ber::set(
        &Fields::new()
            .put(0, opt_str(&c.identifier))
            .put(1, opt_str(&c.description))
            .put(2, opt_value(&c.value))
            .put(3, opt_value(&c.minimum))
            .put(4, opt_value(&c.maximum))
            .put(5, opt_int(&c.access))
            .put(6, opt_str(&c.format))
            .put(7, opt_str(&c.enumeration))
            .put(8, opt_int(&c.factor))
            .put(9, opt_bool(&c.is_online))
            .put(10, opt_str(&c.formula))
            .put(11, opt_int(&c.step))
            .put(12, opt_value(&c.default))
            .put(13, opt_int(&c.kind))
            .put(14, opt_int(&c.stream_identifier))
            .put(15, enum_map)
            .put(16, descriptor)
            .put(17, opt_str(&c.schema_identifiers))
            .put(18, opt_path(&c.template_reference))
            .take(),
    )
}

fn encode_matrix_contents(c: &MatrixContents) -> Vec<u8> {
    let location = c.parameters_location.as_ref().map(|l| match l {
        ParametersLocation::BasePath(p) => ber::roid(p),
        ParametersLocation::Inline(n) => ber::integer(*n),
    });
    let labels = c.labels.as_ref().map(|ls| {
        ber::sequence(&sequence_of(ls.iter().map(|l| {
            app(
                LABEL,
                vec![
                    ber::ctx(0, ber::roid(&l.base_path)),
                    ber::ctx(1, ber::utf8(&l.description)),
                ],
            )
        })))
    });
    ber::set(
        &Fields::new()
            .put(0, opt_str(&c.identifier))
            .put(1, opt_str(&c.description))
            .put(2, opt_int(&c.kind))
            .put(3, opt_int(&c.addressing_mode))
            .put(4, opt_int(&c.target_count))
            .put(5, opt_int(&c.source_count))
            .put(6, opt_int(&c.maximum_total_connects))
            .put(7, opt_int(&c.maximum_connects_per_target))
            .put(8, location)
            .put(9, opt_int(&c.gain_parameter_number))
            .put(10, labels)
            .put(11, opt_str(&c.schema_identifiers))
            .put(12, opt_path(&c.template_reference))
            .take(),
    )
}

fn encode_tuple_description(items: &[TupleItem]) -> Vec<u8> {
    ber::sequence(&sequence_of(items.iter().map(|i| {
        app(
            TUPLE_ITEM_DESCRIPTION,
            Fields::new()
                .put(0, Some(ber::integer(i.kind)))
                .put(1, opt_str(&i.name))
                .take(),
        )
    })))
}

fn encode_tuple(values: &[Value]) -> Vec<u8> {
    ber::sequence(&sequence_of(values.iter().map(encode_value)))
}

fn encode_function_contents(c: &FunctionContents) -> Vec<u8> {
    ber::set(
        &Fields::new()
            .put(0, opt_str(&c.identifier))
            .put(1, opt_str(&c.description))
            .put(2, c.arguments.as_deref().map(encode_tuple_description))
            .put(3, c.result.as_deref().map(encode_tuple_description))
            .put(4, opt_path(&c.template_reference))
            .take(),
    )
}

fn encode_signals(n: u32, signals: &Option<Vec<u32>>) -> Option<Vec<u8>> {
    signals.as_ref().map(|s| {
        ber::sequence(&sequence_of(
            s.iter()
                .map(|&x| app(n, vec![ber::ctx(0, ber::integer(i64::from(x)))])),
        ))
    })
}

fn encode_connection(c: &Connection) -> Vec<u8> {
    app(
        CONNECTION,
        Fields::new()
            .put(0, Some(ber::integer(i64::from(c.target))))
            .put(1, opt_path(&c.sources))
            .put(2, opt_int(&c.operation))
            .put(3, opt_int(&c.disposition))
            .take(),
    )
}

pub(crate) fn encode_element(e: &Element) -> Vec<u8> {
    match e {
        Element::Node(n) => app(
            tag_for(&n.id, NODE, QUALIFIED_NODE),
            Fields::new()
                .put(0, Some(encode_id(&n.id)))
                .put(1, n.contents.as_ref().map(encode_node_contents))
                .put(2, encode_children(&n.children))
                .take(),
        ),
        Element::Parameter(p) => app(
            tag_for(&p.id, PARAMETER, QUALIFIED_PARAMETER),
            Fields::new()
                .put(0, Some(encode_id(&p.id)))
                .put(1, p.contents.as_ref().map(encode_parameter_contents))
                .put(2, encode_children(&p.children))
                .take(),
        ),
        Element::Matrix(m) => app(
            tag_for(&m.id, MATRIX, QUALIFIED_MATRIX),
            Fields::new()
                .put(0, Some(encode_id(&m.id)))
                .put(1, m.contents.as_ref().map(encode_matrix_contents))
                .put(2, encode_children(&m.children))
                .put(3, encode_signals(TARGET, &m.targets))
                .put(4, encode_signals(SOURCE, &m.sources))
                .put(
                    5,
                    m.connections
                        .as_ref()
                        .map(|c| ber::sequence(&sequence_of(c.iter().map(encode_connection)))),
                )
                .take(),
        ),
        Element::Function(f) => app(
            tag_for(&f.id, FUNCTION, QUALIFIED_FUNCTION),
            Fields::new()
                .put(0, Some(encode_id(&f.id)))
                .put(1, f.contents.as_ref().map(encode_function_contents))
                .put(2, encode_children(&f.children))
                .take(),
        ),
        Element::Command(c) => {
            let invocation = c.invocation.as_ref().map(|i| {
                app(
                    INVOCATION,
                    Fields::new()
                        .put(0, opt_int(&i.id))
                        .put(1, i.arguments.as_deref().map(encode_tuple))
                        .take(),
                )
            });
            app(
                COMMAND,
                Fields::new()
                    .put(0, Some(ber::integer(c.number)))
                    .put(1, opt_int(&c.dir_field_mask))
                    .put(2, invocation)
                    .take(),
            )
        }
        // Never sent: a consumer has no reason to send a template.
        Element::Template => app(TEMPLATE, Vec::new()),
    }
}

pub(crate) fn encode(root: &Root) -> Vec<u8> {
    let inner = match root {
        Root::Elements(elements) => app(
            ROOT_ELEMENT_COLLECTION,
            sequence_of(elements.iter().map(encode_element)),
        ),
        Root::Streams(entries) => app(
            STREAM_COLLECTION,
            sequence_of(entries.iter().map(|e| {
                app(
                    STREAM_ENTRY,
                    vec![
                        ber::ctx(0, ber::integer(e.identifier)),
                        ber::ctx(1, encode_value(&e.value)),
                    ],
                )
            })),
        ),
        Root::InvocationResult(r) => app(
            INVOCATION_RESULT,
            Fields::new()
                .put(0, Some(ber::integer(r.invocation_id)))
                .put(1, opt_bool(&r.success))
                .put(2, r.result.as_deref().map(encode_tuple))
                .take(),
        ),
    };
    ber::constructed(Tag::app(ROOT), &[inner])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(root: Root) {
        let bytes = encode(&root);
        assert_eq!(decode(&bytes).unwrap(), vec![root]);
    }

    fn get_directory() -> Element {
        Element::Command(Command {
            number: GET_DIRECTORY,
            dir_field_mask: Some(-1),
            invocation: None,
        })
    }

    // "Querying a data provider": a GetDirectory at the root is a Command in
    // the RootElementCollection.
    #[test]
    fn root_get_directory_bytes() {
        let bytes = encode(&Root::Elements(vec![Element::Command(Command::new(
            GET_DIRECTORY,
        ))]));
        // Root [APP 0] { RootElementCollection [APP 11] { [0] { Command
        // [APP 2] { [0] INTEGER 32 } } } }
        assert_eq!(
            bytes,
            vec![0x60, 0x0B, 0x6B, 0x09, 0xA0, 0x07, 0x62, 0x05, 0xA0, 0x03, 0x02, 0x01, 0x20]
        );
        round_trip(Root::Elements(vec![get_directory()]));
    }

    #[test]
    fn nodes_and_qualified_nodes() {
        round_trip(Root::Elements(vec![
            Element::Node(Node {
                id: Id::Number(1),
                contents: Some(NodeContents {
                    identifier: Some("Device".into()),
                    description: Some("The device".into()),
                    is_root: Some(true),
                    is_online: Some(true),
                    schema_identifiers: Some("de.l-s-b.emberplus.schema1".into()),
                    template_reference: Some(vec![1, 9]),
                }),
                children: Some(vec![Element::Node(Node {
                    id: Id::Number(3),
                    contents: None,
                    children: Some(vec![get_directory()]),
                })]),
            }),
            Element::Node(Node {
                id: Id::Path(vec![1, 2]),
                contents: Some(NodeContents::default()),
                children: Some(vec![]),
            }),
        ]));
    }

    #[test]
    fn parameters_with_every_field() {
        let contents = ParameterContents {
            identifier: Some("gain".into()),
            description: Some("Gain".into()),
            value: Some(Value::Real(-64.0)),
            minimum: Some(Value::Real(-128.0)),
            maximum: Some(Value::Integer(15)),
            access: Some(3),
            format: Some("%.2f\u{b0}\ndB".into()),
            enumeration: Some("Off\nOn\n~Hidden".into()),
            factor: Some(10),
            is_online: Some(false),
            formula: Some("(5 * $)\n($ / 5)".into()),
            step: Some(1),
            default: Some(Value::Null),
            kind: Some(TYPE_REAL),
            stream_identifier: Some(110),
            enum_map: Some(vec![("Off".into(), 0), ("On".into(), 7)]),
            stream_descriptor: Some(StreamDescription {
                format: 21,
                offset: 8,
            }),
            schema_identifiers: Some("a.b".into()),
            template_reference: Some(vec![1, 2, 3]),
        };
        round_trip(Root::Elements(vec![
            Element::Parameter(Parameter {
                id: Id::Number(2),
                contents: Some(contents),
                children: None,
            }),
            Element::Parameter(Parameter {
                id: Id::Path(vec![1, 2, 3]),
                contents: Some(ParameterContents {
                    value: Some(Value::String("255.255.252.0".into())),
                    ..Default::default()
                }),
                children: Some(vec![Element::Command(Command::new(SUBSCRIBE))]),
            }),
        ]));
        for v in [
            Value::Integer(-5),
            Value::Boolean(true),
            Value::Octets(vec![0, 0xFF]),
            Value::String(String::new()),
            Value::Null,
        ] {
            round_trip(Root::Elements(vec![Element::Parameter(Parameter {
                id: Id::Path(vec![4]),
                contents: Some(ParameterContents {
                    value: Some(v),
                    ..Default::default()
                }),
                children: None,
            })]));
        }
    }

    #[test]
    fn matrices_with_signals_and_connections() {
        round_trip(Root::Elements(vec![
            Element::Matrix(Matrix {
                id: Id::Number(1),
                contents: Some(MatrixContents {
                    identifier: Some("matrix".into()),
                    description: Some("Sample Matrix".into()),
                    kind: Some(2),
                    addressing_mode: Some(1),
                    target_count: Some(4),
                    source_count: Some(4),
                    maximum_total_connects: Some(16),
                    maximum_connects_per_target: Some(4),
                    parameters_location: Some(ParametersLocation::BasePath(vec![1, 2, 2])),
                    gain_parameter_number: Some(1),
                    labels: Some(vec![
                        Label {
                            base_path: vec![1, 2, 3, 1],
                            description: "Primary".into(),
                        },
                        Label {
                            base_path: vec![1, 2, 3, 2],
                            description: "Internal".into(),
                        },
                    ]),
                    schema_identifiers: None,
                    template_reference: None,
                }),
                children: None,
                targets: Some(vec![0, 1, 7, 9]),
                sources: Some(vec![2, 3]),
                connections: Some(vec![
                    Connection {
                        target: 0,
                        sources: Some(vec![3]),
                        operation: None,
                        disposition: None,
                    },
                    Connection {
                        target: 3,
                        sources: Some(vec![]),
                        operation: Some(OPERATION_ABSOLUTE),
                        disposition: Some(DISPOSITION_LOCKED),
                    },
                    Connection {
                        target: 9,
                        sources: None,
                        operation: None,
                        disposition: None,
                    },
                ]),
            }),
            Element::Matrix(Matrix {
                id: Id::Path(vec![1, 2, 1]),
                contents: Some(MatrixContents {
                    parameters_location: Some(ParametersLocation::Inline(5)),
                    ..Default::default()
                }),
                children: Some(vec![get_directory()]),
                targets: None,
                sources: None,
                connections: Some(vec![Connection {
                    target: 0,
                    sources: Some(vec![0, 2]),
                    operation: Some(OPERATION_CONNECT),
                    disposition: None,
                }]),
            }),
        ]));
    }

    #[test]
    fn functions_invocations_and_results() {
        round_trip(Root::Elements(vec![Element::Function(Function {
            id: Id::Number(1),
            contents: Some(FunctionContents {
                identifier: Some("setObjectNameRecursive".into()),
                description: Some("Sample Function".into()),
                arguments: Some(vec![
                    TupleItem {
                        kind: TYPE_INTEGER,
                        name: Some("objectId".into()),
                    },
                    TupleItem {
                        kind: TYPE_BOOLEAN,
                        name: Some("isRecursive".into()),
                    },
                    TupleItem {
                        kind: TYPE_STRING,
                        name: Some("newName".into()),
                    },
                ]),
                result: Some(vec![TupleItem {
                    kind: TYPE_INTEGER,
                    name: Some("changeCount".into()),
                }]),
                template_reference: None,
            }),
            children: None,
        })]));
        // "Invocation of Functions": invoke 33 with id 1 and three arguments.
        round_trip(Root::Elements(vec![Element::Function(Function {
            id: Id::Path(vec![1, 2, 1]),
            contents: None,
            children: Some(vec![Element::Command(Command {
                number: INVOKE,
                dir_field_mask: None,
                invocation: Some(Invocation {
                    id: Some(1),
                    arguments: Some(vec![
                        Value::Integer(123),
                        Value::Boolean(true),
                        Value::String("Herbert".into()),
                    ]),
                }),
            })]),
        })]));
        round_trip(Root::InvocationResult(InvocationResult {
            invocation_id: 1,
            success: Some(true),
            result: Some(vec![Value::Integer(74)]),
        }));
        round_trip(Root::InvocationResult(InvocationResult {
            invocation_id: 2,
            success: None,
            result: None,
        }));
    }

    #[test]
    fn streams() {
        round_trip(Root::Streams(vec![
            StreamEntry {
                identifier: 110,
                value: Value::Integer(-20),
            },
            StreamEntry {
                identifier: 111,
                value: Value::Octets(vec![1, 2, 3, 4]),
            },
        ]));
        // The prose's numbering (collection 5, entry 6) decodes the same.
        let swapped = ber::constructed(
            Tag::app(ROOT),
            &[ber::constructed(
                Tag::app(STREAM_ENTRY),
                &[ber::ctx(
                    0,
                    ber::constructed(
                        Tag::app(STREAM_COLLECTION),
                        &[ber::ctx(0, ber::integer(7)), ber::ctx(1, ber::real(0.5))],
                    ),
                )],
            )],
        );
        assert_eq!(
            decode(&swapped).unwrap(),
            vec![Root::Streams(vec![StreamEntry {
                identifier: 7,
                value: Value::Real(0.5)
            }])]
        );
    }

    #[test]
    fn templates_unknown_fields_and_garbage() {
        // A template among elements is recognised and skipped over.
        let template = ber::constructed(
            Tag::app(QUALIFIED_TEMPLATE),
            &[ber::ctx(0, ber::roid(&[1, 9]))],
        );
        let unknown_field = ber::constructed(
            Tag::app(NODE),
            &[
                ber::ctx(0, ber::integer(4)),
                ber::ctx(9, ber::utf8("future")),
            ],
        );
        let root = ber::constructed(
            Tag::app(ROOT),
            &[ber::constructed(
                Tag::app(ROOT_ELEMENT_COLLECTION),
                &[ber::ctx(0, template), ber::ctx(0, unknown_field)],
            )],
        );
        assert_eq!(
            decode(&root).unwrap(),
            vec![Root::Elements(vec![
                Element::Template,
                Element::Node(Node {
                    id: Id::Number(4),
                    contents: None,
                    children: None
                })
            ])]
        );
        assert!(decode(&[0x30, 0x00]).is_err());
        assert!(decode(&[0x60, 0x05, 0x6B]).is_err());
    }
}
