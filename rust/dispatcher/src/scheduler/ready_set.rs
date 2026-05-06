use crate::plan::{ExecutionPlan, NodeId};
use crate::state::GlobalState;

pub fn compute_ready_set(state: &GlobalState, plan: &ExecutionPlan) -> Vec<NodeId> {
    plan.nodes
        .keys()
        .filter(|id| state.is_ready(id) && state.all_predecessors_executed(id, plan))
        .cloned()
        .collect()
}
