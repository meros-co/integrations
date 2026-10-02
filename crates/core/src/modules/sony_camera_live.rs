//! The `live` stream: continuous live view while someone watches it.
//!
//! PTP bodies serve one live view image per GetObject of the live view
//! handle, the same request `get_live_view_image` makes (Camera Control PTP 3
//! and PTP 2 References, Live View; GetObject of the live view object). The
//! stream asks for the next image as soon as the last has arrived, with one
//! request outstanding at most, starts no faster than `live_interval_ms`
//! apart, and backs off briefly when the camera has no new image yet (an empty
//! dataset or Access_Denied, which is how it says "too soon"). Frame requests
//! are background work: an operator command always goes first.
//!
//! Pan/tilt bodies serve live view over HTTP on their own localhost at the
//! LiveViewURL, one live view dataset per HTTP chunk. The stream keeps that
//! response open through the SSH tunnel, publishes each image as its chunk
//! completes, and opens it again if it ends while watched.

use super::*;

/// The stream this module publishes.
pub(super) const LIVE: &str = "live";
/// Ask the camera for the next image.
pub(super) const LIVE_NEXT: Key = "live-next";
/// The tunnelled HTTP live view of a pan/tilt body.
pub(super) const LIVE_HTTP: Key = "live-http";
/// No data on that connection for too long, or time to open it again.
pub(super) const LIVE_HTTP_WAIT: Key = "live-http-wait";

/// The default shortest time between two image requests: the 30 frames a
/// second the references give as the most a camera serves.
pub(super) const DEFAULT_INTERVAL: Millis = 33;
/// Back-off after an empty answer: 40 ms, doubling to this.
const EMPTY_BACKOFF_MAX: Millis = 320;
/// After any other refusal of a frame request.
const ERROR_BACKOFF: Millis = 1_000;
const HTTP_STALL: Millis = 10_000;
const RECONNECT_MIN: Millis = 1_000;
const RECONNECT_MAX: Millis = 30_000;
/// The most one live view dataset may take before it is discarded.
const MAX_DATASET: usize = 16 * 1024 * 1024;

struct LiveHttp {
    reader: HttpReader,
    body: Vec<u8>,
    chunked: bool,
}

/// The stream's state, kept apart from the command machinery.
pub(super) struct Live {
    /// Someone watches the stream.
    pub(super) watched: bool,
    /// The shortest time between the starts of two image requests.
    pub(super) interval: Millis,
    /// An image request is queued or in flight.
    pending: bool,
    last_request_at: Millis,
    empties: u32,
    /// This stream turned the camera's live view on (remote with transfer)
    /// and turns it off again when nobody watches.
    enabled_here: bool,
    http: Option<LiveHttp>,
    reconnect_after: Millis,
    status: &'static str,
    /// Frames published since the stream last started.
    pub(super) published: u64,
}

impl Live {
    pub(super) fn new(interval: Millis) -> Live {
        Live {
            watched: false,
            interval,
            pending: false,
            last_request_at: 0,
            empties: 0,
            enabled_here: false,
            http: None,
            reconnect_after: RECONNECT_MIN,
            status: "stopped",
            published: 0,
        }
    }
}

/// The byte range of the first whole JPEG in `data`.
fn jpeg_range(data: &[u8]) -> Option<(usize, usize)> {
    let jpeg = content::find_jpeg(data)?;
    let start = jpeg.as_ptr() as usize - data.as_ptr() as usize;
    Some((start, start + jpeg.len()))
}

impl SonyCamera {
    fn live_status(&mut self, cx: &mut Cx, status: &'static str) {
        if self.live.status != status {
            self.live.status = status;
            cx.state(json!({"live_view": {"stream": status}}));
        }
    }

    /// The host started or stopped watching.
    pub(super) fn live_watch(&mut self, cx: &mut Cx, watching: bool) {
        if watching == self.live.watched {
            return;
        }
        self.live.watched = watching;
        if watching {
            self.live.published = 0;
            if self.ready {
                self.live_start(cx);
            } else {
                self.live_status(cx, "waiting");
            }
        } else {
            self.live_stop(cx);
        }
    }

    /// Starts producing frames: on connecting while watched, or on the first
    /// watcher.
    pub(super) fn live_start(&mut self, cx: &mut Cx) {
        if !self.live.watched {
            return;
        }
        self.live.empties = 0;
        if let Some(url) = self.text_prop(0xD278).and_then(|u| parse_url(&u)) {
            if self.mode != Mode::Ssh {
                cx.log(
                    Level::Warning,
                    "this camera serves live view over HTTP on its own localhost: open it with connection ssh to watch it",
                );
                return self.live_status(cx, "needs_ssh");
            }
            if self.live.http.is_none() {
                self.live_http_open(cx, &url);
            }
            return;
        }
        if self.function == Function::ContentTransfer {
            return self.live_status(cx, "unavailable");
        }
        if self.function == Function::RemoteWithTransfer
            && !self.live_view_enabled
            && self.has_control(0xD313)
        {
            if let Ok(op) = self.control(0xD313, DOWN) {
                self.background.push_back(op);
                self.live.enabled_here = true;
            }
        }
        self.live_status(cx, "running");
        self.live_request(cx);
    }

    /// Queues the next image request, unless one is already queued or in
    /// flight.
    pub(super) fn live_request(&mut self, cx: &mut Cx) {
        if !self.live.watched || self.live.pending || !self.ready || self.live.http.is_some() {
            return;
        }
        self.live.pending = true;
        self.live.last_request_at = cx.now();
        self.background.push_back(Op::new(
            OP_GET_OBJECT,
            vec![LIVE_VIEW_HANDLE],
            Step::LiveFrame,
        ));
        self.pump(cx);
    }

    /// An image request was answered.
    pub(super) fn live_frame_done(&mut self, cx: &mut Cx, code: u16, data: &[u8]) {
        self.live.pending = false;
        if !self.live.watched {
            return;
        }
        let mut wait: Millis = 0;
        match (code, content::parse_live_view(data)) {
            (RC_OK, Ok(Some(frame))) => {
                self.live.empties = 0;
                self.live.published += 1;
                cx.frame(LIVE, "jpeg", frame.jpeg);
                self.live_status(cx, "running");
            }
            (RC_OK, Ok(None)) | (RC_ACCESS_DENIED, _) => {
                // No new image yet.
                self.live.empties += 1;
                wait = (LIVE_VIEW_RETRY << (self.live.empties - 1).min(3)).min(EMPTY_BACKOFF_MAX);
            }
            (RC_OK, Err(e)) => {
                cx.log(Level::Debug, format!("unreadable live view image: {e}"));
                wait = ERROR_BACKOFF;
            }
            (other, _) => {
                cx.log(
                    Level::Debug,
                    format!("live view image refused: {}", props::response_name(other)),
                );
                wait = ERROR_BACKOFF;
            }
        }
        let paced = (self.live.last_request_at + self.live.interval).saturating_sub(cx.now());
        let wait = wait.max(paced);
        if wait == 0 {
            self.live_request(cx);
        } else {
            cx.set_timer(LIVE_NEXT, wait);
        }
    }

    /// Nobody watches any more: stop asking, and turn off what the stream
    /// turned on.
    pub(super) fn live_stop(&mut self, cx: &mut Cx) {
        cx.cancel_timer(LIVE_NEXT);
        cx.cancel_timer(LIVE_HTTP_WAIT);
        let before = self.background.len();
        self.background.retain(|op| op.step != Step::LiveFrame);
        if self.background.len() != before {
            // Queued, not sent: nothing will answer it.
            self.live.pending = false;
        }
        if self.live.http.take().is_some() {
            cx.tcp_close(LIVE_HTTP);
        }
        if self.live.enabled_here && self.ready {
            if let Ok(op) = self.control(0xD313, UP) {
                self.background.push_back(op);
                self.live_view_enabled = false;
                self.pump(cx);
            }
        }
        self.live.enabled_here = false;
        self.live.reconnect_after = RECONNECT_MIN;
        self.live_status(cx, "stopped");
    }

    /// The PTP session ended; the stream starts again when it is back. The
    /// pan/tilt HTTP stream has its own connection and carries on.
    pub(super) fn live_session_lost(&mut self, cx: &mut Cx) {
        self.live.pending = false;
        self.live.enabled_here = false;
        cx.cancel_timer(LIVE_NEXT);
        if self.live.watched && self.live.http.is_none() {
            self.live_status(cx, "waiting");
        }
    }

    // ----- pan/tilt bodies: the tunnelled HTTP stream -----

    fn live_http_open(&mut self, cx: &mut Cx, url: &Url) {
        cx.tcp_open_ssh(LIVE_HTTP, self.tunnel(&url.host, url.port));
        cx.set_timer(LIVE_HTTP_WAIT, HTTP_STALL);
        self.live.http = Some(LiveHttp {
            reader: HttpReader::default(),
            body: Vec::new(),
            chunked: false,
        });
        self.live_status(cx, "running");
    }

    /// The HTTP stream ended or stalled: open it again later while watched.
    fn live_http_drop(&mut self, cx: &mut Cx, reason: &str) {
        if self.live.http.take().is_none() {
            return;
        }
        cx.tcp_close(LIVE_HTTP);
        if !self.live.watched {
            return;
        }
        cx.log(
            Level::Debug,
            format!(
                "live view HTTP stream ended ({reason}); opening it again in {} ms",
                self.live.reconnect_after
            ),
        );
        cx.set_timer(LIVE_HTTP_WAIT, self.live.reconnect_after);
        self.live.reconnect_after = (self.live.reconnect_after * 2).min(RECONNECT_MAX);
        self.live_status(cx, "reconnecting");
    }

    pub(super) fn live_http_timer(&mut self, cx: &mut Cx) {
        if self.live.http.is_some() {
            return self.live_http_drop(cx, "no data for 10 s");
        }
        if self.live.watched {
            if let Some(url) = self.text_prop(0xD278).and_then(|u| parse_url(&u)) {
                self.live_http_open(cx, &url);
            }
        }
    }

    pub(super) fn live_http_input(&mut self, cx: &mut Cx, input: TcpInput) {
        match input {
            TcpInput::Connected => {
                if self.live.http.is_some() {
                    if let Some(url) = self.text_prop(0xD278).and_then(|u| parse_url(&u)) {
                        cx.tcp_send(LIVE_HTTP, url.request());
                    }
                }
            }
            TcpInput::Data(data) => {
                let Some(http) = self.live.http.as_mut() else {
                    return;
                };
                cx.set_timer(LIVE_HTTP_WAIT, HTTP_STALL);
                match http.reader.feed(&data) {
                    Ok(events) => self.live_http_events(cx, events),
                    Err(e) => self.live_http_drop(cx, &e),
                }
            }
            TcpInput::Closed { reason } => {
                if reason.starts_with(crate::ssh::REFUSED) {
                    // A refused login is not retried on a schedule.
                    self.live.http = None;
                    cx.cancel_timer(LIVE_HTTP_WAIT);
                    cx.log(
                        Level::Warning,
                        format!("live view stream refused: {reason}"),
                    );
                    return self.live_status(cx, "refused");
                }
                self.live_http_drop(cx, &reason);
            }
        }
    }

    fn live_http_events(&mut self, cx: &mut Cx, events: Vec<HttpEvent>) {
        for event in events {
            let Some(http) = self.live.http.as_mut() else {
                return;
            };
            match event {
                HttpEvent::Head { status, length, .. } => {
                    if status != 200 {
                        return self.live_http_drop(cx, &format!("HTTP {status}"));
                    }
                    http.chunked = length.is_none();
                }
                HttpEvent::Body(bytes) => {
                    http.body.extend_from_slice(&bytes);
                    let mut whole = Vec::new();
                    if !http.chunked {
                        // Not one dataset per chunk: take each whole JPEG.
                        while let Some((start, end)) = jpeg_range(&http.body) {
                            whole.push(http.body[start..end].to_vec());
                            http.body.drain(..end);
                        }
                    }
                    if http.body.len() > MAX_DATASET {
                        http.body.clear();
                    }
                    for jpeg in whole {
                        self.live_publish(cx, jpeg);
                    }
                }
                HttpEvent::ChunkEnd => {
                    let body = std::mem::take(&mut http.body);
                    let jpeg = match content::parse_live_view(&body) {
                        Ok(Some(frame)) => Some(frame.jpeg),
                        _ => content::find_jpeg(&body).map(<[u8]>::to_vec),
                    };
                    if let Some(jpeg) = jpeg {
                        self.live_publish(cx, jpeg);
                    }
                }
                HttpEvent::End => return self.live_http_drop(cx, "the response ended"),
            }
        }
    }

    fn live_publish(&mut self, cx: &mut Cx, jpeg: Vec<u8>) {
        if !self.live.watched {
            return;
        }
        self.live.reconnect_after = RECONNECT_MIN;
        self.live.published += 1;
        cx.frame(LIVE, "jpeg", jpeg);
        self.live_status(cx, "running");
    }
}
