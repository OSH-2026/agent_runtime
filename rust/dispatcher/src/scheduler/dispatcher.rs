use crate::plan::NodeId;
use crate::scheduler::SchedulingPolicy;

pub struct Dispatcher {
    policy: Box<dyn SchedulingPolicy>,
}

impl Dispatcher {
    pub fn new(policy: Box<dyn SchedulingPolicy>) -> Self {
        Self { policy }
    }

    pub fn dispatch_ready(&self, ready: Vec<NodeId>) -> Vec<NodeId> {
        self.policy.select(ready)
    }
}
