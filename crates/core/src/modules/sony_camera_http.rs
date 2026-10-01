//! The little HTTP/1.1 the Sony module speaks itself, over a byte stream the
//! session opens through the camera's SSH tunnel: pan/tilt cameras serve
//! live view from a URL on their own localhost, and video-only cameras serve
//! their MediaProfile and clips the same way. The core's HTTP client cannot
//! reach a camera's localhost, so this module writes the request and reads
//! the response from the tunnelled stream.
//!
//! Only what those responses need is handled: a status line, headers, and a
//! body framed by Content-Length, chunked transfer coding, or the end of the
//! connection.

/// `http://host:port/path` split into its parts. The host is as the camera
/// sees it (usually `localhost`), which is where the tunnel forwards to.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Url {
    pub host: String,
    pub port: u16,
    pub path: String,
}

pub(crate) fn parse_url(url: &str) -> Option<Url> {
    let rest = url.trim().strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse().ok()?),
        None => (authority, 80),
    };
    if host.is_empty() {
        return None;
    }
    Some(Url {
        host: host.to_string(),
        port,
        path: path.to_string(),
    })
}

impl Url {
    /// A path relative to this URL's directory: `./Clip/A.MXF` against
    /// `/A/MEDIAPRO.XML` is `/A/Clip/A.MXF`.
    pub(crate) fn join(&self, relative: &str) -> Url {
        let path = if relative.starts_with('/') {
            relative.to_string()
        } else {
            let dir = match self.path.rfind('/') {
                Some(i) => &self.path[..=i],
                None => "/",
            };
            format!("{dir}{}", relative.trim_start_matches("./"))
        };
        Url {
            host: self.host.clone(),
            port: self.port,
            path,
        }
    }

    pub(crate) fn request(&self) -> Vec<u8> {
        format!(
            "GET {} HTTP/1.1\r\nHost: {}:{}\r\nUser-Agent: meros-integrations\r\nAccept: */*\r\nConnection: close\r\n\r\n",
            self.path, self.host, self.port
        )
        .into_bytes()
    }
}

/// What a response delivers, in order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum HttpEvent {
    Head {
        status: u16,
        content_type: String,
        length: Option<u64>,
    },
    /// Body bytes, with transfer coding removed.
    Body(Vec<u8>),
    /// One chunk of a chunked body is complete. Sony's pan/tilt live view
    /// sends one live view dataset per chunk.
    ChunkEnd,
    /// The body is complete.
    End,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Framing {
    Length(u64),
    Chunked,
    UntilClose,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Stage {
    Head,
    Body,
    ChunkSize,
    ChunkData(u64),
    ChunkDataEnd,
    Trailer,
    Done,
}

/// Reads one HTTP response from a byte stream.
pub(crate) struct HttpReader {
    buffer: Vec<u8>,
    stage: Stage,
    framing: Framing,
    received: u64,
}

const MAX_HEAD: usize = 64 * 1024;

impl Default for HttpReader {
    fn default() -> Self {
        HttpReader {
            buffer: Vec::new(),
            stage: Stage::Head,
            framing: Framing::UntilClose,
            received: 0,
        }
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

impl HttpReader {
    pub(crate) fn feed(&mut self, data: &[u8]) -> Result<Vec<HttpEvent>, String> {
        self.buffer.extend_from_slice(data);
        let mut out = Vec::new();
        loop {
            match self.stage {
                Stage::Head => {
                    let Some(end) = find(&self.buffer, b"\r\n\r\n") else {
                        if self.buffer.len() > MAX_HEAD {
                            return Err("HTTP response head too long".into());
                        }
                        break;
                    };
                    let head = String::from_utf8_lossy(&self.buffer[..end]).into_owned();
                    self.buffer.drain(..end + 4);
                    let mut lines = head.split("\r\n");
                    let status_line = lines.next().unwrap_or("");
                    let status: u16 = status_line
                        .split_whitespace()
                        .nth(1)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| format!("not an HTTP status line: {status_line}"))?;
                    let mut content_type = String::new();
                    let mut length = None;
                    let mut chunked = false;
                    for line in lines {
                        let Some((name, value)) = line.split_once(':') else {
                            continue;
                        };
                        let value = value.trim();
                        match name.trim().to_ascii_lowercase().as_str() {
                            "content-type" => content_type = value.to_string(),
                            "content-length" => length = value.parse().ok(),
                            "transfer-encoding" => {
                                chunked = value.to_ascii_lowercase().contains("chunked")
                            }
                            _ => {}
                        }
                    }
                    self.framing = if chunked {
                        Framing::Chunked
                    } else if let Some(n) = length {
                        Framing::Length(n)
                    } else {
                        Framing::UntilClose
                    };
                    out.push(HttpEvent::Head {
                        status,
                        content_type,
                        length: if chunked { None } else { length },
                    });
                    self.stage = match self.framing {
                        Framing::Chunked => Stage::ChunkSize,
                        Framing::Length(0) => {
                            out.push(HttpEvent::End);
                            Stage::Done
                        }
                        _ => Stage::Body,
                    };
                }
                Stage::Body => {
                    if self.buffer.is_empty() {
                        break;
                    }
                    let take = match self.framing {
                        Framing::Length(n) => ((n - self.received) as usize).min(self.buffer.len()),
                        _ => self.buffer.len(),
                    };
                    let body: Vec<u8> = self.buffer.drain(..take).collect();
                    self.received += body.len() as u64;
                    out.push(HttpEvent::Body(body));
                    if let Framing::Length(n) = self.framing {
                        if self.received >= n {
                            out.push(HttpEvent::End);
                            self.stage = Stage::Done;
                        }
                    }
                }
                Stage::ChunkSize => {
                    let Some(end) = find(&self.buffer, b"\r\n") else {
                        break;
                    };
                    let line = String::from_utf8_lossy(&self.buffer[..end]).into_owned();
                    self.buffer.drain(..end + 2);
                    let digits = line.split(';').next().unwrap_or("").trim();
                    let size = u64::from_str_radix(digits, 16)
                        .map_err(|_| format!("bad chunk size '{digits}'"))?;
                    self.stage = if size == 0 {
                        Stage::Trailer
                    } else {
                        Stage::ChunkData(size)
                    };
                }
                Stage::ChunkData(left) => {
                    if self.buffer.is_empty() {
                        break;
                    }
                    let take = (left as usize).min(self.buffer.len());
                    let body: Vec<u8> = self.buffer.drain(..take).collect();
                    self.received += body.len() as u64;
                    out.push(HttpEvent::Body(body));
                    let left = left - take as u64;
                    self.stage = if left == 0 {
                        Stage::ChunkDataEnd
                    } else {
                        Stage::ChunkData(left)
                    };
                }
                Stage::ChunkDataEnd => {
                    if self.buffer.len() < 2 {
                        break;
                    }
                    self.buffer.drain(..2);
                    out.push(HttpEvent::ChunkEnd);
                    self.stage = Stage::ChunkSize;
                }
                Stage::Trailer => {
                    let Some(end) = find(&self.buffer, b"\r\n") else {
                        break;
                    };
                    let empty = end == 0;
                    self.buffer.drain(..end + 2);
                    if empty {
                        out.push(HttpEvent::End);
                        self.stage = Stage::Done;
                    }
                }
                Stage::Done => {
                    self.buffer.clear();
                    break;
                }
            }
        }
        Ok(out)
    }

    /// The connection ended: the end of a body framed by the connection, or
    /// an error for anything else.
    pub(crate) fn closed(&mut self) -> Result<Vec<HttpEvent>, String> {
        match (self.stage, self.framing) {
            (Stage::Done, _) => Ok(vec![]),
            (Stage::Body, Framing::UntilClose) => {
                self.stage = Stage::Done;
                Ok(vec![HttpEvent::End])
            }
            _ => Err("the connection closed before the response was complete".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_and_relative_paths() {
        let u = parse_url("http://localhost:8080/A/MEDIAPRO.XML").unwrap();
        assert_eq!(
            u,
            Url {
                host: "localhost".into(),
                port: 8080,
                path: "/A/MEDIAPRO.XML".into()
            }
        );
        assert_eq!(u.join("./Clip/000_0001.MXF").path, "/A/Clip/000_0001.MXF");
        assert_eq!(u.join("/B/x").path, "/B/x");
        assert_eq!(parse_url("http://cam/").unwrap().port, 80);
        assert!(parse_url("ftp://x/").is_none());
        let req = String::from_utf8(u.request()).unwrap();
        assert!(req.starts_with("GET /A/MEDIAPRO.XML HTTP/1.1\r\nHost: localhost:8080\r\n"));
        assert!(req.ends_with("\r\n\r\n"));
    }

    fn body(events: &[HttpEvent]) -> Vec<u8> {
        events
            .iter()
            .flat_map(|e| match e {
                HttpEvent::Body(b) => b.clone(),
                _ => vec![],
            })
            .collect()
    }

    #[test]
    fn content_length_body_split_anywhere() {
        let response =
            b"HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: 5\r\n\r\nhello";
        let mut r = HttpReader::default();
        let mut events = Vec::new();
        for b in response.iter() {
            events.extend(r.feed(&[*b]).unwrap());
        }
        assert_eq!(
            events[0],
            HttpEvent::Head {
                status: 200,
                content_type: "text/xml".into(),
                length: Some(5)
            }
        );
        assert_eq!(body(&events), b"hello");
        assert_eq!(events.last(), Some(&HttpEvent::End));
    }

    #[test]
    fn chunked_bodies_mark_each_chunk() {
        let response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2;x=y\r\nde\r\n0\r\n\r\n";
        let mut r = HttpReader::default();
        let events = r.feed(response).unwrap();
        let kinds: Vec<&str> = events
            .iter()
            .map(|e| match e {
                HttpEvent::Head { .. } => "head",
                HttpEvent::Body(_) => "body",
                HttpEvent::ChunkEnd => "chunk",
                HttpEvent::End => "end",
            })
            .collect();
        assert_eq!(kinds, ["head", "body", "chunk", "body", "chunk", "end"]);
        assert_eq!(body(&events), b"abcde");
    }

    #[test]
    fn a_body_to_the_end_of_the_connection() {
        let mut r = HttpReader::default();
        let events = r.feed(b"HTTP/1.0 200 OK\r\n\r\nxyz").unwrap();
        assert_eq!(body(&events), b"xyz");
        assert_eq!(r.closed().unwrap(), [HttpEvent::End]);
        let mut short = HttpReader::default();
        short
            .feed(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nab")
            .unwrap();
        assert!(short.closed().is_err());
    }
}
