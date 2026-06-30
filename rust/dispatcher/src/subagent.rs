use actions::subagent::{ToolExecution, ToolExecutor};
use actions::{ActionError, ActionMetadata, ActionRegistry, ActionRisk, ActionSideEffect};
use async_trait::async_trait;
use serde_yaml::{self, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::executor::ActionExecutor;
use crate::loader::{ActionFlowFile, load_action_flow_from_str};
use crate::plan::{ExecutionPlan, SideEffectLevel};
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

#[derive(Clone, Debug)]
pub struct ConfirmationRequest {
    pub node_id: String,
    pub action: String,
    pub inputs: Option<serde_json::Value>,
    pub risk: ActionRisk,
}

#[async_trait]
pub trait ConfirmationHandler: Send + Sync {
    async fn confirm(&self, request: ConfirmationRequest) -> Result<bool, ActionError>;
}

pub trait ActionRegistryFactory: Send + Sync {
    fn create_registry(&self, tools: Arc<dyn ToolExecutor>) -> Result<ActionRegistry, ActionError>;
}

pub struct DispatcherToolExecutor {
    factory: Arc<dyn ActionRegistryFactory>,
    confirmation_handler: Option<Arc<dyn ConfirmationHandler>>,
}

impl DispatcherToolExecutor {
    pub fn new(factory: Arc<dyn ActionRegistryFactory>) -> Self {
        Self {
            factory,
            confirmation_handler: None,
        }
    }

    pub fn with_confirmation_handler(
        factory: Arc<dyn ActionRegistryFactory>,
        confirmation_handler: Arc<dyn ConfirmationHandler>,
    ) -> Self {
        Self {
            factory,
            confirmation_handler: Some(confirmation_handler),
        }
    }
}

#[async_trait]
impl ToolExecutor for DispatcherToolExecutor {
    async fn validate_workflow_message(
        &self,
        yaml: &str,
        final_message_template: &str,
    ) -> Result<(), ActionError> {
        let plan = load_subagent_plan(yaml)?;
        validate_final_message_template(
            final_message_template,
            plan.nodes.keys().map(String::as_str),
        )
        .map_err(|message| ActionError::new_with("FINAL_MESSAGE_TEMPLATE", message, false))
    }

    async fn execute_yaml(&self, yaml: &str) -> Result<ToolExecution, ActionError> {
        let recursive_tools: Arc<dyn ToolExecutor> = match &self.confirmation_handler {
            Some(handler) => Arc::new(DispatcherToolExecutor::with_confirmation_handler(
                Arc::clone(&self.factory),
                Arc::clone(handler),
            )),
            None => Arc::new(DispatcherToolExecutor::new(Arc::clone(&self.factory))),
        };
        let registry = Arc::new(self.factory.create_registry(recursive_tools)?);
        let mut plan = load_subagent_plan(yaml)?;
        for node in plan.nodes.values_mut() {
            let metadata = registry.trusted_metadata(&node.action).ok_or_else(|| {
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
            apply_action_metadata(node, metadata);
        }

        let plan = Arc::new(plan);
        let executor = ActionExecutor::new(Arc::clone(&registry), Arc::clone(&plan));
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
        loop {
            engine
                .run(&ExecutionContext::default())
                .await
                .map_err(to_action_error)?;
            let waiting = engine.waiting_human_nodes();
            if waiting.is_empty() {
                break;
            }
            let handler = self.confirmation_handler.as_ref().ok_or_else(|| {
                ActionError::new_with(
                    "CONFIRMATION_REQUIRED",
                    format!(
                        "workflow is waiting for confirmation: {}",
                        waiting.join(", ")
                    ),
                    false,
                )
            })?;
            for node_id in waiting {
                let (action, inputs, risk) = {
                    let node = engine.plan.nodes.get(&node_id).ok_or_else(|| {
                        ActionError::new_with(
                            "MISSING_NODE",
                            format!("node not found: {node_id}"),
                            false,
                        )
                    })?;
                    (
                        node.action.clone(),
                        node.inputs.clone(),
                        action_risk(&node.config.policy.risk_level),
                    )
                };
                let approved = handler
                    .confirm(ConfirmationRequest {
                        node_id: node_id.clone(),
                        action: action.clone(),
                        inputs,
                        risk,
                    })
                    .await?;
                if approved {
                    engine.approve_node(&node_id).map_err(to_action_error)?;
                } else {
                    engine.reject_node(&node_id).map_err(to_action_error)?;
                    return Err(ActionError::new_with(
                        "USER_REJECTED",
                        format!("user rejected action '{action}'"),
                        false,
                    ));
                }
            }
        }

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

        let raw_outputs = output_reader.outputs();
        if let Some(output_node) = &plan.output_node {
            let output = raw_outputs.get(output_node).cloned().ok_or_else(|| {
                ActionError::new_with(
                    "OUTPUT_MISSING",
                    format!("output not found for node '{output_node}'"),
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
        }

        let node_outputs = raw_outputs
            .iter()
            .map(|(id, bytes)| (id.clone(), bytes_to_string(bytes)))
            .collect::<BTreeMap<_, _>>();
        let output = match &plan.output_node {
            Some(output_node) => node_outputs.get(output_node).cloned().ok_or_else(|| {
                ActionError::new_with(
                    "OUTPUT_MISSING",
                    format!("output not found for node '{output_node}'"),
                    false,
                )
            })?,
            None => String::new(),
        };
        if output.len() > MAX_TOOL_OUTPUT_BYTES {
            return Err(ActionError::new_with(
                "RESOURCE_LIMIT",
                format!("tool output exceeds maximum of {MAX_TOOL_OUTPUT_BYTES} bytes"),
                false,
            ));
        }

        Ok(ToolExecution {
            output,
            node_outputs,
        })
    }
}

fn load_subagent_plan(yaml: &str) -> Result<ExecutionPlan, ActionError> {
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
    load_action_flow_from_str(yaml).map_err(to_action_error)
}

fn validate_final_message_template<'a>(
    template: &str,
    node_ids: impl IntoIterator<Item = &'a str>,
) -> Result<(), String> {
    let node_ids = node_ids
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let mut cursor = 0usize;
    while let Some(start) = template[cursor..].find("${") {
        let start_index = cursor + start;
        let name_start = start_index + 2;
        let end_index = template[name_start..]
            .find('}')
            .map(|offset| name_start + offset)
            .ok_or_else(|| format!("unclosed placeholder in final message: {template}"))?;
        let node_id = template[name_start..end_index].trim();
        if node_id.is_empty() {
            return Err("empty placeholder in final message".to_string());
        }
        if !node_ids.contains(node_id) {
            return Err(format!("final message references unknown node '{node_id}'"));
        }
        cursor = end_index + 1;
    }
    Ok(())
}

fn action_risk(risk: &RiskLevel) -> ActionRisk {
    match risk {
        RiskLevel::Low => ActionRisk::Low,
        RiskLevel::Medium => ActionRisk::Medium,
        RiskLevel::High => ActionRisk::High,
        RiskLevel::Critical => ActionRisk::Critical,
    }
}

fn bytes_to_string(bytes: &[u8]) -> String {
    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => text,
        Err(err) => STANDARD.encode(err.into_bytes()),
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

pub fn apply_action_metadata(node: &mut crate::plan::Node, metadata: &ActionMetadata) {
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
