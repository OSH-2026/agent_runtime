use crate::plan::{ExecutionPlan, NodeId};
use crate::state::NodeState;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct GlobalState {
    pub nodes: HashMap<NodeId, NodeState>,
    pub plan_version: u64,
}

impl GlobalState {
    pub fn new(plan: &ExecutionPlan) -> Self {
        let nodes = plan
            .nodes
            .keys()
            .map(|id| (id.clone(), NodeState::Pending))
            .collect();
        Self {
            nodes,
            plan_version: plan.version,
        }
    }

    pub fn is_ready(&self, id: &NodeId) -> bool {
        matches!(self.nodes.get(id), Some(NodeState::Ready))
    }

    pub fn all_predecessors_executed(&self, id: &NodeId, plan: &ExecutionPlan) -> bool {
        plan.edges
            .iter()
            .filter(|edge| &edge.to == id)
            .all(|edge| matches!(self.nodes.get(&edge.from), Some(NodeState::Executed)))
    }

    pub fn set_state(&mut self, id: &NodeId, state: NodeState) {
        if let Some(node_state) = self.nodes.get_mut(id) {
            *node_state = state;
        }
    }

    pub fn mark_running(&mut self, id: &NodeId) {
        self.set_state(id, NodeState::Running);
    }

    pub fn mark_executed(&mut self, id: &NodeId) {
        self.set_state(id, NodeState::Executed);
    }

    pub fn mark_retryable(&mut self, id: &NodeId) {
        self.set_state(id, NodeState::FailedRetryable);
    }

    pub fn mark_failed(&mut self, id: &NodeId) {
        self.set_state(id, NodeState::Failed);
    }
}
