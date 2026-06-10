use actions::subagent::ToolExecutor;
use actions::{ActionError, ActionMetadata, ActionRegistry, ActionRisk, ActionSideEffect};
use async_trait::async_trait;
use serde_yaml::{self, Value};
use std::sync::Arc;

use crate::executor::ActionExecutor;
use crate::loader::{ActionFlowFile, load_action_flow_from_str};
use crate::plan::SideEffectLevel;
use crate::policy::RiskLevel;
use crate::recovery::SimpleRecovery;
use crate::runtime::{DiagnosticContext, Engine, ExecutionContext};
use crate::scheduler::{Dispatcher, TopoPolicy};
use crate::state::{GlobalState, NodeState};
use crate::storage::{InMemoryAuditLog, InMemoryStateStore};
use crate::{DispatcherError, PlanError};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

const MAX_TOOL_STEPS: usize = 32;
const MAX_TOOL_OUTPUT_BYTES: usize = 1024 * 1024;

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
        let raw_flow: Value = serde_yaml::from_str(yaml)
            .map_err(|err| ActionError::new_with("INVALID_FORMAT", err.to_string(), false))?;
        reject_subagent_policy_fields(&raw_flow)?;
        let flow: ActionFlowFile = serde_yaml::from_value(raw_flow)
            .map_err(|err| ActionError::new_with("INVALID_FORMAT", err.to_string(), false))?;
        if flow.steps.is_empty() {
            return Err(ActionError::new_with(
                "INVALID_FORMAT",
                "flow has no steps",
                false,
            ));
        }
        if flow.steps.len() > MAX_TOOL_STEPS {
            return Err(ActionError::new_with(
                "RESOURCE_LIMIT",
                format!("flow exceeds maximum of {MAX_TOOL_STEPS} steps"),
                false,
            ));
        }

        let mut plan = load_action_flow_from_str(yaml).map_err(to_action_error)?;
        for node in plan.nodes.values_mut() {
            let metadata = self
                .registry
                .trusted_metadata(&node.action)
                .ok_or_else(|| {
                    ActionError::new_with(
                        "UNTRUSTED_ACTION",
                        format!(
                            "action '{}' is not registered with trusted subagent metadata",
                            node.action
                        ),
                        false,
                    )
                })?;
            if !metadata.callable_by_subagent {
                return Err(ActionError::new_with(
                    "ACTION_NOT_ALLOWED",
                    format!("action '{}' is not callable by subagents", node.action),
                    false,
                ));
            }
            apply_trusted_metadata(node, metadata);
        }

        let plan = Arc::new(plan);
        let executor = ActionExecutor::new(Arc::clone(&self.registry), Arc::clone(&plan));
        let output_reader = executor.clone();
        let mut engine = Engine {
            plan: (*plan).clone(),
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

        let incomplete: Vec<String> = engine
            .state
            .nodes
            .iter()
            .filter(|(_, state)| !matches!(state, NodeState::Executed))
            .map(|(id, state)| format!("{id}={state:?}"))
            .collect();
        if !incomplete.is_empty() {
            let cause = engine
                .diagnostic
                .history
                .last()
                .map(|failure| failure.message.as_str())
                .unwrap_or("workflow did not reach a successful terminal state");
            return Err(ActionError::new_with(
                "WORKFLOW_INCOMPLETE",
                format!("{cause}; states: {}", incomplete.join(", ")),
                false,
            ));
        }

        let output = output_reader
            .outputs()
            .get(&plan.output_node)
            .cloned()
            .ok_or_else(|| {
                ActionError::new_with(
                    "OUTPUT_MISSING",
                    format!("output not found for node '{}'", plan.output_node),
                    false,
                )
            })?;
        if output.len() > MAX_TOOL_OUTPUT_BYTES {
            return Err(ActionError::new_with(
                "RESOURCE_LIMIT",
                format!("tool output exceeds maximum of {MAX_TOOL_OUTPUT_BYTES} bytes"),
                false,
            ));
        }

        match String::from_utf8(output) {
            Ok(text) => Ok(text),
            Err(err) => Ok(STANDARD.encode(err.into_bytes())),
        }
    }
}

fn reject_subagent_policy_fields(flow: &Value) -> Result<(), ActionError> {
    let mapping = flow.as_mapping().ok_or_else(|| {
        ActionError::new_with("INVALID_FORMAT", "flow must be a YAML mapping", false)
    })?;

    if let Some(globals) = mapping.get(Value::String("globals".to_string())) {
        if let Some(defaults) = globals
            .as_mapping()
            .and_then(|globals| globals.get(Value::String("defaults".to_string())))
        {
            reject_policy_mapping(defaults, "globals.defaults")?;
        }
    }

    if let Some(steps) = mapping.get(Value::String("steps".to_string())) {
        if let Some(steps) = steps.as_sequence() {
            for (index, step) in steps.iter().enumerate() {
                reject_policy_mapping(step, &format!("steps[{index}]"))?;
            }
        }
    }
    Ok(())
}

fn reject_policy_mapping(value: &Value, location: &str) -> Result<(), ActionError> {
    let Some(mapping) = value.as_mapping() else {
        return Ok(());
    };
    const FORBIDDEN_FIELDS: [&str; 7] = [
        "policy",
        "sideEffect",
        "side_effect",
        "retryBudget",
        "retry_budget",
        "timeoutMs",
        "timeout_ms",
    ];
    for field in FORBIDDEN_FIELDS {
        if mapping.contains_key(Value::String(field.to_string())) {
            return Err(ActionError::new_with(
                "POLICY_NOT_ALLOWED",
                format!(
                    "{location}.{field} is not allowed in subagent YAML; execution policy comes from action metadata"
                ),
                false,
            ));
        }
    }
    Ok(())
}

fn apply_trusted_metadata(node: &mut crate::plan::Node, metadata: &ActionMetadata) {
    node.config.side_effect = match metadata.side_effect {
        ActionSideEffect::Pure => SideEffectLevel::Pure,
        ActionSideEffect::Idempotent => SideEffectLevel::Idempotent,
        ActionSideEffect::NonIdempotent => SideEffectLevel::NonIdempotent,
    };
    node.config.policy.risk_level = match metadata.risk {
        ActionRisk::Low => RiskLevel::Low,
        ActionRisk::Medium => RiskLevel::Medium,
        ActionRisk::High => RiskLevel::High,
        ActionRisk::Critical => RiskLevel::Critical,
    };
    node.config.policy.requires_confirmation = metadata.requires_confirmation;
    node.config.policy.collect_evidence = metadata.collect_evidence;
    node.config.policy.timeout_ms = metadata.timeout_ms;
    node.config.policy.max_retries = metadata.max_retries;
    node.config.retry_budget = metadata.max_retries;
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
