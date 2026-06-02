use crate::executor::{ExecutionResult, Executor, Outcome};
use crate::plan::{ExecutionPlan, NodeId};
use crate::runtime::ExecutionContext;
use actions::{ActionInput, ActionRegistry};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::task::JoinSet;
use std::collections::HashMap;
use std::sync::Mutex;
use crate::input_resolver::resolve_node_payload;

pub struct ActionExecutor {
    registry: Arc<ActionRegistry>,
    plan: Arc<ExecutionPlan>,
    outputs: Arc<Mutex<HashMap<NodeId, Vec<u8>>>>,
}

impl ActionExecutor {
    pub fn new(registry: Arc<ActionRegistry>, plan: Arc<ExecutionPlan>) -> Self {
        Self {
            registry,
            plan,
            outputs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl Executor for ActionExecutor {
    async fn execute_batch(
        &self,
        nodes: Vec<NodeId>,
        context: &ExecutionContext,
    ) -> Vec<ExecutionResult> {
        let mut join_set = JoinSet::new();
        for node_id in nodes {
            let registry = Arc::clone(&self.registry);
            let plan = Arc::clone(&self.plan);
            let context = context.clone();
            let outputs = Arc::clone(&self.outputs);
            join_set.spawn(async move {
                let node = match plan.nodes.get(&node_id) {
                    Some(node) => node,
                    None => {
                        return ExecutionResult {
                            node_id,
                            outcome: Outcome::Failure,
                            error: Some("node not found in plan".to_string()),
                        };
                    }
                };
                let action_name = node.action.clone();
                let handle = match registry.get(&action_name) {
                    Some(handle) => handle,
                    None => {
                        return ExecutionResult {
                            node_id,
                            outcome: Outcome::Failure,
                            error: Some(format!("action not found: {action_name}")),
                        };
                    }
                };
                let payload = match outputs.lock() {
                    Ok(guard) => resolve_node_payload(node, &guard, &context.inputs),
                    Err(_) => Err(crate::error::DispatcherError::Execution(
                        "output lock poisoned".to_string(),
                    )),
                };
                let payload = match payload {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        return ExecutionResult {
                            node_id,
                            outcome: Outcome::Failure,
                            error: Some(error.to_string()),
                        };
                    }
                };
                let input = ActionInput {
                    payload,
                    metadata: std::collections::HashMap::new(),
                };
                let output = handle.execute(input).await;
                if output.is_ok() {
                    if let Ok(mut guard) = outputs.lock() {
                        guard.insert(node_id.clone(), output.payload.clone());
                    }
                    ExecutionResult {
                        node_id,
                        outcome: Outcome::Success,
                        error: None,
                    }
                } else {
                    ExecutionResult {
                        node_id,
                        outcome: Outcome::Failure,
                        error: output.error.map(|err| format!("{}: {}", err.code, err.message)),
                    }
                }
            });
        }
        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(output) => results.push(output),
                Err(error) => results.push(ExecutionResult {
                    node_id: "unknown".to_string(),
                    outcome: Outcome::Failure,
                    error: Some(format!("task join error: {error}")),
                }),
            }
        }
        results
    }
}
