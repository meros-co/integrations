//! YAML loading for specs and conformance vectors.
//!
//! `granit-parser` (pure Rust, no unsafe code) reads the YAML; this module
//! turns its events into Serde values with the scalar rules the core has always
//! used (those of serde_yaml 0.9), so every spec and vector keeps its values:
//!
//! - a plain `null`, `Null`, `NULL`, `~` or empty scalar is null;
//! - a plain `true`, `True`, `TRUE` (and the `false` forms) is a boolean, and
//!   nothing else is (`yes`, `on` stay strings);
//! - a plain integer is decimal, `0x`, `0o` or `0b`, optionally signed; a
//!   leading zero (`0123`) or an underscore (`1_000`) keeps it a string;
//! - a plain float is what Rust parses as a finite float, or `.inf`/`.nan`;
//! - quoted and block scalars are strings, and any scalar reads into a string
//!   field exactly as written;
//! - a `!Name` tag selects an enum variant, and `!!str`, `!!int`, `!!float`,
//!   `!!bool`, `!!null` force a scalar's type;
//! - anchors and aliases are expanded; `<<` is an ordinary key (no merging);
//! - a key repeated in one mapping is an error, as is a second document.

use granit_parser::{Event, Marker, Parser, ScalarStyle, ScanError, Span, StrInput, Tag};
use serde::de::{
    self, value::StrDeserializer, DeserializeOwned, DeserializeSeed, Expected, Unexpected, Visitor,
};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display};
use std::num::ParseIntError;
use std::rc::Rc;

const TAG_BOOL: &str = "tag:yaml.org,2002:bool";
const TAG_INT: &str = "tag:yaml.org,2002:int";
const TAG_FLOAT: &str = "tag:yaml.org,2002:float";
const TAG_NULL: &str = "tag:yaml.org,2002:null";

/// How deep collections may nest while being deserialized.
const MAX_DEPTH: u8 = 128;
/// How deep collections may nest while being read, to keep the stack bounded.
const MAX_LOAD_DEPTH: usize = 1024;
/// Alias expansions allowed per event in the document (guards alias bombs).
const JUMPS_PER_EVENT: usize = 100;

/// Deserialize one YAML document.
pub(crate) fn from_str<T: DeserializeOwned>(input: &str) -> Result<T, Error> {
    let (root, events) = load(input)?;
    let jumps = Cell::new(0);
    T::deserialize(De {
        node: &root,
        path: Path::Root,
        jumps: &jumps,
        max_jumps: events.saturating_mul(JUMPS_PER_EVENT),
        depth: MAX_DEPTH,
        current_enum: None,
    })
}

/// A YAML parse or deserialization error, with its position when known.
#[derive(Debug)]
pub(crate) struct Error {
    msg: String,
    pos: Option<(Mark, String)>,
}

impl Error {
    fn at(msg: impl Into<String>, mark: Mark) -> Error {
        Error {
            msg: msg.into(),
            pos: Some((mark, ".".into())),
        }
    }

    fn scan(err: ScanError) -> Error {
        Error::at(err.info(), Mark::from(*err.marker()))
    }

    fn fix(mut self, mark: Mark, path: Path) -> Error {
        if self.pos.is_none() {
            self.pos = Some((mark, path.to_string()));
        }
        self
    }
}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Error {
        Error {
            msg: msg.to_string(),
            pos: None,
        }
    }
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.pos {
            None => f.write_str(&self.msg),
            Some((mark, path)) => {
                if path != "." {
                    write!(f, "{path}: ")?;
                }
                f.write_str(&self.msg)?;
                if mark.line != 1 || mark.column != 1 {
                    write!(f, " at line {} column {}", mark.line, mark.column)?;
                }
                Ok(())
            }
        }
    }
}

/// A 1-based source position.
#[derive(Clone, Copy, Debug)]
struct Mark {
    line: usize,
    column: usize,
}

impl From<Marker> for Mark {
    fn from(m: Marker) -> Mark {
        Mark {
            line: m.line(),
            column: m.col() + 1,
        }
    }
}

/// Where a value sits in the document, for error messages.
#[derive(Clone, Copy)]
enum Path<'a> {
    Root,
    Seq { parent: &'a Path<'a>, index: usize },
    Map { parent: &'a Path<'a>, key: &'a str },
    Alias { parent: &'a Path<'a> },
    Unknown { parent: &'a Path<'a> },
}

impl Display for Path<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct Parent<'a>(&'a Path<'a>);
        impl Display for Parent<'_> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self.0 {
                    Path::Root => Ok(()),
                    path => write!(f, "{path}."),
                }
            }
        }
        match self {
            Path::Root => f.write_str("."),
            Path::Seq { parent, index } => write!(f, "{parent}[{index}]"),
            Path::Map { parent, key } => write!(f, "{}{key}", Parent(parent)),
            Path::Alias { parent } => write!(f, "{parent}"),
            Path::Unknown { parent } => write!(f, "{}?", Parent(parent)),
        }
    }
}

struct Scalar {
    value: String,
    style: ScalarStyle,
    tag: Option<String>,
}

struct Node {
    mark: Mark,
    kind: Kind,
}

enum Kind {
    Scalar(Scalar),
    Seq(Option<String>, Vec<Node>),
    Map(Option<String>, Vec<(Node, Node)>),
    /// A node that carries an anchor; aliases share it.
    Anchor(Rc<Node>),
    Alias(Rc<Node>),
    /// An empty stream.
    Void,
}

impl Node {
    /// The node itself, past any anchor or alias.
    fn resolved(&self) -> &Node {
        match &self.kind {
            Kind::Anchor(n) | Kind::Alias(n) => n.resolved(),
            _ => self,
        }
    }
}

struct Loader<'i> {
    parser: Parser<'i, StrInput<'i>>,
    anchors: HashMap<usize, Rc<Node>>,
    events: usize,
}

/// Read the single document in `input` into a tree, with the number of events.
fn load(input: &str) -> Result<(Node, usize), Error> {
    let mut loader = Loader {
        parser: Parser::new_from_str(input),
        anchors: HashMap::new(),
        events: 0,
    };
    let mut root = None;
    loop {
        let (event, _) = loader.next()?;
        match event {
            Event::StreamEnd => break,
            Event::DocumentStart(..) => {
                if root.is_some() {
                    return Err(de::Error::custom(
                        "deserializing from YAML containing more than one document is not \
                         supported",
                    ));
                }
                let (event, span) = loader.next()?;
                root = Some(loader.node(event, span, 0)?);
            }
            _ => {}
        }
    }
    let root = root.unwrap_or(Node {
        mark: Mark { line: 1, column: 1 },
        kind: Kind::Void,
    });
    Ok((root, loader.events))
}

impl<'i> Loader<'i> {
    fn next(&mut self) -> Result<(Event<'i>, Span), Error> {
        match self.parser.next_event() {
            Some(Ok(event)) => {
                self.events += 1;
                Ok(event)
            }
            Some(Err(err)) => Err(Error::scan(err)),
            None => Err(de::Error::custom("unexpected end of the YAML event stream")),
        }
    }

    fn node(&mut self, event: Event<'i>, span: Span, depth: usize) -> Result<Node, Error> {
        let mark = Mark::from(span.start);
        let tag = |tag: Option<std::borrow::Cow<Tag>>| {
            tag.map(|t| format!("{}{}", t.handle(), t.suffix()))
        };
        let (anchor, kind) = match event {
            Event::Alias(id) => {
                let target = self
                    .anchors
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| Error::at("unknown anchor", mark))?;
                return Ok(Node {
                    mark,
                    kind: Kind::Alias(target),
                });
            }
            Event::Scalar(value, style, anchor, t) => (
                anchor,
                Kind::Scalar(Scalar {
                    value: value.into_owned(),
                    style,
                    tag: tag(t),
                }),
            ),
            Event::SequenceStart(_, anchor, t) => {
                if depth >= MAX_LOAD_DEPTH {
                    return Err(Error::at("recursion limit exceeded", mark));
                }
                let mut items = Vec::new();
                loop {
                    let (event, span) = self.next()?;
                    if let Event::SequenceEnd = event {
                        break;
                    }
                    items.push(self.node(event, span, depth + 1)?);
                }
                (anchor, Kind::Seq(tag(t), items))
            }
            Event::MappingStart(_, anchor, t) => {
                if depth >= MAX_LOAD_DEPTH {
                    return Err(Error::at("recursion limit exceeded", mark));
                }
                let mut entries = Vec::new();
                let mut keys = HashSet::new();
                loop {
                    let (event, span) = self.next()?;
                    if let Event::MappingEnd = event {
                        break;
                    }
                    let key = self.node(event, span, depth + 1)?;
                    if let Kind::Scalar(s) = &key.resolved().kind {
                        if !keys.insert(s.value.clone()) {
                            return Err(Error::at(
                                format!("duplicate entry with key \"{}\"", s.value),
                                key.mark,
                            ));
                        }
                    }
                    let (event, span) = self.next()?;
                    let value = self.node(event, span, depth + 1)?;
                    entries.push((key, value));
                }
                (anchor, Kind::Map(tag(t), entries))
            }
            other => {
                return Err(Error::at(format!("unexpected YAML event {other:?}"), mark));
            }
        };
        let node = Node { mark, kind };
        if anchor == 0 {
            return Ok(node);
        }
        let shared = Rc::new(node);
        self.anchors.insert(anchor, Rc::clone(&shared));
        Ok(Node {
            mark,
            kind: Kind::Anchor(shared),
        })
    }
}

#[derive(Clone, Copy)]
struct CurrentEnum<'a> {
    name: Option<&'static str>,
    tag: &'a str,
}

#[derive(Clone, Copy)]
struct De<'a> {
    node: &'a Node,
    path: Path<'a>,
    jumps: &'a Cell<usize>,
    max_jumps: usize,
    depth: u8,
    current_enum: Option<CurrentEnum<'a>>,
}

impl<'a> De<'a> {
    /// The deserializer for the node an anchor or alias stands for.
    fn follow(&self) -> Result<Option<De<'_>>, Error> {
        match &self.node.kind {
            Kind::Anchor(node) => Ok(Some(De { node, ..*self })),
            Kind::Alias(node) => {
                self.jumps.set(self.jumps.get() + 1);
                if self.jumps.get() > self.max_jumps {
                    return Err(Error::at("repetition limit exceeded", self.node.mark));
                }
                Ok(Some(De {
                    node,
                    path: Path::Alias { parent: &self.path },
                    current_enum: None,
                    ..*self
                }))
            }
            _ => Ok(None),
        }
    }

    fn fix(&self, err: Error) -> Error {
        err.fix(self.node.mark, self.path)
    }

    fn child(&self, node: &'a Node, path: Path<'a>) -> De<'a> {
        De {
            node,
            path,
            current_enum: None,
            ..*self
        }
    }

    fn deeper(&self) -> Result<De<'a>, Error> {
        match self.depth.checked_sub(1) {
            Some(depth) => Ok(De { depth, ..*self }),
            None => Err(Error::at("recursion limit exceeded", self.node.mark)),
        }
    }

    fn invalid_type(&self, exp: &dyn Expected) -> Error {
        match &self.node.kind {
            Kind::Scalar(s) => {
                let visitor = InvalidType { exp };
                match visit_scalar(visitor, s, false) {
                    Ok(void) => match void {},
                    Err(err) => err,
                }
            }
            Kind::Seq(..) => de::Error::invalid_type(Unexpected::Seq, exp),
            Kind::Map(..) => de::Error::invalid_type(Unexpected::Map, exp),
            _ => de::Error::custom("EOF while parsing a value"),
        }
    }

    fn visit_seq<'de, V: Visitor<'de>>(
        self,
        visitor: V,
        items: &'a [Node],
    ) -> Result<V::Value, Error> {
        let mut access = SeqAccess {
            items,
            index: 0,
            de: self.deeper()?,
        };
        let value = visitor.visit_seq(&mut access)?;
        if access.index < items.len() {
            return Err(de::Error::invalid_length(
                items.len(),
                &ExpectedSeq(access.index),
            ));
        }
        Ok(value)
    }

    fn visit_map<'de, V: Visitor<'de>>(
        self,
        visitor: V,
        entries: &'a [(Node, Node)],
    ) -> Result<V::Value, Error> {
        let mut access = MapAccess {
            entries,
            index: 0,
            de: self.deeper()?,
        };
        let value = visitor.visit_map(&mut access)?;
        if access.index < entries.len() {
            return Err(de::Error::invalid_length(
                entries.len(),
                &ExpectedMap(access.index),
            ));
        }
        Ok(value)
    }

    fn is_empty_plain(&self) -> bool {
        match &self.node.kind {
            Kind::Void => true,
            Kind::Scalar(s) => s.value.is_empty() && s.style == ScalarStyle::Plain,
            _ => false,
        }
    }

    /// A plain scalar, or a literal block scalar tagged with `tag`, to read as a
    /// typed value.
    fn typed_scalar(&self, tag: &str) -> Option<&'a str> {
        let Kind::Scalar(s) = &self.node.kind else {
            return None;
        };
        let typed = match (s.style, &s.tag, self.current_enum.is_some()) {
            (ScalarStyle::Plain, _, _) => true,
            (ScalarStyle::Literal, Some(t), false) => t == tag,
            _ => false,
        };
        typed.then_some(s.value.as_str())
    }
}

/// Every scalar is valid for this visitor's type except the one tried, so
/// visiting yields serde's "invalid type" error for the scalar as resolved.
struct InvalidType<'a> {
    exp: &'a dyn Expected,
}

enum Never {}

impl<'de> Visitor<'de> for InvalidType<'_> {
    type Value = Never;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.exp.fmt(f)
    }
}

struct ExpectedSeq(usize);

impl Expected for ExpectedSeq {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0 == 1 {
            write!(f, "sequence of 1 element")
        } else {
            write!(f, "sequence of {} elements", self.0)
        }
    }
}

struct ExpectedMap(usize);

impl Expected for ExpectedMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0 == 1 {
            write!(f, "map containing 1 entry")
        } else {
            write!(f, "map containing {} entries", self.0)
        }
    }
}

/// The enum variant a `!Name` tag selects.
fn parse_tag(tag: &Option<String>) -> Option<&str> {
    let rest = tag.as_deref()?.strip_prefix('!')?;
    Some(if rest.is_empty() { "!" } else { rest })
}

fn parse_null(scalar: &str) -> Option<()> {
    matches!(scalar, "null" | "Null" | "NULL" | "~").then_some(())
}

fn parse_bool(scalar: &str) -> Option<bool> {
    match scalar {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

/// Leading zeros followed by digits are a string (YAML 1.2), not a number.
fn digits_but_not_number(scalar: &str) -> bool {
    let scalar = scalar.strip_prefix(['-', '+']).unwrap_or(scalar);
    scalar.len() > 1 && scalar.starts_with('0') && scalar[1..].bytes().all(|b| b.is_ascii_digit())
}

/// `rest` in `radix`, refusing a sign after the prefix.
fn radix<T>(rest: &str, radix: u32, parse: fn(&str, u32) -> Result<T, ParseIntError>) -> Option<T> {
    if rest.starts_with(['+', '-']) {
        return None;
    }
    parse(rest, radix).ok()
}

/// `-0x..`, `-0o..` or `-0b..` as a negative number.
fn negative_radix<T>(scalar: &str, parse: fn(&str, u32) -> Result<T, ParseIntError>) -> Option<T> {
    for (prefix, base) in [("-0x", 16), ("-0o", 8), ("-0b", 2)] {
        if let Some(rest) = scalar.strip_prefix(prefix) {
            if let Ok(int) = parse(&format!("-{rest}"), base) {
                return Some(int);
            }
        }
    }
    None
}

fn parse_unsigned_int<T>(
    scalar: &str,
    parse: fn(&str, u32) -> Result<T, ParseIntError>,
) -> Option<T> {
    let unpositive = scalar.strip_prefix('+').unwrap_or(scalar);
    for (prefix, base) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(rest) = unpositive.strip_prefix(prefix) {
            if rest.starts_with(['+', '-']) {
                return None;
            }
            if let Some(int) = radix(rest, base, parse) {
                return Some(int);
            }
        }
    }
    if unpositive.starts_with(['+', '-']) || digits_but_not_number(scalar) {
        return None;
    }
    parse(unpositive, 10).ok()
}

fn parse_signed_int<T>(
    scalar: &str,
    parse: fn(&str, u32) -> Result<T, ParseIntError>,
) -> Option<T> {
    let unpositive = match scalar.strip_prefix('+') {
        Some(rest) if rest.starts_with(['+', '-']) => return None,
        Some(rest) => rest,
        None => scalar,
    };
    for (prefix, base) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(rest) = unpositive.strip_prefix(prefix) {
            if rest.starts_with(['+', '-']) {
                return None;
            }
            if let Some(int) = radix(rest, base, parse) {
                return Some(int);
            }
        }
        let negative = format!("-{prefix}");
        if let Some(rest) = scalar.strip_prefix(negative.as_str()) {
            if let Ok(int) = parse(&format!("-{rest}"), base) {
                return Some(int);
            }
        }
    }
    if digits_but_not_number(scalar) {
        return None;
    }
    parse(unpositive, 10).ok()
}

fn parse_negative_int<T>(
    scalar: &str,
    parse: fn(&str, u32) -> Result<T, ParseIntError>,
) -> Option<T> {
    if let Some(int) = negative_radix(scalar, parse) {
        return Some(int);
    }
    if digits_but_not_number(scalar) {
        return None;
    }
    parse(scalar, 10).ok()
}

fn parse_f64(scalar: &str) -> Option<f64> {
    let unpositive = match scalar.strip_prefix('+') {
        Some(rest) if rest.starts_with(['+', '-']) => return None,
        Some(rest) => rest,
        None => scalar,
    };
    if let ".inf" | ".Inf" | ".INF" = unpositive {
        return Some(f64::INFINITY);
    }
    if let "-.inf" | "-.Inf" | "-.INF" = scalar {
        return Some(f64::NEG_INFINITY);
    }
    if let ".nan" | ".NaN" | ".NAN" = scalar {
        return Some(f64::NAN.copysign(1.0));
    }
    unpositive.parse::<f64>().ok().filter(|f| f.is_finite())
}

fn visit_int<'de, V: Visitor<'de>>(visitor: V, v: &str) -> Result<Result<V::Value, Error>, V> {
    if let Some(int) = parse_unsigned_int(v, u64::from_str_radix) {
        return Ok(visitor.visit_u64(int));
    }
    if let Some(int) = parse_negative_int(v, i64::from_str_radix) {
        return Ok(visitor.visit_i64(int));
    }
    if let Some(int) = parse_unsigned_int(v, u128::from_str_radix) {
        return Ok(visitor.visit_u128(int));
    }
    if let Some(int) = parse_negative_int(v, i128::from_str_radix) {
        return Ok(visitor.visit_i128(int));
    }
    Err(visitor)
}

fn visit_untagged_scalar<'de, V: Visitor<'de>>(visitor: V, v: &str) -> Result<V::Value, Error> {
    if v.is_empty() || parse_null(v).is_some() {
        return visitor.visit_unit();
    }
    if let Some(boolean) = parse_bool(v) {
        return visitor.visit_bool(boolean);
    }
    let visitor = match visit_int(visitor, v) {
        Ok(result) => return result,
        Err(visitor) => visitor,
    };
    if !digits_but_not_number(v) {
        if let Some(float) = parse_f64(v) {
            return visitor.visit_f64(float);
        }
    }
    visitor.visit_str(v)
}

fn visit_scalar<'de, V: Visitor<'de>>(
    visitor: V,
    scalar: &Scalar,
    tagged_already: bool,
) -> Result<V::Value, Error> {
    let v = scalar.value.as_str();
    if let (Some(tag), false) = (&scalar.tag, tagged_already) {
        match tag.as_str() {
            TAG_BOOL => {
                return match parse_bool(v) {
                    Some(b) => visitor.visit_bool(b),
                    None => Err(de::Error::invalid_value(Unexpected::Str(v), &"a boolean")),
                }
            }
            TAG_INT => {
                return match visit_int(visitor, v) {
                    Ok(result) => result,
                    Err(_) => Err(de::Error::invalid_value(Unexpected::Str(v), &"an integer")),
                }
            }
            TAG_FLOAT => {
                return match parse_f64(v) {
                    Some(f) => visitor.visit_f64(f),
                    None => Err(de::Error::invalid_value(Unexpected::Str(v), &"a float")),
                }
            }
            TAG_NULL => {
                return match parse_null(v) {
                    Some(()) => visitor.visit_unit(),
                    None => Err(de::Error::invalid_value(Unexpected::Str(v), &"null")),
                }
            }
            _ if tag.starts_with('!') && scalar.style == ScalarStyle::Plain => {
                return visit_untagged_scalar(visitor, v);
            }
            _ => {}
        }
    } else if scalar.style == ScalarStyle::Plain {
        return visit_untagged_scalar(visitor, v);
    }
    visitor.visit_str(v)
}

/// Follow an anchor or alias, then rerun the same method on its target.
macro_rules! follow {
    ($self:ident . $method:ident ( $($arg:expr),* )) => {
        if let Some(target) = $self.follow()? {
            return target.$method($($arg),*);
        }
    };
}

impl<'de, 'a> de::Deserializer<'de> for De<'a> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        follow!(self.deserialize_any(visitor));
        let tagged_already = self.current_enum.is_some();
        let enum_tag = |tag: &'a Option<String>| {
            if tagged_already {
                None
            } else {
                parse_tag(tag)
            }
        };
        match &self.node.kind {
            Kind::Scalar(Scalar { tag, .. }) | Kind::Seq(tag, _) | Kind::Map(tag, _)
                if enum_tag(tag).is_some() =>
            {
                let tag = enum_tag(tag).unwrap_or_default();
                visitor.visit_enum(EnumAccess {
                    de: self,
                    name: None,
                    tag,
                })
            }
            Kind::Scalar(s) => visit_scalar(visitor, s, tagged_already),
            Kind::Seq(_, items) => self.visit_seq(visitor, items),
            Kind::Map(_, entries) => self.visit_map(visitor, entries),
            _ => visitor.visit_none(),
        }
        .map_err(|e| self.fix(e))
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        follow!(self.deserialize_bool(visitor));
        match self.typed_scalar(TAG_BOOL).and_then(parse_bool) {
            Some(b) => visitor.visit_bool(b),
            None => Err(self.invalid_type(&visitor)),
        }
        .map_err(|e| self.fix(e))
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_i64(visitor)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_i64(visitor)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_i64(visitor)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        follow!(self.deserialize_i64(visitor));
        let int = self
            .typed_scalar(TAG_INT)
            .and_then(|v| parse_signed_int(v, i64::from_str_radix));
        match int {
            Some(int) => visitor.visit_i64(int),
            None => Err(self.invalid_type(&visitor)),
        }
        .map_err(|e| self.fix(e))
    }

    fn deserialize_i128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        follow!(self.deserialize_i128(visitor));
        let int = self
            .typed_scalar(TAG_INT)
            .and_then(|v| parse_signed_int(v, i128::from_str_radix));
        match int {
            Some(int) => visitor.visit_i128(int),
            None => Err(self.invalid_type(&visitor)),
        }
        .map_err(|e| self.fix(e))
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_u64(visitor)
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_u64(visitor)
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_u64(visitor)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        follow!(self.deserialize_u64(visitor));
        let int = self
            .typed_scalar(TAG_INT)
            .and_then(|v| parse_unsigned_int(v, u64::from_str_radix));
        match int {
            Some(int) => visitor.visit_u64(int),
            None => Err(self.invalid_type(&visitor)),
        }
        .map_err(|e| self.fix(e))
    }

    fn deserialize_u128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        follow!(self.deserialize_u128(visitor));
        let int = self
            .typed_scalar(TAG_INT)
            .and_then(|v| parse_unsigned_int(v, u128::from_str_radix));
        match int {
            Some(int) => visitor.visit_u128(int),
            None => Err(self.invalid_type(&visitor)),
        }
        .map_err(|e| self.fix(e))
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_f64(visitor)
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        follow!(self.deserialize_f64(visitor));
        match self.typed_scalar(TAG_FLOAT).and_then(parse_f64) {
            Some(f) => visitor.visit_f64(f),
            None => Err(self.invalid_type(&visitor)),
        }
        .map_err(|e| self.fix(e))
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        follow!(self.deserialize_str(visitor));
        match &self.node.kind {
            Kind::Scalar(s) => visitor.visit_str(&s.value),
            _ => Err(self.invalid_type(&visitor)),
        }
        .map_err(|e| self.fix(e))
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Error> {
        Err(de::Error::custom(
            "serialization and deserialization of bytes in YAML is not implemented",
        ))
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_bytes(visitor)
    }

    /// `null` is `None`; anything else is `Some(...)`.
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        follow!(self.deserialize_option(visitor));
        let is_some = match &self.node.kind {
            Kind::Scalar(s) if s.style != ScalarStyle::Plain => true,
            Kind::Scalar(s) => match (&s.tag, self.current_enum.is_some()) {
                (Some(tag), false) if tag == TAG_NULL => {
                    if parse_null(&s.value).is_none() {
                        return Err(
                            self.fix(de::Error::invalid_value(Unexpected::Str(&s.value), &"null"))
                        );
                    }
                    false
                }
                (Some(_), false) => true,
                _ => !s.value.is_empty() && parse_null(&s.value).is_none(),
            },
            Kind::Void => false,
            _ => true,
        };
        if is_some {
            visitor.visit_some(self)
        } else {
            visitor.visit_none()
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        follow!(self.deserialize_unit(visitor));
        match &self.node.kind {
            Kind::Scalar(s) => {
                let is_null = if s.style != ScalarStyle::Plain {
                    false
                } else if let (Some(tag), false) = (&s.tag, self.current_enum.is_some()) {
                    tag == TAG_NULL && parse_null(&s.value).is_some()
                } else {
                    s.value.is_empty() || parse_null(&s.value).is_some()
                };
                if is_null {
                    visitor.visit_unit()
                } else {
                    Err(de::Error::invalid_value(Unexpected::Str(&s.value), &"null"))
                }
            }
            Kind::Void => visitor.visit_unit(),
            _ => Err(self.invalid_type(&visitor)),
        }
        .map_err(|e| self.fix(e))
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self.deeper()?)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        follow!(self.deserialize_seq(visitor));
        match &self.node.kind {
            Kind::Seq(_, items) => self.visit_seq(visitor, items),
            _ if self.is_empty_plain() => self.visit_seq(visitor, &[]),
            _ => Err(self.invalid_type(&visitor)),
        }
        .map_err(|e| self.fix(e))
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        follow!(self.deserialize_map(visitor));
        match &self.node.kind {
            Kind::Map(_, entries) => self.visit_map(visitor, entries),
            _ if self.is_empty_plain() => self.visit_map(visitor, &[]),
            _ => Err(self.invalid_type(&visitor)),
        }
        .map_err(|e| self.fix(e))
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_map(visitor)
    }

    /// A unit variant is its name; any other variant is a `!Name` tag on its
    /// content.
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        if let Some(current) = self.current_enum {
            if let Kind::Scalar(s) = &self.node.resolved().kind {
                if !s.value.is_empty() {
                    return visitor.visit_enum(UnitVariantAccess { de: self });
                }
            }
            let message = match current.name {
                Some(name) => format!(
                    "deserializing nested enum in {name}::{} from YAML is not supported yet",
                    current.tag
                ),
                None => format!(
                    "deserializing nested enum in !{} from YAML is not supported yet",
                    current.tag
                ),
            };
            return Err(self.fix(de::Error::custom(message)));
        }
        follow!(self.deserialize_enum(name, variants, visitor));
        match &self.node.kind {
            Kind::Scalar(s) => match parse_tag(&s.tag) {
                Some(tag) => visitor.visit_enum(EnumAccess {
                    de: self,
                    name: Some(name),
                    tag,
                }),
                None => visitor.visit_enum(UnitVariantAccess { de: self }),
            },
            Kind::Map(tag, _) | Kind::Seq(tag, _) => match parse_tag(tag) {
                Some(tag) => visitor.visit_enum(EnumAccess {
                    de: self,
                    name: Some(name),
                    tag,
                }),
                None => {
                    let unexpected = match &self.node.kind {
                        Kind::Map(..) => Unexpected::Map,
                        _ => Unexpected::Seq,
                    };
                    Err(de::Error::invalid_type(
                        unexpected,
                        &"a YAML tag starting with '!'",
                    ))
                }
            },
            _ => Err(de::Error::custom("EOF while parsing a value")),
        }
        .map_err(|e| self.fix(e))
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }
}

struct SeqAccess<'a> {
    items: &'a [Node],
    index: usize,
    de: De<'a>,
}

impl<'de> de::SeqAccess<'de> for SeqAccess<'_> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        let Some(node) = self.items.get(self.index) else {
            return Ok(None);
        };
        let index = self.index;
        self.index += 1;
        let path = Path::Seq {
            parent: &self.de.path,
            index,
        };
        seed.deserialize(self.de.child(node, path)).map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.items.len() - self.index)
    }
}

struct MapAccess<'a> {
    entries: &'a [(Node, Node)],
    index: usize,
    de: De<'a>,
}

impl<'de> de::MapAccess<'de> for MapAccess<'_> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        let Some((key, _)) = self.entries.get(self.index) else {
            return Ok(None);
        };
        self.index += 1;
        seed.deserialize(self.de.child(key, self.de.path)).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        let (key, value) = &self.entries[self.index - 1];
        let key = match &key.kind {
            Kind::Anchor(node) => node,
            _ => key,
        };
        let path = match &key.kind {
            Kind::Scalar(s) => Path::Map {
                parent: &self.de.path,
                key: &s.value,
            },
            _ => Path::Unknown {
                parent: &self.de.path,
            },
        };
        seed.deserialize(self.de.child(value, path))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.entries.len() - self.index)
    }
}

/// A variant selected by a `!Name` tag; its content is the tagged node.
struct EnumAccess<'a> {
    de: De<'a>,
    name: Option<&'static str>,
    tag: &'a str,
}

impl<'de, 'a> de::EnumAccess<'de> for EnumAccess<'a> {
    type Error = Error;
    type Variant = De<'a>;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, De<'a>), Error> {
        let variant = seed.deserialize(StrDeserializer::<Error>::new(self.tag))?;
        let content = De {
            current_enum: Some(CurrentEnum {
                name: self.name,
                tag: self.tag,
            }),
            ..self.de
        };
        Ok((variant, content))
    }
}

impl<'de> de::VariantAccess<'de> for De<'_> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        de::Deserialize::deserialize(self)
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(self)
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_seq(self, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_struct(self, "", fields, visitor)
    }
}

/// A unit variant written as its name.
struct UnitVariantAccess<'a> {
    de: De<'a>,
}

impl<'de, 'a> de::EnumAccess<'de> for UnitVariantAccess<'a> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self), Error> {
        Ok((seed.deserialize(self.de)?, self))
    }
}

impl<'de> de::VariantAccess<'de> for UnitVariantAccess<'_> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, _seed: T) -> Result<T::Value, Error> {
        Err(de::Error::invalid_type(
            Unexpected::UnitVariant,
            &"newtype variant",
        ))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value, Error> {
        Err(de::Error::invalid_type(
            Unexpected::UnitVariant,
            &"tuple variant",
        ))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Error> {
        Err(de::Error::invalid_type(
            Unexpected::UnitVariant,
            &"struct variant",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::from_str;
    use serde::Deserialize;
    use serde_json::{json, Value};

    fn value(yaml: &str) -> Value {
        from_str::<Value>(yaml).unwrap_or_else(|e| panic!("{yaml:?}: {e}"))
    }

    fn error(yaml: &str) -> String {
        match from_str::<Value>(yaml) {
            Ok(v) => panic!("{yaml:?} parsed as {v}"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn scalars_resolve_as_specs_expect() {
        let cases = [
            ("~", json!(null)),
            ("null", json!(null)),
            ("NULL", json!(null)),
            ("", json!(null)),
            ("nULL", json!("nULL")),
            ("true", json!(true)),
            ("False", json!(false)),
            ("tRue", json!("tRue")),
            ("yes", json!("yes")),
            ("on", json!("on")),
            ("12", json!(12)),
            ("+12", json!(12)),
            ("-12", json!(-12)),
            ("0x1F", json!(31)),
            ("-0x10", json!(-16)),
            ("0o17", json!(15)),
            ("0b101", json!(5)),
            ("0X1F", json!("0X1F")),
            ("0123456789", json!("0123456789")),
            ("017", json!("017")),
            ("-017", json!("-017")),
            ("1_000", json!("1_000")),
            ("18446744073709551615", json!(u64::MAX)),
            ("-9223372036854775808", json!(i64::MIN)),
            ("1.5", json!(1.5)),
            ("1e3", json!(1000.0)),
            ("1.", json!(1.0)),
            (".5", json!(0.5)),
            ("1e400", json!("1e400")),
            (".inf", json!(null)),
            ("12:30", json!("12:30")),
            ("2001-12-14", json!("2001-12-14")),
            ("'12'", json!("12")),
            ("\"true\"", json!("true")),
            ("!!str 12", json!("12")),
            ("!!int '12'", json!(12)),
            ("!!float 1", json!(1.0)),
            ("|\n  a\n  b\n", json!("a\nb\n")),
            (">-\n  a\n  b\n", json!("a b")),
            ("[1, '2', x]", json!([1, "2", "x"])),
        ];
        for (yaml, want) in cases {
            assert_eq!(value(&format!("v: {yaml}"))["v"], want, "{yaml:?}");
        }
    }

    #[test]
    fn rejected_documents() {
        for (yaml, why) in [
            ("v: 18446744073709551616", "out of range"),
            ("v: !Tag x", "invalid type: enum"),
            ("a: 1\na: 2", "duplicate entry with key \"a\""),
            ("{a: 1, 'a': 2}", "duplicate entry with key \"a\""),
            ("a: 1\n---\nb: 2", "more than one document"),
            ("a: *missing", "unknown anchor"),
            ("a: [1", "line 1"),
        ] {
            let e = error(yaml);
            assert!(e.contains(why), "{yaml:?}: {e}");
        }
    }

    #[test]
    fn typed_fields_read_scalars_as_written() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Fields {
            s: String,
            n: Option<u16>,
            f: f64,
            b: bool,
        }
        let parsed: Fields = from_str("s: 0123\nn: ~\nf: 5\nb: true").unwrap();
        let want = Fields {
            s: "0123".into(),
            n: None,
            f: 5.0,
            b: true,
        };
        assert_eq!(parsed, want);
        let parsed: Fields = from_str("s: null\nn: 0x10\nf: -1.5e1\nb: FALSE").unwrap();
        assert_eq!(
            (parsed.s.as_str(), parsed.n, parsed.f, parsed.b),
            ("null", Some(16), -15.0, false)
        );
        for yaml in [
            "s: x\nf: '5'\nb: true",
            "s: x\nf: 5\nb: yes",
            "s: x\nn: 70000\nf: 5\nb: true",
            "s: [x]\nf: 5\nb: true",
        ] {
            assert!(from_str::<Fields>(yaml).is_err(), "{yaml:?}");
        }
    }

    #[test]
    fn enums_take_names_and_tags() {
        #[derive(Deserialize, Debug, PartialEq)]
        enum Outer {
            Unit,
            New(String),
            Struct { x: i32 },
        }
        #[derive(Deserialize, Debug, PartialEq)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum Inner {
            Text { v: Value },
        }
        let parsed: Vec<Outer> = from_str("[Unit, !New x, !Struct {x: 2}]").unwrap();
        let want = vec![Outer::Unit, Outer::New("x".into()), Outer::Struct { x: 2 }];
        assert_eq!(parsed, want);
        assert!(from_str::<Outer>("{New: x}").is_err());
        let parsed: Inner = from_str("type: text\nv: 01").unwrap();
        assert_eq!(parsed, Inner::Text { v: json!("01") });
    }

    #[test]
    fn anchors_expand_and_merge_keys_stay_keys() {
        let v = value("a: &x {k: 1}\nb: *x\nc:\n  <<: *x\n  d: 2");
        assert_eq!(v["b"], json!({"k": 1}));
        assert_eq!(v["c"], json!({"<<": {"k": 1}, "d": 2}));
    }

    #[test]
    fn a_dash_inside_a_flow_scalar_is_text() {
        let v = value("p: { label: Detail gain -, description: x }");
        assert_eq!(v["p"]["label"], json!("Detail gain -"));
    }

    #[test]
    fn alias_bombs_and_deep_nesting_are_refused() {
        let mut bomb = String::from("a0: &a0 [x, x, x, x, x, x, x, x, x, x]\n");
        for i in 1..10 {
            let p = i - 1;
            bomb +=
                &format!("a{i}: &a{i} [*a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}]\n");
        }
        assert!(error(&bomb).contains("repetition limit exceeded"));
        let deep = format!("{}{}", "[".repeat(130), "]".repeat(130));
        assert!(error(&deep).contains("recursion limit exceeded"));
    }
}
