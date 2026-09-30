//! Message framing on byte streams, in both directions.

/// Splits a byte stream into lines. CR, LF and CRLF each end a line, so a
/// device's choice of line ending never changes what is read. Empty lines are
/// kept: blocks end at them.
#[derive(Default)]
struct Lines {
    current: Vec<u8>,
    after_cr: bool,
}

impl Lines {
    fn feed(&mut self, bytes: &[u8], out: &mut Vec<String>) {
        for &b in bytes {
            if self.after_cr && b == b'\n' {
                self.after_cr = false;
                continue;
            }
            self.after_cr = b == b'\r';
            if b == b'\r' || b == b'\n' {
                out.push(String::from_utf8_lossy(&std::mem::take(&mut self.current)).into_owned());
            } else {
                self.current.push(b);
            }
        }
    }
}

/// How a reply is delimited: SPEC.md §2, "Replies".
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ReplyFraming {
    /// One message per non-empty line.
    Line,
    /// Lines up to a blank line.
    Block,
    /// One line, unless it ends in ':', then lines up to a blank line.
    HeadedBlock,
    /// Text between `open` and `close`, whole, including the delimiters.
    Delimited { open: String, close: String },
}

/// Turns received bytes into complete reply messages.
pub(crate) struct Framer {
    framing: ReplyFraming,
    lines: Lines,
    block: Vec<String>,
    in_block: bool,
    text: String,
}

impl Framer {
    pub(crate) fn new(framing: ReplyFraming) -> Framer {
        Framer {
            framing,
            lines: Lines::default(),
            block: Vec::new(),
            in_block: false,
            text: String::new(),
        }
    }

    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        if let ReplyFraming::Delimited { open, close } = &self.framing {
            self.text.push_str(&String::from_utf8_lossy(bytes));
            let (open, close) = (open.trim(), close.trim());
            while let Some(end) = self.text.find(close) {
                let segment: String = self.text.drain(..end + close.len()).collect();
                let start = segment.find(open).unwrap_or(0);
                out.push(segment[start..].trim().to_string());
            }
            return out;
        }

        let mut lines = Vec::new();
        self.lines.feed(bytes, &mut lines);
        for line in lines {
            match self.framing {
                ReplyFraming::Line => {
                    if !line.trim().is_empty() {
                        out.push(line);
                    }
                }
                ReplyFraming::Block => {
                    if line.is_empty() {
                        if !self.block.is_empty() {
                            out.push(std::mem::take(&mut self.block).join("\n"));
                        }
                    } else {
                        self.block.push(line);
                    }
                }
                ReplyFraming::HeadedBlock => {
                    if self.in_block {
                        if line.is_empty() {
                            self.in_block = false;
                            out.push(std::mem::take(&mut self.block).join("\n"));
                        } else {
                            self.block.push(line);
                        }
                    } else if line.trim_end().ends_with(':') {
                        self.in_block = true;
                        self.block.push(line);
                    } else if !line.is_empty() {
                        out.push(line);
                    }
                }
                ReplyFraming::Delimited { .. } => unreachable!(),
            }
        }
        out
    }
}

/// How an outgoing line/TCP message is framed: SPEC.md §2, `line-tcp`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SendFraming {
    Delimited {
        open: String,
        close: String,
    },
    Terminated(&'static str),
    /// Lines ending in a blank line.
    Block,
}

pub(crate) fn frame(framing: &SendFraming, payload: &str) -> String {
    match framing {
        SendFraming::Delimited { open, close } => format!("{open}{payload}{close}"),
        SendFraming::Terminated(t) => format!("{payload}{t}"),
        SendFraming::Block => format!("{}\n\n", payload.trim_end_matches(['\n', '\r'])),
    }
}

const SLIP_END: u8 = 0xC0;
const SLIP_ESC: u8 = 0xDB;
const SLIP_ESC_END: u8 = 0xDC;
const SLIP_ESC_ESC: u8 = 0xDD;

/// Packet framing for OSC over TCP.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PacketFraming {
    /// RFC 1055, double-ended as OSC 1.1 specifies.
    Slip,
    /// A big-endian int32 byte count before each packet (OSC 1.0).
    LengthPrefixed,
}

pub(crate) fn frame_packet(framing: PacketFraming, packet: &[u8]) -> Vec<u8> {
    match framing {
        PacketFraming::Slip => {
            let mut out = vec![SLIP_END];
            for &b in packet {
                match b {
                    SLIP_END => out.extend_from_slice(&[SLIP_ESC, SLIP_ESC_END]),
                    SLIP_ESC => out.extend_from_slice(&[SLIP_ESC, SLIP_ESC_ESC]),
                    b => out.push(b),
                }
            }
            out.push(SLIP_END);
            out
        }
        PacketFraming::LengthPrefixed => {
            let mut out = (packet.len() as u32).to_be_bytes().to_vec();
            out.extend_from_slice(packet);
            out
        }
    }
}

pub(crate) struct PacketReader {
    framing: PacketFraming,
    buf: Vec<u8>,
}

impl PacketReader {
    pub(crate) fn new(framing: PacketFraming) -> PacketReader {
        PacketReader {
            framing,
            buf: Vec::new(),
        }
    }

    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        match self.framing {
            PacketFraming::Slip => {
                while let Some(end) = self.buf.iter().position(|b| *b == SLIP_END) {
                    let raw: Vec<u8> = self.buf.drain(..=end).collect();
                    let mut packet = Vec::new();
                    let mut escaped = false;
                    for &b in &raw[..raw.len() - 1] {
                        match (escaped, b) {
                            (true, SLIP_ESC_END) => packet.push(SLIP_END),
                            (true, SLIP_ESC_ESC) => packet.push(SLIP_ESC),
                            (true, other) => packet.push(other),
                            (false, SLIP_ESC) => {
                                escaped = true;
                                continue;
                            }
                            (false, other) => packet.push(other),
                        }
                        escaped = false;
                    }
                    if !packet.is_empty() {
                        out.push(packet);
                    }
                }
            }
            PacketFraming::LengthPrefixed => loop {
                if self.buf.len() < 4 {
                    break;
                }
                let len = u32::from_be_bytes(self.buf[..4].try_into().unwrap()) as usize;
                if self.buf.len() < 4 + len {
                    break;
                }
                let packet = self.buf[4..4 + len].to_vec();
                self.buf.drain(..4 + len);
                out.push(packet);
            },
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_line_ending_ends_a_line() {
        let mut f = Framer::new(ReplyFraming::Line);
        assert_eq!(f.feed(b"~01@ROUTE 1,2,3 OK\r"), ["~01@ROUTE 1,2,3 OK"]);
        assert_eq!(f.feed(b"\na\nb\r\n"), ["a", "b"]);
    }

    #[test]
    fn headed_blocks_follow_hyperdeck() {
        let mut f = Framer::new(ReplyFraming::HeadedBlock);
        let out = f.feed(b"200 ok\r\n208 transport info:\r\nstatus: play\r\nspeed: 100\r\n\r\n");
        assert_eq!(
            out,
            ["200 ok", "208 transport info:\nstatus: play\nspeed: 100"]
        );
    }

    #[test]
    fn blocks_follow_videohub() {
        let mut f = Framer::new(ReplyFraming::Block);
        assert_eq!(f.feed(b"ACK\n\nVIDEO OUTPUT ROUTING:\n0 5\n"), ["ACK"]);
        assert_eq!(f.feed(b"\n"), ["VIDEO OUTPUT ROUTING:\n0 5"]);
    }

    #[test]
    fn delimited_follows_shure() {
        let mut f = Framer::new(ReplyFraming::Delimited {
            open: "< ".into(),
            close: " >".into(),
        });
        assert_eq!(
            f.feed(b"< REP 1 CHAN_NAME {Pulpit        } >< REP 1 BATT_BARS 5 >"),
            [
                "< REP 1 CHAN_NAME {Pulpit        } >",
                "< REP 1 BATT_BARS 5 >"
            ]
        );
    }

    #[test]
    fn outgoing_framing() {
        let open = "< ".to_string();
        let close = " >".to_string();
        assert_eq!(
            frame(&SendFraming::Delimited { open, close }, "GET 1 CHAN_NAME"),
            "< GET 1 CHAN_NAME >"
        );
        assert_eq!(
            frame(&SendFraming::Terminated("\r"), "#MODEL?"),
            "#MODEL?\r"
        );
        assert_eq!(
            frame(&SendFraming::Block, "VIDEO OUTPUT ROUTING:\n4 0\n"),
            "VIDEO OUTPUT ROUTING:\n4 0\n\n"
        );
    }

    #[test]
    fn slip_and_length_prefix_round_trip() {
        for framing in [PacketFraming::Slip, PacketFraming::LengthPrefixed] {
            let packet = vec![1, SLIP_END, 2, SLIP_ESC, 3];
            let mut r = PacketReader::new(framing);
            let framed = frame_packet(framing, &packet);
            let (a, b) = framed.split_at(3);
            let mut got = r.feed(a);
            got.extend(r.feed(b));
            assert_eq!(got, [packet]);
        }
    }
}
