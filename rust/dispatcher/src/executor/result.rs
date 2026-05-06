use crate::plan::NodeId;

#[derive(Clone, Debug)]
pub enum Outcome {
    Success,
    Failure,
    Retry,
}

#[derive(Clone, Debug)]
pub struct ExecutionResult {
    pub node_id: NodeId,
    pub outcome: Outcome,
    pub error: Option<String>,
}
