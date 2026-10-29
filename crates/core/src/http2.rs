//! HTTP/2 (RFC 9113) as a sans-IO client connection, for the core's gRPC
//! client (`crate::grpc`) and any native module that speaks HTTP/2 to a
//! device over a connection it opened (plain TCP with prior knowledge, or
//! TLS with ALPN `h2`).
//!
//! The module writes what [`Connection::take_output`] returns on its
//! connection and feeds what arrives to [`Connection::receive`], which
//! answers with [`Event`]s. Nothing here reads a clock or a socket.
//!
//! What it does, by section of RFC 9113:
//! - the client connection preface and a SETTINGS frame turning server push
//!   off (§3.4, §6.5.2), and every SETTINGS frame of the peer acknowledged
//!   and applied (§6.5.3);
//! - frames read whole (§4.1) at the default maximum size, which this side
//!   never raises; padding stripped (§6.1, §6.2); a header block gathered
//!   from HEADERS and its CONTINUATION frames (§6.10) and decoded with HPACK
//!   (`crate::hpack`); PRIORITY ignored (§5.3.2);
//! - streams opened with odd identifiers, at most as many at once as the
//!   peer's SETTINGS_MAX_CONCURRENT_STREAMS allows (§5.1.2): [`Connection::can_open`]
//!   says whether one more may open;
//! - flow control both ways (§5.2, §6.9): data sent waits for window and
//!   goes in frames no larger than the peer's SETTINGS_MAX_FRAME_SIZE, and
//!   the windows this side grants are 16 MiB for the connection and 1 MiB
//!   for each stream, topped up whenever half is used, so a large message
//!   never stalls and nothing caps a body's size;
//! - PING answered (§6.7), and pings this side sends reported when acked;
//! - RST_STREAM and GOAWAY reported (§6.4, §6.8).

use std::collections::{BTreeMap, VecDeque};

use crate::hpack;

const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

const DATA: u8 = 0x0;
const HEADERS: u8 = 0x1;
const PRIORITY: u8 = 0x2;
const RST_STREAM: u8 = 0x3;
const SETTINGS: u8 = 0x4;
const PUSH_PROMISE: u8 = 0x5;
const PING: u8 = 0x6;
const GOAWAY: u8 = 0x7;
const WINDOW_UPDATE: u8 = 0x8;
const CONTINUATION: u8 = 0x9;

const END_STREAM: u8 = 0x1;
const ACK: u8 = 0x1;
const END_HEADERS: u8 = 0x4;
const PADDED: u8 = 0x8;
const PRIORITY_FLAG: u8 = 0x20;

const SETTINGS_HEADER_TABLE_SIZE: u16 = 0x1;
const SETTINGS_ENABLE_PUSH: u16 = 0x2;
const SETTINGS_MAX_CONCURRENT_STREAMS: u16 = 0x3;
const SETTINGS_INITIAL_WINDOW_SIZE: u16 = 0x4;
const SETTINGS_MAX_FRAME_SIZE: u16 = 0x5;

/// The frame size every peer accepts (§4.2), and the largest this side reads.
const DEFAULT_MAX_FRAME: usize = 16_384;
const DEFAULT_WINDOW: i64 = 65_535;
const MAX_WINDOW: i64 = (1 << 31) - 1;
/// The window this side grants each stream, and the connection.
const STREAM_WINDOW: i64 = 1 << 20;
const CONNECTION_WINDOW: i64 = 1 << 24;

/// RST_STREAM and GOAWAY error codes (§7).
pub(crate) const NO_ERROR: u32 = 0x0;
pub(crate) const CANCEL: u32 = 0x8;

/// What the peer did.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    /// The peer's first SETTINGS frame arrived: the connection is up.
    Ready,
    /// A header block on a stream: the response headers, or trailers.
    Headers {
        stream: u32,
        /// Names in lower case, as HTTP/2 sends them.
        fields: Vec<(String, String)>,
        end_stream: bool,
    },
    Data {
        stream: u32,
        data: Vec<u8>,
        end_stream: bool,
    },
    /// The peer reset a stream (RST_STREAM) with this error code.
    Reset { stream: u32, code: u32 },
    /// The peer is closing the connection: streams above `last_stream` were
    /// not processed and may be retried on a new connection.
    GoAway {
        last_stream: u32,
        code: u32,
        debug: String,
    },
    /// The ack of a ping this side sent, with its payload.
    Pong([u8; 8]),
}

#[derive(Debug, Default)]
struct Stream {
    /// Data waiting for window, and whether it ends the stream.
    pending: VecDeque<(Vec<u8>, bool)>,
    send_window: i64,
    /// What this side has granted and the peer not yet used.
    recv_window: i64,
    /// Received and not yet granted back.
    recv_unacked: i64,
    local_closed: bool,
    remote_closed: bool,
}

/// One HTTP/2 connection's protocol state.
#[derive(Debug)]
pub(crate) struct Connection {
    server: bool,
    out: Vec<u8>,
    inbound: Vec<u8>,
    /// The client preface still to be read (a server's side).
    awaiting_preface: bool,
    decoder: hpack::Decoder,
    streams: BTreeMap<u32, Stream>,
    next_stream: u32,
    /// The peer's settings.
    max_concurrent: usize,
    initial_window: i64,
    max_frame: usize,
    /// The connection-level windows.
    send_window: i64,
    recv_unacked: i64,
    /// A header block being gathered: its stream, whether it ends the
    /// stream, and the fragments so far.
    gathering: Option<(u32, bool, Vec<u8>)>,
    ready: bool,
    /// The peer sent GOAWAY: no new streams.
    going_away: bool,
}

impl Connection {
    /// A client connection: the preface and this side's settings are queued.
    pub(crate) fn client() -> Connection {
        let mut c = Connection::new(false);
        c.out.extend_from_slice(PREFACE);
        c.send_settings();
        c
    }

    /// A server connection, for tests standing in for a device.
    #[cfg(test)]
    pub(crate) fn server() -> Connection {
        let mut c = Connection::new(true);
        c.awaiting_preface = true;
        c.send_settings();
        c
    }

    fn new(server: bool) -> Connection {
        Connection {
            server,
            out: Vec::new(),
            inbound: Vec::new(),
            awaiting_preface: false,
            decoder: hpack::Decoder::new(4096),
            streams: BTreeMap::new(),
            next_stream: if server { 2 } else { 1 },
            max_concurrent: usize::MAX,
            initial_window: DEFAULT_WINDOW,
            max_frame: DEFAULT_MAX_FRAME,
            send_window: DEFAULT_WINDOW,
            recv_unacked: 0,
            gathering: None,
            ready: false,
            going_away: false,
        }
    }

    fn send_settings(&mut self) {
        let mut body = Vec::new();
        for (id, value) in [
            (SETTINGS_ENABLE_PUSH, 0u32),
            (SETTINGS_INITIAL_WINDOW_SIZE, STREAM_WINDOW as u32),
        ] {
            if self.server && id == SETTINGS_ENABLE_PUSH {
                continue;
            }
            body.extend_from_slice(&id.to_be_bytes());
            body.extend_from_slice(&value.to_be_bytes());
        }
        frame(SETTINGS, 0, 0, &body, &mut self.out);
        // The connection window starts at 65,535 whatever the settings say.
        let grant = (CONNECTION_WINDOW - DEFAULT_WINDOW) as u32;
        frame(WINDOW_UPDATE, 0, 0, &grant.to_be_bytes(), &mut self.out);
    }

    /// What to write on the connection now.
    pub(crate) fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.out)
    }

    /// Whether a stream can be opened now: the peer's settings arrived, it
    /// is not going away, and it allows one more stream at once.
    pub(crate) fn can_open(&self) -> bool {
        self.ready && !self.going_away && self.open_streams() < self.max_concurrent
    }

    fn open_streams(&self) -> usize {
        self.streams.len()
    }

    /// Open a stream with these headers (pseudo-headers first, names in lower
    /// case); `end_stream` when no body follows. Returns its identifier.
    pub(crate) fn open(&mut self, fields: &[(&str, &str)], end_stream: bool) -> u32 {
        let id = self.next_stream;
        self.next_stream += 2;
        self.streams.insert(
            id,
            Stream {
                send_window: self.initial_window,
                recv_window: STREAM_WINDOW,
                local_closed: end_stream,
                ..Stream::default()
            },
        );
        self.send_headers(id, fields, end_stream);
        id
    }

    /// Send a header block on an open stream: a server's response headers
    /// or trailers, or a client's trailers.
    pub(crate) fn send_headers(&mut self, stream: u32, fields: &[(&str, &str)], end_stream: bool) {
        let block = hpack::encode(fields);
        let mut chunks = block.chunks(self.max_frame.max(1)).peekable();
        let mut first = true;
        loop {
            let chunk = chunks.next().unwrap_or(&[]);
            let last = chunks.peek().is_none();
            let mut flags = if last { END_HEADERS } else { 0 };
            let kind = if first { HEADERS } else { CONTINUATION };
            if first && end_stream {
                flags |= END_STREAM;
            }
            frame(kind, flags, stream, chunk, &mut self.out);
            first = false;
            if last {
                break;
            }
        }
        if end_stream {
            self.close_local(stream);
        }
    }

    /// Send data on a stream; it waits for flow-control window if needed.
    /// `end_stream` closes this side of the stream after it.
    pub(crate) fn send_data(&mut self, stream: u32, data: Vec<u8>, end_stream: bool) {
        let Some(s) = self.streams.get_mut(&stream) else {
            return;
        };
        if s.local_closed {
            return;
        }
        if end_stream {
            s.local_closed = true;
        }
        s.pending.push_back((data, end_stream));
        self.flush();
    }

    /// Reset a stream (RST_STREAM) and forget it.
    pub(crate) fn reset(&mut self, stream: u32, code: u32) {
        if self.streams.remove(&stream).is_some() {
            frame(RST_STREAM, 0, stream, &code.to_be_bytes(), &mut self.out);
        }
    }

    /// Send a PING; its ack arrives as [`Event::Pong`].
    pub(crate) fn ping(&mut self, payload: [u8; 8]) {
        frame(PING, 0, 0, &payload, &mut self.out);
    }

    /// Tell the peer this side is closing (GOAWAY with NO_ERROR).
    pub(crate) fn goaway(&mut self) {
        let last = if self.server {
            self.streams.keys().next_back().copied().unwrap_or(0)
        } else {
            0
        };
        let mut body = last.to_be_bytes().to_vec();
        body.extend_from_slice(&NO_ERROR.to_be_bytes());
        frame(GOAWAY, 0, 0, &body, &mut self.out);
    }

    fn close_local(&mut self, stream: u32) {
        if let Some(s) = self.streams.get_mut(&stream) {
            s.local_closed = true;
            if s.remote_closed && s.pending.is_empty() {
                self.streams.remove(&stream);
            }
        }
    }

    fn close_remote(&mut self, stream: u32) {
        if let Some(s) = self.streams.get_mut(&stream) {
            s.remote_closed = true;
            if s.local_closed && s.pending.is_empty() {
                self.streams.remove(&stream);
            }
        }
    }

    /// Send what the windows allow of the data waiting.
    fn flush(&mut self) {
        let ids: Vec<u32> = self
            .streams
            .iter()
            .filter(|(_, s)| !s.pending.is_empty())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            loop {
                let max_frame = self.max_frame;
                let conn_window = self.send_window;
                let Some(s) = self.streams.get_mut(&id) else {
                    break;
                };
                let Some((data, end)) = s.pending.front_mut() else {
                    break;
                };
                let room = conn_window.min(s.send_window).max(0) as usize;
                if data.is_empty() {
                    // An empty frame needs no window: the end of a stream.
                    let end = *end;
                    s.pending.pop_front();
                    frame(
                        DATA,
                        if end { END_STREAM } else { 0 },
                        id,
                        &[],
                        &mut self.out,
                    );
                    continue;
                }
                if room == 0 {
                    break;
                }
                let n = data.len().min(room).min(max_frame);
                let chunk: Vec<u8> = data.drain(..n).collect();
                let finished = data.is_empty();
                let end = *end && finished;
                if finished {
                    s.pending.pop_front();
                }
                s.send_window -= n as i64;
                self.send_window -= n as i64;
                frame(
                    DATA,
                    if end { END_STREAM } else { 0 },
                    id,
                    &chunk,
                    &mut self.out,
                );
            }
            if let Some(s) = self.streams.get(&id) {
                if s.local_closed && s.remote_closed && s.pending.is_empty() {
                    self.streams.remove(&id);
                }
            }
        }
    }

    /// Feed bytes received on the connection. An error is a connection
    /// error: the connection should be closed.
    pub(crate) fn receive(&mut self, bytes: &[u8]) -> Result<Vec<Event>, String> {
        self.inbound.extend_from_slice(bytes);
        let mut events = Vec::new();
        if self.awaiting_preface {
            if self.inbound.len() < PREFACE.len() {
                return Ok(events);
            }
            if &self.inbound[..PREFACE.len()] != PREFACE {
                return Err("http2: no client preface".into());
            }
            self.inbound.drain(..PREFACE.len());
            self.awaiting_preface = false;
        }
        let mut pos = 0;
        while self.inbound.len() - pos >= 9 {
            let head = &self.inbound[pos..pos + 9];
            let len =
                (usize::from(head[0]) << 16) | (usize::from(head[1]) << 8) | usize::from(head[2]);
            if len > DEFAULT_MAX_FRAME {
                return Err(format!(
                    "http2: a frame of {len} bytes, above the {DEFAULT_MAX_FRAME} allowed"
                ));
            }
            if self.inbound.len() - pos < 9 + len {
                break;
            }
            let kind = head[3];
            let flags = head[4];
            let stream = u32::from_be_bytes([head[5], head[6], head[7], head[8]]) & 0x7fff_ffff;
            let payload = self.inbound[pos + 9..pos + 9 + len].to_vec();
            pos += 9 + len;
            self.frame_in(kind, flags, stream, payload, &mut events)?;
        }
        self.inbound.drain(..pos);
        self.grant();
        Ok(events)
    }

    fn frame_in(
        &mut self,
        kind: u8,
        flags: u8,
        stream: u32,
        payload: Vec<u8>,
        events: &mut Vec<Event>,
    ) -> Result<(), String> {
        if let Some((gathering, _, _)) = &self.gathering {
            if kind != CONTINUATION || stream != *gathering {
                return Err("http2: a header block interrupted".into());
            }
        }
        match kind {
            DATA => {
                if stream == 0 {
                    return Err("http2: DATA on stream 0".into());
                }
                let total = payload.len() as i64;
                let body = unpad(flags, payload)?;
                self.recv_unacked += total;
                let end = flags & END_STREAM != 0;
                if let Some(s) = self.streams.get_mut(&stream) {
                    s.recv_window -= total;
                    s.recv_unacked += total;
                    if s.recv_window < 0 {
                        return Err("http2: the peer overran a stream's window".into());
                    }
                    events.push(Event::Data {
                        stream,
                        data: body,
                        end_stream: end,
                    });
                    if end {
                        self.close_remote(stream);
                    }
                }
            }
            HEADERS => {
                if stream == 0 {
                    return Err("http2: HEADERS on stream 0".into());
                }
                let mut body = unpad(flags, payload)?;
                if flags & PRIORITY_FLAG != 0 {
                    if body.len() < 5 {
                        return Err("http2: short HEADERS priority".into());
                    }
                    body.drain(..5);
                }
                let end = flags & END_STREAM != 0;
                if flags & END_HEADERS != 0 {
                    self.headers_in(stream, end, &body, events)?;
                } else {
                    self.gathering = Some((stream, end, body));
                }
            }
            CONTINUATION => {
                let Some((s, end, mut block)) = self.gathering.take() else {
                    return Err("http2: CONTINUATION without HEADERS".into());
                };
                block.extend_from_slice(&payload);
                if flags & END_HEADERS != 0 {
                    self.headers_in(s, end, &block, events)?;
                } else {
                    self.gathering = Some((s, end, block));
                }
            }
            PRIORITY => {}
            RST_STREAM => {
                if payload.len() != 4 {
                    return Err("http2: RST_STREAM of the wrong size".into());
                }
                let code = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
                if self.streams.remove(&stream).is_some() {
                    events.push(Event::Reset { stream, code });
                }
            }
            SETTINGS => {
                if stream != 0 {
                    return Err("http2: SETTINGS on a stream".into());
                }
                if flags & ACK != 0 {
                    return Ok(());
                }
                if !payload.len().is_multiple_of(6) {
                    return Err("http2: SETTINGS of the wrong size".into());
                }
                for s in payload.chunks(6) {
                    let id = u16::from_be_bytes([s[0], s[1]]);
                    let value = u32::from_be_bytes([s[2], s[3], s[4], s[5]]);
                    match id {
                        SETTINGS_MAX_CONCURRENT_STREAMS => self.max_concurrent = value as usize,
                        SETTINGS_INITIAL_WINDOW_SIZE => {
                            if i64::from(value) > MAX_WINDOW {
                                return Err("http2: initial window too large".into());
                            }
                            let delta = i64::from(value) - self.initial_window;
                            self.initial_window = i64::from(value);
                            for s in self.streams.values_mut() {
                                s.send_window += delta;
                            }
                        }
                        SETTINGS_MAX_FRAME_SIZE => {
                            if !(16_384..=16_777_215).contains(&value) {
                                return Err("http2: invalid SETTINGS_MAX_FRAME_SIZE".into());
                            }
                            self.max_frame = value as usize;
                        }
                        // This side's encoder never uses the dynamic table,
                        // so the peer's table size does not matter; other
                        // settings are not this side's to apply.
                        SETTINGS_HEADER_TABLE_SIZE | SETTINGS_ENABLE_PUSH => {}
                        _ => {}
                    }
                }
                frame(SETTINGS, ACK, 0, &[], &mut self.out);
                if !self.ready {
                    self.ready = true;
                    events.push(Event::Ready);
                }
                self.flush();
            }
            PUSH_PROMISE => return Err("http2: PUSH_PROMISE, which this side turned off".into()),
            PING => {
                if payload.len() != 8 || stream != 0 {
                    return Err("http2: malformed PING".into());
                }
                let mut data = [0u8; 8];
                data.copy_from_slice(&payload);
                if flags & ACK != 0 {
                    events.push(Event::Pong(data));
                } else {
                    frame(PING, ACK, 0, &data, &mut self.out);
                }
            }
            GOAWAY => {
                if payload.len() < 8 {
                    return Err("http2: short GOAWAY".into());
                }
                let last = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]])
                    & 0x7fff_ffff;
                let code = u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]);
                self.going_away = true;
                events.push(Event::GoAway {
                    last_stream: last,
                    code,
                    debug: String::from_utf8_lossy(&payload[8..]).into_owned(),
                });
            }
            WINDOW_UPDATE => {
                if payload.len() != 4 {
                    return Err("http2: WINDOW_UPDATE of the wrong size".into());
                }
                let increment = i64::from(
                    u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]])
                        & 0x7fff_ffff,
                );
                if stream == 0 {
                    self.send_window += increment;
                    if self.send_window > MAX_WINDOW {
                        return Err("http2: connection window overflow".into());
                    }
                } else if let Some(s) = self.streams.get_mut(&stream) {
                    s.send_window += increment;
                }
                self.flush();
            }
            // Unknown frame types are ignored (§4.1).
            _ => {}
        }
        Ok(())
    }

    fn headers_in(
        &mut self,
        stream: u32,
        end_stream: bool,
        block: &[u8],
        events: &mut Vec<Event>,
    ) -> Result<(), String> {
        let fields = self
            .decoder
            .decode(block)?
            .into_iter()
            .map(|(n, v)| {
                (
                    String::from_utf8_lossy(&n).to_ascii_lowercase(),
                    String::from_utf8_lossy(&v).into_owned(),
                )
            })
            .collect();
        if !self.streams.contains_key(&stream) {
            if !self.server || stream.is_multiple_of(2) {
                // A stream this side reset, or one it never opened.
                return Ok(());
            }
            self.streams.insert(
                stream,
                Stream {
                    send_window: self.initial_window,
                    recv_window: STREAM_WINDOW,
                    ..Stream::default()
                },
            );
        }
        events.push(Event::Headers {
            stream,
            fields,
            end_stream,
        });
        if end_stream {
            self.close_remote(stream);
        }
        Ok(())
    }

    /// Grant back the window used, once half of one is used.
    fn grant(&mut self) {
        if self.recv_unacked >= CONNECTION_WINDOW / 2 {
            frame(
                WINDOW_UPDATE,
                0,
                0,
                &(self.recv_unacked as u32).to_be_bytes(),
                &mut self.out,
            );
            self.recv_unacked = 0;
        }
        for (id, s) in self.streams.iter_mut() {
            if s.recv_unacked >= STREAM_WINDOW / 2 && !s.remote_closed {
                frame(
                    WINDOW_UPDATE,
                    0,
                    *id,
                    &(s.recv_unacked as u32).to_be_bytes(),
                    &mut self.out,
                );
                s.recv_window += s.recv_unacked;
                s.recv_unacked = 0;
            }
        }
    }
}

fn frame(kind: u8, flags: u8, stream: u32, payload: &[u8], out: &mut Vec<u8>) {
    let len = payload.len();
    out.extend_from_slice(&[(len >> 16) as u8, (len >> 8) as u8, len as u8, kind, flags]);
    out.extend_from_slice(&(stream & 0x7fff_ffff).to_be_bytes());
    out.extend_from_slice(payload);
}

fn unpad(flags: u8, mut payload: Vec<u8>) -> Result<Vec<u8>, String> {
    if flags & PADDED == 0 {
        return Ok(payload);
    }
    let pad = usize::from(
        *payload
            .first()
            .ok_or("http2: padded frame without a length")?,
    );
    if pad + 1 > payload.len() {
        return Err("http2: padding longer than the frame".into());
    }
    payload.truncate(payload.len() - pad);
    payload.remove(0);
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frames in `bytes`: (type, flags, stream, payload).
    fn frames(mut bytes: &[u8]) -> Vec<(u8, u8, u32, Vec<u8>)> {
        let mut out = Vec::new();
        if bytes.starts_with(PREFACE) {
            bytes = &bytes[PREFACE.len()..];
        }
        while bytes.len() >= 9 {
            let len = (usize::from(bytes[0]) << 16)
                | (usize::from(bytes[1]) << 8)
                | usize::from(bytes[2]);
            let stream = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
            out.push((bytes[3], bytes[4], stream, bytes[9..9 + len].to_vec()));
            bytes = &bytes[9 + len..];
        }
        out
    }

    fn raw(kind: u8, flags: u8, stream: u32, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        frame(kind, flags, stream, payload, &mut out);
        out
    }

    fn ready_client() -> Connection {
        let mut c = Connection::client();
        c.take_output();
        let events = c.receive(&raw(SETTINGS, 0, 0, &[])).unwrap();
        assert_eq!(events, [Event::Ready]);
        c.take_output();
        c
    }

    #[test]
    fn a_client_starts_with_the_preface_settings_and_a_larger_window() {
        let mut c = Connection::client();
        let out = c.take_output();
        assert!(out.starts_with(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"));
        let f = frames(&out);
        // SETTINGS: ENABLE_PUSH 0, INITIAL_WINDOW_SIZE 1 MiB.
        assert_eq!(
            f[0],
            (SETTINGS, 0, 0, vec![0, 2, 0, 0, 0, 0, 0, 4, 0, 0x10, 0, 0])
        );
        // WINDOW_UPDATE on the connection to 16 MiB.
        assert_eq!(
            f[1],
            (
                WINDOW_UPDATE,
                0,
                0,
                ((1u32 << 24) - 65_535).to_be_bytes().to_vec()
            )
        );
        assert!(!c.can_open(), "not before the server's settings");
        // The server's settings are acknowledged.
        let events = c
            .receive(&raw(SETTINGS, 0, 0, &[0, 3, 0, 0, 0, 1]))
            .unwrap();
        assert_eq!(events, [Event::Ready]);
        assert_eq!(frames(&c.take_output()), [(SETTINGS, ACK, 0, vec![])]);
        assert!(c.can_open());
        c.open(&[(":method", "POST")], false);
        assert!(!c.can_open(), "the server allows one stream at a time");
    }

    #[test]
    fn data_waits_for_window_and_splits_into_frames() {
        let mut c = ready_client();
        let id = c.open(&[(":method", "POST")], false);
        assert_eq!(id, 1);
        c.take_output();
        // 70,000 bytes against the default 65,535-byte windows: four frames
        // of 16,384 and one of 1,999, then the rest waits.
        c.send_data(id, vec![7; 70_000], true);
        let f = frames(&c.take_output());
        let sizes: Vec<usize> = f.iter().map(|x| x.3.len()).collect();
        assert_eq!(sizes, [16_384, 16_384, 16_384, 16_383]);
        assert!(f.iter().all(|x| x.0 == DATA && x.1 == 0));
        // A stream window update alone is not enough: the connection's is spent too.
        c.receive(&raw(WINDOW_UPDATE, 0, 1, &10_000u32.to_be_bytes()))
            .unwrap();
        assert!(c.take_output().is_empty());
        c.receive(&raw(WINDOW_UPDATE, 0, 0, &10_000u32.to_be_bytes()))
            .unwrap();
        let f = frames(&c.take_output());
        assert_eq!(f.len(), 1);
        assert_eq!((f[0].1, f[0].3.len()), (END_STREAM, 70_000 - 65_535));
    }

    #[test]
    fn headers_continuations_padding_and_data_are_reported() {
        let mut c = ready_client();
        let id = c.open(&[(":method", "POST")], true);
        let block = hpack::encode(&[(":status", "200"), ("content-type", "application/grpc")]);
        let (a, b) = block.split_at(3);
        let mut bytes = raw(HEADERS, 0, id, a);
        bytes.extend(raw(CONTINUATION, END_HEADERS, id, b));
        // DATA padded with 2 bytes: pad length 2, "hi", two zeros.
        bytes.extend(raw(DATA, PADDED, id, &[2, b'h', b'i', 0, 0]));
        bytes.extend(raw(
            HEADERS,
            END_HEADERS | END_STREAM,
            id,
            &hpack::encode(&[("grpc-status", "0")]),
        ));
        let events = c.receive(&bytes).unwrap();
        assert_eq!(
            events,
            [
                Event::Headers {
                    stream: 1,
                    fields: vec![
                        (":status".into(), "200".into()),
                        ("content-type".into(), "application/grpc".into())
                    ],
                    end_stream: false
                },
                Event::Data {
                    stream: 1,
                    data: b"hi".to_vec(),
                    end_stream: false
                },
                Event::Headers {
                    stream: 1,
                    fields: vec![("grpc-status".into(), "0".into())],
                    end_stream: true
                },
            ]
        );
        assert!(c.can_open(), "the stream closed both ways");
        assert_eq!(c.open_streams(), 0);
    }

    #[test]
    fn a_frame_arriving_in_pieces_is_read_once_whole() {
        let mut c = ready_client();
        c.open(&[(":method", "POST")], true);
        let bytes = raw(DATA, END_STREAM, 1, b"abc");
        assert_eq!(c.receive(&bytes[..5]).unwrap(), []);
        assert_eq!(
            c.receive(&bytes[5..]).unwrap(),
            [Event::Data {
                stream: 1,
                data: b"abc".to_vec(),
                end_stream: true
            }]
        );
    }

    #[test]
    fn pings_resets_and_goaway() {
        let mut c = ready_client();
        c.receive(&raw(PING, 0, 0, &[1, 2, 3, 4, 5, 6, 7, 8]))
            .unwrap();
        assert_eq!(
            frames(&c.take_output()),
            [(PING, ACK, 0, vec![1, 2, 3, 4, 5, 6, 7, 8])]
        );
        c.ping([9; 8]);
        assert_eq!(frames(&c.take_output()), [(PING, 0, 0, vec![9; 8])]);
        assert_eq!(
            c.receive(&raw(PING, ACK, 0, &[9; 8])).unwrap(),
            [Event::Pong([9; 8])]
        );
        let id = c.open(&[(":method", "POST")], false);
        assert_eq!(
            c.receive(&raw(RST_STREAM, 0, id, &CANCEL.to_be_bytes()))
                .unwrap(),
            [Event::Reset {
                stream: id,
                code: CANCEL
            }]
        );
        let mut goaway = 1u32.to_be_bytes().to_vec();
        goaway.extend_from_slice(&NO_ERROR.to_be_bytes());
        goaway.extend_from_slice(b"bye");
        assert_eq!(
            c.receive(&raw(GOAWAY, 0, 0, &goaway)).unwrap(),
            [Event::GoAway {
                last_stream: 1,
                code: 0,
                debug: "bye".into()
            }]
        );
        assert!(!c.can_open());
    }

    #[test]
    fn half_a_window_received_is_granted_back() {
        let mut c = ready_client();
        let id = c.open(&[(":method", "POST")], true);
        c.take_output();
        let chunk = vec![0u8; 16_384];
        let mut bytes = Vec::new();
        for _ in 0..32 {
            bytes.extend(raw(DATA, 0, id, &chunk));
        }
        c.receive(&bytes).unwrap();
        let f = frames(&c.take_output());
        assert_eq!(
            f,
            [(
                WINDOW_UPDATE,
                0,
                id,
                (32u32 * 16_384).to_be_bytes().to_vec()
            )]
        );
    }

    #[test]
    fn protocol_errors_end_the_connection() {
        let mut c = ready_client();
        assert!(c
            .receive(&raw(PUSH_PROMISE, END_HEADERS, 1, &[0, 0, 0, 2]))
            .is_err());
        let mut c = ready_client();
        let mut big = vec![0, 0x40, 1, DATA, 0, 0, 0, 0, 1];
        big.extend(vec![0; 16_385]);
        assert!(
            c.receive(&big).is_err(),
            "a frame above the size this side allows"
        );
    }

    #[test]
    fn a_test_server_and_the_client_talk() {
        let mut client = Connection::client();
        let mut server = Connection::server();
        let pump =
            |from: &mut Connection, to: &mut Connection| to.receive(&from.take_output()).unwrap();
        pump(&mut client, &mut server);
        pump(&mut server, &mut client);
        pump(&mut client, &mut server);
        let id = client.open(&[(":method", "POST"), (":path", "/a.B/C")], false);
        client.send_data(id, b"req".to_vec(), true);
        let events = pump(&mut client, &mut server);
        assert_eq!(events.len(), 2);
        server.send_headers(id, &[(":status", "200")], false);
        server.send_data(id, b"resp".to_vec(), false);
        server.send_headers(id, &[("grpc-status", "0")], true);
        let events = pump(&mut server, &mut client);
        assert_eq!(events.len(), 3);
        assert_eq!(client.open_streams(), 0);
        assert_eq!(server.open_streams(), 0);
    }
}
