use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ProxyEvent {
    RequestMatched {
        rule_id: String,
        url: String,
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
