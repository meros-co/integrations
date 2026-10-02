//! Continuous media from devices, such as a camera's live view.
//!
//! Frames do not go through the event queue: a 300 KB JPEG thirty times a
//! second would crowd out state patches, and an old frame is worth nothing
//! once a newer one exists. Each watcher instead has a slot holding at most
//! one undelivered frame. A frame arriving before the last was taken replaces
//! it and is counted as dropped, so a slow consumer gets the latest picture,
//! never a backlog, and memory stays bounded at one frame per watcher (frames
//! are shared, so in practice one per stream).
//!
//! A stream runs only while someone watches it: the first watcher and the last
//! to leave wake the device's session, which tells its module through
//! [`crate::module::Module::stream_watch`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::sync::Notify;

use crate::session::DeviceId;

/// One frame of a stream.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// The encoding, as the spec's stream declares it: `jpeg`.
    pub format: &'static str,
    /// The encoded frame, shared by every watcher that receives it.
    pub data: Arc<[u8]>,
    /// Counts every frame the device's stream has published, from 1, so a gap
    /// shows frames this watcher did not see.
    pub sequence: u64,
    /// Frames replaced before this watcher took them, since its last frame.
    pub dropped: u64,
}

/// Why a stream cannot be watched. Serialised like a command error.
#[derive(Debug, Clone, PartialEq, Serialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum StreamError {
    #[error("the device declares no stream '{stream}'")]
    UnknownStream { stream: String },
    #[error("stream '{stream}' is not available on model '{model}'")]
    UnsupportedForModel { stream: String, model: String },
    /// The device is not open, or was closed.
    #[error("session closed")]
    Closed,
}

#[derive(Debug, Clone)]
struct Published {
    format: &'static str,
    data: Arc<[u8]>,
    sequence: u64,
}

#[derive(Default)]
struct SlotState {
    frame: Option<Published>,
    dropped: u64,
    closed: bool,
}

#[derive(Default)]
struct Slot {
    state: Mutex<SlotState>,
    notify: Notify,
}

impl Slot {
    fn offer(&self, frame: Published) {
        {
            let mut s = self.state.lock().unwrap();
            if s.closed {
                return;
            }
            if s.frame.replace(frame).is_some() {
                s.dropped += 1;
            }
        }
        self.notify.notify_waiters();
    }

    fn end(&self) {
        self.state.lock().unwrap().closed = true;
        self.notify.notify_waiters();
    }

    fn take(&self) -> Result<Option<Frame>, ()> {
        let mut s = self.state.lock().unwrap();
        if let Some(p) = s.frame.take() {
            let dropped = std::mem::take(&mut s.dropped);
            return Ok(Some(Frame {
                format: p.format,
                data: p.data,
                sequence: p.sequence,
                dropped,
            }));
        }
        if s.closed {
            Err(())
        } else {
            Ok(None)
        }
    }
}

#[derive(Default)]
struct Channel {
    /// Frames published while watched, ever.
    sequence: u64,
    /// The newest frame, kept only while watched, so a new watcher of a running
    /// stream has a picture at once.
    latest: Option<Published>,
    watchers: HashMap<u64, Arc<Slot>>,
}

struct DeviceStreams {
    /// Wakes the session when the set of watched streams may have changed.
    wake: Arc<Notify>,
    channels: HashMap<String, Channel>,
}

/// Every device's streams. One per core.
#[derive(Default)]
pub(crate) struct Streams {
    devices: Mutex<HashMap<DeviceId, DeviceStreams>>,
    next_watcher: AtomicU64,
}

impl Streams {
    /// A session has started. The returned notify fires when a stream of the
    /// device gains its first watcher or loses its last.
    pub(crate) fn add_device(&self, device: DeviceId) -> Arc<Notify> {
        let wake = Arc::new(Notify::new());
        self.devices.lock().unwrap().insert(
            device,
            DeviceStreams {
                wake: wake.clone(),
                channels: HashMap::new(),
            },
        );
        wake
    }

    /// A session has ended: every watcher's next frame is the end.
    pub(crate) fn remove_device(&self, device: DeviceId) {
        let removed = self.devices.lock().unwrap().remove(&device);
        for channel in removed.into_iter().flat_map(|d| d.channels.into_values()) {
            for slot in channel.watchers.into_values() {
                slot.end();
            }
        }
    }

    /// Watch a stream. `None` if the device has no session.
    pub(crate) fn watch(
        self: &Arc<Self>,
        device: DeviceId,
        stream: &str,
        format: String,
    ) -> Option<StreamHandle> {
        let id = self.next_watcher.fetch_add(1, Ordering::Relaxed);
        let slot = Arc::new(Slot::default());
        {
            let mut devices = self.devices.lock().unwrap();
            let entry = devices.get_mut(&device)?;
            let channel = entry.channels.entry(stream.to_string()).or_default();
            if let Some(latest) = &channel.latest {
                slot.offer(latest.clone());
            }
            channel.watchers.insert(id, slot.clone());
            if channel.watchers.len() == 1 {
                entry.wake.notify_one();
            }
        }
        Some(StreamHandle {
            streams: self.clone(),
            device,
            stream: stream.to_string(),
            format,
            id,
            slot,
            closed: AtomicBool::new(false),
        })
    }

    fn unwatch(&self, device: DeviceId, stream: &str, id: u64) {
        let mut devices = self.devices.lock().unwrap();
        let Some(entry) = devices.get_mut(&device) else {
            return;
        };
        let Some(channel) = entry.channels.get_mut(stream) else {
            return;
        };
        if channel.watchers.remove(&id).is_some() && channel.watchers.is_empty() {
            channel.latest = None;
            entry.wake.notify_one();
        }
    }

    /// Hand a frame to every watcher of the stream. Returns whether anyone
    /// is watching; a frame nobody watches is discarded.
    pub(crate) fn publish(
        &self,
        device: DeviceId,
        stream: &str,
        format: &'static str,
        data: Vec<u8>,
    ) -> bool {
        let mut devices = self.devices.lock().unwrap();
        let Some(channel) = devices
            .get_mut(&device)
            .and_then(|d| d.channels.get_mut(stream))
        else {
            return false;
        };
        if channel.watchers.is_empty() {
            return false;
        }
        channel.sequence += 1;
        let frame = Published {
            format,
            data: data.into(),
            sequence: channel.sequence,
        };
        for slot in channel.watchers.values() {
            slot.offer(frame.clone());
        }
        channel.latest = Some(frame);
        true
    }

    /// The device's streams that have at least one watcher.
    pub(crate) fn watched(&self, device: DeviceId) -> Vec<String> {
        let devices = self.devices.lock().unwrap();
        let mut out: Vec<String> = devices
            .get(&device)
            .map(|d| {
                d.channels
                    .iter()
                    .filter(|(_, c)| !c.watchers.is_empty())
                    .map(|(name, _)| name.clone())
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }
}

/// One consumer watching one stream. The stream runs while any handle for it
/// is open; dropping the handle closes it.
pub struct StreamHandle {
    streams: Arc<Streams>,
    device: DeviceId,
    stream: String,
    format: String,
    id: u64,
    slot: Arc<Slot>,
    closed: AtomicBool,
}

impl std::fmt::Debug for StreamHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamHandle")
            .field("device", &self.device)
            .field("stream", &self.stream)
            .field("format", &self.format)
            .finish()
    }
}

impl StreamHandle {
    pub fn device(&self) -> DeviceId {
        self.device
    }

    pub fn stream(&self) -> &str {
        &self.stream
    }

    /// The format the spec declares for this stream, such as `jpeg`.
    pub fn format(&self) -> &str {
        &self.format
    }

    /// The newest undelivered frame, without waiting.
    pub fn try_frame(&self) -> Option<Frame> {
        self.slot.take().ok().flatten()
    }

    /// Wait for the next frame. `None` once the handle is closed or the
    /// device's session has ended; every later call returns `None` at once.
    /// Safe to call from several tasks; each frame goes to one of them.
    pub async fn next_frame(&self) -> Option<Frame> {
        loop {
            let notified = self.slot.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            match self.slot.take() {
                Ok(Some(frame)) => return Some(frame),
                Ok(None) => {}
                Err(()) => return None,
            }
            notified.await;
        }
    }

    /// Whether the handle is closed or the session ended, with no frame left.
    pub fn is_closed(&self) -> bool {
        let s = self.slot.state.lock().unwrap();
        s.closed && s.frame.is_none()
    }

    /// Stop watching. Wakes a pending [`StreamHandle::next_frame`], which
    /// returns `None`. Idempotent; also done on drop.
    pub fn close(&self) {
        if !self.closed.swap(true, Ordering::AcqRel) {
            {
                let mut s = self.slot.state.lock().unwrap();
                s.closed = true;
                s.frame = None;
            }
            self.slot.notify.notify_waiters();
            self.streams.unwatch(self.device, &self.stream, self.id);
        }
    }
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hub() -> (Arc<Streams>, Arc<Notify>) {
        let streams = Arc::new(Streams::default());
        let wake = streams.add_device(1);
        (streams, wake)
    }

    fn woken(wake: &Notify) -> bool {
        let notified = wake.notified();
        tokio::pin!(notified);
        // A stored permit completes the first poll.
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        notified.as_mut().poll(&mut cx).is_ready()
    }

    use std::future::Future;

    #[test]
    fn latest_frame_wins_and_drops_are_counted() {
        let (streams, _wake) = hub();
        let h = streams.watch(1, "live", "jpeg".into()).unwrap();
        for n in 1..=3u8 {
            assert!(streams.publish(1, "live", "jpeg", vec![n]));
        }
        let f = h.try_frame().unwrap();
        assert_eq!(&*f.data, &[3]);
        assert_eq!(f.sequence, 3);
        assert_eq!(f.dropped, 2);
        assert_eq!(h.try_frame(), None);
        streams.publish(1, "live", "jpeg", vec![4]);
        let f = h.try_frame().unwrap();
        assert_eq!((f.sequence, f.dropped), (4, 0));
    }

    #[test]
    fn each_watcher_has_its_own_slot() {
        let (streams, _wake) = hub();
        let fast = streams.watch(1, "live", "jpeg".into()).unwrap();
        let slow = streams.watch(1, "live", "jpeg".into()).unwrap();
        streams.publish(1, "live", "jpeg", vec![1]);
        assert_eq!(fast.try_frame().unwrap().dropped, 0);
        streams.publish(1, "live", "jpeg", vec![2]);
        assert_eq!(fast.try_frame().unwrap().dropped, 0);
        let f = slow.try_frame().unwrap();
        assert_eq!((&*f.data, f.dropped), (&[2u8][..], 1));
    }

    #[test]
    fn unwatched_frames_are_discarded() {
        let (streams, _wake) = hub();
        assert!(!streams.publish(1, "live", "jpeg", vec![1]));
        let h = streams.watch(1, "live", "jpeg".into()).unwrap();
        assert_eq!(h.try_frame(), None);
        h.close();
        assert!(!streams.publish(1, "live", "jpeg", vec![2]));
    }

    #[test]
    fn a_new_watcher_of_a_running_stream_gets_the_latest_frame() {
        let (streams, _wake) = hub();
        let first = streams.watch(1, "live", "jpeg".into()).unwrap();
        streams.publish(1, "live", "jpeg", vec![7]);
        let second = streams.watch(1, "live", "jpeg".into()).unwrap();
        assert_eq!(&*second.try_frame().unwrap().data, &[7]);
        drop(first);
        drop(second);
        // Nobody watching: the latest frame is not kept.
        let third = streams.watch(1, "live", "jpeg".into()).unwrap();
        assert_eq!(third.try_frame(), None);
    }

    #[test]
    fn first_and_last_watcher_wake_the_session() {
        let (streams, wake) = hub();
        assert!(!woken(&wake));
        let a = streams.watch(1, "live", "jpeg".into()).unwrap();
        assert!(woken(&wake));
        assert_eq!(streams.watched(1), vec!["live".to_string()]);
        let b = streams.watch(1, "live", "jpeg".into()).unwrap();
        assert!(!woken(&wake));
        drop(a);
        assert!(!woken(&wake));
        assert_eq!(streams.watched(1), vec!["live".to_string()]);
        drop(b);
        assert!(woken(&wake));
        assert!(streams.watched(1).is_empty());
    }

    #[test]
    fn unknown_device_cannot_be_watched() {
        let streams = Arc::new(Streams::default());
        assert!(streams.watch(9, "live", "jpeg".into()).is_none());
    }

    #[tokio::test]
    async fn next_frame_waits_and_ends_with_the_session() {
        let (streams, _wake) = hub();
        let h = Arc::new(streams.watch(1, "live", "jpeg".into()).unwrap());
        let waiter = {
            let h = h.clone();
            tokio::spawn(async move { h.next_frame().await })
        };
        tokio::task::yield_now().await;
        streams.publish(1, "live", "jpeg", vec![5]);
        assert_eq!(&*waiter.await.unwrap().unwrap().data, &[5]);

        let waiter = {
            let h = h.clone();
            tokio::spawn(async move { h.next_frame().await })
        };
        tokio::task::yield_now().await;
        streams.remove_device(1);
        assert_eq!(waiter.await.unwrap(), None);
        assert!(h.is_closed());
        assert_eq!(h.next_frame().await, None);
    }

    #[tokio::test]
    async fn close_wakes_a_pending_wait() {
        let (streams, _wake) = hub();
        let h = Arc::new(streams.watch(1, "live", "jpeg".into()).unwrap());
        let waiter = {
            let h = h.clone();
            tokio::spawn(async move { h.next_frame().await })
        };
        tokio::task::yield_now().await;
        h.close();
        assert_eq!(waiter.await.unwrap(), None);
        assert!(streams.watched(1).is_empty());
    }

    /// A module recording what it is told, publishing a frame when watched.
    struct Recorder(Arc<Mutex<Vec<(String, bool)>>>);

    impl crate::module::Module for Recorder {
        fn start(&mut self, _cx: &mut crate::module::Cx) {}
        fn command(
            &mut self,
            _cx: &mut crate::module::Cx,
            _id: crate::module::CommandId,
            _name: &str,
            _params: &crate::catalog::Params,
        ) {
        }
        fn timer(&mut self, _cx: &mut crate::module::Cx, _key: crate::module::Key) {}
        fn stream_watch(&mut self, cx: &mut crate::module::Cx, stream: &str, watching: bool) {
            self.0.lock().unwrap().push((stream.to_string(), watching));
            if watching {
                cx.frame("live", "jpeg", vec![0xFF, 0xD8]);
            }
        }
    }

    #[tokio::test]
    async fn the_session_tells_the_module_when_a_stream_is_watched() {
        use crate::events::EventQueue;
        use crate::session::{DeviceSnapshot, Services, Session, SessionMsg};

        let events = Arc::new(EventQueue::new(16));
        let services = Arc::new(Services {
            events: events.clone(),
            bind_address: "127.0.0.1".parse().unwrap(),
            shared_udp: crate::udp::SharedUdp::new(events),
            shared_tcp: Default::default(),
            http: crate::http::HttpClients::new().unwrap(),
            streams: Default::default(),
        });
        let calls = Arc::new(Mutex::new(Vec::new()));
        let snapshot = Arc::new(Mutex::new(DeviceSnapshot {
            connection: crate::module::Connection::Connecting,
            state: serde_json::Value::Null,
        }));
        let session = Session::new(
            7,
            "127.0.0.1".parse().unwrap(),
            Box::new(Recorder(calls.clone())),
            services.clone(),
            snapshot,
        );
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        let task = tokio::spawn(session.run(rx));

        let a = services.streams.watch(7, "live", "jpeg".into()).unwrap();
        let b = services.streams.watch(7, "live", "jpeg".into()).unwrap();
        let frame = tokio::time::timeout(std::time::Duration::from_secs(5), a.next_frame())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&*frame.data, &[0xFF, 0xD8]);
        assert!(b.try_frame().is_some());
        drop(a);
        drop(b);
        let (done, finished) = tokio::sync::oneshot::channel();
        // Let the session see the last watcher leave before closing.
        for _ in 0..100 {
            if calls.lock().unwrap().len() == 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let late = services.streams.watch(7, "live", "jpeg".into()).unwrap();
        tx.send(SessionMsg::Close { done }).await.ok().unwrap();
        finished.await.unwrap();
        task.await.unwrap();

        // One call per change, not per watcher.
        let calls = calls.lock().unwrap().clone();
        assert_eq!(calls[..2], [("live".into(), true), ("live".into(), false)]);
        // A watcher left open when the session ends sees the end.
        while late.next_frame().await.is_some() {}
        assert!(late.is_closed());
    }
}
