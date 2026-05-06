use crate::executor::ExecutionResult;
use crate::plan::NodeId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryLevel {
    Retry,
    Patch,
    Replan,
}

#[derive(Clone, Debug)]
pub struct RecoveryAction {
    pub node_id: NodeId,
    pub level: RecoveryLevel,
}

pub trait RecoveryStrategy: Send + Sync {
    fn handle_failure(&self, result: &ExecutionResult) -> RecoveryAction;
}
