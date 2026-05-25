use crate::plan::Contract;
use crate::policy::ActionPolicy;
use std::time::Duration;

pub type NodeId = String;
pub type ActionRef = String;

#[derive(Clone, Debug)]
pub struct Node {
    pub id: NodeId,
    pub action: ActionRef,
    pub config: NodeConfig,
    pub contract: Contract,
}

#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub retry_budget: u32,
    pub timeout: Duration,
    pub side_effect: SideEffectLevel,
    pub policy: ActionPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SideEffectLevel {
    Pure,
    Idempotent,
    NonIdempotent,
}
