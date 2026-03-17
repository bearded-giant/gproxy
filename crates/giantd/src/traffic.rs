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
