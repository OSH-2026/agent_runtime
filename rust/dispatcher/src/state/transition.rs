use crate::plan::NodeId;
use crate::state::NodeState;

#[derive(Clone, Debug)]
pub struct Transition {
    pub node_id: NodeId,
    pub from: NodeState,
    pub to: NodeState,
}
