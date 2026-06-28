use crate::error::{DispatcherError, PlanError};
use crate::plan::{Contract, Edge, ExecutionPlan, Node, NodeConfig, NodeId, SideEffectLevel};
use crate::policy::{ActionPolicy, RiskLevel};
use serde::Deserialize;
use serde_yaml::Value;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

#[derive(Clone, Debug, Deserialize)]
pub struct ActionFlowFile {
    pub version: u64,
    pub id: String,
    #[serde(default)]
    pub globals: Option<FlowGlobals>,
    pub steps: Vec<FlowStep>,
    #[serde(default, alias = "outputStep")]
    pub output: Option<String>,
    #[serde(default, alias = "outputContract")]
    pub output_contract: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FlowGlobals {
    #[serde(default)]
    pub defaults: Option<FlowDefaults>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FlowDefaults {
    #[serde(default, alias = "retryBudget")]
    pub retry_budget: Option<u32>,
    #[serde(default, alias = "timeoutMs")]
    pub timeout_ms: Option<u64>,
    #[serde(default, alias = "sideEffect")]
    pub side_effect: Option<String>,
    #[serde(default)]
    pub policy: Option<FlowPolicyYaml>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FlowPolicyYaml {
    #[serde(default, alias = "timeoutMs")]
    pub timeout_ms: Option<u64>,
    #[serde(default, alias = "maxRetries")]
    pub max_retries: Option<u32>,
    #[serde(default, alias = "requiresConfirmation")]
    pub requires_confirmation: Option<bool>,
    #[serde(default, alias = "collectEvidence")]
    pub collect_evidence: Option<bool>,
    #[serde(default, alias = "riskLevel")]
    pub risk_level: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FlowStep {
    pub id: String,
    pub action: String,
    #[serde(default)]
    pub inputs: Option<Value>,
    #[serde(default)]
    pub outputs: Option<FlowOutputs>,
    #[serde(default, alias = "retryBudget")]
    pub retry_budget: Option<u32>,
    #[serde(default, alias = "timeoutMs")]
    pub timeout_ms: Option<u64>,
    #[serde(default, alias = "sideEffect")]
    pub side_effect: Option<String>,
    #[serde(default)]
    pub policy: Option<FlowPolicyYaml>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FlowOutputs {
    #[serde(default)]
    pub contract: Option<String>,
}

pub fn load_action_flow_from_str(input: &str) -> Result<ExecutionPlan, DispatcherError> {
    let flow: ActionFlowFile =
        serde_yaml::from_str(input).map_err(|err| PlanError::InvalidFormat(err.to_string()))?;
    let requested_output = flow.output.clone();
    let defaults = flow.globals.and_then(|globals| globals.defaults);

    let mut nodes = HashMap::new();
    for step in &flow.steps {
        if nodes.contains_key(&step.id) {
            return Err(PlanError::DuplicateNode(step.id.clone()).into());
        }
        let policy = build_policy(
            step.policy.as_ref(),
            defaults.as_ref().and_then(|d| d.policy.as_ref()),
        );
        let inputs = match &step.inputs {
            Some(value) => Some(
                serde_json::to_value(value)
                    .map_err(|err| PlanError::InvalidFormat(err.to_string()))?,
            ),
            None => None,
        };
        let node = Node {
            id: step.id.clone(),
            action: step.action.clone(),
            inputs,
            config: NodeConfig {
                retry_budget: step
                    .retry_budget
                    .or(defaults.as_ref().and_then(|d| d.retry_budget))
                    .unwrap_or(0),
                timeout: Duration::from_millis(
                    step.timeout_ms
                        .or(defaults.as_ref().and_then(|d| d.timeout_ms))
                        .unwrap_or(0),
                ),
                side_effect: parse_side_effect(
                    step.side_effect
                        .as_deref()
                        .or(defaults.as_ref().and_then(|d| d.side_effect.as_deref()))
                        .unwrap_or("pure"),
                )?,
                policy,
            },
            contract: Contract {
                schema: step
                    .outputs
                    .as_ref()
                    .and_then(|outputs| outputs.contract.clone())
                    .unwrap_or_else(|| "bytes".to_string()),
            },
        };
        nodes.insert(step.id.clone(), node);
    }

    let mut edges = HashSet::new();
    let step_ids: HashSet<&str> = nodes.keys().map(|id| id.as_str()).collect();
    for step in &flow.steps {
        let mut refs = Vec::new();
        if let Some(inputs) = &step.inputs {
            collect_references(inputs, &mut refs)?;
        }
        for reference in refs {
            if reference == step.id {
                return Err(PlanError::SelfLoop(step.id.clone()).into());
            }
            if !step_ids.contains(reference.as_str()) {
                return Err(PlanError::MissingNode(reference).into());
            }
            edges.insert((reference, step.id.clone()));
        }
    }

    let output_node = select_output_node(requested_output.as_deref(), &nodes, &edges)?;
    let edges = edges
        .into_iter()
        .map(|(from, to)| Edge { from, to })
        .collect();

    let plan = ExecutionPlan {
        id: flow.id,
        version: flow.version,
        nodes,
        edges,
        output_node,
        output_contract: Contract {
            schema: flow.output_contract.unwrap_or_else(|| "bytes".to_string()),
        },
    };

    Ok(plan)
}

fn select_output_node(
    requested_output: Option<&str>,
    nodes: &HashMap<NodeId, Node>,
    edges: &HashSet<(NodeId, NodeId)>,
) -> Result<Option<NodeId>, DispatcherError> {
    if let Some(output) = requested_output {
        if nodes.contains_key(output) {
            return Ok(Some(output.to_string()));
        }
        return Err(PlanError::MissingNode(output.to_string()).into());
    }

    let non_sinks: HashSet<&str> = edges.iter().map(|(from, _)| from.as_str()).collect();
    let mut sinks: Vec<NodeId> = nodes
        .keys()
        .filter(|id| !non_sinks.contains(id.as_str()))
        .cloned()
        .collect();
    sinks.sort();
    match sinks.as_slice() {
        [only] => Ok(Some(only.clone())),
        [] => Err(PlanError::InvalidFormat("flow has no output node".to_string()).into()),
        _ => Ok(None),
    }
}

fn parse_side_effect(value: &str) -> Result<SideEffectLevel, DispatcherError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "pure" => Ok(SideEffectLevel::Pure),
        "idempotent" => Ok(SideEffectLevel::Idempotent),
        "nonidempotent" | "non_idempotent" | "non-idempotent" => Ok(SideEffectLevel::NonIdempotent),
        _ => Err(PlanError::InvalidFormat(format!("unknown side_effect: {value}")).into()),
    }
}

fn build_policy(
    step_policy: Option<&FlowPolicyYaml>,
    default_policy: Option<&FlowPolicyYaml>,
) -> ActionPolicy {
    let mut policy = ActionPolicy::default();
    // merge defaults first
    if let Some(defaults) = default_policy {
        merge_policy(&mut policy, defaults);
    }
    // step-level overrides
    if let Some(step) = step_policy {
        merge_policy(&mut policy, step);
    }
    policy
}

fn merge_policy(policy: &mut ActionPolicy, yaml: &FlowPolicyYaml) {
    if let Some(v) = yaml.timeout_ms {
        policy.timeout_ms = v;
    }
    if let Some(v) = yaml.max_retries {
        policy.max_retries = v;
    }
    if let Some(v) = yaml.requires_confirmation {
        policy.requires_confirmation = v;
    }
    if let Some(v) = yaml.collect_evidence {
        policy.collect_evidence = v;
    }
    if let Some(ref v) = yaml.risk_level {
        policy.risk_level = parse_risk_level(v);
    }
}

fn parse_risk_level(value: &str) -> RiskLevel {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" => RiskLevel::Low,
        "medium" => RiskLevel::Medium,
        "high" => RiskLevel::High,
        "critical" => RiskLevel::Critical,
        _ => RiskLevel::Low,
    }
}

fn collect_references(value: &Value, out: &mut Vec<NodeId>) -> Result<(), DispatcherError> {
    match value {
        Value::String(text) => {
            for reference in extract_refs_from_str(text)? {
                out.push(reference);
            }
            Ok(())
        }
        Value::Sequence(items) => {
            for item in items {
                collect_references(item, out)?;
            }
            Ok(())
        }
        Value::Mapping(map) => {
            for (_, value) in map {
                collect_references(value, out)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn extract_refs_from_str(input: &str) -> Result<Vec<NodeId>, DispatcherError> {
    let mut refs = Vec::new();
    let mut cursor = 0usize;
    while let Some(start) = input[cursor..].find("${") {
        let start_index = cursor + start + 2;
        let end_index = match input[start_index..].find('}') {
            Some(offset) => start_index + offset,
            None => {
                return Err(PlanError::InvalidReference(input.to_string()).into());
            }
        };
        let raw = input[start_index..end_index].trim();
        if raw.is_empty() {
            return Err(PlanError::InvalidReference(input.to_string()).into());
        }
        let step_id = raw.split('.').next().unwrap_or("");
        if step_id.is_empty() {
            return Err(PlanError::InvalidReference(input.to_string()).into());
        }
        refs.push(step_id.to_string());
        cursor = end_index + 1;
    }
    Ok(refs)
}
