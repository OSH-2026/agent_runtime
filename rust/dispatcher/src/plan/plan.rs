use crate::plan::{Contract, Edge, Node, NodeId};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ExecutionPlan {
    pub id: String,
    pub version: u64,
    pub nodes: HashMap<NodeId, Node>,
    pub edges: Vec<Edge>,
    pub output_node: NodeId,
    pub output_contract: Contract,
}
