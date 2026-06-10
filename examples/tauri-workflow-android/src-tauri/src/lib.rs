use actions::client::{GrpcClient, RemoteAction};
use actions::catalog::metadata_for_action;
use actions::{Action, ActionInput, ActionOutput, ActionRegistry};
use async_trait::async_trait;
use dispatcher::scheduler::{Dispatcher, TopoPolicy};
use dispatcher::{
    ActionExecutor, Engine, ExecutionContext, GlobalState, InMemoryAuditLog, InMemoryStateStore,
    NodeState, SimpleRecovery, load_action_flow_from_str,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

struct EchoAction;

#[async_trait]
impl Action for EchoAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        ActionOutput {
            payload: input.payload,
            error: None,
        }
    }
}

struct TextAction;

#[async_trait]
impl Action for TextAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        let payload = extract_text(&input.payload, "value");
        ActionOutput {
            payload: payload.into_bytes(),
            error: None,
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
    yaml: String,
    input: Option<String>,
    grpc_endpoint: Option<String>,
) -> Result<WorkflowResult, String> {
    let plan = load_action_flow_from_str(&yaml).map_err(|error| error.to_string())?;
    let plan_id = plan.id.clone();
    let grpc_endpoint = grpc_endpoint
        .filter(|endpoint| !endpoint.trim().is_empty())
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    let mut registry = ActionRegistry::default();
    registry.register_local_with_metadata(
        "echo",
        Arc::new(EchoAction),
        required_metadata("echo")?,
    );
    registry.register_local_with_metadata(
        "text",
        Arc::new(TextAction),
        required_metadata("text")?,
    );
    registry.register_local_with_metadata(
        "uppercase",
        Arc::new(UppercaseAction),
        required_metadata("uppercase")?,
    );
    let remote_actions = plan
        .nodes
        .values()
        .map(|node| node.action.as_str())
        .filter(|action| !matches!(*action, "echo" | "text" | "uppercase"))
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let grpc_client = GrpcClient::new(grpc_endpoint);
    for action_name in remote_actions {
        let metadata = required_metadata(&action_name)?;
        registry.register_remote_with_metadata(
            action_name.clone(),
            RemoteAction::from_grpc(grpc_client.clone(), action_name),
            metadata,
        );
    }
    let registry = Arc::new(registry);

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

    engine
        .run(&ExecutionContext {
            inputs: input.unwrap_or_default().into_bytes(),
        })
        .await
        .map_err(|error| error.to_string())?;

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
        .invoke_handler(tauri::generate_handler![run_workflow])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn executes_yaml_with_references() {
        let result = run_workflow(
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
}
