use crate::error::PlanError;
use crate::plan::{ExecutionPlan, NodeId};

pub fn validate_dag(plan: &ExecutionPlan) -> Result<(), PlanError> {
    for edge in &plan.edges {
        if edge.from == edge.to {
            return Err(PlanError::SelfLoop(edge.from.clone()));
        }
        if !plan.nodes.contains_key(&edge.from) {
            return Err(PlanError::MissingNode(edge.from.clone()));
        }
        if !plan.nodes.contains_key(&edge.to) {
            return Err(PlanError::MissingNode(edge.to.clone()));
        }
    }
    Ok(())
}
