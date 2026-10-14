//! Server-sent events, parsed incrementally from a byte stream.
//!
//! Follows the WHATWG EventSource rules that SSCv2 §3.4.1 refers to: events end
//! at a blank line, `data:` lines are joined with `\n`, one optional space after
//! the colon is removed, lines starting with `:` are comments, and a line
//! ending is CRLF, LF or CR.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// The `event:` field; `message` when absent.
    pub event: String,
    pub data: String,
}

#[derive(Default)]
pub(crate) struct SseParser {
    line: Vec<u8>,
    last_was_cr: bool,
    event: Option<String>,
    data: Option<String>,
    /// A chunked JSON stream (ProPresenter's /v1/status/updates) rather than
    /// event-stream: each part, up to a blank line, is one event's data,
    /// its lines joined with `\n`.
    json: bool,
}

impl SseParser {
    /// A parser for a stream of JSON documents, each ended by a blank line.
    pub(crate) fn json() -> SseParser {
        SseParser {
            json: true,
            ..SseParser::default()
        }
    }

    /// Feed a chunk; returns every event it completes.
    pub(crate) fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        let mut out = Vec::new();
        for &b in chunk {
            if self.last_was_cr && b == b'\n' {
                // The LF of a CRLF already ended the line.
                self.last_was_cr = false;
                continue;
            }
            self.last_was_cr = b == b'\r';
            if b == b'\r' || b == b'\n' {
                let line = std::mem::take(&mut self.line);
                if let Some(event) = self.line_done(&line) {
                    out.push(event);
                }
            } else {
                self.line.push(b);
            }
        }
        out
    }

    fn line_done(&mut self, line: &[u8]) -> Option<SseEvent> {
        if self.json {
            if line.iter().all(u8::is_ascii_whitespace) {
                return self.data.take().map(|data| SseEvent {
                    event: "message".into(),
                    data,
                });
            }
            let line = String::from_utf8_lossy(line);
            match &mut self.data {
                Some(d) => {
                    d.push('\n');
                    d.push_str(&line);
                }
                None => self.data = Some(line.into_owned()),
            }
            return None;
        }
        if line.is_empty() {
            let data = self.data.take();
            let event = self.event.take();
            return data.map(|data| SseEvent {
                event: event.unwrap_or_else(|| "message".into()),
                data,
            });
        }
        if line[0] == b':' {
            return None;
        }
        let line = String::from_utf8_lossy(line);
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line.as_ref(), ""),
        };
        match field {
            "event" => self.event = Some(value.to_string()),
            "data" => match &mut self.data {
                Some(d) => {
                    d.push('\n');
                    d.push_str(value);
                }
                None => self.data = Some(value.to_string()),
            },
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_events_across_chunks_and_line_endings() {
        let mut p = SseParser::default();
        assert!(p.feed(b"event: open\r\ndata: {\"a\"").is_empty());
        let events = p.feed(b":1}\r\n\r\n: keepalive\n\ndata: x\ndata: y\n\n");
        assert_eq!(
            events,
            vec![
                SseEvent {
                    event: "open".into(),
                    data: "{\"a\":1}".into()
                },
                SseEvent {
                    event: "message".into(),
                    data: "x\ny".into()
                },
            ]
        );
    }

    #[test]
    fn a_json_stream_is_split_at_blank_lines() {
        let mut p = SseParser::json();
        assert!(p.feed(b"{\"url\":\"/v1/status/slide\",").is_empty());
        let events = p.feed(b"\r\n\"data\":1}\r\n\r\n{\"url\":\"/v1/x\",\"data\":2}\r\n\r\n");
        assert_eq!(
            events,
            vec![
                SseEvent {
                    event: "message".into(),
                    data: "{\"url\":\"/v1/status/slide\",\n\"data\":1}".into()
                },
                SseEvent {
                    event: "message".into(),
                    data: "{\"url\":\"/v1/x\",\"data\":2}".into()
                },
            ]
        );
    }

    #[test]
    fn a_crlf_split_across_chunks_is_one_line_ending() {
        let mut p = SseParser::default();
        assert!(p.feed(b"data: z\r").is_empty());
        assert_eq!(
            p.feed(b"\n\r\n"),
            vec![SseEvent {
                event: "message".into(),
                data: "z".into()
            }]
        );
    }
}
