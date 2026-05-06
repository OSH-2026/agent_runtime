use crate::plan::NodeId;
use crate::recovery::RecoveryLevel;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct RecoveryState {
    pub level: RecoveryLevel,
    pub retries_used: u32,
}

#[derive(Default)]
pub struct EscalationGuard {
    pub state: HashMap<NodeId, RecoveryState>,
}

impl EscalationGuard {
    pub fn register_failure(&mut self, node_id: &NodeId, retry_budget: u32) -> RecoveryLevel {
        let entry = self.state.entry(node_id.clone()).or_insert(RecoveryState {
            level: RecoveryLevel::Retry,
            retries_used: 0,
        });
        if entry.level != RecoveryLevel::Retry {
            return entry.level;
        }
        if entry.retries_used < retry_budget {
            entry.retries_used += 1;
            RecoveryLevel::Retry
        } else {
            entry.level = RecoveryLevel::Patch;
            RecoveryLevel::Patch
        }
    }
}
