//! Wire pieces of the Meros Camera Control protocol (version 1) that are not
//! JSON requests: the head and body of a streamed HTTP/1.1 response (the
//! event stream and the preview, read on a raw TCP connection because they
//! never end), NDJSON lines, preview frames, and fleet links.

use crate::{json, value::Value};

/// A response head larger than this is not a camera's.
const MAX_HEAD: usize = 16 * 1024;
/// An NDJSON line larger than this is not a status document.
pub(crate) const MAX_LINE: usize = 4 << 20;
/// Preview frames over this are corrupt (protocol §9).
pub(crate) const MAX_FRAME: usize = 8 << 20;
/// Each preview frame's header: u32 length, i64 pts, u8 flags.
pub(crate) const FRAME_HEADER: usize = 13;
/// What a fleet link may unpack to: links come from anywhere.
const MAX_LINK_JSON: usize = 1 << 20;

/// The request line and headers of a streamed GET.
pub(crate) fn stream_request(path: &str, host: &str, key: &str, accept: &str) -> Vec<u8> {
    format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {key}\r\n\
         Accept: {accept}\r\nConnection: close\r\n\r\n"
    )
    .into_bytes()
}

/// What a streamed response yields.
#[derive(Debug, PartialEq)]
pub(crate) enum Piece {
    /// The status line and headers have arrived.
    Head { status: u16 },
    /// Body bytes, with any chunked transfer coding removed.
    Body(Vec<u8>),
}

#[derive(Debug, PartialEq)]
enum Chunk {
    Size(Vec<u8>),
    Data(usize),
    DataEnd,
    Trailer(Vec<u8>),
    Done,
}

/// A streamed HTTP/1.1 response read incrementally. The camera sends its
/// streams with no length, ending them by closing; chunked coding is undone
/// too, in case a proxy adds it.
#[derive(Debug, Default)]
pub(crate) struct ResponseReader {
    head: Vec<u8>,
    status: Option<u16>,
    chunk: Option<Chunk>,
}

impl ResponseReader {
    pub(crate) fn feed(&mut self, data: &[u8]) -> Result<Vec<Piece>, String> {
        let mut out = Vec::new();
        if self.status.is_none() {
            self.head.extend_from_slice(data);
            let Some(end) = find(&self.head, b"\r\n\r\n") else {
                if self.head.len() > MAX_HEAD {
                    return Err("the response head is too long".into());
                }
                return Ok(out);
            };
            let head = String::from_utf8_lossy(&self.head[..end]).into_owned();
            let mut lines = head.split("\r\n");
            let status = lines
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|s| s.parse::<u16>().ok())
                .ok_or("not an HTTP response")?;
            let chunked = lines.any(|l| {
                l.split_once(':').is_some_and(|(k, v)| {
                    k.trim().eq_ignore_ascii_case("transfer-encoding")
                        && v.to_ascii_lowercase().contains("chunked")
                })
            });
            self.status = Some(status);
            self.chunk = chunked.then(|| Chunk::Size(Vec::new()));
            out.push(Piece::Head { status });
            let rest = self.head.split_off(end + 4);
            self.head = Vec::new();
            if !rest.is_empty() {
                self.body(&rest, &mut out)?;
            }
            return Ok(out);
        }
        if !data.is_empty() {
            self.body(data, &mut out)?;
        }
        Ok(out)
    }

    fn body(&mut self, data: &[u8], out: &mut Vec<Piece>) -> Result<(), String> {
        let Some(chunk) = &mut self.chunk else {
            out.push(Piece::Body(data.to_vec()));
            return Ok(());
        };
        let mut body = Vec::new();
        let mut i = 0;
        while i < data.len() {
            match chunk {
                Chunk::Size(line) => {
                    let b = data[i];
                    i += 1;
                    if b == b'\n' {
                        let text = String::from_utf8_lossy(line).into_owned();
                        let size = text.split(';').next().unwrap_or("").trim();
                        let size = usize::from_str_radix(size, 16)
                            .map_err(|_| format!("bad chunk size '{size}'"))?;
                        *chunk = if size == 0 {
                            Chunk::Trailer(Vec::new())
                        } else {
                            Chunk::Data(size)
                        };
                    } else if b != b'\r' {
                        line.push(b);
                        if line.len() > 64 {
                            return Err("bad chunk size line".into());
                        }
                    }
                }
                Chunk::Data(left) => {
                    let n = (*left).min(data.len() - i);
                    body.extend_from_slice(&data[i..i + n]);
                    i += n;
                    *left -= n;
                    if *left == 0 {
                        *chunk = Chunk::DataEnd;
                    }
                }
                Chunk::DataEnd => {
                    let b = data[i];
                    i += 1;
                    if b == b'\n' {
                        *chunk = Chunk::Size(Vec::new());
                    } else if b != b'\r' {
                        return Err("chunk data overruns its size".into());
                    }
                }
                Chunk::Trailer(line) => {
                    let b = data[i];
                    i += 1;
                    if b == b'\n' {
                        if line.is_empty() {
                            *chunk = Chunk::Done;
                        } else {
                            line.clear();
                        }
                    } else if b != b'\r' {
                        line.push(b);
                    }
                }
                Chunk::Done => break,
            }
        }
        if !body.is_empty() {
            out.push(Piece::Body(body));
        }
        Ok(())
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Splits a byte stream into lines (LF, with an optional CR before it).
#[derive(Debug, Default)]
pub(crate) struct Lines {
    partial: Vec<u8>,
}

impl Lines {
    pub(crate) fn feed(&mut self, data: &[u8]) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        for &b in data {
            if b == b'\n' {
                let mut line = std::mem::take(&mut self.partial);
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                out.push(String::from_utf8_lossy(&line).into_owned());
            } else {
                self.partial.push(b);
                if self.partial.len() > MAX_LINE {
                    return Err(format!("an event line over {MAX_LINE} bytes"));
                }
            }
        }
        Ok(out)
    }
}

/// Splits the preview stream into frames, each returned whole as the camera
/// sent it: `u32 length (BE) | i64 pts µs (BE) | u8 flags | data`.
#[derive(Debug, Default)]
pub(crate) struct Frames {
    buffer: Vec<u8>,
}

impl Frames {
    pub(crate) fn feed(&mut self, data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        self.buffer.extend_from_slice(data);
        let mut out = Vec::new();
        let mut at = 0;
        while self.buffer.len() - at >= FRAME_HEADER {
            let b = &self.buffer[at..];
            let length = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize;
            if length > MAX_FRAME {
                self.buffer.clear();
                return Err(format!(
                    "a preview frame of {length} bytes, over the {MAX_FRAME} that marks one corrupt"
                ));
            }
            let total = FRAME_HEADER + length;
            if b.len() < total {
                break;
            }
            out.push(b[..total].to_vec());
            at += total;
        }
        self.buffer.drain(..at);
        Ok(out)
    }
}

/// A frame's pts in microseconds and whether it is a keyframe.
#[cfg(test)]
pub(crate) fn frame_info(frame: &[u8]) -> (i64, bool) {
    let pts = i64::from_be_bytes(frame[4..12].try_into().unwrap());
    (pts, frame[12] & 1 != 0)
}

/// A fleet link, `meros-camera://fleet?d=<data>` where `<data>` is base64url
/// (no padding) of raw DEFLATE of `{"key": ..., "preset"?: ...}`, decoded to
/// `{"key", "preset"?}` (protocol §4).
pub(crate) fn decode_fleet_link(link: &str) -> Result<Value, String> {
    use base64::engine::{general_purpose, DecodePaddingMode, GeneralPurpose};
    use base64::Engine;
    const B64: GeneralPurpose = GeneralPurpose::new(
        &base64::alphabet::URL_SAFE,
        general_purpose::NO_PAD.with_decode_padding_mode(DecodePaddingMode::Indifferent),
    );
    let link = link.trim();
    let scheme = "meros-camera://";
    let rest = match link.get(..scheme.len()) {
        Some(s) if s.eq_ignore_ascii_case(scheme) => &link[scheme.len()..],
        _ => return Err("not a Meros Camera link (meros-camera://)".into()),
    };
    let (kind, query) = rest.split_once('?').ok_or("the link carries nothing")?;
    let kind = kind.trim_end_matches('/');
    if kind != "fleet" {
        return Err(format!("a '{kind}' link, not a fleet link"));
    }
    let data = query
        .split('&')
        .find_map(|p| p.strip_prefix("d="))
        .ok_or("the link carries nothing")?;
    let packed = B64.decode(data).map_err(|_| "the link is damaged")?;
    let json = miniz_oxide::inflate::decompress_to_vec_with_limit(&packed, MAX_LINK_JSON)
        .map_err(|_| "the link is damaged")?;
    let doc: Value = serde_json::from_slice(&json).map_err(|_| "the link is damaged")?;
    let key = doc
        .get("key")
        .and_then(Value::as_str)
        .ok_or("the link carries no fleet key")?;
    if !is_fleet_key(key) {
        return Err("the link's key is not a fleet key (32 lowercase hex characters)".into());
    }
    let mut out = json!({ "key": key });
    if let Some(preset) = doc.get("preset").filter(|p| !p.is_null()) {
        out["preset"] = fleet_preset(preset)?;
    }
    Ok(out)
}

/// A fleet link's preset (§4): `name` and `settings` required, `monitoring`
/// optional, and `outputs` cut to `format` and `look`, since a preset never
/// carries a destination. A preset without its name or settings refuses the
/// whole link.
fn fleet_preset(preset: &Value) -> Result<Value, String> {
    let refused = |why: &str| format!("the link's preset {why}, so the link is refused");
    let p = preset
        .as_object()
        .ok_or_else(|| refused("is not an object"))?;
    if !p
        .get("name")
        .and_then(Value::as_str)
        .is_some_and(|n| !n.trim().is_empty())
    {
        return Err(refused("has no name"));
    }
    if !p.get("settings").is_some_and(Value::is_object) {
        return Err(refused("has no settings"));
    }
    if p.get("monitoring")
        .is_some_and(|m| !m.is_null() && !m.is_object())
    {
        return Err(refused("has monitoring that is not an object"));
    }
    let mut out = p.clone();
    match p.get("outputs") {
        None | Some(Value::Null) => {
            out.remove("outputs");
        }
        Some(Value::Object(outputs)) => {
            let kept: crate::value::Map<String, Value> = outputs
                .iter()
                .filter(|(kind, _)| matches!(kind.as_str(), "format" | "look"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if kept.is_empty() {
                out.remove("outputs");
            } else {
                out.insert("outputs".into(), Value::Object(kept));
            }
        }
        Some(_) => return Err(refused("has outputs that are not an object")),
    }
    if out.get("monitoring").is_some_and(Value::is_null) {
        out.remove("monitoring");
    }
    Ok(Value::Object(out))
}

/// 32 lowercase hex characters (16 random bytes).
pub(crate) fn is_fleet_key(key: &str) -> bool {
    key.len() == 32 && key.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// `name` percent-encoded as one path segment.
pub(crate) fn encode_segment(name: &str) -> String {
    let mut out = String::new();
    for b in name.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A `Content-Range`: the first and last byte sent (none for `*`), and the
/// file's total size (none for `*`, unknown).
pub(crate) type ContentRange = (Option<(u64, u64)>, Option<u64>);

/// `Content-Range: bytes <first>-<last>/<total>` or `bytes */<total>`.
pub(crate) fn content_range(value: &str) -> Option<ContentRange> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (range, total) = rest.split_once('/')?;
    let total = total.trim().parse().ok();
    let range = match range.trim() {
        "*" => None,
        r => {
            let (a, b) = r.split_once('-')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        }
    };
    Some((range, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    /// A link built as the protocol documents it (§4).
    fn link(kind: &str, doc: &Value) -> String {
        let packed = miniz_oxide::deflate::compress_to_vec(doc.to_string().as_bytes(), 9);
        let data = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(packed);
        format!("meros-camera://{kind}?d={data}")
    }

    const KEY: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn a_fleet_link_gives_its_key() {
        let l = link("fleet", &json!({ "key": KEY }));
        assert!(
            !l.split_once("?d=").unwrap().1.contains('='),
            "no padding: {l}"
        );
        assert_eq!(decode_fleet_link(&l).unwrap(), json!({ "key": KEY }));
    }

    #[test]
    fn a_fleet_link_with_a_preset_gives_both() {
        let preset = json!({"name": "Interview", "version": 1,
                            "settings": {"fps": 25, "exposure": {"shutter": {"angle": 180}}},
                            "monitoring": {"lut": "rec709", "zebra": true},
                            "outputs": {"format": {"height": 720, "bitrateMbps": 8, "halfRate": false},
                                        "look": {"look": "rec709", "overlays": "clean"}}});
        let l = link("fleet", &json!({ "key": KEY, "preset": preset }));
        assert_eq!(
            decode_fleet_link(&format!("  {l}\n")).unwrap(),
            json!({ "key": KEY, "preset": preset })
        );
    }

    #[test]
    fn a_preset_without_its_name_or_settings_refuses_the_whole_link() {
        for preset in [
            json!({"settings": {"fps": 25}}),
            json!({"name": "", "settings": {"fps": 25}}),
            json!({"name": "Interview"}),
            json!({"name": "Interview", "settings": null}),
            json!("Interview"),
        ] {
            let l = link("fleet", &json!({ "key": KEY, "preset": preset }));
            let e = decode_fleet_link(&l).unwrap_err();
            assert!(e.contains("refused"), "{e}");
            assert!(!e.contains(KEY), "the key in {e}");
        }
    }

    #[test]
    fn a_presets_destinations_are_dropped() {
        let preset = json!({"name": "Live", "version": 1, "settings": {"fps": 25},
                            "outputs": {"srt": {"on": true, "target": "10.0.0.2:9730"},
                                        "rtmp": {"url": "rtmp://x/live", "key": "secret"},
                                        "whip": {"token": "t"},
                                        "format": {"height": 720}}});
        let l = link("fleet", &json!({ "key": KEY, "preset": preset }));
        assert_eq!(
            decode_fleet_link(&l).unwrap()["preset"],
            json!({"name": "Live", "version": 1, "settings": {"fps": 25},
                   "outputs": {"format": {"height": 720}}})
        );
        // Destinations alone: no outputs left.
        let preset = json!({"name": "Live", "settings": {}, "outputs": {"ndi": {"on": true}}});
        let l = link("fleet", &json!({ "key": KEY, "preset": preset }));
        assert_eq!(
            decode_fleet_link(&l).unwrap()["preset"],
            json!({"name": "Live", "settings": {}})
        );
    }

    #[test]
    fn other_links_and_damage_are_refused() {
        let preset = link("preset", &json!({ "name": "A" }));
        assert!(decode_fleet_link(&preset)
            .unwrap_err()
            .contains("not a fleet link"));
        assert!(decode_fleet_link("https://example.com/?d=abc").is_err());
        assert!(decode_fleet_link("meros-camera://fleet").is_err());
        assert!(decode_fleet_link("meros-camera://fleet?d=!!!").is_err());
        assert!(decode_fleet_link("meros-camera://fleet?d=AAAA").is_err());
        let short = link("fleet", &json!({ "key": "abc" }));
        assert!(decode_fleet_link(&short)
            .unwrap_err()
            .contains("not a fleet key"));
        let upper = link("fleet", &json!({ "key": KEY.to_uppercase() }));
        assert!(decode_fleet_link(&upper).is_err());
        let none = link("fleet", &json!({ "name": "x" }));
        assert!(decode_fleet_link(&none)
            .unwrap_err()
            .contains("no fleet key"));
    }

    #[test]
    fn a_link_unpacking_past_the_bound_is_refused() {
        let big = "a".repeat(2 << 20);
        let l = link("fleet", &json!({ "key": KEY, "preset": big }));
        assert!(decode_fleet_link(&l).is_err());
    }

    #[test]
    fn a_streamed_head_then_body() {
        let mut r = ResponseReader::default();
        assert_eq!(r.feed(b"HTTP/1.1 200 OK\r\nContent-Ty").unwrap(), vec![]);
        assert_eq!(
            r.feed(b"pe: application/x-ndjson\r\nConnection: close\r\n\r\n{\"a\"")
                .unwrap(),
            vec![Piece::Head { status: 200 }, Piece::Body(b"{\"a\"".to_vec())]
        );
        assert_eq!(
            r.feed(b":1}\n").unwrap(),
            vec![Piece::Body(b":1}\n".to_vec())]
        );
    }

    #[test]
    fn chunked_coding_is_undone() {
        let mut r = ResponseReader::default();
        let mut out = r
            .feed(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhel")
            .unwrap();
        out.extend(r.feed(b"lo\r\n3;x=y\r\n wo\r\n0\r\n\r\n").unwrap());
        let body: Vec<u8> = out
            .into_iter()
            .filter_map(|p| match p {
                Piece::Body(b) => Some(b),
                _ => None,
            })
            .flatten()
            .collect();
        assert_eq!(body, b"hello wo");
    }

    #[test]
    fn a_refusal_is_read_from_its_status_line() {
        let mut r = ResponseReader::default();
        let out = r
            .feed(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 2\r\n\r\n{}")
            .unwrap();
        assert_eq!(out[0], Piece::Head { status: 401 });
        assert!(ResponseReader::default().feed(b"SSH-2.0\r\n\r\n").is_err());
    }

    #[test]
    fn lines_split_on_lf_with_or_without_cr() {
        let mut l = Lines::default();
        assert_eq!(l.feed(b"{\"a\":1}\r\n{\"b\"").unwrap(), vec!["{\"a\":1}"]);
        assert_eq!(l.feed(b":2}\n\n").unwrap(), vec!["{\"b\":2}", ""]);
    }

    fn frame(pts: i64, key: bool, data: &[u8]) -> Vec<u8> {
        let mut f = (data.len() as u32).to_be_bytes().to_vec();
        f.extend_from_slice(&pts.to_be_bytes());
        f.push(key as u8);
        f.extend_from_slice(data);
        f
    }

    #[test]
    fn frames_are_split_whole_across_reads() {
        let a = frame(1234, true, &[0, 0, 0, 1, 0x67, 1, 2]);
        let b = frame(-5, false, &[0, 0, 1, 0x41]);
        let mut all = a.clone();
        all.extend_from_slice(&b);
        let mut f = Frames::default();
        assert_eq!(f.feed(&all[..5]).unwrap(), Vec::<Vec<u8>>::new());
        assert_eq!(f.feed(&all[5..a.len() + 3]).unwrap(), vec![a.clone()]);
        let rest = f.feed(&all[a.len() + 3..]).unwrap();
        assert_eq!(rest, vec![b.clone()]);
        assert_eq!(frame_info(&a), (1234, true));
        assert_eq!(frame_info(&b), (-5, false));
    }

    #[test]
    fn a_frame_over_8_mib_is_corrupt() {
        let mut f = Frames::default();
        let mut header = ((8u32 << 20) + 1).to_be_bytes().to_vec();
        header.extend_from_slice(&[0; 9]);
        assert!(f.feed(&header).is_err());
        let ok = frame(0, true, &[1]);
        assert_eq!(f.feed(&ok).unwrap(), vec![ok]);
    }

    #[test]
    fn content_ranges_are_read() {
        assert_eq!(
            content_range("bytes 10-19/100"),
            Some((Some((10, 19)), Some(100)))
        );
        assert_eq!(content_range("bytes */100"), Some((None, Some(100))));
        assert_eq!(content_range("bytes 0-0/*"), Some((Some((0, 0)), None)));
        assert_eq!(content_range("items 1-2/3"), None);
    }

    #[test]
    fn clip_names_are_one_segment() {
        assert_eq!(encode_segment("A001C007.mov"), "A001C007.mov");
        assert_eq!(encode_segment("a b/c?.json"), "a%20b%2Fc%3F.json");
    }
}
