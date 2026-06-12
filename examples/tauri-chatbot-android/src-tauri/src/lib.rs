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
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::oneshot;

const ACTION_CATALOG_DOCUMENT: &str =
    include_str!("../../../../docs/action_fabric/action-catalog-for-llm.md");
const DEFAULT_MODEL_URL: &str = "http://10.0.2.2:8000";
const DEFAULT_MODEL: &str = "local-model";
const DEFAULT_GRPC_ENDPOINT: &str = "127.0.0.1:8080";
const MAX_CHAT_TURNS: u32 = 16;
const REQUEST_TIMEOUT_MS: u64 = 120_000;

const CHAT_SYSTEM_PROMPT: &str = r#"你是运行在 Android 设备上的 Action Fabric 助手。

你可以用两种方式回复：
1. 如果不需要操作设备，直接回复用户自然语言。任何普通文本都会结束本轮 agent loop。
2. 如果需要读取状态或执行操作，只回复一个 ActionFlow YAML。workflow 成功后，系统会把其顶层 output 直接作为最终消息返回给用户，本轮立即结束，你不会再收到成功结果。只有 workflow 失败时，系统才会把已执行节点和诊断作为 tool 消息返回给你，供你修正 workflow 或向用户解释。

ActionFlow 基本格式：
version: 1
id: concise-unique-id
output: final_step
steps:
  - id: step_id
    action: action_name
    inputs:
      field: value
outputContract: json

严格规则：
- 生成 workflow 时，整条回复必须是可直接解析的纯 YAML。第一个非空字符必须属于 `version: 1` 这一行。
- 禁止在 YAML 前后添加 `ActionFlow YAML:`、`YAML:`、说明文字、总结、注释或任何其他前后缀。
- 禁止使用 Markdown 代码围栏（例如 ```yaml）。
- 只能使用可信 action catalog 中存在的 action 和输入字段。
- 不要生成 policy、sideEffect、retryBudget 或 timeoutMs，策略由可信 registry 注入。
- 用 ${step_id} 引用上游完整输出；不支持字段级引用。
- 多个无后继节点时必须设置顶层 output。
- 可并行的只读操作应写成无依赖步骤。
- workflow 的顶层 output 必须指向你希望直接展示给用户的最终节点。该节点输出应当本身就是适合用户阅读的最终结果。
- 收到失败 tool 消息后不要机械复述 JSON；可以生成修正后的纯 YAML workflow，或用普通文本清晰解释失败情况。"#;

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

#[async_trait]
impl Action for TextAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        ActionOutput {
            payload: extract_text(&input.payload, "value").into_bytes(),
            error: None,
        }
    }
}

struct UppercaseAction;

#[async_trait]
impl Action for UppercaseAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        ActionOutput {
            payload: extract_text(&input.payload, "text")
                .to_uppercase()
                .into_bytes(),
            error: None,
        }
    }
}

struct ChatRegistryFactory {
    grpc_client: GrpcClient,
}

impl ActionRegistryFactory for ChatRegistryFactory {
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
                    model: DEFAULT_MODEL.to_string(),
                    base_url: "http://10.0.2.2:8080".to_string(),
                    api_key: None,
                    max_turns: 4,
                    temperature: 0.2,
                    request_timeout_ms: 60_000,
                    system_prompt: None,
                    action_catalog: action_catalog(),
                },
            )),
            required_metadata("subagent").map_err(ActionError::new)?,
        );
        for action_name in ANDROID_ACTION_NAMES {
            registry.register_remote_with_metadata(
                *action_name,
                RemoteAction::from_grpc(self.grpc_client.clone(), *action_name),
                required_metadata(action_name).map_err(ActionError::new)?,
            );
        }
        Ok(registry)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatConfig {
    model_base_url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    temperature: Option<f32>,
    max_turns: Option<u32>,
    grpc_endpoint: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatLoopRequest {
    message: String,
    history: Vec<ChatMessage>,
    config: ChatConfig,
}

#[derive(Serialize)]
struct LlmRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
}

#[derive(Deserialize)]
struct LlmResponse {
    choices: Vec<LlmChoice>,
}

#[derive(Deserialize)]
struct LlmChoice {
    message: ChatMessage,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentStatusEvent {
    kind: String,
    turn: u32,
    message: String,
    yaml: Option<String>,
    workflow: Option<WorkflowReport>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowReport {
    plan_id: String,
    success: bool,
    output_node: Option<String>,
    final_output: Option<String>,
    node_states: BTreeMap<String, String>,
    executed_outputs: BTreeMap<String, String>,
    diagnostics: Vec<DiagnosticEntry>,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticEntry {
    node_id: String,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatLoopResponse {
    message: String,
    turns: u32,
    workflows: Vec<WorkflowReport>,
}

#[tauri::command]
async fn run_chat_loop(
    app: AppHandle,
    confirmation_broker: State<'_, Arc<ConfirmationBroker>>,
    request: ChatLoopRequest,
) -> Result<ChatLoopResponse, String> {
    if request.message.trim().is_empty() {
        return Err("message must not be empty".to_string());
    }

    let config = request.config;
    let model_url = non_empty(config.model_base_url, DEFAULT_MODEL_URL);
    let model = non_empty(config.model, DEFAULT_MODEL);
    let grpc_endpoint = non_empty(config.grpc_endpoint, DEFAULT_GRPC_ENDPOINT);
    let temperature = config.temperature.unwrap_or(0.2).clamp(0.0, 2.0);
    let max_turns = config.max_turns.unwrap_or(8).clamp(1, MAX_CHAT_TURNS);
    let confirmation_handler: Arc<dyn ConfirmationHandler> = Arc::new(TauriConfirmationHandler {
        app: app.clone(),
        broker: Arc::clone(confirmation_broker.inner()),
    });

    let mut history = vec![ChatMessage {
        role: "system".to_string(),
        content: format!(
            "{CHAT_SYSTEM_PROMPT}\n\n可信 action catalog：\n\n{}",
            action_catalog()
        ),
        name: None,
    }];
    history.extend(
        request
            .history
            .into_iter()
            .filter(|message| matches!(message.role.as_str(), "user" | "assistant")),
    );
    history.push(ChatMessage {
        role: "user".to_string(),
        content: request.message,
        name: None,
    });

    let http = Client::builder()
        .timeout(Duration::from_millis(REQUEST_TIMEOUT_MS))
        .build()
        .map_err(|error| error.to_string())?;
    let mut workflows = Vec::new();

    for turn in 1..=max_turns {
        emit_status(&app, "thinking", turn, "正在请求模型", None, None);
        let assistant = call_llm(
            &http,
            &model_url,
            &model,
            config.api_key.as_deref(),
            temperature,
            &history,
        )
        .await?;
        history.push(assistant.clone());

        let Some(yaml) = extract_yaml(&assistant.content) else {
            emit_status(&app, "complete", turn, "模型已返回最终消息", None, None);
            return Ok(ChatLoopResponse {
                message: assistant.content,
                turns: turn,
                workflows,
            });
        };

        emit_status(
            &app,
            "workflow",
            turn,
            "模型生成了 workflow",
            Some(yaml.clone()),
            None,
        );
        emit_status(&app, "executing", turn, "正在执行 workflow", None, None);
        let report =
            execute_workflow(yaml, &grpc_endpoint, Arc::clone(&confirmation_handler)).await;
        emit_status(
            &app,
            if report.success {
                "workflowSuccess"
            } else {
                "workflowFailure"
            },
            turn,
            if report.success {
                "workflow 执行成功，output 已作为最终消息返回"
            } else {
                "workflow 未完整执行，已反馈已执行节点"
            },
            None,
            Some(report.clone()),
        );
        if report.success {
            let message = report.final_output.clone().ok_or_else(|| {
                "successful workflow did not produce its final output".to_string()
            })?;
            workflows.push(report);
            emit_status(
                &app,
                "complete",
                turn,
                "workflow output 已直接返回用户",
                None,
                None,
            );
            return Ok(ChatLoopResponse {
                message,
                turns: turn,
                workflows,
            });
        }

        history.push(ChatMessage {
            role: "tool".to_string(),
            content: workflow_failure_feedback(&report),
            name: Some("dispatcher".to_string()),
        });
        workflows.push(report);
    }

    Err(format!(
        "agent loop reached the maximum of {max_turns} turns"
    ))
}

async fn call_llm(
    http: &Client,
    base_url: &str,
    model: &str,
    api_key: Option<&str>,
    temperature: f32,
    history: &[ChatMessage],
) -> Result<ChatMessage, String> {
    let mut request = http
        .post(normalize_chat_endpoint(base_url))
        .json(&LlmRequest {
            model: model.to_string(),
            messages: history.to_vec(),
            temperature,
        });
    if let Some(api_key) = api_key.filter(|key| !key.trim().is_empty()) {
        request = request.bearer_auth(api_key);
    }
    let response = request
        .send()
        .await
        .map_err(|error| format!("LLM request failed: {error}"))?
        .error_for_status()
        .map_err(|error| format!("LLM returned an error: {error}"))?
        .json::<LlmResponse>()
        .await
        .map_err(|error| format!("invalid LLM response: {error}"))?;
    response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message)
        .ok_or_else(|| "LLM response has no choices".to_string())
}

async fn execute_workflow(
    yaml: String,
    grpc_endpoint: &str,
    confirmation_handler: Arc<dyn ConfirmationHandler>,
) -> WorkflowReport {
    let mut plan = match load_action_flow_from_str(&yaml) {
        Ok(plan) => plan,
        Err(error) => return failed_report("invalid-workflow", error.to_string()),
    };
    for node in plan.nodes.values_mut() {
        let metadata = match required_metadata(&node.action) {
            Ok(metadata) => metadata,
            Err(error) => return failed_report(&plan.id, error),
        };
        apply_action_metadata(node, &metadata);
    }

    let plan_id = plan.id.clone();
    let output_node = plan.output_node.clone();
    let factory: Arc<dyn ActionRegistryFactory> = Arc::new(ChatRegistryFactory {
        grpc_client: GrpcClient::new(grpc_endpoint.to_string()),
    });
    let tools: Arc<dyn ToolExecutor> = Arc::new(DispatcherToolExecutor::with_confirmation_handler(
        Arc::clone(&factory),
        Arc::clone(&confirmation_handler),
    ));
    let registry = match factory.create_registry(tools) {
        Ok(registry) => Arc::new(registry),
        Err(error) => return failed_report(&plan_id, error.message),
    };

    let executor = ActionExecutor::new(Arc::clone(&registry), Arc::new(plan.clone()));
    let output_reader = executor.clone();
    let mut engine = Engine {
        state: GlobalState::new(&plan),
        dispatcher: Dispatcher::new(Box::new(TopoPolicy)),
        executor: Box::new(executor),
        recovery: Box::new(SimpleRecovery::default()),
        audit_log: Box::new(InMemoryAuditLog::default()),
        state_store: Box::new(InMemoryStateStore::default()),
        diagnostic: Default::default(),
        plan,
    };

    let run_error = run_with_confirmations(
        &mut engine,
        &ExecutionContext::default(),
        confirmation_handler.as_ref(),
    )
    .await
    .err();
    let node_states = engine
        .state
        .nodes
        .iter()
        .map(|(id, state)| (id.clone(), state_name(*state).to_string()))
        .collect::<BTreeMap<_, _>>();
    let success = run_error.is_none()
        && engine
            .state
            .nodes
            .values()
            .all(|state| *state == NodeState::Executed);
    let executed_outputs = output_reader
        .outputs()
        .into_iter()
        .map(|(id, bytes)| (id, String::from_utf8_lossy(&bytes).into_owned()))
        .collect::<BTreeMap<_, _>>();
    let final_output = success
        .then(|| executed_outputs.get(&output_node).cloned())
        .flatten();
    let diagnostics = engine
        .diagnostic
        .history
        .iter()
        .map(|record| DiagnosticEntry {
            node_id: record.node_id.clone(),
            message: record.message.clone(),
        })
        .collect();

    WorkflowReport {
        plan_id,
        success,
        output_node: Some(output_node),
        final_output,
        node_states,
        executed_outputs,
        diagnostics,
        error: run_error,
    }
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

fn workflow_failure_feedback(report: &WorkflowReport) -> String {
    debug_assert!(!report.success);
    json!({
        "ok": false,
        "planId": report.plan_id,
        "nodeStates": report.node_states,
        "executedNodeResults": report.executed_outputs,
        "diagnostics": report.diagnostics,
        "error": report.error,
    })
    .to_string()
}

fn failed_report(plan_id: &str, error: String) -> WorkflowReport {
    WorkflowReport {
        plan_id: plan_id.to_string(),
        success: false,
        output_node: None,
        final_output: None,
        node_states: BTreeMap::new(),
        executed_outputs: BTreeMap::new(),
        diagnostics: Vec::new(),
        error: Some(error),
    }
}

fn emit_status(
    app: &AppHandle,
    kind: &str,
    turn: u32,
    message: &str,
    yaml: Option<String>,
    workflow: Option<WorkflowReport>,
) {
    let _ = app.emit(
        "agent-status",
        AgentStatusEvent {
            kind: kind.to_string(),
            turn,
            message: message.to_string(),
            yaml,
            workflow,
        },
    );
}

fn extract_text(payload: &[u8], preferred_key: &str) -> String {
    let Ok(value) = serde_json::from_slice::<Value>(payload) else {
        return String::from_utf8_lossy(payload).into_owned();
    };
    value
        .get(preferred_key)
        .and_then(Value::as_str)
        .or_else(|| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn extract_yaml(content: &str) -> Option<String> {
    let trimmed = content.trim();
    if is_action_flow_yaml(trimmed) {
        return Some(trimmed.to_string());
    }
    for prefix in ["ActionFlow YAML:", "ActionFlow YAML："] {
        if let Some(candidate) = trimmed.strip_prefix(prefix).map(str::trim)
            && is_action_flow_yaml(candidate)
        {
            return Some(candidate.to_string());
        }
    }
    let mut lines = content.lines();
    while let Some(line) = lines.next() {
        if line.trim().eq_ignore_ascii_case("```yaml") || line.trim().eq_ignore_ascii_case("```yml")
        {
            let block = lines
                .by_ref()
                .take_while(|line| !line.trim_start().starts_with("```"))
                .collect::<Vec<_>>()
                .join("\n");
            return (!block.trim().is_empty()).then(|| block.trim().to_string());
        }
    }
    None
}

fn is_action_flow_yaml(content: &str) -> bool {
    content.starts_with("version:") && content.contains("\nsteps:")
}

fn normalize_chat_endpoint(base: &str) -> String {
    let trimmed = base.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1/chat/completions") {
        trimmed.to_string()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    }
}

fn non_empty(value: Option<String>, fallback: &str) -> String {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn action_catalog() -> String {
    ACTION_CATALOG_DOCUMENT
        .split_once("## 内置 Action（39）")
        .and_then(|(_, actions)| actions.split_once("## 输出结构"))
        .map(|(signatures, _)| signatures.trim().to_string())
        .expect("action catalog document must contain action signatures")
}

fn required_metadata(action_name: &str) -> Result<actions::ActionMetadata, String> {
    metadata_for_action(action_name)
        .ok_or_else(|| format!("action is not present in the trusted catalog: {action_name}"))
}

fn risk_name(risk: actions::ActionRisk) -> &'static str {
    match risk {
        actions::ActionRisk::Low => "low",
        actions::ActionRisk::Medium => "medium",
        actions::ActionRisk::High => "high",
        actions::ActionRisk::Critical => "critical",
    }
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
        .invoke_handler(tauri::generate_handler![
            run_chat_loop,
            resolve_confirmation
        ])
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

    #[test]
    fn extracts_plain_fenced_and_prefixed_yaml() {
        let prefixed_alarm = r#"ActionFlow YAML:
version: 1
id: set-alarm-830
output: set_alarm_result
steps:
  - id: set_alarm_result
    action: set_alarm
    inputs:
      hour: 8
      minutes: 30
      message: "Alarm at 8:30"
      skipUi: true
outputContract: json"#;

        assert!(extract_yaml("普通消息").is_none());
        assert_eq!(
            extract_yaml("```yaml\nversion: 1\nsteps: []\n```").as_deref(),
            Some("version: 1\nsteps: []")
        );
        assert_eq!(
            extract_yaml("version: 1\nid: demo\nsteps:\n  - id: a").as_deref(),
            Some("version: 1\nid: demo\nsteps:\n  - id: a")
        );
        let yaml = extract_yaml(prefixed_alarm).expect("prefixed workflow should be extracted");
        let plan = load_action_flow_from_str(&yaml).expect("extracted workflow should parse");
        assert_eq!(plan.id, "set-alarm-830");
        assert_eq!(plan.output_node, "set_alarm_result");
    }

    #[test]
    fn system_prompt_requires_bare_yaml_and_direct_success_output() {
        assert!(CHAT_SYSTEM_PROMPT.contains("禁止在 YAML 前后添加 `ActionFlow YAML:`"));
        assert!(CHAT_SYSTEM_PROMPT.contains("顶层 output 直接作为最终消息返回给用户"));
        assert!(CHAT_SYSTEM_PROMPT.contains("只有 workflow 失败时"));
    }

    #[tokio::test]
    async fn successful_workflow_exposes_final_output() {
        let report = execute_workflow(
            r#"
version: 1
id: feedback-test
output: result
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
            DEFAULT_GRPC_ENDPOINT,
            Arc::new(ApproveAll),
        )
        .await;

        assert!(report.success);
        assert_eq!(report.final_output.as_deref(), Some("HELLO"));
    }

    #[test]
    fn failed_feedback_includes_executed_nodes() {
        let report = WorkflowReport {
            plan_id: "failed".to_string(),
            success: false,
            output_node: Some("b".to_string()),
            final_output: None,
            node_states: BTreeMap::from([
                ("a".to_string(), "executed".to_string()),
                ("b".to_string(), "failed".to_string()),
            ]),
            executed_outputs: BTreeMap::from([("a".to_string(), "done".to_string())]),
            diagnostics: vec![DiagnosticEntry {
                node_id: "b".to_string(),
                message: "boom".to_string(),
            }],
            error: None,
        };
        let feedback: Value =
            serde_json::from_str(&workflow_failure_feedback(&report)).expect("valid feedback");
        assert_eq!(feedback["executedNodeResults"]["a"], "done");
        assert_eq!(feedback["nodeStates"]["b"], "failed");
    }
}
