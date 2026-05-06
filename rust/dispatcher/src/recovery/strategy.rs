use crate::executor::ExecutionResult;
use crate::plan::{ExecutionPlan, NodeId};
use crate::runtime::DiagnosticContext;

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
    fn handle_failure(
        &mut self,
        result: &ExecutionResult,
        plan: &ExecutionPlan,
        diagnostic: &mut DiagnosticContext,
    ) -> RecoveryAction;
}
