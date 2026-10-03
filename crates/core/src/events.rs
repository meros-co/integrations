//! Events from every device, queued for the consumer to drain.
//!
//! One model for every delivery: the core queues, each binding drains in its
//! host's idiom. Nothing calls into a host from the core's threads.

use std::collections::{BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use serde_json::Value;
use tokio::sync::Notify;

use crate::module::{Connection, Level};
use crate::session::DeviceId;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Connection {
        device: DeviceId,
        connection: Connection,
    },
    /// An RFC 7386 merge patch against the device's state.
    State { device: DeviceId, patch: Value },
    /// The device was heard from. At most one per device per second.
    /// `latency_ms` is the device's most recent request-to-reply time, where
    /// its protocol answers requests: what a ping would measure.
    Alive {
        device: DeviceId,
        #[serde(skip_serializing_if = "Option::is_none")]
        latency_ms: Option<u64>,
    },
    Log {
        device: DeviceId,
        level: Level,
        message: String,
    },
    /// The device's session has ended after `close`.
    Closed { device: DeviceId },
    /// State patches were discarded because the consumer fell behind. Read
    /// each affected device's snapshot to resynchronise.
    Dropped { count: u64 },
    /// Discovery found a device, or learned more about one (its name, or
    /// whether it is a receiver or a transmitter). `models` are the models it
    /// can be; more than one when the protocol cannot tell them apart.
    /// `evidence` says what each part of the identification rests on.
    Discovered {
        protocol: String,
        address: String,
        port: u16,
        device: String,
        models: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        evidence: Value,
    },
    /// Something about discovery itself, such as a scan bounded short of the
    /// whole network.
    Discovery { protocol: String, message: String },
}

/// Bounded so a stalled consumer cannot grow memory without limit. On
/// overflow, state patches are discarded oldest first and counted; every other
/// event is kept, because a missed connection change cannot be recovered from a
/// snapshot's history.
pub struct EventQueue {
    queue: Mutex<VecDeque<Event>>,
    notify: Notify,
    dropped: AtomicU64,
    interrupted: AtomicBool,
    capacity: usize,
    /// The specs `Discovered` events may name; every spec when absent.
    discoverable: Option<BTreeSet<String>>,
}

impl EventQueue {
    pub fn new(capacity: usize) -> EventQueue {
        EventQueue {
            queue: Mutex::new(VecDeque::new()),
            notify: Notify::new(),
            dropped: AtomicU64::new(0),
            interrupted: AtomicBool::new(false),
            capacity,
            discoverable: None,
        }
    }

    /// Drop `Discovered` events for any spec not in `devices`, so a core
    /// started for some devices never reports others.
    pub(crate) fn discovering_only(mut self, devices: BTreeSet<String>) -> EventQueue {
        self.discoverable = Some(devices);
        self
    }

    pub(crate) fn push(&self, event: Event) {
        if let (Event::Discovered { device, .. }, Some(allowed)) = (&event, &self.discoverable) {
            if !allowed.contains(device) {
                return;
            }
        }
        {
            let mut q = self.queue.lock().unwrap();
            if q.len() >= self.capacity {
                if let Some(pos) = q.iter().position(|e| matches!(e, Event::State { .. })) {
                    q.remove(pos);
                    self.dropped.fetch_add(1, Ordering::Relaxed);
                }
            }
            q.push_back(event);
        }
        self.notify.notify_one();
    }

    /// Take up to `max` queued events without waiting.
    pub fn drain(&self, max: usize) -> Vec<Event> {
        let mut out = Vec::new();
        let dropped = self.dropped.swap(0, Ordering::Relaxed);
        if dropped > 0 {
            out.push(Event::Dropped { count: dropped });
        }
        let mut q = self.queue.lock().unwrap();
        let n = max.min(q.len());
        out.extend(q.drain(..n));
        out
    }

    /// Make a pending or the next call to [`EventQueue::next`] return, empty if
    /// nothing is queued, so a consumer waiting on events can stop.
    pub fn interrupt(&self) {
        self.interrupted.store(true, Ordering::Release);
        self.notify.notify_one();
    }

    /// Wait until at least one event is queued, then take up to `max`. Returns
    /// empty only after [`EventQueue::interrupt`].
    pub async fn next(&self, max: usize) -> Vec<Event> {
        loop {
            let notified = self.notify.notified();
            let batch = self.drain(max);
            if !batch.is_empty() || self.interrupted.swap(false, Ordering::AcqRel) {
                return batch;
            }
            notified.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn discovered(device: &str) -> Event {
        Event::Discovered {
            protocol: "test".into(),
            address: "192.0.2.1".into(),
            port: 1,
            device: device.into(),
            models: vec![],
            name: None,
            evidence: json!({}),
        }
    }

    #[test]
    fn discovered_events_are_only_for_the_selected_devices() {
        let q = EventQueue::new(10).discovering_only(["pjlink".to_string()].into());
        q.push(discovered("sony-camera"));
        q.push(discovered("pjlink"));
        q.push(Event::Closed { device: 1 });
        assert_eq!(
            q.drain(10),
            [discovered("pjlink"), Event::Closed { device: 1 }]
        );

        let unfiltered = EventQueue::new(10);
        unfiltered.push(discovered("sony-camera"));
        assert_eq!(unfiltered.drain(10).len(), 1);
    }

    #[test]
    fn overflow_drops_state_patches_not_connection_changes() {
        let q = EventQueue::new(2);
        q.push(Event::Connection {
            device: 1,
            connection: Connection::Connected,
        });
        q.push(Event::State {
            device: 1,
            patch: json!({"a": 1}),
        });
        q.push(Event::State {
            device: 1,
            patch: json!({"a": 2}),
        });
        let events = q.drain(10);
        assert_eq!(events[0], Event::Dropped { count: 1 });
        assert!(matches!(events[1], Event::Connection { .. }));
        assert_eq!(
            events[2],
            Event::State {
                device: 1,
                patch: json!({"a": 2})
            }
        );
    }
}
