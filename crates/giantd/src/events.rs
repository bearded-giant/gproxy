use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ProxyEvent {
    RequestMatched {
        rule_id: String,
        url: String,
        method: String,
    },
    RequestPassthrough {
        url: String,
        method: String,
    },
    RuleToggled {
        rule_id: String,
        enabled: bool,
    },
    ProfileSwitched {
        profile: String,
        rules_loaded: usize,
    },
    ProxyStarted {
        listen_addr: String,
        profile: String,
    },
    ProxyStopped,
    ConfigChanged,
    TrafficEntry(TrafficRecord),
    TrafficCaptureChanged {
        enabled: bool,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct TrafficRecord {
    pub id: u64,
    pub timestamp: String,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub duration_ms: u64,
    pub rule_id: Option<String>,
    pub request_headers: Vec<(String, String)>,
    pub response_headers: Vec<(String, String)>,
}

pub struct EventBus {
    tx: broadcast::Sender<ProxyEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn send(&self, event: ProxyEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ProxyEvent> {
        self.tx.subscribe()
    }

    pub fn sender(&self) -> broadcast::Sender<ProxyEvent> {
        self.tx.clone()
    }
}
