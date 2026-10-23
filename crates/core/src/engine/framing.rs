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
    /// One line, unless it ends in ':', then lines up to a blank line; or,
    /// with headers given, as they say (SPEC.md §2, Replies).
    HeadedBlock(Headers),
    /// Text between `open` and `close`, whole, including the delimiters.
    Delimited { open: String, close: String },
    /// Text up to and including any of these, with no line ending needed:
    /// Roland's `;`-ended replies and its bare ACK byte.
    Terminated(Vec<String>),
    /// One message per non-empty line, except a block between two lines
    /// matching this regex (the fences), which is one message without them:
    /// Turtle AV's status between lines of `=`.
    Fenced(String),
    /// Each complete top-level JSON object or array, however it is laid out
    /// and whatever lies between them: Barco Pulse's JSON-RPC, which sends
    /// bare JSON values back to back on one stream.
    Json,
}

/// `reply_block_header` and `reply_line_header`: which first lines of a
/// `headed-block` reply have more lines after them. CasparCG's AMCP answers
/// `200 ... OK` with lines up to a blank line, and `201 ... OK` with exactly
/// one line of data; neither ends in ':'.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Headers {
    /// A first line matching this regex opens a block ended by a blank
    /// line, in place of the line ending in ':'.
    pub(crate) block: Option<String>,
    /// A first line matching this regex is followed by exactly one more
    /// line of the message, whatever that line holds.
    pub(crate) line: Option<String>,
}

/// Where a `Json` framer is in the value it is reading.
#[derive(Default)]
struct JsonScan {
    /// The value's bytes so far, from its opening bracket.
    value: Vec<u8>,
    /// Brackets opened and not yet closed.
    depth: usize,
    in_string: bool,
    escaped: bool,
}

impl JsonScan {
    fn feed(&mut self, bytes: &[u8], out: &mut Vec<String>) {
        for &b in bytes {
            if self.depth == 0 {
                // Between values only an opening bracket starts one; anything
                // else (whitespace, stray bytes) is no message.
                if b == b'{' || b == b'[' {
                    self.value.push(b);
                    self.depth = 1;
                }
                continue;
            }
            self.value.push(b);
            if self.in_string {
                if self.escaped {
                    self.escaped = false;
                } else if b == b'\\' {
                    self.escaped = true;
                } else if b == b'"' {
                    self.in_string = false;
                }
                continue;
            }
            match b {
                b'"' => self.in_string = true,
                b'{' | b'[' => self.depth += 1,
                b'}' | b']' => {
                    self.depth -= 1;
                    if self.depth == 0 {
                        let value = std::mem::take(&mut self.value);
                        out.push(String::from_utf8_lossy(&value).into_owned());
                    }
                }
                _ => {}
            }
        }
    }
}

/// Turns received bytes into complete reply messages.
pub(crate) struct Framer {
    framing: ReplyFraming,
    /// `Fenced`: the fence, compiled.
    fence: Option<regex::Regex>,
    /// `HeadedBlock`'s headers, compiled.
    block_header: Option<regex::Regex>,
    line_header: Option<regex::Regex>,
    /// A line header was read: the next line completes the message.
    line_pending: bool,
    lines: Lines,
    block: Vec<String>,
    in_block: bool,
    text: String,
    json: JsonScan,
}

impl Framer {
    pub(crate) fn new(framing: ReplyFraming) -> Framer {
        let fence = match &framing {
            ReplyFraming::Fenced(re) => regex::Regex::new(re).ok(),
            _ => None,
        };
        let header = |pick: fn(&Headers) -> &Option<String>| match &framing {
            ReplyFraming::HeadedBlock(h) => {
                pick(h).as_deref().and_then(|re| regex::Regex::new(re).ok())
            }
            _ => None,
        };
        let (block_header, line_header) = (header(|h| &h.block), header(|h| &h.line));
        Framer {
            framing,
            fence,
            block_header,
            line_header,
            line_pending: false,
            lines: Lines::default(),
            block: Vec::new(),
            in_block: false,
            text: String::new(),
            json: JsonScan::default(),
        }
    }

    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        if self.framing == ReplyFraming::Json {
            self.json.feed(bytes, &mut out);
            return out;
        }
        if let ReplyFraming::Terminated(ends) = &self.framing {
            self.text.push_str(&String::from_utf8_lossy(bytes));
            loop {
                let first = ends
                    .iter()
                    .filter_map(|e| self.text.find(e.as_str()).map(|at| at + e.len()))
                    .min();
                let Some(end) = first else {
                    break;
                };
                let segment: String = self.text.drain(..end).collect();
                let segment = segment.trim_matches(|c: char| c == '\r' || c == '\n' || c == ' ');
                if !segment.is_empty() {
                    out.push(segment.to_string());
                }
            }
            return out;
        }
        if let ReplyFraming::Delimited { open, close } = &self.framing {
            self.text.push_str(&String::from_utf8_lossy(bytes));
            let (open, close) = (open.trim(), close.trim());
            if close.is_empty() {
                // No marker ends a message, so none can be read (the engine
                // refuses such a spec); nothing is kept, so nothing grows.
                self.text.clear();
                return out;
            }
            while let Some(end) = self.text.find(close) {
                let segment: String = self.text.drain(..end + close.len()).collect();
                if open.is_empty() {
                    // Only a closing marker: a separator between messages
                    // (PIXERA's `0xPX` after each JSON message), not part of
                    // the message.
                    let message = segment[..end].trim();
                    if !message.is_empty() {
                        out.push(message.to_string());
                    }
                    continue;
                }
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
                ReplyFraming::HeadedBlock(_) => {
                    let opens_block = match &self.block_header {
                        Some(re) => re.is_match(&line),
                        None => line.trim_end().ends_with(':'),
                    };
                    if self.line_pending {
                        // The one line after a line header, even an empty one.
                        self.line_pending = false;
                        self.block.push(line);
                        out.push(std::mem::take(&mut self.block).join("\n"));
                    } else if self.in_block {
                        if line.is_empty() {
                            self.in_block = false;
                            out.push(std::mem::take(&mut self.block).join("\n"));
                        } else {
                            self.block.push(line);
                        }
                    } else if self
                        .line_header
                        .as_ref()
                        .is_some_and(|re| re.is_match(&line))
                    {
                        self.line_pending = true;
                        self.block.push(line);
                    } else if opens_block {
                        self.in_block = true;
                        self.block.push(line);
                    } else if !line.is_empty() {
                        out.push(line);
                    }
                }
                ReplyFraming::Fenced(_) => {
                    let fence = self.fence.as_ref().is_some_and(|re| re.is_match(&line));
                    if fence && self.in_block {
                        // A fence right after a fence (DARWIN's status opens
                        // with two) keeps the block open; one after content
                        // closes it.
                        if self.block.iter().any(|l| !l.trim().is_empty()) {
                            self.in_block = false;
                            let lines = std::mem::take(&mut self.block);
                            let first = lines.iter().position(|l| !l.trim().is_empty());
                            let last = lines.iter().rposition(|l| !l.trim().is_empty());
                            if let (Some(a), Some(b)) = (first, last) {
                                out.push(lines[a..=b].join("\n"));
                            }
                        }
                    } else if fence {
                        self.in_block = true;
                    } else if self.in_block {
                        self.block.push(line);
                    } else if !line.trim().is_empty() {
                        out.push(line);
                    }
                }
                ReplyFraming::Delimited { .. }
                | ReplyFraming::Terminated(_)
                | ReplyFraming::Json => unreachable!(),
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
    /// The payload as it is, nothing added: a JSON value is its own frame.
    Json,
}

pub(crate) fn frame(framing: &SendFraming, payload: &str) -> String {
    match framing {
        SendFraming::Delimited { open, close } => format!("{open}{payload}{close}"),
        SendFraming::Json => payload.to_string(),
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
    fn an_empty_close_reads_nothing_and_keeps_nothing() {
        for close in ["", "  ", "\r\n"] {
            let mut f = Framer::new(ReplyFraming::Delimited {
                open: "<".into(),
                close: close.into(),
            });
            assert!(f.feed(b"< REP 1 >\r\n").is_empty());
            assert!(f.text.is_empty());
        }
    }

    #[test]
    fn any_line_ending_ends_a_line() {
        let mut f = Framer::new(ReplyFraming::Line);
        assert_eq!(f.feed(b"~01@ROUTE 1,2,3 OK\r"), ["~01@ROUTE 1,2,3 OK"]);
        assert_eq!(f.feed(b"\na\nb\r\n"), ["a", "b"]);
    }

    #[test]
    fn terminated_replies_need_no_line_ending() {
        let mut f = Framer::new(ReplyFraming::Terminated(vec![";".into(), "\u{6}".into()]));
        // A bare ACK byte, a value split across reads, then two at once.
        assert_eq!(f.feed(b"\x06"), vec!["\u{6}"]);
        assert_eq!(f.feed(b"\x02VFL:"), Vec::<String>::new());
        assert_eq!(f.feed(b"a;\r\nACK;"), vec!["\u{2}VFL:a;", "ACK;"]);
    }

    #[test]
    fn json_values_are_read_whole_however_they_arrive() {
        let mut f = Framer::new(ReplyFraming::Json);
        // Pretty-printed across lines and reads, with brackets, quotes and
        // escapes inside a string, then two compact values back to back.
        assert!(f
            .feed(b"{\n  \"jsonrpc\": \"2.0\",\n  \"result\": \"a}")
            .is_empty());
        assert_eq!(
            f.feed(b" \\\"{[\"\n}\r\n{\"id\":1,\"result\":[1,{\"x\":2}]}[3]"),
            [
                "{\n  \"jsonrpc\": \"2.0\",\n  \"result\": \"a} \\\"{[\"\n}",
                "{\"id\":1,\"result\":[1,{\"x\":2}]}",
                "[3]"
            ]
        );
        // Bytes between values are no message, and a character split across
        // reads arrives whole.
        assert!(f.feed(b"junk {\"n\":\"\xc3").is_empty());
        assert_eq!(f.feed(b"\xa9\"}"), ["{\"n\":\"\u{e9}\"}"]);
        assert_eq!(frame(&SendFraming::Json, "{\"a\":1}"), "{\"a\":1}");
    }

    #[test]
    fn fenced_blocks_are_one_message() {
        let mut f = Framer::new(ReplyFraming::Fenced(r"^=+\s*$".into()));
        // Lines outside fences are messages of their own; consecutive fences
        // open one block, whose blank edge lines are dropped.
        let out = f.feed(
            b"[SUCCESS]Set decoder 001 output on.\r\n====\r\n==== \r\n  Status Info\r\n\r\n001 On\r\n",
        );
        assert_eq!(out, ["[SUCCESS]Set decoder 001 output on."]);
        let out = f.feed(b"002 Off\r\n\r\n====\r\n[ERROR]x\r\n");
        assert_eq!(out, ["  Status Info\n\n001 On\n002 Off", "[ERROR]x"]);
    }

    #[test]
    fn headed_blocks_follow_hyperdeck() {
        let mut f = Framer::new(ReplyFraming::HeadedBlock(Headers::default()));
        let out = f.feed(b"200 ok\r\n208 transport info:\r\nstatus: play\r\nspeed: 100\r\n\r\n");
        assert_eq!(
            out,
            ["200 ok", "208 transport info:\nstatus: play\nspeed: 100"]
        );
    }

    #[test]
    fn headed_blocks_follow_amcp_headers() {
        let mut f = Framer::new(ReplyFraming::HeadedBlock(Headers {
            block: Some("^200 ".into()),
            line: Some("^(101|201|400|500) ".into()),
        }));
        // 202 is one line; 201 one line of data; 200 lines up to a blank
        // line.
        let out = f.feed(b"202 PLAY OK\r\n201 VERSION OK\r\n2.3.3 LTS\r\n200 INFO OK\r\n");
        assert_eq!(out, ["202 PLAY OK", "201 VERSION OK\n2.3.3 LTS"]);
        let out =
            f.feed(b"1 1080i5000 PLAYING\r\n2 PAL PLAYING\r\n\r\n201 DATA RETRIEVE OK\r\n\r\n");
        assert_eq!(
            out,
            [
                "200 INFO OK\n1 1080i5000 PLAYING\n2 PAL PLAYING",
                "201 DATA RETRIEVE OK\n"
            ]
        );
        // A 200 with nothing listed ends at its blank line, and with a block
        // header a line ending in ':' is one line.
        assert_eq!(
            f.feed(b"200 TLS OK\r\n\r\nnote:\r\n"),
            ["200 TLS OK", "note:"]
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

    #[test]
    fn a_closing_marker_alone_separates_messages() {
        let mut f = Framer::new(ReplyFraming::Delimited {
            open: String::new(),
            close: "0xPX".into(),
        });
        assert_eq!(
            f.feed(b"{\"id\":1}0xPX{\"id\""),
            vec!["{\"id\":1}".to_string()]
        );
        assert_eq!(f.feed(b":2}0xPX"), vec!["{\"id\":2}".to_string()]);
    }
}
