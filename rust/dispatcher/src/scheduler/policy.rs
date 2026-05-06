use crate::plan::NodeId;

pub trait SchedulingPolicy: Send + Sync {
    fn select(&self, ready: Vec<NodeId>) -> Vec<NodeId>;
}

#[derive(Default)]
pub struct TopoPolicy;

impl SchedulingPolicy for TopoPolicy {
    fn select(&self, ready: Vec<NodeId>) -> Vec<NodeId> {
        ready
    }
}
