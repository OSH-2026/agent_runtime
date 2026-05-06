use crate::plan::NodeId;
use std::fmt;

#[derive(Debug)]
pub enum PlanError {
    MissingNode(NodeId),
    SelfLoop(NodeId),
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::MissingNode(id) => write!(f, "missing node: {id}"),
            PlanError::SelfLoop(id) => write!(f, "self loop on node: {id}"),
        }
    }
}

#[derive(Debug)]
pub enum DispatcherError {
    Plan(PlanError),
    Execution(String),
}

impl fmt::Display for DispatcherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DispatcherError::Plan(err) => write!(f, "plan error: {err}"),
            DispatcherError::Execution(message) => write!(f, "execution error: {message}"),
        }
    }
}
