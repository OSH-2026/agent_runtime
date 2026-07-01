use actions::catalog::{ANDROID_ACTION_NAMES, metadata_for_action};
use actions::client::{GrpcClient, RemoteAction};
use actions::{
    Action, ActionError, ActionInput, ActionOutput, ActionRegistry, SubagentAction, SubagentConfig,
    ToolExecutor,
};
use async_trait::async_trait;
use dispatcher::scheduler::{Dispatcher, TopoPolicy};
use dispatcher::{
    ActionExecutor, ActionFlowFile, ActionRegistryFactory, ConfirmationHandler,
    ConfirmationRequest, DispatcherToolExecutor, Engine, ExecutionContext, ExecutionPlan,
    GlobalState, InMemoryAuditLog, InMemoryStateStore, NodeState, SimpleRecovery,
    apply_action_metadata, load_action_flow_from_str,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::oneshot;

const ACTION_CATALOG_DOCUMENT: &str =
    include_str!("../../../../docs/action_fabric/action-catalog-for-llm.md");
const DEFAULT_MODEL_ENDPOINT: &str = "http://10.0.2.2:8080/v1/chat/completions";
const DEFAULT_MODEL: &str = "local-model";
const DEFAULT_GRPC_ENDPOINT: &str = "127.0.0.1:8080";
const MAX_CHAT_TURNS: u32 = 16;
const REQUEST_TIMEOUT_MS: u64 = 120_000;
const WORKFLOW_CONFIRMATION_ACTION: &str = "__workflow_confirmation__";

const CHAT_SYSTEM_PROMPT: &str = r#"你是运行在 Android 设备上的 Action Chat 主助手。你负责理解用户请求，并决定是否调用设备能力。

你只能用两种方式回复：

1. 直接回答
不需要读取设备状态或执行设备操作时，直接用用户语言自然回复。直接回答不会执行 workflow，也不会渲染 `${step_id}`。

2. 执行 workflow
需要读取设备状态、本机数据或执行设备操作时，回复必须以 fenced YAML workflow 开头，并在 closing fence 后写最终消息模板。workflow 成功后，系统会渲染最终消息模板；workflow 失败时，系统会把诊断作为 tool 消息发回给你。

示例：用户问“查看当前设备、网络和电量状态，并给我一份简洁摘要。”

```yaml
version: 1
id: device-status-summary
steps:
  - id: device
    action: device_info
    inputs:
      includeHardware: true
  - id: network
    action: network_status
    inputs:
      includeDetails: true
  - id: power
    action: power_status
    inputs:
      includeDetails: true
  - id: final_report
    action: subagent
    inputs:
      prompt: "用中文简洁总结以下设备、网络和电量信息；如果某项不可用，就自然说明。设备：${device}；网络：${network}；电量：${power}"
```

${final_report}

规则：
- 生成 workflow 时，第一个非空字符必须是开头代码围栏 ```。
- 只使用可信 action catalog 中存在的 action 和输入字段。
- 用 `${step_id}` 引用步骤完整输出，不要假设可以引用 JSON 内部字段。
- 可并行的只读步骤应写成互不依赖的步骤。
- 简单成功确认、完成说明和短结果直接写在最终消息模板中，不要交给 subagent。
- 只有当步骤输出较长、结构化或需要提炼时，才使用 subagent。
- 给 subagent 的 prompt 只处理已提供的 `${step_id}` 内容，不要复述用户原始任务；例如在需要总结剪贴板内容时，给 subagent 的 prompt 写成“用中文总结以下文本；如果没有文本，输出‘剪贴板为空’。文本：${clip}”，不要写成“查看剪贴板并总结”。
- 最终回复直接回答用户请求，不暴露 action 名、workflow、JSON、路径、包名或调度细节，除非用户明确要求。"#;

const SUBAGENT_SYSTEM_PROMPT: &str = r#"你是 Action Chat 的结果整理子代理。

你的主要任务是把上游 action 输出整理成用户可读的自然语言。默认直接返回 plain final answer；只有当任务确实需要继续读取数据或执行工具时，才生成 workflow。

你可以用两种方式回复：

1. 直接回答
直接输出可展示给用户的自然语言内容。

2. 调用工具
回复必须以 fenced YAML workflow 开头，随后在 closing fence 后写最终消息模板。

格式：

```yaml
version: 1
id: concise-unique-id
steps:
  - id: step_id
    action: action_name
    inputs:
      field: value
```

最终消息模板写在 YAML 代码块之后，可用 `${step_id}` 插入步骤输出。

规则：
- 使用上游 prompt 指定的语言；如果没有指定，默认中文。
- 不要添加 Final answer、Answer、Result、摘要：等模板前缀。
- 不要输出 JSON、原始字段名、action 名、workflow、路径、包名或调度细节，除非用户明确要求技术信息。
- 对设备状态、网络、电量、剪贴板、媒体等结果，只提炼对用户有意义的信息。
- 如果输入为空、权限不足、结果不可用或 action 失败，简洁说明无法完成的原因。
- 保持简洁、自然、准确；不要编造输入中没有的信息。"#;

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

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmationItem {
    node_id: String,
    action: String,
    inputs: Option<Value>,
    risk: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkflowConfirmationPayload {
    workflow_id: String,
    items: Vec<ConfirmationItem>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmationEvent {
    request_id: String,
    node_id: String,
    action: String,
    inputs: Option<Value>,
    risk: String,
    workflow_id: Option<String>,
    items: Vec<ConfirmationItem>,
}

#[async_trait]
impl ConfirmationHandler for TauriConfirmationHandler {
    async fn confirm(&self, request: ConfirmationRequest) -> Result<bool, ActionError> {
        let event_details = confirmation_event_details(request);
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
            node_id: event_details.node_id,
            action: event_details.action,
            inputs: event_details.inputs,
            risk: event_details.risk,
            workflow_id: event_details.workflow_id,
            items: event_details.items,
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

struct ConfirmationEventDetails {
    node_id: String,
    action: String,
    inputs: Option<Value>,
    risk: String,
    workflow_id: Option<String>,
    items: Vec<ConfirmationItem>,
}

fn confirmation_event_details(request: ConfirmationRequest) -> ConfirmationEventDetails {
    let ConfirmationRequest {
        node_id,
        action,
        inputs,
        risk,
    } = request;
    let risk = risk_name(risk).to_string();

    if action == WORKFLOW_CONFIRMATION_ACTION {
        if let Some(inputs_value) = inputs.clone() {
            if let Ok(payload) = serde_json::from_value::<WorkflowConfirmationPayload>(inputs_value)
            {
                return ConfirmationEventDetails {
                    node_id: payload.workflow_id.clone(),
                    action: "workflow".to_string(),
                    inputs: None,
                    risk,
                    workflow_id: Some(payload.workflow_id),
                    items: payload.items,
                };
            }
        }
    }

    ConfirmationEventDetails {
        node_id: node_id.clone(),
        action: action.clone(),
        inputs: inputs.clone(),
        risk: risk.clone(),
        workflow_id: None,
        items: vec![ConfirmationItem {
            node_id,
            action,
            inputs,
            risk,
        }],
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
    model_config: ModelRuntimeConfig,
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
                self.model_config.subagent_config(),
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

#[derive(Clone, Debug)]
struct ModelRuntimeConfig {
    model: String,
    endpoint: String,
    api_key: Option<String>,
    temperature: f32,
}

impl ModelRuntimeConfig {
    fn subagent_config(&self) -> SubagentConfig {
        SubagentConfig {
            model: self.model.clone(),
            endpoint: self.endpoint.clone(),
            api_key: self.api_key.clone(),
            max_turns: 4,
            temperature: self.temperature,
            request_timeout_ms: 60_000,
            system_prompt: Some(SUBAGENT_SYSTEM_PROMPT.to_string()),
            action_catalog: action_catalog(),
        }
    }
}

#[cfg(test)]
fn default_model_runtime_config() -> ModelRuntimeConfig {
    ModelRuntimeConfig {
        model: DEFAULT_MODEL.to_string(),
        endpoint: DEFAULT_MODEL_ENDPOINT.to_string(),
        api_key: None,
        temperature: 0.2,
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
    model_endpoint: Option<String>,
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

    let ChatConfig {
        model_endpoint,
        model,
        api_key,
        temperature,
        max_turns,
        grpc_endpoint,
    } = request.config;
    let model_config = ModelRuntimeConfig {
        model: non_empty(model, DEFAULT_MODEL),
        endpoint: non_empty(model_endpoint, DEFAULT_MODEL_ENDPOINT),
        api_key: api_key.filter(|key| !key.trim().is_empty()),
        temperature: temperature.unwrap_or(0.2).clamp(0.0, 2.0),
    };
    let grpc_endpoint = non_empty(grpc_endpoint, DEFAULT_GRPC_ENDPOINT);
    let max_turns = max_turns.unwrap_or(8).clamp(1, MAX_CHAT_TURNS);
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
            &model_config.endpoint,
            &model_config.model,
            model_config.api_key.as_deref(),
            model_config.temperature,
            &history,
        )
        .await?;
        history.push(assistant.clone());

        let workflow_message = match extract_workflow_message(&assistant.content) {
            Ok(Some(message)) => message,
            Ok(None) => {
                emit_status(&app, "complete", turn, "模型已返回最终消息", None, None);
                return Ok(ChatLoopResponse {
                    message: assistant.content,
                    turns: turn,
                    workflows,
                });
            }
            Err(error) => {
                let report = failed_report("invalid-workflow-response", error);
                emit_status(
                    &app,
                    "workflowFailure",
                    turn,
                    "workflow 回复格式无效，已反馈模型",
                    None,
                    Some(report.clone()),
                );
                history.push(ChatMessage {
                    role: "tool".to_string(),
                    content: workflow_failure_feedback(&report),
                    name: Some("dispatcher".to_string()),
                });
                workflows.push(report);
                continue;
            }
        };

        if let Err((plan_id, error)) = preflight_workflow_message(&workflow_message) {
            let report = failed_report(&plan_id, error);
            emit_status(
                &app,
                "workflowFailure",
                turn,
                "workflow 回复格式无效，已反馈模型",
                Some(workflow_message.yaml.clone()),
                Some(report.clone()),
            );
            history.push(ChatMessage {
                role: "tool".to_string(),
                content: workflow_failure_feedback(&report),
                name: Some("dispatcher".to_string()),
            });
            workflows.push(report);
            continue;
        }

        emit_status(
            &app,
            "workflow",
            turn,
            "模型生成了 workflow",
            Some(workflow_message.yaml.clone()),
            None,
        );
        emit_status(&app, "executing", turn, "正在执行 workflow", None, None);
        let mut report = execute_workflow(
            workflow_message.yaml,
            &grpc_endpoint,
            model_config.clone(),
            Arc::clone(&confirmation_handler),
        )
        .await;
        let mut final_message_error = None;
        let rendered_message = if report.success {
            match render_final_message(
                &workflow_message.final_message_template,
                &report.executed_outputs,
            ) {
                Ok(message) => {
                    report.final_output = Some(message.clone());
                    Some(message)
                }
                Err(error) => {
                    let message = format!("final message template is invalid: {error}");
                    report.success = false;
                    report.error = Some(message.clone());
                    final_message_error = Some(message);
                    None
                }
            }
        } else {
            None
        };
        emit_status(
            &app,
            if report.success {
                "workflowSuccess"
            } else {
                "workflowFailure"
            },
            turn,
            if report.success {
                "workflow 执行成功，最终消息已渲染"
            } else {
                "workflow 未完整执行，已反馈已执行节点"
            },
            None,
            Some(report.clone()),
        );
        if let Some(error) = final_message_error {
            workflows.push(report);
            return Err(format!("workflow completed, but {error}"));
        }
        if report.success {
            let message = rendered_message.ok_or_else(|| {
                "successful workflow did not produce its final message".to_string()
            })?;
            workflows.push(report);
            emit_status(
                &app,
                "complete",
                turn,
                "workflow 最终消息已返回用户",
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
    endpoint: &str,
    model: &str,
    api_key: Option<&str>,
    temperature: f32,
    history: &[ChatMessage],
) -> Result<ChatMessage, String> {
    let mut request = http.post(endpoint).json(&LlmRequest {
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
    model_config: ModelRuntimeConfig,
    confirmation_handler: Arc<dyn ConfirmationHandler>,
) -> WorkflowReport {
    let step_order = workflow_step_order(&yaml);
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
        model_config,
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

    let run_error =
        match authorize_workflow(&mut engine, &step_order, confirmation_handler.as_ref()).await {
            Ok(()) => run_with_confirmations(
                &mut engine,
                &ExecutionContext::default(),
                confirmation_handler.as_ref(),
            )
            .await
            .err(),
            Err(error) => Some(error),
        };
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
        .then(|| {
            output_node
                .as_ref()
                .and_then(|node_id| executed_outputs.get(node_id).cloned())
        })
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
        output_node,
        final_output,
        node_states,
        executed_outputs,
        diagnostics,
        error: run_error,
    }
}

async fn authorize_workflow(
    engine: &mut Engine,
    step_order: &[String],
    confirmation_handler: &dyn ConfirmationHandler,
) -> Result<(), String> {
    let requests = workflow_confirmation_requests(&engine.plan, step_order);
    if requests.is_empty() {
        return Ok(());
    }

    let payload = WorkflowConfirmationPayload {
        workflow_id: engine.plan.id.clone(),
        items: requests
            .iter()
            .map(|request| ConfirmationItem {
                node_id: request.node_id.clone(),
                action: request.action.clone(),
                inputs: request.inputs.clone(),
                risk: risk_name(request.risk).to_string(),
            })
            .collect(),
    };
    let approved = confirmation_handler
        .confirm(ConfirmationRequest {
            node_id: engine.plan.id.clone(),
            action: WORKFLOW_CONFIRMATION_ACTION.to_string(),
            inputs: Some(serde_json::to_value(payload).map_err(|error| error.to_string())?),
            risk: highest_risk(&requests),
        })
        .await
        .map_err(|error| error.message)?;

    if approved {
        for request in requests {
            engine
                .approve_node(&request.node_id)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    } else {
        for request in requests {
            engine
                .reject_node(&request.node_id)
                .map_err(|error| error.to_string())?;
        }
        Err(format!("用户拒绝执行 workflow '{}'", engine.plan.id))
    }
}

fn workflow_confirmation_requests(
    plan: &ExecutionPlan,
    step_order: &[String],
) -> Vec<ConfirmationRequest> {
    let mut ordered_node_ids = Vec::new();
    let mut seen = HashSet::new();
    for node_id in step_order {
        if plan.nodes.contains_key(node_id) && seen.insert(node_id.clone()) {
            ordered_node_ids.push(node_id.clone());
        }
    }
    let mut remaining = plan
        .nodes
        .keys()
        .filter(|node_id| !seen.contains(*node_id))
        .cloned()
        .collect::<Vec<_>>();
    remaining.sort();
    ordered_node_ids.extend(remaining);

    ordered_node_ids
        .into_iter()
        .filter_map(|node_id| {
            let node = plan.nodes.get(&node_id)?;
            node.config.policy.requires_confirmation.then(|| {
                let action = node.action.clone();
                ConfirmationRequest {
                    node_id,
                    action: action.clone(),
                    inputs: node.inputs.clone(),
                    risk: metadata_for_action(&action)
                        .map(|metadata| metadata.risk)
                        .unwrap_or(actions::ActionRisk::High),
                }
            })
        })
        .collect()
}

fn workflow_step_order(yaml: &str) -> Vec<String> {
    serde_yaml::from_str::<ActionFlowFile>(yaml)
        .map(|flow| flow.steps.into_iter().map(|step| step.id).collect())
        .unwrap_or_default()
}

fn highest_risk(requests: &[ConfirmationRequest]) -> actions::ActionRisk {
    requests
        .iter()
        .map(|request| request.risk)
        .max_by_key(|risk| risk_rank(*risk))
        .unwrap_or(actions::ActionRisk::High)
}

fn risk_rank(risk: actions::ActionRisk) -> u8 {
    match risk {
        actions::ActionRisk::Low => 0,
        actions::ActionRisk::Medium => 1,
        actions::ActionRisk::High => 2,
        actions::ActionRisk::Critical => 3,
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
        "expectedResponseFormat": "For another workflow attempt, start with ```yaml fenced YAML and put the final message template after the closing fence. For a plain explanation, do not start with ```.",
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkflowMessage {
    yaml: String,
    final_message_template: String,
}

fn extract_workflow_message(content: &str) -> Result<Option<WorkflowMessage>, String> {
    let content = content.trim_start();
    if !content.starts_with("```") {
        return Ok(None);
    }
    let opening_line_end = content.find('\n').ok_or_else(|| {
        "workflow response must start with a fenced YAML block and include a final message"
            .to_string()
    })?;
    let fence_label = content[3..opening_line_end].trim();
    if !fence_label.is_empty()
        && !fence_label.eq_ignore_ascii_case("yaml")
        && !fence_label.eq_ignore_ascii_case("yml")
    {
        return Err("workflow fence must be ```yaml, ```yml, or ```".to_string());
    }

    let body = &content[opening_line_end + 1..];
    let mut cursor = 0usize;
    for line in body.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']).trim_start();
        if trimmed.starts_with("```") {
            let yaml = body[..cursor].trim();
            let final_message_template = body[cursor + line.len()..].trim();
            if yaml.is_empty() {
                return Err("workflow YAML block must not be empty".to_string());
            }
            if final_message_template.is_empty() {
                return Err(
                    "workflow response must include a final message after the closing fence"
                        .to_string(),
                );
            }
            return Ok(Some(WorkflowMessage {
                yaml: yaml.to_string(),
                final_message_template: final_message_template.to_string(),
            }));
        }
        cursor += line.len();
    }
    Err("workflow response is missing the closing ``` fence".to_string())
}

fn preflight_workflow_message(message: &WorkflowMessage) -> Result<(), (String, String)> {
    let plan = load_action_flow_from_str(&message.yaml)
        .map_err(|error| ("invalid-workflow-response".to_string(), error.to_string()))?;
    validate_final_message_template(
        &message.final_message_template,
        plan.nodes.keys().map(String::as_str),
    )
    .map_err(|error| {
        (
            plan.id.clone(),
            format!("final message template is invalid: {error}"),
        )
    })
}

fn validate_final_message_template<'a>(
    template: &str,
    node_ids: impl IntoIterator<Item = &'a str>,
) -> Result<(), String> {
    let node_ids = node_ids.into_iter().collect::<HashSet<_>>();
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

fn render_final_message(
    template: &str,
    node_outputs: &BTreeMap<String, String>,
) -> Result<String, String> {
    validate_final_message_template(template, node_outputs.keys().map(String::as_str))?;

    let mut result = String::new();
    let mut cursor = 0usize;
    while let Some(start) = template[cursor..].find("${") {
        let start_index = cursor + start;
        result.push_str(&template[cursor..start_index]);
        let name_start = start_index + 2;
        let end_index = template[name_start..]
            .find('}')
            .map(|offset| name_start + offset)
            .ok_or_else(|| format!("unclosed placeholder in final message: {template}"))?;
        let node_id = template[name_start..end_index].trim();
        if node_id.is_empty() {
            return Err("empty placeholder in final message".to_string());
        }
        let output = node_outputs
            .get(node_id)
            .ok_or_else(|| format!("final message references unknown node '{node_id}'"))?;
        result.push_str(output);
        cursor = end_index + 1;
    }
    result.push_str(&template[cursor..]);
    Ok(result)
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
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    struct ApproveAll;

    #[async_trait]
    impl ConfirmationHandler for ApproveAll {
        async fn confirm(&self, _request: ConfirmationRequest) -> Result<bool, ActionError> {
            Ok(true)
        }
    }

    struct RejectAndRecord {
        count: AtomicUsize,
        last_request: Mutex<Option<ConfirmationRequest>>,
    }

    impl RejectAndRecord {
        fn new() -> Self {
            Self {
                count: AtomicUsize::new(0),
                last_request: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl ConfirmationHandler for RejectAndRecord {
        async fn confirm(&self, request: ConfirmationRequest) -> Result<bool, ActionError> {
            self.count.fetch_add(1, AtomicOrdering::Relaxed);
            *self.last_request.lock().expect("confirmation lock") = Some(request);
            Ok(false)
        }
    }

    #[test]
    fn parses_fenced_workflow_response_and_treats_bare_yaml_as_plain() {
        let response = r#"```yaml
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
outputContract: json
```
已为你设置好 8:30 的闹钟。"#;

        assert!(extract_workflow_message("普通消息").unwrap().is_none());
        assert!(
            extract_workflow_message("version: 1\nid: demo\nsteps:\n  - id: a")
                .unwrap()
                .is_none()
        );
        let message = extract_workflow_message(response)
            .expect("workflow response should parse")
            .expect("workflow should be detected");
        assert_eq!(message.final_message_template, "已为你设置好 8:30 的闹钟。");
        let plan =
            load_action_flow_from_str(&message.yaml).expect("extracted workflow should parse");
        assert_eq!(plan.id, "set-alarm-830");
        assert_eq!(plan.output_node.as_deref(), Some("set_alarm_result"));
    }

    #[test]
    fn system_prompt_keeps_compact_workflow_guidance() {
        assert!(CHAT_SYSTEM_PROMPT.contains("Action Chat 主助手"));
        assert!(CHAT_SYSTEM_PROMPT.contains("closing fence 后"));
        assert!(CHAT_SYSTEM_PROMPT.contains("device-status-summary"));
        assert!(CHAT_SYSTEM_PROMPT.contains("power_status"));
        assert!(CHAT_SYSTEM_PROMPT.contains("不要交给 subagent"));
        assert!(CHAT_SYSTEM_PROMPT.contains("只处理已提供的 `${step_id}` 内容"));
        assert!(CHAT_SYSTEM_PROMPT.contains("剪贴板为空"));
        assert!(CHAT_SYSTEM_PROMPT.contains("${final_report}"));
        assert!(!CHAT_SYSTEM_PROMPT.contains("不要生成 policy"));
        assert!(!CHAT_SYSTEM_PROMPT.contains("sideEffect"));
        assert!(!CHAT_SYSTEM_PROMPT.contains("retryBudget"));
        assert!(!CHAT_SYSTEM_PROMPT.contains("timeoutMs"));
        assert!(!CHAT_SYSTEM_PROMPT.contains("顶层 output"));
        assert!(!CHAT_SYSTEM_PROMPT.contains("text 只接受 value"));
        assert!(!CHAT_SYSTEM_PROMPT.contains("wait_for"));
    }

    #[test]
    fn subagent_prompt_teaches_plain_answers_and_workflows() {
        assert!(SUBAGENT_SYSTEM_PROMPT.contains("结果整理子代理"));
        assert!(SUBAGENT_SYSTEM_PROMPT.contains("默认直接返回 plain final answer"));
        assert!(SUBAGENT_SYSTEM_PROMPT.contains("fenced YAML workflow"));
        assert!(SUBAGENT_SYSTEM_PROMPT.contains("version: 1"));
        assert!(SUBAGENT_SYSTEM_PROMPT.contains("${step_id}"));
        assert!(SUBAGENT_SYSTEM_PROMPT.contains("不要添加 Final answer"));
    }

    #[test]
    fn renders_final_message_from_node_outputs() {
        let message = render_final_message(
            "摘要：${report}；状态：${status}",
            &BTreeMap::from([
                ("report".to_string(), "网络正常".to_string()),
                ("status".to_string(), "电量充足".to_string()),
            ]),
        )
        .expect("template should render");

        assert_eq!(message, "摘要：网络正常；状态：电量充足");
        assert!(render_final_message("${missing}", &BTreeMap::new()).is_err());
        let literal_braces = render_final_message(
            "摘要：{report}",
            &BTreeMap::from([("report".to_string(), "网络正常".to_string())]),
        )
        .expect("bare braces should be treated as literal text");
        assert_eq!(literal_braces, "摘要：{report}");
    }

    #[test]
    fn default_model_endpoint_is_chat_completions_endpoint() {
        assert_eq!(
            DEFAULT_MODEL_ENDPOINT,
            "http://10.0.2.2:8080/v1/chat/completions"
        );
        assert_eq!(
            non_empty(
                Some(" http://10.0.2.2:8080/v1/chat/completions/ ".to_string()),
                DEFAULT_MODEL_ENDPOINT
            ),
            " http://10.0.2.2:8080/v1/chat/completions/ "
        );
    }

    #[test]
    fn subagent_config_uses_resolved_model_runtime_config() {
        let config = ModelRuntimeConfig {
            model: "custom-model".to_string(),
            endpoint: "http://10.0.2.2:8081/v1/chat/completions".to_string(),
            api_key: Some("secret".to_string()),
            temperature: 0.7,
        };

        let subagent = config.subagent_config();

        assert_eq!(subagent.model, "custom-model");
        assert_eq!(
            subagent.endpoint,
            "http://10.0.2.2:8081/v1/chat/completions"
        );
        assert_eq!(subagent.api_key.as_deref(), Some("secret"));
        assert_eq!(subagent.temperature, 0.7);
        assert_eq!(
            subagent.system_prompt.as_deref(),
            Some(SUBAGENT_SYSTEM_PROMPT)
        );
    }

    #[test]
    fn preflight_rejects_final_message_template_before_execution() {
        let workflow = WorkflowMessage {
            yaml: r#"
version: 1
id: preflight-test
output: result
steps:
  - id: result
    action: text
    inputs:
      value: done
"#
            .to_string(),
            final_message_template: "完成：${missing}".to_string(),
        };

        let (plan_id, error) = preflight_workflow_message(&workflow)
            .expect_err("unknown node should fail before execution");
        assert_eq!(plan_id, "preflight-test");
        assert!(error.contains("unknown node 'missing'"));
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
            default_model_runtime_config(),
            Arc::new(ApproveAll),
        )
        .await;

        assert!(report.success);
        assert_eq!(report.final_output.as_deref(), Some("HELLO"));
    }

    #[tokio::test]
    async fn text_action_rejects_extra_dependency_fields() {
        let report = execute_workflow(
            r#"
version: 1
id: friendly-confirmation
output: final_message
steps:
  - id: machine_result
    action: text
    inputs:
      value: '{"launched":true,"resolvedPackage":"internal.package"}'
  - id: final_message
    action: text
    inputs:
      value: "操作已成功完成。"
      wait_for: "${machine_result}"
"#
            .to_string(),
            DEFAULT_GRPC_ENDPOINT,
            default_model_runtime_config(),
            Arc::new(ApproveAll),
        )
        .await;

        assert!(!report.success);
        assert!(report.final_output.is_none());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|entry| entry.message.contains("unknown field `wait_for`"))
        );
    }

    #[tokio::test]
    async fn workflow_confirmation_batches_all_confirmable_nodes() {
        let handler = Arc::new(RejectAndRecord::new());
        let report = execute_workflow(
            r#"
version: 1
id: set-ten-alarms-630-700
steps:
  - id: alarm_1
    action: set_alarm
    inputs:
      hour: 6
      minutes: 30
      message: "Alarm 1"
      skipUi: true
  - id: alarm_2
    action: set_alarm
    inputs:
      hour: 6
      minutes: 40
      message: "Alarm 2"
      skipUi: true
  - id: alarm_3
    action: set_alarm
    inputs:
      hour: 6
      minutes: 50
      message: "Alarm 3"
      skipUi: true
  - id: alarm_4
    action: set_alarm
    inputs:
      hour: 7
      minutes: 0
      message: "Alarm 4"
      skipUi: true
  - id: alarm_5
    action: set_alarm
    inputs:
      hour: 6
      minutes: 35
      message: "Alarm 5"
      skipUi: true
  - id: alarm_6
    action: set_alarm
    inputs:
      hour: 6
      minutes: 45
      message: "Alarm 6"
      skipUi: true
  - id: alarm_7
    action: set_alarm
    inputs:
      hour: 6
      minutes: 55
      message: "Alarm 7"
      skipUi: true
  - id: alarm_8
    action: set_alarm
    inputs:
      hour: 7
      minutes: 5
      message: "Alarm 8"
      skipUi: true
  - id: alarm_9
    action: set_alarm
    inputs:
      hour: 7
      minutes: 15
      message: "Alarm 9"
      skipUi: true
  - id: alarm_10
    action: set_alarm
    inputs:
      hour: 7
      minutes: 25
      message: "Alarm 10"
      skipUi: true
"#
            .to_string(),
            DEFAULT_GRPC_ENDPOINT,
            default_model_runtime_config(),
            handler.clone(),
        )
        .await;

        assert!(!report.success);
        assert_eq!(handler.count.load(AtomicOrdering::Relaxed), 1);
        assert!(
            report
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("workflow")
        );
        let request = handler
            .last_request
            .lock()
            .expect("confirmation lock")
            .clone()
            .expect("confirmation request");
        assert_eq!(request.action, WORKFLOW_CONFIRMATION_ACTION);
        assert_eq!(request.node_id, "set-ten-alarms-630-700");
        assert_eq!(risk_name(request.risk), "medium");
        let payload: WorkflowConfirmationPayload =
            serde_json::from_value(request.inputs.expect("workflow payload")).expect("payload");
        assert_eq!(payload.workflow_id, "set-ten-alarms-630-700");
        assert_eq!(payload.items.len(), 10);
        assert_eq!(payload.items[0].node_id, "alarm_1");
        assert_eq!(payload.items[9].node_id, "alarm_10");
        assert!(payload.items.iter().all(|item| item.action == "set_alarm"));
        assert!(payload.items.iter().all(|item| item.risk == "medium"));
        assert!(
            report
                .node_states
                .values()
                .all(|state| state == "cancelled")
        );
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
