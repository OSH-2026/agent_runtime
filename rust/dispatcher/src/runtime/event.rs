use crate::plan::NodeId;

#[derive(Clone, Debug)]
pub enum RuntimeEvent {
    NodeStarted(NodeId),
    NodeCompleted(NodeId),
    NodeFailed(NodeId),
}
