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
    pub fn next_level(&mut self, node_id: &NodeId) -> RecoveryLevel {
        let entry = self.state.entry(node_id.clone()).or_insert(RecoveryState {
            level: RecoveryLevel::Retry,
            retries_used: 0,
        });
        match entry.level {
            RecoveryLevel::Retry => {
                entry.retries_used += 1;
                RecoveryLevel::Retry
            }
            RecoveryLevel::Patch => RecoveryLevel::Patch,
            RecoveryLevel::Replan => RecoveryLevel::Replan,
        }
    }
}
