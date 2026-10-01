//! EmBER: the subset of the Basic Encoding Rules (ITU-T X.690) that Ember+
//! uses, written from Lawo's Ember+ Specification 2.50, chapter "EmBER".
//!
//! - Types: Boolean, Integer (at most 64 bits), Real (binary, base 2, at most
//!   double precision), UTF8String, Octet String (primitive only), Null,
//!   Relative Object Identifier, Set and Sequence.
//! - Application and context tags are explicit around primitive values
//!   ("TLTLV"); constructed application types are tagged implicitly.
//! - Definite and indefinite lengths are both allowed, the indefinite form for
//!   containers only. This encoder always writes definite lengths.

/// Tag classes, as the specification's "The tag format" table gives them.
pub(crate) const UNIVERSAL: u8 = 0x00;
pub(crate) const APPLICATION: u8 = 0x40;
pub(crate) const CONTEXT: u8 = 0x80;

/// Universal type numbers (X.690; the specification's type table).
pub(crate) const BOOLEAN: u32 = 1;
pub(crate) const INTEGER: u32 = 2;
pub(crate) const OCTET_STRING: u32 = 4;
pub(crate) const NULL: u32 = 5;
pub(crate) const REAL: u32 = 9;
pub(crate) const UTF8_STRING: u32 = 12;
pub(crate) const RELATIVE_OID: u32 = 13;
pub(crate) const SEQUENCE: u32 = 16;
pub(crate) const SET: u32 = 17;

/// Containers nest this deep at most; a message deeper than this is refused
/// rather than overflowing the stack.
const MAX_DEPTH: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tag {
    pub class: u8,
    pub number: u32,
}

impl Tag {
    pub(crate) const fn universal(number: u32) -> Tag {
        Tag {
            class: UNIVERSAL,
            number,
        }
    }
    pub(crate) const fn app(number: u32) -> Tag {
        Tag {
            class: APPLICATION,
            number,
        }
    }
    pub(crate) const fn ctx(number: u32) -> Tag {
        Tag {
            class: CONTEXT,
            number,
        }
    }
}

/// One decoded tag-length-value. A constructed value holds its children, a
/// primitive one its content octets.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Tlv {
    pub tag: Tag,
    pub constructed: bool,
    pub content: Vec<u8>,
    pub children: Vec<Tlv>,
}

impl Tlv {
    /// The single value inside an explicitly tagged field.
    pub(crate) fn inner(&self) -> Option<&Tlv> {
        if self.constructed {
            self.children.first()
        } else {
            None
        }
    }

    /// The child explicitly tagged `[CONTEXT n]`, unwrapped.
    pub(crate) fn field(&self, n: u32) -> Option<&Tlv> {
        self.children
            .iter()
            .find(|c| c.tag == Tag::ctx(n))
            .and_then(Tlv::inner)
    }

    pub(crate) fn is(&self, number: u32) -> bool {
        self.tag == Tag::universal(number)
    }

    pub(crate) fn as_bool(&self) -> Option<bool> {
        (self.is(BOOLEAN) && !self.content.is_empty()).then(|| self.content.iter().any(|&b| b != 0))
    }

    pub(crate) fn as_i64(&self) -> Option<i64> {
        if self.is(INTEGER) {
            decode_integer(&self.content)
        } else {
            None
        }
    }

    pub(crate) fn as_f64(&self) -> Option<f64> {
        if self.is(REAL) {
            decode_real(&self.content)
        } else {
            None
        }
    }

    pub(crate) fn as_string(&self) -> Option<String> {
        if self.is(UTF8_STRING) {
            Some(String::from_utf8_lossy(&self.content).into_owned())
        } else {
            None
        }
    }

    pub(crate) fn as_roid(&self) -> Option<Vec<u32>> {
        if self.is(RELATIVE_OID) {
            decode_roid(&self.content)
        } else {
            None
        }
    }
}

// --- Decoding ----------------------------------------------------------------

fn take<'a>(buf: &'a [u8], at: &mut usize, n: usize) -> Result<&'a [u8], String> {
    let end = at
        .checked_add(n)
        .filter(|&e| e <= buf.len())
        .ok_or("truncated")?;
    let s = &buf[*at..end];
    *at = end;
    Ok(s)
}

fn read_tag(buf: &[u8], at: &mut usize) -> Result<(Tag, bool), String> {
    let first = take(buf, at, 1)?[0];
    let class = first & 0xC0;
    let constructed = first & 0x20 != 0;
    let mut number = u32::from(first & 0x1F);
    if number == 0x1F {
        // High tag number form: base-128, most significant group first.
        number = 0;
        loop {
            let b = take(buf, at, 1)?[0];
            if number > (u32::MAX >> 7) {
                return Err("tag number too large".into());
            }
            number = (number << 7) | u32::from(b & 0x7F);
            if b & 0x80 == 0 {
                break;
            }
        }
    }
    Ok((Tag { class, number }, constructed))
}

/// `None` is the indefinite form.
fn read_length(buf: &[u8], at: &mut usize) -> Result<Option<usize>, String> {
    let first = take(buf, at, 1)?[0];
    if first < 0x80 {
        return Ok(Some(usize::from(first)));
    }
    if first == 0x80 {
        return Ok(None);
    }
    let count = usize::from(first & 0x7F);
    if count > 8 {
        return Err("length of more than 8 octets".into());
    }
    let mut len: u64 = 0;
    for &b in take(buf, at, count)? {
        len = (len << 8) | u64::from(b);
    }
    usize::try_from(len)
        .map_err(|_| "length too large".to_string())
        .map(Some)
}

fn parse_at(buf: &[u8], at: &mut usize, depth: usize) -> Result<Tlv, String> {
    if depth > MAX_DEPTH {
        return Err("nested too deeply".into());
    }
    let (tag, constructed) = read_tag(buf, at)?;
    let length = read_length(buf, at)?;
    if !constructed && tag.class != UNIVERSAL {
        // The specification's own library example writes an explicit
        // application tag with the constructed bit clear (0x41 for
        // Application-1). Glow has no primitive application or context
        // values, so such a tag is read as the wrapper it stands for.
        let len = length.ok_or("indefinite length on a primitive value")?;
        let content = take(buf, at, len)?;
        if let Ok(children) = parse_all(content) {
            return Ok(Tlv {
                tag,
                constructed: true,
                content: Vec::new(),
                children,
            });
        }
        return Ok(Tlv {
            tag,
            constructed,
            content: content.to_vec(),
            children: Vec::new(),
        });
    }
    if !constructed {
        let len = length.ok_or("indefinite length on a primitive value")?;
        return Ok(Tlv {
            tag,
            constructed,
            content: take(buf, at, len)?.to_vec(),
            children: Vec::new(),
        });
    }
    let mut children = Vec::new();
    match length {
        Some(len) => {
            let end = at
                .checked_add(len)
                .filter(|&e| e <= buf.len())
                .ok_or("truncated")?;
            let inner = &buf[..end];
            while *at < end {
                children.push(parse_at(inner, at, depth + 1)?);
            }
        }
        None => loop {
            if buf.get(*at..*at + 2) == Some(&[0, 0][..]) {
                *at += 2;
                break;
            }
            if *at >= buf.len() {
                return Err("indefinite length without its end".into());
            }
            children.push(parse_at(buf, at, depth + 1)?);
        },
    }
    Ok(Tlv {
        tag,
        constructed,
        content: Vec::new(),
        children,
    })
}

/// Every top-level value in `buf`.
pub(crate) fn parse_all(buf: &[u8]) -> Result<Vec<Tlv>, String> {
    let mut at = 0;
    let mut out = Vec::new();
    while at < buf.len() {
        out.push(parse_at(buf, &mut at, 0)?);
    }
    Ok(out)
}

/// A two's complement integer of at most 64 bits [X.690 8.3].
pub(crate) fn decode_integer(content: &[u8]) -> Option<i64> {
    if content.is_empty() || content.len() > 8 {
        return None;
    }
    let mut v: i64 = if content[0] & 0x80 != 0 { -1 } else { 0 };
    for &b in content {
        v = (v << 8) | i64::from(b);
    }
    Some(v)
}

/// A REAL [X.690 8.5]: binary form in any base the standard allows, the
/// special values, and zero. The decimal form is not Ember (the specification
/// requires binary) and is refused.
pub(crate) fn decode_real(content: &[u8]) -> Option<f64> {
    let Some((&first, rest)) = content.split_first() else {
        return Some(0.0);
    };
    if first & 0x80 == 0 {
        return match (first, rest.is_empty()) {
            (0x40, true) => Some(f64::INFINITY),
            (0x41, true) => Some(f64::NEG_INFINITY),
            (0x42, true) => Some(f64::NAN),
            (0x43, true) => Some(-0.0),
            _ => None,
        };
    }
    let negative = first & 0x40 != 0;
    let base_bits: i32 = match (first >> 4) & 0x03 {
        0 => 1,
        1 => 3,
        2 => 4,
        _ => return None,
    };
    let scale = i32::from((first >> 2) & 0x03);
    let (exp_len, rest) = match first & 0x03 {
        3 => {
            let (&n, rest) = rest.split_first()?;
            (usize::from(n), rest)
        }
        n => (usize::from(n) + 1, rest),
    };
    if exp_len == 0 || exp_len > 4 || rest.len() < exp_len {
        return None;
    }
    let (exp_bytes, mantissa) = rest.split_at(exp_len);
    let exponent = decode_integer(exp_bytes)?;
    // Leading zero octets carry nothing; more than 64 significant bits is
    // beyond double precision anyway.
    let mantissa: Vec<u8> = mantissa.iter().copied().skip_while(|&b| b == 0).collect();
    if mantissa.len() > 8 {
        return None;
    }
    let n = mantissa
        .iter()
        .fold(0u64, |acc, &b| (acc << 8) | u64::from(b));
    let power = exponent.checked_mul(i64::from(base_bits))? + i64::from(scale);
    let power = i32::try_from(power.clamp(-4000, 4000)).ok()?;
    // In two steps so an intermediate power of two does not underflow or
    // overflow when the result itself is representable.
    let half = power / 2;
    let v = (n as f64) * 2f64.powi(half) * 2f64.powi(power - half);
    Some(if negative { -v } else { v })
}

/// A RELATIVE-OID [X.690 8.20]: base-128 sub-identifiers.
pub(crate) fn decode_roid(content: &[u8]) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut open = false;
    for &b in content {
        if acc > (u32::MAX >> 7) {
            return None;
        }
        acc = (acc << 7) | u32::from(b & 0x7F);
        open = b & 0x80 != 0;
        if !open {
            out.push(acc);
            acc = 0;
        }
    }
    (!open).then_some(out)
}

// --- Encoding ----------------------------------------------------------------

fn put_tag(out: &mut Vec<u8>, tag: Tag, constructed: bool) {
    let flags = tag.class | if constructed { 0x20 } else { 0 };
    if tag.number < 0x1F {
        out.push(flags | tag.number as u8);
        return;
    }
    out.push(flags | 0x1F);
    let mut groups = Vec::new();
    let mut n = tag.number;
    loop {
        groups.push((n & 0x7F) as u8);
        n >>= 7;
        if n == 0 {
            break;
        }
    }
    for (i, g) in groups.iter().enumerate().rev() {
        out.push(if i == 0 { *g } else { g | 0x80 });
    }
}

fn put_length(out: &mut Vec<u8>, len: usize) {
    if len < 0x80 {
        out.push(len as u8);
        return;
    }
    let bytes = (len as u64).to_be_bytes();
    let skip = bytes.iter().take_while(|&&b| b == 0).count();
    out.push(0x80 | (8 - skip) as u8);
    out.extend_from_slice(&bytes[skip..]);
}

/// A primitive value with its tag and definite length.
pub(crate) fn primitive(tag: Tag, content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(content.len() + 6);
    put_tag(&mut out, tag, false);
    put_length(&mut out, content.len());
    out.extend_from_slice(content);
    out
}

/// A constructed value holding already-encoded children.
pub(crate) fn constructed(tag: Tag, children: &[Vec<u8>]) -> Vec<u8> {
    let len: usize = children.iter().map(Vec::len).sum();
    let mut out = Vec::with_capacity(len + 6);
    put_tag(&mut out, tag, true);
    put_length(&mut out, len);
    for c in children {
        out.extend_from_slice(c);
    }
    out
}

/// An explicitly tagged context field `[n]` around one encoded value.
pub(crate) fn ctx(n: u32, inner: Vec<u8>) -> Vec<u8> {
    constructed(Tag::ctx(n), &[inner])
}

pub(crate) fn sequence(children: &[Vec<u8>]) -> Vec<u8> {
    constructed(Tag::universal(SEQUENCE), children)
}

pub(crate) fn set(children: &[Vec<u8>]) -> Vec<u8> {
    constructed(Tag::universal(SET), children)
}

/// The shortest two's complement content: the first nine bits are never all
/// equal [X.690 8.3.2].
pub(crate) fn integer_content(v: i64) -> Vec<u8> {
    let bytes = v.to_be_bytes();
    let mut start = 0;
    while start < 7 {
        let (a, b) = (bytes[start], bytes[start + 1]);
        if (a == 0x00 && b & 0x80 == 0) || (a == 0xFF && b & 0x80 != 0) {
            start += 1;
        } else {
            break;
        }
    }
    bytes[start..].to_vec()
}

pub(crate) fn integer(v: i64) -> Vec<u8> {
    primitive(Tag::universal(INTEGER), &integer_content(v))
}

pub(crate) fn boolean(v: bool) -> Vec<u8> {
    // "It is recommended for the encoder to use FF16 to encode true."
    primitive(Tag::universal(BOOLEAN), &[if v { 0xFF } else { 0x00 }])
}

pub(crate) fn utf8(s: &str) -> Vec<u8> {
    primitive(Tag::universal(UTF8_STRING), s.as_bytes())
}

pub(crate) fn octets(b: &[u8]) -> Vec<u8> {
    primitive(Tag::universal(OCTET_STRING), b)
}

pub(crate) fn null() -> Vec<u8> {
    primitive(Tag::universal(NULL), &[])
}

pub(crate) fn roid_content(path: &[u32]) -> Vec<u8> {
    let mut out = Vec::new();
    for &n in path {
        let mut groups = Vec::new();
        let mut v = n;
        loop {
            groups.push((v & 0x7F) as u8);
            v >>= 7;
            if v == 0 {
                break;
            }
        }
        for (i, g) in groups.iter().enumerate().rev() {
            out.push(if i == 0 { *g } else { g | 0x80 });
        }
    }
    out
}

pub(crate) fn roid(path: &[u32]) -> Vec<u8> {
    primitive(Tag::universal(RELATIVE_OID), &roid_content(path))
}

/// A REAL in binary form, base 2, scale 0, with an odd mantissa [X.690
/// 8.5.7]; zero is empty and the special values use their single octet
/// [X.690 8.5.9].
pub(crate) fn real_content(v: f64) -> Vec<u8> {
    if v.is_nan() {
        return vec![0x42];
    }
    if v.is_infinite() {
        return vec![if v > 0.0 { 0x40 } else { 0x41 }];
    }
    if v == 0.0 {
        return if v.is_sign_negative() {
            vec![0x43]
        } else {
            Vec::new()
        };
    }
    let bits = v.to_bits();
    let negative = bits >> 63 != 0;
    let biased = ((bits >> 52) & 0x7FF) as i64;
    let fraction = bits & ((1u64 << 52) - 1);
    let (mut mantissa, mut exponent) = if biased == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1u64 << 52), biased - 1075)
    };
    while mantissa & 1 == 0 {
        mantissa >>= 1;
        exponent += 1;
    }
    let exp = integer_content(exponent);
    let mut out = vec![0x80 | if negative { 0x40 } else { 0 } | (exp.len() as u8 - 1)];
    out.extend_from_slice(&exp);
    let m = mantissa.to_be_bytes();
    let skip = m.iter().take_while(|&&b| b == 0).count();
    out.extend_from_slice(&m[skip..]);
    out
}

pub(crate) fn real(v: f64) -> Vec<u8> {
    primitive(Tag::universal(REAL), &real_content(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|b| u8::from_str_radix(b, 16).unwrap())
            .collect()
    }

    // EmBER, "Integer [X.690 8.3]": the table the specification gives "to
    // verify the correctness of the implementation" (length octets, then
    // value octets).
    #[test]
    fn integers_match_the_specification_table() {
        let table: &[(i64, &str)] = &[
            (1, "01 01"),
            (-1, "01 FF"),
            (255, "02 00 FF"),
            (127, "01 7F"),
            (128, "02 00 80"),
            (-128, "01 80"),
            ((1 << 16) - 1, "03 00 FF FF"),
            (1 << 15, "03 00 80 00"),
            (-(1 << 15), "02 80 00"),
        ];
        for &(v, expected) in table {
            let mut want = vec![0x02];
            want.extend(hex(expected));
            assert_eq!(integer(v), want, "{v}");
            let tlv = &parse_all(&want).unwrap()[0];
            assert_eq!(tlv.as_i64(), Some(v));
        }
        for v in [0, i64::MIN, i64::MAX, -129, 1 << 40] {
            assert_eq!(parse_all(&integer(v)).unwrap()[0].as_i64(), Some(v));
        }
    }

    // "The EmBER library": an Application-1 tag around the integer 1333 is
    // printed as 41 04 02 02 05 35. X.690 makes an explicit tag constructed
    // (0x61), which is what this encoder writes; the example as printed
    // decodes too.
    #[test]
    fn the_library_example_decodes() {
        for wire in [hex("41 04 02 02 05 35"), hex("61 04 02 02 05 35")] {
            let tlv = &parse_all(&wire).unwrap()[0];
            assert_eq!(tlv.tag, Tag::app(1));
            assert_eq!(tlv.inner().and_then(Tlv::as_i64), Some(1333));
        }
        assert_eq!(
            constructed(Tag::app(1), &[integer(1333)]),
            hex("61 04 02 02 05 35")
        );
    }

    #[test]
    fn reals_round_trip() {
        for v in [
            1.0,
            -1.0,
            0.5,
            -64.0,
            15.0,
            0.1,
            -128.25,
            1e300,
            -1e-300,
            f64::MIN_POSITIVE,
            5e-324,
            f64::MAX,
            0.0,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            let decoded = parse_all(&real(v)).unwrap()[0].as_f64().unwrap();
            assert_eq!(decoded.to_bits(), v.to_bits(), "{v}");
        }
        assert!(decode_real(&real_content(f64::NAN)).unwrap().is_nan());
        let neg_zero = decode_real(&real_content(-0.0)).unwrap();
        assert!(neg_zero == 0.0 && neg_zero.is_sign_negative());
        // 1.0 is mantissa 1, exponent 0: 80 00 01.
        assert_eq!(real_content(1.0), vec![0x80, 0x00, 0x01]);
        // Base 16 and a scale factor decode too: 0xA0 is base 16, so
        // 3 * 16^1 = 48.
        assert_eq!(decode_real(&[0xA0, 0x01, 0x03]), Some(48.0));
    }

    #[test]
    fn strings_booleans_octets_null_and_paths() {
        let t = &parse_all(&utf8("Grüße")).unwrap()[0];
        assert_eq!(t.as_string().as_deref(), Some("Grüße"));
        assert_eq!(boolean(true), vec![0x01, 0x01, 0xFF]);
        assert_eq!(
            parse_all(&boolean(false)).unwrap()[0].as_bool(),
            Some(false)
        );
        assert_eq!(
            parse_all(&[0x01, 0x01, 0x05]).unwrap()[0].as_bool(),
            Some(true)
        );
        assert_eq!(octets(&[1, 2]), vec![0x04, 0x02, 1, 2]);
        assert_eq!(null(), vec![0x05, 0x00]);
        let path = [1, 2, 300, 0, 16384];
        assert_eq!(roid_content(&[1, 2, 300]), vec![1, 2, 0x82, 0x2C]);
        assert_eq!(
            parse_all(&roid(&path)).unwrap()[0].as_roid(),
            Some(path.to_vec())
        );
        assert_eq!(decode_roid(&[]), Some(vec![]));
        assert_eq!(decode_roid(&[0x82]), None);
    }

    #[test]
    fn lengths_tags_and_indefinite_containers() {
        // Long definite form, and the long form for a short length.
        let long = utf8(&"x".repeat(300));
        assert_eq!(&long[..4], &[0x0C, 0x82, 0x01, 0x2C]);
        assert_eq!(
            parse_all(&[0x0C, 0x84, 0, 0, 0, 1, b'a']).unwrap()[0].content,
            b"a"
        );
        // An indefinite-length set with an explicit context field inside.
        let msg = [0x31, 0x80, 0xA0, 0x80, 0x02, 0x01, 0x07, 0, 0, 0, 0];
        let tlv = &parse_all(&msg).unwrap()[0];
        assert_eq!(tlv.field(0).and_then(Tlv::as_i64), Some(7));
        // High tag numbers.
        let t = primitive(Tag::app(40), &[]);
        assert_eq!(t, vec![0x5F, 40, 0]);
        assert_eq!(parse_all(&t).unwrap()[0].tag, Tag::app(40));
        let t = primitive(Tag::ctx(200), &[]);
        assert_eq!(parse_all(&t).unwrap()[0].tag, Tag::ctx(200));
        // Truncation is an error, not a panic.
        assert!(parse_all(&[0x30, 0x05, 0x02, 0x01]).is_err());
        assert!(parse_all(&[0x30, 0x80, 0x02, 0x01, 0x01]).is_err());
    }
}
