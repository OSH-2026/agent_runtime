use crate::plan::{ExecutionPlan, NodeId};
use crate::state::{NodeState, Transition};
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

    pub fn refresh_ready(&mut self, plan: &ExecutionPlan) -> Vec<Transition> {
        let mut transitions = Vec::new();
        let candidates: Vec<NodeId> = self
            .nodes
            .iter()
            .filter_map(|(id, state)| {
                if matches!(state, NodeState::Pending | NodeState::FailedRetryable) {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();
        for id in candidates {
            if self.all_predecessors_executed(&id, plan) {
                if let Some(transition) = self.transition(&id, NodeState::Ready) {
                    transitions.push(transition);
                }
            }
        }
        transitions
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

    pub fn transition(&mut self, id: &NodeId, state: NodeState) -> Option<Transition> {
        let from = self.nodes.get(id).copied()?;
        if from == state {
            return None;
        }
        self.nodes.insert(id.clone(), state);
        Some(Transition {
            node_id: id.clone(),
            from,
            to: state,
        })
    }

    pub fn mark_running(&mut self, id: &NodeId) -> Option<Transition> {
        self.transition(id, NodeState::Running)
    }

    pub fn mark_executed(&mut self, id: &NodeId) -> Option<Transition> {
        self.transition(id, NodeState::Executed)
    }

    pub fn mark_retryable(&mut self, id: &NodeId) -> Option<Transition> {
        self.transition(id, NodeState::FailedRetryable)
    }

    pub fn mark_failed(&mut self, id: &NodeId) -> Option<Transition> {
        self.transition(id, NodeState::Failed)
    }
}
