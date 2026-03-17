use crate::events::TrafficRecord;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub struct TrafficBuffer {
    entries: VecDeque<TrafficRecord>,
    capacity: usize,
}

impl TrafficBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, entry: TrafficRecord) {
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn list(&self) -> Vec<TrafficSummary> {
        self.entries
            .iter()
            .rev()
            .map(|e| TrafficSummary {
                id: e.id,
                timestamp: e.timestamp.clone(),
                method: e.method.clone(),
                url: e.url.clone(),
                status: e.status,
                duration_ms: e.duration_ms,
                rule_id: e.rule_id.clone(),
            })
            .collect()
    }

    pub fn get(&self, id: u64) -> Option<&TrafficRecord> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[derive(serde::Serialize)]
pub struct TrafficSummary {
    pub id: u64,
    pub timestamp: String,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub duration_ms: u64,
    pub rule_id: Option<String>,
}

// shared state for the proxy handler to check without locking
pub static CAPTURE_ENABLED: AtomicBool = AtomicBool::new(false);
pub static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

pub fn is_capture_enabled() -> bool {
    CAPTURE_ENABLED.load(Ordering::Relaxed)
}

pub fn set_capture_enabled(enabled: bool) {
    CAPTURE_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn next_id() -> u64 {
    NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::TrafficRecord;

    fn make_record(id: u64) -> TrafficRecord {
        TrafficRecord {
            id,
            timestamp: format!("00:00:00.{:03}", id),
            method: "GET".to_string(),
            url: format!("https://example.com/{}", id),
            status: 200,
            duration_ms: 10,
            rule_id: None,
            request_headers: vec![],
            response_headers: vec![],
        }
    }

    #[test]
    fn new_buffer_is_empty() {
        let buf = TrafficBuffer::new(10);
        assert!(buf.list().is_empty());
        assert!(buf.get(1).is_none());
    }

    #[test]
    fn push_and_list_returns_entry() {
        let mut buf = TrafficBuffer::new(10);
        buf.push(make_record(1));
        let list = buf.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, 1);
        assert_eq!(list[0].method, "GET");
    }

    #[test]
    fn list_returns_entries_in_reverse_order() {
        let mut buf = TrafficBuffer::new(10);
        buf.push(make_record(1));
        buf.push(make_record(2));
        buf.push(make_record(3));
        let list = buf.list();
        assert_eq!(list[0].id, 3);
        assert_eq!(list[1].id, 2);
        assert_eq!(list[2].id, 1);
    }

    #[test]
    fn get_returns_correct_entry() {
        let mut buf = TrafficBuffer::new(10);
        buf.push(make_record(1));
        buf.push(make_record(2));
        let entry = buf.get(2).unwrap();
        assert_eq!(entry.id, 2);
        assert_eq!(entry.url, "https://example.com/2");
    }

    #[test]
    fn get_missing_id_returns_none() {
        let mut buf = TrafficBuffer::new(10);
        buf.push(make_record(1));
        assert!(buf.get(999).is_none());
    }

    #[test]
    fn clear_empties_buffer() {
        let mut buf = TrafficBuffer::new(10);
        buf.push(make_record(1));
        buf.push(make_record(2));
        buf.clear();
        assert!(buf.list().is_empty());
        assert!(buf.get(1).is_none());
    }

    #[test]
    fn evicts_oldest_at_capacity() {
        let mut buf = TrafficBuffer::new(3);
        buf.push(make_record(1));
        buf.push(make_record(2));
        buf.push(make_record(3));
        buf.push(make_record(4));
        let list = buf.list();
        assert_eq!(list.len(), 3);
        // oldest (id=1) should be gone
        assert!(buf.get(1).is_none());
        assert!(buf.get(4).is_some());
    }

    #[test]
    fn exact_capacity_does_not_evict() {
        let mut buf = TrafficBuffer::new(3);
        buf.push(make_record(1));
        buf.push(make_record(2));
        buf.push(make_record(3));
        assert_eq!(buf.list().len(), 3);
        assert!(buf.get(1).is_some());
    }

    #[test]
    fn capacity_one_holds_latest() {
        let mut buf = TrafficBuffer::new(1);
        buf.push(make_record(1));
        buf.push(make_record(2));
        assert_eq!(buf.list().len(), 1);
        assert_eq!(buf.list()[0].id, 2);
    }

    #[test]
    fn capture_toggle() {
        set_capture_enabled(true);
        assert!(is_capture_enabled());
        set_capture_enabled(false);
        assert!(!is_capture_enabled());
    }

    #[test]
    fn next_id_increments() {
        let a = next_id();
        let b = next_id();
        assert_eq!(b, a + 1);
    }
}
