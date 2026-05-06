use crate::plan::NodeId;
use crate::state::NodeState;

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
