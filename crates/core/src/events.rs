//! Events from every device, queued for the consumer to drain.
//!
//! One model for every delivery: the core queues, each binding drains in its
//! host's idiom. Nothing calls into a host from the core's threads.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
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
    Alive { device: DeviceId },
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
}

/// Bounded so a stalled consumer cannot grow memory without limit. On
/// overflow, state patches are discarded oldest first and counted; every other
/// event is kept, because a missed connection change cannot be recovered from a
/// snapshot's history.
pub struct EventQueue {
    queue: Mutex<VecDeque<Event>>,
    notify: Notify,
    dropped: AtomicU64,
    capacity: usize,
}

impl EventQueue {
    pub fn new(capacity: usize) -> EventQueue {
        EventQueue {
            queue: Mutex::new(VecDeque::new()),
            notify: Notify::new(),
            dropped: AtomicU64::new(0),
            capacity,
        }
    }

    pub(crate) fn push(&self, event: Event) {
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

    /// Wait until at least one event is queued, then take up to `max`.
    pub async fn next(&self, max: usize) -> Vec<Event> {
        loop {
            let notified = self.notify.notified();
            let batch = self.drain(max);
            if !batch.is_empty() {
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
