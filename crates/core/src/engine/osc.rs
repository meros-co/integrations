//! OSC 1.0 messages: encoding for sends, decoding for replies.

use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Arg {
    Int(i32),
    Float(f32),
    Str(String),
    Blob(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Message {
    pub(crate) address: String,
    pub(crate) args: Vec<Value>,
}

fn pad(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

fn push_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0);
    pad(out);
}

pub(crate) fn encode(address: &str, args: &[Arg]) -> Vec<u8> {
    let mut out = Vec::new();
    push_str(&mut out, address);
    let tags: String = std::iter::once(',')
        .chain(args.iter().map(|a| match a {
            Arg::Int(_) => 'i',
            Arg::Float(_) => 'f',
            Arg::Str(_) => 's',
            Arg::Blob(_) => 'b',
        }))
        .collect();
    push_str(&mut out, &tags);
    for arg in args {
        match arg {
            Arg::Int(n) => out.extend_from_slice(&n.to_be_bytes()),
            Arg::Float(f) => out.extend_from_slice(&f.to_be_bytes()),
            Arg::Str(s) => push_str(&mut out, s),
            Arg::Blob(b) => {
                out.extend_from_slice(&(b.len() as i32).to_be_bytes());
                out.extend_from_slice(b);
                pad(&mut out);
            }
        }
    }
    out
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn string(&mut self) -> Option<String> {
        let rest = &self.data[self.pos..];
        let end = rest.iter().position(|b| *b == 0)?;
        let s = String::from_utf8_lossy(&rest[..end]).into_owned();
        self.pos += (end + 4) & !3;
        Some(s)
    }

    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let slice = self.data.get(self.pos..self.pos + n)?;
        self.pos += n;
        Some(slice)
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_be_bytes(self.take(4)?.try_into().ok()?))
    }
}

/// Every message in a packet: a plain message, or the contents of a bundle.
pub(crate) fn decode(packet: &[u8]) -> Vec<Message> {
    let mut out = Vec::new();
    decode_into(packet, &mut out);
    out
}

fn decode_into(packet: &[u8], out: &mut Vec<Message>) {
    if packet.starts_with(b"#bundle\0") {
        let mut r = Reader {
            data: packet,
            pos: 16,
        };
        while r.pos < packet.len() {
            let Some(size) = r.i32() else { return };
            let Some(element) = r.take(size.max(0) as usize) else {
                return;
            };
            decode_into(element, out);
        }
        return;
    }
    let mut r = Reader {
        data: packet,
        pos: 0,
    };
    let Some(address) = r.string() else { return };
    let tags = if r.pos < packet.len() {
        r.string().unwrap_or_default()
    } else {
        String::new()
    };
    let mut args = Vec::new();
    for tag in tags.chars().skip_while(|c| *c == ',') {
        let arg = match tag {
            'i' => r.i32().map(|n| json!(n)),
            'f' => r
                .take(4)
                .map(|b| json!(f32::from_be_bytes(b.try_into().unwrap()) as f64)),
            's' => r.string().map(|s| json!(s)),
            'b' => r.i32().and_then(|n| {
                let bytes = r.take(n.max(0) as usize)?.to_vec();
                r.pos = (r.pos + 3) & !3;
                Some(json!(bytes))
            }),
            'h' => r
                .take(8)
                .map(|b| json!(i64::from_be_bytes(b.try_into().unwrap()))),
            'd' => r
                .take(8)
                .map(|b| json!(f64::from_be_bytes(b.try_into().unwrap()))),
            'T' => Some(json!(true)),
            'F' => Some(json!(false)),
            'N' => Some(Value::Null),
            _ => None,
        };
        match arg {
            Some(a) => args.push(a),
            None => break,
        }
    }
    out.push(Message { address, args });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_the_spec_example() {
        // SPEC.md §7: /ch/07/mix/on, int 0.
        let bytes = encode("/ch/07/mix/on", &[Arg::Int(0)]);
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "2f63682f30372f6d69782f6f6e0000002c69000000000000");
    }

    #[test]
    fn round_trips() {
        let bytes = encode(
            "/x",
            &[Arg::Str("Vox".into()), Arg::Float(0.75), Arg::Int(-2)],
        );
        let m = &decode(&bytes)[0];
        assert_eq!(m.address, "/x");
        assert_eq!(m.args, vec![json!("Vox"), json!(0.75), json!(-2)]);
    }

    #[test]
    fn a_message_without_type_tags_decodes() {
        let mut bytes = Vec::new();
        push_str(&mut bytes, "/eos/ping");
        assert_eq!(decode(&bytes)[0].address, "/eos/ping");
    }
}
