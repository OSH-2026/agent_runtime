use actions::catalog::{ANDROID_ACTION_NAMES, metadata_for_action};
use actions::client::{GrpcClient, RemoteAction};
use actions::{
    Action, ActionError, ActionInput, ActionOutput, ActionRegistry, SubagentAction, SubagentConfig,
    ToolExecutor,
};
use async_trait::async_trait;
use dispatcher::scheduler::{Dispatcher, TopoPolicy};
use dispatcher::{
    ActionExecutor, ActionRegistryFactory, ConfirmationHandler, ConfirmationRequest,
    DispatcherToolExecutor, Engine, ExecutionContext, GlobalState, InMemoryAuditLog,
    InMemoryStateStore, NodeState, SimpleRecovery, apply_action_metadata,
    load_action_flow_from_str,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::oneshot;

const ACTION_CATALOG_DOCUMENT: &str =
    include_str!("../../../../docs/action_fabric/action-catalog-for-llm.md");

#[derive(Default)]
struct ConfirmationBroker {
    next_id: AtomicU64,
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

#[derive(Clone)]
struct TauriConfirmationHandler {
    app: AppHandle,
    broker: Arc<ConfirmationBroker>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmationEvent {
    request_id: String,
    node_id: String,
    action: String,
    inputs: Option<Value>,
    risk: String,
}

#[async_trait]
impl ConfirmationHandler for TauriConfirmationHandler {
    async fn confirm(&self, request: ConfirmationRequest) -> Result<bool, ActionError> {
        let request_id = format!(
            "confirmation-{}",
            self.broker.next_id.fetch_add(1, Ordering::Relaxed)
        );
        let (sender, receiver) = oneshot::channel();
        self.broker
            .pending
            .lock()
            .map_err(|_| ActionError::new("confirmation state lock poisoned"))?
            .insert(request_id.clone(), sender);

        let event = ConfirmationEvent {
            request_id: request_id.clone(),
            node_id: request.node_id,
            action: request.action,
            inputs: request.inputs,
            risk: risk_name(request.risk).to_string(),
        };
        if let Err(error) = self.app.emit("confirmation-request", event) {
            if let Ok(mut pending) = self.broker.pending.lock() {
                pending.remove(&request_id);
            }
            return Err(ActionError::new_with(
                "CONFIRMATION_UI",
                error.to_string(),
                false,
            ));
        }

        receiver.await.map_err(|_| {
            ActionError::new_with(
                "CONFIRMATION_CANCELLED",
                "confirmation UI closed without a decision",
                false,
            )
        })
    }
}

struct TextAction;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextInput {
    value: String,
}

#[async_trait]
impl Action for TextAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        match serde_json::from_slice::<TextInput>(&input.payload) {
            Ok(text) => ActionOutput {
                payload: text.value.into_bytes(),
                error: None,
            },
            Err(error) => ActionOutput {
                payload: Vec::new(),
                error: Some(ActionError::new_with(
                    "INVALID_INPUT",
                    error.to_string(),
                    false,
                )),
            },
        }
    }
}

struct UppercaseAction;

#[async_trait]
impl Action for UppercaseAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        let payload = extract_text(&input.payload, "text").to_uppercase();
        ActionOutput {
            payload: payload.into_bytes(),
            error: None,
        }
    }
}

struct TauriRegistryFactory {
    grpc_client: GrpcClient,
}

impl ActionRegistryFactory for TauriRegistryFactory {
    fn create_registry(&self, tools: Arc<dyn ToolExecutor>) -> Result<ActionRegistry, ActionError> {
        let mut registry = ActionRegistry::default();
        registry.register_local_with_metadata(
            "text",
            Arc::new(TextAction),
            required_metadata("text").map_err(ActionError::new)?,
        );
        registry.register_local_with_metadata(
            "uppercase",
            Arc::new(UppercaseAction),
            required_metadata("uppercase").map_err(ActionError::new)?,
        );
        registry.register_local_with_metadata(
            "subagent",
            Arc::new(SubagentAction::new(
                tools,
                SubagentConfig {
                    model: "local-model".to_string(),
                    base_url: "http://10.0.2.2:8080".to_string(),
                    api_key: None,
                    max_turns: 2,
                    temperature: 0.2,
                    request_timeout_ms: 60_000,
                    system_prompt: None,
                    action_catalog: subagent_action_catalog(),
                },
            )),
            required_metadata("subagent").map_err(ActionError::new)?,
        );
        for action_name in ANDROID_ACTION_NAMES {
            let metadata = required_metadata(action_name).map_err(ActionError::new)?;
            registry.register_remote_with_metadata(
                *action_name,
                RemoteAction::from_grpc(self.grpc_client.clone(), *action_name),
                metadata,
            );
        }
        Ok(registry)
    }
}

fn extract_text(payload: &[u8], preferred_key: &str) -> String {
    let Ok(value) = serde_json::from_slice::<Value>(payload) else {
        return String::from_utf8_lossy(payload).into_owned();
    };

    if let Some(text) = value.get(preferred_key).and_then(Value::as_str) {
        return text.to_string();
    }
    if let Some(text) = value.as_str() {
        return text.to_string();
    }
    value.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowResult {
    plan_id: String,
    success: bool,
    node_states: BTreeMap<String, String>,
    outputs: BTreeMap<String, String>,
    audit: Vec<AuditEntry>,
    diagnostics: Vec<DiagnosticEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditEntry {
    node_id: String,
    from: String,
    to: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticEntry {
    node_id: String,
    message: String,
}

#[tauri::command]
async fn run_workflow(
    app: AppHandle,
    confirmation_broker: State<'_, Arc<ConfirmationBroker>>,
    yaml: String,
    input: Option<String>,
    grpc_endpoint: Option<String>,
) -> Result<WorkflowResult, String> {
    let confirmation_handler: Arc<dyn ConfirmationHandler> = Arc::new(TauriConfirmationHandler {
        app,
        broker: Arc::clone(confirmation_broker.inner()),
    });
    run_workflow_inner(yaml, input, grpc_endpoint, confirmation_handler).await
}

async fn run_workflow_inner(
    yaml: String,
    input: Option<String>,
    grpc_endpoint: Option<String>,
    confirmation_handler: Arc<dyn ConfirmationHandler>,
) -> Result<WorkflowResult, String> {
    let mut plan = load_action_flow_from_str(&yaml).map_err(|error| error.to_string())?;
    for node in plan.nodes.values_mut() {
        let metadata = required_metadata(&node.action)?;
        apply_action_metadata(node, &metadata);
    }
    let plan_id = plan.id.clone();
    let grpc_endpoint = grpc_endpoint
        .filter(|endpoint| !endpoint.trim().is_empty())
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let factory: Arc<dyn ActionRegistryFactory> = Arc::new(TauriRegistryFactory {
        grpc_client: GrpcClient::new(grpc_endpoint),
    });
    let tools: Arc<dyn ToolExecutor> = Arc::new(DispatcherToolExecutor::with_confirmation_handler(
        Arc::clone(&factory),
        Arc::clone(&confirmation_handler),
    ));
    let registry = Arc::new(
        factory
            .create_registry(tools)
            .map_err(|error| error.message)?,
    );

    let executor = ActionExecutor::new(Arc::clone(&registry), Arc::new(plan.clone()));
    let output_reader = executor.clone();
    let audit_log = InMemoryAuditLog::default();
    let audit_reader = audit_log.clone();

    let mut engine = Engine {
        state: GlobalState::new(&plan),
        dispatcher: Dispatcher::new(Box::new(TopoPolicy)),
        executor: Box::new(executor),
        recovery: Box::new(SimpleRecovery::default()),
        audit_log: Box::new(audit_log),
        state_store: Box::new(InMemoryStateStore::default()),
        diagnostic: Default::default(),
        plan,
    };

    let context = ExecutionContext {
        inputs: input.unwrap_or_default().into_bytes(),
    };
    run_with_confirmations(&mut engine, &context, confirmation_handler.as_ref()).await?;

    let node_states = engine
        .state
        .nodes
        .iter()
        .map(|(id, state)| (id.clone(), state_name(*state).to_string()))
        .collect::<BTreeMap<_, _>>();
    let success = engine
        .state
        .nodes
        .values()
        .all(|state| *state == NodeState::Executed);
    let outputs = output_reader
        .outputs()
        .into_iter()
        .map(|(id, bytes)| (id, String::from_utf8_lossy(&bytes).into_owned()))
        .collect::<BTreeMap<_, _>>();
    let diagnostics = engine
        .diagnostic
        .history
        .iter()
        .map(|record| DiagnosticEntry {
            node_id: record.node_id.clone(),
            message: record.message.clone(),
        })
        .collect();

    let audit = audit_reader
        .events()
        .into_iter()
        .map(|event| AuditEntry {
            node_id: event.node_id,
            from: state_name(event.from).to_string(),
            to: state_name(event.to).to_string(),
        })
        .collect();

    Ok(WorkflowResult {
        plan_id,
        success,
        node_states,
        outputs,
        audit,
        diagnostics,
    })
}

async fn run_with_confirmations(
    engine: &mut Engine,
    context: &ExecutionContext,
    confirmation_handler: &dyn ConfirmationHandler,
) -> Result<(), String> {
    loop {
        engine
            .run(context)
            .await
            .map_err(|error| error.to_string())?;
        let waiting = engine.waiting_human_nodes();
        if waiting.is_empty() {
            return Ok(());
        }
        for node_id in waiting {
            let node = engine
                .plan
                .nodes
                .get(&node_id)
                .ok_or_else(|| format!("node not found: {node_id}"))?;
            let action = node.action.clone();
            let approved = confirmation_handler
                .confirm(ConfirmationRequest {
                    node_id: node_id.clone(),
                    action: action.clone(),
                    inputs: node.inputs.clone(),
                    risk: metadata_for_action(&action)
                        .map(|metadata| metadata.risk)
                        .unwrap_or(actions::ActionRisk::High),
                })
                .await
                .map_err(|error| error.message)?;
            if approved {
                engine
                    .approve_node(&node_id)
                    .map_err(|error| error.to_string())?;
            } else {
                engine
                    .reject_node(&node_id)
                    .map_err(|error| error.to_string())?;
                return Err(format!("用户拒绝执行 action '{action}'"));
            }
        }
    }
}

#[tauri::command]
fn resolve_confirmation(
    confirmation_broker: State<'_, Arc<ConfirmationBroker>>,
    request_id: String,
    approved: bool,
) -> Result<(), String> {
    let sender = confirmation_broker
        .pending
        .lock()
        .map_err(|_| "confirmation state lock poisoned".to_string())?
        .remove(&request_id)
        .ok_or_else(|| format!("confirmation request not found: {request_id}"))?;
    sender
        .send(approved)
        .map_err(|_| "workflow is no longer waiting for confirmation".to_string())
}

fn risk_name(risk: actions::ActionRisk) -> &'static str {
    match risk {
        actions::ActionRisk::Low => "low",
        actions::ActionRisk::Medium => "medium",
        actions::ActionRisk::High => "high",
        actions::ActionRisk::Critical => "critical",
    }
}

fn subagent_action_catalog() -> String {
    ACTION_CATALOG_DOCUMENT
        .split_once("## 内置 Action（39）")
        .and_then(|(_, actions)| actions.split_once("## 输出结构"))
        .map(|(signatures, _)| signatures.trim().to_string())
        .expect("action catalog document must contain the action signature sections")
}

fn required_metadata(action_name: &str) -> Result<actions::ActionMetadata, String> {
    metadata_for_action(action_name)
        .ok_or_else(|| format!("action is not present in the trusted catalog: {action_name}"))
}

fn state_name(state: NodeState) -> &'static str {
    match state {
        NodeState::Pending => "pending",
        NodeState::Ready => "ready",
        NodeState::Running => "running",
        NodeState::WaitingHuman => "waitingHuman",
        NodeState::Blocked => "blocked",
        NodeState::Executed => "executed",
        NodeState::FailedRetryable => "failedRetryable",
        NodeState::Failed => "failed",
        NodeState::Cancelled => "cancelled",
        NodeState::Skipped => "skipped",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(ConfirmationBroker::default()))
        .invoke_handler(tauri::generate_handler![run_workflow, resolve_confirmation])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ApproveAll;

    #[async_trait]
    impl ConfirmationHandler for ApproveAll {
        async fn confirm(&self, _request: ConfirmationRequest) -> Result<bool, ActionError> {
            Ok(true)
        }
    }

    #[tokio::test]
    async fn executes_yaml_with_references() {
        let result = run_workflow_inner(
            r#"
version: 1
id: test-flow
steps:
  - id: source
    action: text
    inputs:
      value: hello
  - id: result
    action: uppercase
    inputs:
      text: "${source}"
"#
            .to_string(),
            None,
            None,
            Arc::new(ApproveAll),
        )
        .await
        .expect("workflow should execute");

        assert!(result.success);
        assert_eq!(
            result.outputs.get("result").map(String::as_str),
            Some("HELLO")
        );
        assert!(result.audit.iter().any(|entry| {
            entry.node_id == "result" && entry.from == "running" && entry.to == "executed"
        }));
    }

    #[test]
    fn subagent_catalog_contains_every_registered_action_signature() {
        let catalog = subagent_action_catalog();
        for action_name in ANDROID_ACTION_NAMES
            .iter()
            .chain(["text", "uppercase", "subagent"].iter())
        {
            assert!(
                catalog.contains(&format!("{action_name}(")),
                "missing signature for {action_name}"
            );
        }
        assert!(!catalog.contains("最终只输出可解析的 YAML"));
        assert!(!catalog.contains("## 生成规则"));
    }
}
