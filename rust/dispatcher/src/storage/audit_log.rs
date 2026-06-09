use crate::plan::NodeId;
use crate::state::NodeState;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct AuditEvent {
    pub node_id: NodeId,
    pub from: NodeState,
    pub to: NodeState,
    pub plan_version: u64,
}

pub trait AuditLog: Send + Sync {
    fn record(&mut self, event: AuditEvent);
}

#[derive(Clone)]
pub struct InMemoryAuditLog {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl InMemoryAuditLog {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn events(&self) -> Vec<AuditEvent> {
        self.events
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }
}

impl Default for InMemoryAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLog for InMemoryAuditLog {
    fn record(&mut self, event: AuditEvent) {
        if let Ok(mut guard) = self.events.lock() {
            guard.push(event);
        }
    }
}
