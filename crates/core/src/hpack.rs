//! HPACK, HTTP/2's header compression (RFC 7541), for the core's HTTP/2
//! client (`crate::http2`).
//!
//! The decoder is complete: the static table (Appendix A), the dynamic table
//! with its size updates and evictions (§4), integers with prefixes
//! (§5.1), and string literals plain or Huffman-coded (§5.2, Appendix B),
//! as a server may send any of them. The encoder sends each field as a
//! literal without indexing (§6.2.2), its name from the static table when it
//! is there, never Huffman-coded: a valid choice any decoder takes, which
//! keeps nothing of this side's headers in the peer's table.

/// The static table (RFC 7541 Appendix A), index 1 first.
const STATIC: [(&str, &str); 61] = [
    (":authority", ""),
    (":method", "GET"),
    (":method", "POST"),
    (":path", "/"),
    (":path", "/index.html"),
    (":scheme", "http"),
    (":scheme", "https"),
    (":status", "200"),
    (":status", "204"),
    (":status", "206"),
    (":status", "304"),
    (":status", "400"),
    (":status", "404"),
    (":status", "500"),
    ("accept-charset", ""),
    ("accept-encoding", "gzip, deflate"),
    ("accept-language", ""),
    ("accept-ranges", ""),
    ("accept", ""),
    ("access-control-allow-origin", ""),
    ("age", ""),
    ("allow", ""),
    ("authorization", ""),
    ("cache-control", ""),
    ("content-disposition", ""),
    ("content-encoding", ""),
    ("content-language", ""),
    ("content-length", ""),
    ("content-location", ""),
    ("content-range", ""),
    ("content-type", ""),
    ("cookie", ""),
    ("date", ""),
    ("etag", ""),
    ("expect", ""),
    ("expires", ""),
    ("from", ""),
    ("host", ""),
    ("if-match", ""),
    ("if-modified-since", ""),
    ("if-none-match", ""),
    ("if-range", ""),
    ("if-unmodified-since", ""),
    ("last-modified", ""),
    ("link", ""),
    ("location", ""),
    ("max-forwards", ""),
    ("proxy-authenticate", ""),
    ("proxy-authorization", ""),
    ("range", ""),
    ("referer", ""),
    ("refresh", ""),
    ("retry-after", ""),
    ("server", ""),
    ("set-cookie", ""),
    ("strict-transport-security", ""),
    ("transfer-encoding", ""),
    ("user-agent", ""),
    ("vary", ""),
    ("via", ""),
    ("www-authenticate", ""),
];

/// The Huffman code (RFC 7541 Appendix B): code and length in bits for each
/// symbol 0 to 255, then EOS (256).
const HUFFMAN: [(u32, u8); 257] = [
    (0x1ff8, 13),
    (0x7fffd8, 23),
    (0xfffffe2, 28),
    (0xfffffe3, 28),
    (0xfffffe4, 28),
    (0xfffffe5, 28),
    (0xfffffe6, 28),
    (0xfffffe7, 28),
    (0xfffffe8, 28),
    (0xffffea, 24),
    (0x3ffffffc, 30),
    (0xfffffe9, 28),
    (0xfffffea, 28),
    (0x3ffffffd, 30),
    (0xfffffeb, 28),
    (0xfffffec, 28),
    (0xfffffed, 28),
    (0xfffffee, 28),
    (0xfffffef, 28),
    (0xffffff0, 28),
    (0xffffff1, 28),
    (0xffffff2, 28),
    (0x3ffffffe, 30),
    (0xffffff3, 28),
    (0xffffff4, 28),
    (0xffffff5, 28),
    (0xffffff6, 28),
    (0xffffff7, 28),
    (0xffffff8, 28),
    (0xffffff9, 28),
    (0xffffffa, 28),
    (0xffffffb, 28),
    (0x14, 6),
    (0x3f8, 10),
    (0x3f9, 10),
    (0xffa, 12),
    (0x1ff9, 13),
    (0x15, 6),
    (0xf8, 8),
    (0x7fa, 11),
    (0x3fa, 10),
    (0x3fb, 10),
    (0xf9, 8),
    (0x7fb, 11),
    (0xfa, 8),
    (0x16, 6),
    (0x17, 6),
    (0x18, 6),
    (0x0, 5),
    (0x1, 5),
    (0x2, 5),
    (0x19, 6),
    (0x1a, 6),
    (0x1b, 6),
    (0x1c, 6),
    (0x1d, 6),
    (0x1e, 6),
    (0x1f, 6),
    (0x5c, 7),
    (0xfb, 8),
    (0x7ffc, 15),
    (0x20, 6),
    (0xffb, 12),
    (0x3fc, 10),
    (0x1ffa, 13),
    (0x21, 6),
    (0x5d, 7),
    (0x5e, 7),
    (0x5f, 7),
    (0x60, 7),
    (0x61, 7),
    (0x62, 7),
    (0x63, 7),
    (0x64, 7),
    (0x65, 7),
    (0x66, 7),
    (0x67, 7),
    (0x68, 7),
    (0x69, 7),
    (0x6a, 7),
    (0x6b, 7),
    (0x6c, 7),
    (0x6d, 7),
    (0x6e, 7),
    (0x6f, 7),
    (0x70, 7),
    (0x71, 7),
    (0x72, 7),
    (0xfc, 8),
    (0x73, 7),
    (0xfd, 8),
    (0x1ffb, 13),
    (0x7fff0, 19),
    (0x1ffc, 13),
    (0x3ffc, 14),
    (0x22, 6),
    (0x7ffd, 15),
    (0x3, 5),
    (0x23, 6),
    (0x4, 5),
    (0x24, 6),
    (0x5, 5),
    (0x25, 6),
    (0x26, 6),
    (0x27, 6),
    (0x6, 5),
    (0x74, 7),
    (0x75, 7),
    (0x28, 6),
    (0x29, 6),
    (0x2a, 6),
    (0x7, 5),
    (0x2b, 6),
    (0x76, 7),
    (0x2c, 6),
    (0x8, 5),
    (0x9, 5),
    (0x2d, 6),
    (0x77, 7),
    (0x78, 7),
    (0x79, 7),
    (0x7a, 7),
    (0x7b, 7),
    (0x7ffe, 15),
    (0x7fc, 11),
    (0x3ffd, 14),
    (0x1ffd, 13),
    (0xffffffc, 28),
    (0xfffe6, 20),
    (0x3fffd2, 22),
    (0xfffe7, 20),
    (0xfffe8, 20),
    (0x3fffd3, 22),
    (0x3fffd4, 22),
    (0x3fffd5, 22),
    (0x7fffd9, 23),
    (0x3fffd6, 22),
    (0x7fffda, 23),
    (0x7fffdb, 23),
    (0x7fffdc, 23),
    (0x7fffdd, 23),
    (0x7fffde, 23),
    (0xffffeb, 24),
    (0x7fffdf, 23),
    (0xffffec, 24),
    (0xffffed, 24),
    (0x3fffd7, 22),
    (0x7fffe0, 23),
    (0xffffee, 24),
    (0x7fffe1, 23),
    (0x7fffe2, 23),
    (0x7fffe3, 23),
    (0x7fffe4, 23),
    (0x1fffdc, 21),
    (0x3fffd8, 22),
    (0x7fffe5, 23),
    (0x3fffd9, 22),
    (0x7fffe6, 23),
    (0x7fffe7, 23),
    (0xffffef, 24),
    (0x3fffda, 22),
    (0x1fffdd, 21),
    (0xfffe9, 20),
    (0x3fffdb, 22),
    (0x3fffdc, 22),
    (0x7fffe8, 23),
    (0x7fffe9, 23),
    (0x1fffde, 21),
    (0x7fffea, 23),
    (0x3fffdd, 22),
    (0x3fffde, 22),
    (0xfffff0, 24),
    (0x1fffdf, 21),
    (0x3fffdf, 22),
    (0x7fffeb, 23),
    (0x7fffec, 23),
    (0x1fffe0, 21),
    (0x1fffe1, 21),
    (0x3fffe0, 22),
    (0x1fffe2, 21),
    (0x7fffed, 23),
    (0x3fffe1, 22),
    (0x7fffee, 23),
    (0x7fffef, 23),
    (0xfffea, 20),
    (0x3fffe2, 22),
    (0x3fffe3, 22),
    (0x3fffe4, 22),
    (0x7ffff0, 23),
    (0x3fffe5, 22),
    (0x3fffe6, 22),
    (0x7ffff1, 23),
    (0x3ffffe0, 26),
    (0x3ffffe1, 26),
    (0xfffeb, 20),
    (0x7fff1, 19),
    (0x3fffe7, 22),
    (0x7ffff2, 23),
    (0x3fffe8, 22),
    (0x1ffffec, 25),
    (0x3ffffe2, 26),
    (0x3ffffe3, 26),
    (0x3ffffe4, 26),
    (0x7ffffde, 27),
    (0x7ffffdf, 27),
    (0x3ffffe5, 26),
    (0xfffff1, 24),
    (0x1ffffed, 25),
    (0x7fff2, 19),
    (0x1fffe3, 21),
    (0x3ffffe6, 26),
    (0x7ffffe0, 27),
    (0x7ffffe1, 27),
    (0x3ffffe7, 26),
    (0x7ffffe2, 27),
    (0xfffff2, 24),
    (0x1fffe4, 21),
    (0x1fffe5, 21),
    (0x3ffffe8, 26),
    (0x3ffffe9, 26),
    (0xffffffd, 28),
    (0x7ffffe3, 27),
    (0x7ffffe4, 27),
    (0x7ffffe5, 27),
    (0xfffec, 20),
    (0xfffff3, 24),
    (0xfffed, 20),
    (0x1fffe6, 21),
    (0x3fffe9, 22),
    (0x1fffe7, 21),
    (0x1fffe8, 21),
    (0x7ffff3, 23),
    (0x3fffea, 22),
    (0x3fffeb, 22),
    (0x1ffffee, 25),
    (0x1ffffef, 25),
    (0xfffff4, 24),
    (0xfffff5, 24),
    (0x3ffffea, 26),
    (0x7ffff4, 23),
    (0x3ffffeb, 26),
    (0x7ffffe6, 27),
    (0x3ffffec, 26),
    (0x3ffffed, 26),
    (0x7ffffe7, 27),
    (0x7ffffe8, 27),
    (0x7ffffe9, 27),
    (0x7ffffea, 27),
    (0x7ffffeb, 27),
    (0xffffffe, 28),
    (0x7ffffec, 27),
    (0x7ffffed, 27),
    (0x7ffffee, 27),
    (0x7ffffef, 27),
    (0x7fffff0, 27),
    (0x3ffffee, 26),
    (0x3fffffff, 30),
];

/// Encode an integer with an `n`-bit prefix (§5.1); `first` holds the bits
/// above the prefix in the first octet.
pub(crate) fn encode_int(value: usize, n: u8, first: u8, out: &mut Vec<u8>) {
    let max = (1usize << n) - 1;
    if value < max {
        out.push(first | value as u8);
        return;
    }
    out.push(first | max as u8);
    let mut rest = value - max;
    while rest >= 128 {
        out.push((rest % 128) as u8 | 0x80);
        rest /= 128;
    }
    out.push(rest as u8);
}

/// Decode an integer with an `n`-bit prefix at the start of `data`: the
/// value and the octets read.
pub(crate) fn decode_int(data: &[u8], n: u8) -> Result<(usize, usize), String> {
    let first = *data.first().ok_or("hpack: truncated integer")?;
    let max = (1usize << n) - 1;
    let mut value = usize::from(first) & max;
    if value < max {
        return Ok((value, 1));
    }
    let mut shift = 0u32;
    for (i, b) in data[1..].iter().enumerate() {
        if shift > 28 {
            return Err("hpack: integer too large".into());
        }
        value += usize::from(b & 0x7f) << shift;
        shift += 7;
        if b & 0x80 == 0 {
            return Ok((value, i + 2));
        }
    }
    Err("hpack: truncated integer".into())
}

/// Encode a string literal, not Huffman-coded (§5.2).
fn encode_string(s: &str, out: &mut Vec<u8>) {
    encode_int(s.len(), 7, 0, out);
    out.extend_from_slice(s.as_bytes());
}

/// Decode a string literal at the start of `data`: its bytes and the octets read.
fn decode_string(data: &[u8]) -> Result<(Vec<u8>, usize), String> {
    let huffman = data.first().ok_or("hpack: truncated string")? & 0x80 != 0;
    let (len, n) = decode_int(data, 7)?;
    let end = n
        .checked_add(len)
        .filter(|e| *e <= data.len())
        .ok_or("hpack: truncated string")?;
    let body = &data[n..end];
    let bytes = if huffman {
        huffman_decode(body)?
    } else {
        body.to_vec()
    };
    Ok((bytes, end))
}

/// The Huffman code's symbols by (length, code), built once.
fn huffman_table() -> &'static std::collections::HashMap<(u8, u32), u16> {
    static TABLE: std::sync::OnceLock<std::collections::HashMap<(u8, u32), u16>> =
        std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        HUFFMAN
            .iter()
            .enumerate()
            .map(|(sym, (code, len))| ((*len, *code), sym as u16))
            .collect()
    })
}

/// Decode a Huffman-coded string (§5.2): padding of up to 7 bits, all ones
/// (the start of EOS), ends it; EOS itself, or other padding, is an error.
pub(crate) fn huffman_decode(data: &[u8]) -> Result<Vec<u8>, String> {
    let table = huffman_table();
    let mut out = Vec::with_capacity(data.len() * 8 / 5);
    let mut code = 0u32;
    let mut len = 0u8;
    for byte in data {
        for bit in (0..8).rev() {
            code = (code << 1) | u32::from((byte >> bit) & 1);
            len += 1;
            if len >= 5 {
                if let Some(sym) = table.get(&(len, code)) {
                    if *sym == 256 {
                        return Err("hpack: EOS in a Huffman string".into());
                    }
                    out.push(*sym as u8);
                    code = 0;
                    len = 0;
                    continue;
                }
            }
            if len > 30 {
                return Err("hpack: invalid Huffman code".into());
            }
        }
    }
    if len > 7 || code != (1u32 << len) - 1 {
        return Err("hpack: invalid Huffman padding".into());
    }
    Ok(out)
}

/// Huffman-code a string (Appendix B): for tests standing in for a server.
#[cfg(test)]
pub(crate) fn huffman_encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut acc = 0u64;
    let mut bits = 0u32;
    for b in data {
        let (code, len) = HUFFMAN[usize::from(*b)];
        acc = (acc << len) | u64::from(code);
        bits += u32::from(len);
        while bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
        acc &= (1u64 << bits) - 1;
    }
    if bits > 0 {
        out.push(((acc << (8 - bits)) as u8) | (0xff >> bits));
    }
    out
}

/// One header field: name and value, as octets.
pub(crate) type HeaderField = (Vec<u8>, Vec<u8>);

/// The decoding side of one connection (§2.3, §4).
#[derive(Debug)]
pub(crate) struct Decoder {
    /// Newest first.
    dynamic: std::collections::VecDeque<HeaderField>,
    size: usize,
    /// The table size the peer may use: this side's SETTINGS_HEADER_TABLE_SIZE.
    max_size: usize,
    /// The size the peer last set with an update, at most `max_size`.
    current_max: usize,
}

impl Decoder {
    pub(crate) fn new(max_size: usize) -> Decoder {
        Decoder {
            dynamic: std::collections::VecDeque::new(),
            size: 0,
            max_size,
            current_max: max_size,
        }
    }

    fn get(&self, index: usize) -> Result<HeaderField, String> {
        match index {
            0 => Err("hpack: index 0".into()),
            1..=61 => {
                let (n, v) = STATIC[index - 1];
                Ok((n.as_bytes().to_vec(), v.as_bytes().to_vec()))
            }
            _ => self
                .dynamic
                .get(index - 62)
                .cloned()
                .ok_or_else(|| format!("hpack: index {index} is not in the table")),
        }
    }

    fn evict(&mut self) {
        while self.size > self.current_max {
            let Some((n, v)) = self.dynamic.pop_back() else {
                break;
            };
            self.size -= n.len() + v.len() + 32;
        }
    }

    fn insert(&mut self, field: HeaderField) {
        self.size += field.0.len() + field.1.len() + 32;
        self.dynamic.push_front(field);
        // An entry larger than the table empties it (§4.4).
        self.evict();
    }

    /// Decode one header block (the fragments of a HEADERS frame and its
    /// CONTINUATION frames, joined).
    pub(crate) fn decode(&mut self, block: &[u8]) -> Result<Vec<HeaderField>, String> {
        let mut out = Vec::new();
        let mut pos = 0;
        while pos < block.len() {
            let b = block[pos];
            if b & 0x80 != 0 {
                // Indexed (§6.1).
                let (index, n) = decode_int(&block[pos..], 7)?;
                pos += n;
                out.push(self.get(index)?);
            } else if b & 0xe0 == 0x20 {
                // Dynamic table size update (§6.3).
                let (size, n) = decode_int(&block[pos..], 5)?;
                pos += n;
                if size > self.max_size {
                    return Err(format!("hpack: table size {size} above {}", self.max_size));
                }
                self.current_max = size;
                self.evict();
            } else {
                // Literal with incremental indexing (§6.2.1, 6-bit prefix),
                // without indexing (§6.2.2) or never indexed (§6.2.3), both
                // with a 4-bit prefix.
                let indexing = b & 0xc0 == 0x40;
                let prefix = if indexing { 6 } else { 4 };
                let (index, n) = decode_int(&block[pos..], prefix)?;
                pos += n;
                let name = if index == 0 {
                    let (name, n) = decode_string(&block[pos..])?;
                    pos += n;
                    name
                } else {
                    self.get(index)?.0
                };
                let (value, n) = decode_string(&block[pos..])?;
                pos += n;
                if indexing {
                    self.insert((name.clone(), value.clone()));
                }
                out.push((name, value));
            }
        }
        Ok(out)
    }
}

/// Encode a header block: every field a literal without indexing, its name
/// indexed from the static table when it is there (§6.2.2).
pub(crate) fn encode(fields: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, value) in fields {
        match STATIC.iter().position(|(n, _)| n == name) {
            Some(i) => encode_int(i + 1, 4, 0x00, &mut out),
            None => {
                out.push(0x00);
                encode_string(name, &mut out);
            }
        }
        encode_string(value, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    fn text(fields: Vec<HeaderField>) -> Vec<String> {
        fields
            .into_iter()
            .map(|(n, v)| {
                format!(
                    "{}: {}",
                    String::from_utf8(n).unwrap(),
                    String::from_utf8(v).unwrap()
                )
            })
            .collect()
    }

    #[test]
    fn integers_follow_the_rfc_examples() {
        // RFC 7541 C.1.1 to C.1.3.
        let mut out = Vec::new();
        encode_int(10, 5, 0, &mut out);
        assert_eq!(out, [0x0a]);
        out.clear();
        encode_int(1337, 5, 0, &mut out);
        assert_eq!(out, [0x1f, 0x9a, 0x0a]);
        assert_eq!(decode_int(&out, 5).unwrap(), (1337, 3));
        out.clear();
        encode_int(42, 8, 0, &mut out);
        assert_eq!(out, [0x2a]);
        assert!(decode_int(&[0x1f, 0x9a], 5).is_err(), "truncated");
    }

    #[test]
    fn requests_with_huffman_coding_decode_as_c4() {
        let mut d = Decoder::new(4096);
        assert_eq!(
            text(
                d.decode(&hex("828684418cf1e3c2e5f23a6ba0ab90f4ff"))
                    .unwrap()
            ),
            [
                ":method: GET",
                ":scheme: http",
                ":path: /",
                ":authority: www.example.com"
            ]
        );
        assert_eq!(
            text(d.decode(&hex("828684be5886a8eb10649cbf")).unwrap()),
            [
                ":method: GET",
                ":scheme: http",
                ":path: /",
                ":authority: www.example.com",
                "cache-control: no-cache"
            ]
        );
        assert_eq!(
            text(
                d.decode(&hex("828785bf408825a849e95ba97d7f8925a849e95bb8e8b4bf"))
                    .unwrap()
            ),
            [
                ":method: GET",
                ":scheme: https",
                ":path: /index.html",
                ":authority: www.example.com",
                "custom-key: custom-value"
            ]
        );
    }

    #[test]
    fn responses_with_huffman_coding_and_evictions_decode_as_c6() {
        // C.6 uses a 256-octet table, so entries are evicted.
        let mut d = Decoder::new(256);
        assert_eq!(
            text(d.decode(&hex("488264025885aec3771a4b6196d07abe941054d444a8200595040b8166e082a62d1bff6e919d29ad171863c78f0b97c8e9ae82ae43d3")).unwrap()),
            [":status: 302", "cache-control: private", "date: Mon, 21 Oct 2013 20:13:21 GMT", "location: https://www.example.com"]
        );
        assert_eq!(
            text(d.decode(&hex("4883640effc1c0bf")).unwrap()),
            [
                ":status: 307",
                "cache-control: private",
                "date: Mon, 21 Oct 2013 20:13:21 GMT",
                "location: https://www.example.com"
            ]
        );
        assert_eq!(
            text(d.decode(&hex("88c16196d07abe941054d444a8200595040b8166e084a62d1bffc05a839bd9ab77ad94e7821dd7f2e6c7b335dfdfcd5b3960d5af27087f3672c1ab270fb5291f9587316065c003ed4ee5b1063d5007")).unwrap()),
            [":status: 200", "cache-control: private", "date: Mon, 21 Oct 2013 20:13:22 GMT", "location: https://www.example.com",
             "content-encoding: gzip", "set-cookie: foo=ASDJKHQKBZXOQWEOPIUAXQWEOIU; max-age=3600; version=1"]
        );
        assert_eq!(d.size, 215, "the table holds what C.6.3 lists");
    }

    #[test]
    fn huffman_strings_round_trip_and_bad_padding_is_refused() {
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(huffman_decode(&huffman_encode(&all)).unwrap(), all);
        assert_eq!(
            huffman_encode(b"www.example.com"),
            hex("f1e3c2e5f23a6ba0ab90f4ff")
        );
        // Padding of eight bits, or of zeros, is an error.
        assert!(huffman_decode(&[0x1f, 0xff]).is_err());
        assert!(huffman_decode(&[0x00]).is_err());
    }

    #[test]
    fn this_side_encodes_literals_the_decoder_reads() {
        let block = encode(&[
            (":method", "POST"),
            ("content-type", "application/grpc"),
            ("x-session", "abc"),
        ]);
        // :method is static index 2, a literal without indexing with a 4-bit
        // index: 02, then the value "POST" (04 50 4f 53 54).
        assert_eq!(&block[..6], &[0x02, 0x04, b'P', b'O', b'S', b'T']);
        let mut d = Decoder::new(4096);
        assert_eq!(
            text(d.decode(&block).unwrap()),
            [
                ":method: POST",
                "content-type: application/grpc",
                "x-session: abc"
            ]
        );
        assert_eq!(d.size, 0, "nothing is added to the peer's table");
    }
}
