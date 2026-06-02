use actions::subagent::ToolExecutor;
use actions::{ActionError, ActionInput, ActionRegistry};
use async_trait::async_trait;
use serde_yaml;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::executor::{ExecutionResult, Executor, Outcome};
use crate::loader::{load_action_flow_from_str, ActionFlowFile};
use crate::plan::{ExecutionPlan, NodeId};
use crate::recovery::SimpleRecovery;
use crate::runtime::{DiagnosticContext, Engine, ExecutionContext};
use crate::scheduler::{Dispatcher, TopoPolicy};
use crate::state::GlobalState;
use crate::storage::{InMemoryAuditLog, InMemoryStateStore};
use crate::{DispatcherError, PlanError};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

pub struct DispatcherToolExecutor {
    registry: Arc<ActionRegistry>,
}

impl DispatcherToolExecutor {
    pub fn new(registry: Arc<ActionRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl ToolExecutor for DispatcherToolExecutor {
    async fn execute_yaml(&self, yaml: &str) -> Result<String, ActionError> {
        let flow: ActionFlowFile = serde_yaml::from_str(yaml)
            .map_err(|err| ActionError::new(err.to_string()))?;
        let last_step = flow
            .steps
            .last()
            .map(|step| step.id.clone())
            .ok_or_else(|| ActionError::new("flow has no steps"))?;
        let plan = load_action_flow_from_str(yaml).map_err(to_action_error)?;
        let output_map: Arc<Mutex<HashMap<NodeId, Vec<u8>>>> = Arc::new(Mutex::new(HashMap::new()));

        let executor = CapturingExecutor {
            registry: Arc::clone(&self.registry),
            outputs: Arc::clone(&output_map),
            plan: plan.clone(),
        };
        let mut engine = Engine {
            plan: plan.clone(),
            state: GlobalState::new(&plan),
            dispatcher: Dispatcher::new(Box::new(TopoPolicy::default())),
            executor: Box::new(executor),
            recovery: Box::new(SimpleRecovery::default()),
            audit_log: Box::new(InMemoryAuditLog::new()),
            state_store: Box::new(InMemoryStateStore::new()),
            diagnostic: DiagnosticContext::default(),
        };
        engine
            .run(&ExecutionContext::default())
            .await
            .map_err(to_action_error)?;

        let output = output_map
            .lock()
            .map_err(|_| ActionError::new("output lock poisoned"))?
            .get(&last_step)
            .cloned()
            .ok_or_else(|| ActionError::new("last step output not found"))?;

        match String::from_utf8(output) {
            Ok(text) => Ok(text),
            Err(err) => Ok(STANDARD.encode(err.into_bytes())),
        }
    }
}

struct CapturingExecutor {
    registry: Arc<ActionRegistry>,
    outputs: Arc<Mutex<HashMap<NodeId, Vec<u8>>>>,
    plan: ExecutionPlan,
}

#[async_trait]
impl Executor for CapturingExecutor {
    async fn execute_batch(
        &self,
        nodes: Vec<NodeId>,
        context: &ExecutionContext,
    ) -> Vec<ExecutionResult> {
        let mut results = Vec::new();
        for node_id in nodes {
            let action_name = match self.plan.nodes.get(&node_id) {
                Some(node) => node.action.clone(),
                None => {
                    results.push(ExecutionResult {
                        node_id,
                        outcome: Outcome::Failure,
                        error: Some("node not found".to_string()),
                    });
                    continue;
                }
            };
            let handle = match self.registry.get(&action_name) {
                Some(handle) => handle,
                None => {
                    results.push(ExecutionResult {
                        node_id,
                        outcome: Outcome::Failure,
                        error: Some(format!("action not found: {action_name}")),
                    });
                    continue;
                }
            };
            let input = ActionInput {
                payload: context.inputs.clone(),
                metadata: HashMap::new(),
            };
            let output = handle.execute(input).await;
            if output.is_ok() {
                if let Ok(mut guard) = self.outputs.lock() {
                    guard.insert(node_id.clone(), output.payload.clone());
                }
                results.push(ExecutionResult {
                    node_id,
                    outcome: Outcome::Success,
                    error: None,
                });
            } else {
                let message = output
                    .error
                    .as_ref()
                    .map(|err| format!("{}: {}", err.code, err.message))
                    .unwrap_or_else(|| "unknown error".to_string());
                results.push(ExecutionResult {
                    node_id,
                    outcome: Outcome::Failure,
                    error: Some(message),
                });
            }
        }
        results
    }
}

fn to_action_error(err: DispatcherError) -> ActionError {
    match err {
        DispatcherError::Plan(PlanError::InvalidFormat(message)) => {
            ActionError::new_with("INVALID_FORMAT", message, false)
        }
        DispatcherError::Plan(PlanError::MissingNode(id)) => {
            ActionError::new_with("MISSING_NODE", id, false)
        }
        DispatcherError::Plan(PlanError::DuplicateNode(id)) => {
            ActionError::new_with("DUPLICATE_NODE", id, false)
        }
        DispatcherError::Plan(PlanError::InvalidReference(message)) => {
            ActionError::new_with("INVALID_REFERENCE", message, false)
        }
        DispatcherError::Plan(PlanError::SelfLoop(id)) => {
            ActionError::new_with("SELF_LOOP", id, false)
        }
        DispatcherError::Plan(PlanError::Cycle(nodes)) => {
            ActionError::new_with("CYCLE", format!("{:?}", nodes), false)
        }
        DispatcherError::Execution(message) => ActionError::new_with("EXECUTION", message, true),
    }
}
