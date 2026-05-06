use crate::error::PlanError;
use crate::plan::ExecutionPlan;
use std::collections::HashMap;
use std::collections::VecDeque;

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
    let mut in_degree: HashMap<&String, usize> = plan
        .nodes
        .keys()
        .map(|id| (id, 0))
        .collect();
    for edge in &plan.edges {
        if let Some(count) = in_degree.get_mut(&edge.to) {
            *count += 1;
        }
    }
    let mut queue: VecDeque<&String> = in_degree
        .iter()
        .filter_map(|(node, degree)| if *degree == 0 { Some(*node) } else { None })
        .collect();
    let mut visited = 0usize;
    let mut in_degree = in_degree;
    while let Some(node) = queue.pop_front() {
        visited += 1;
        for edge in plan.edges.iter().filter(|edge| &edge.from == node) {
            if let Some(count) = in_degree.get_mut(&edge.to) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    queue.push_back(&edge.to);
                }
            }
        }
    }
    if visited != plan.nodes.len() {
        let cycle_nodes = in_degree
            .into_iter()
            .filter_map(|(node, degree)| if degree > 0 { Some(node.clone()) } else { None })
            .collect();
        return Err(PlanError::Cycle(cycle_nodes));
    }
    Ok(())
}
