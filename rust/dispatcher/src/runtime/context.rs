use crate::plan::NodeId;

#[derive(Clone, Debug, Default)]
pub struct ExecutionContext {
    pub inputs: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct DiagnosticContext {
    pub history: Vec<FailureRecord>,
}

#[derive(Clone, Debug)]
pub struct FailureRecord {
    pub node_id: NodeId,
    pub message: String,
}
