use crate::Action;
use crate::types::{ActionError, ActionInput, ActionOutput};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_SYSTEM_PROMPT: &str = r#"You are a subagent that can call tools by emitting fenced YAML workflows.

Reply in exactly one of these formats:
1. Plain final answer: if the first non-whitespace characters are not ```, the response is returned as-is. Plain answers do not execute workflows and do not process {node_id} placeholders.
2. Workflow request: start with a fenced YAML block, then place the final answer template after the closing fence. The final answer template is returned only if the workflow fully executes. Use {node_id} placeholders to insert complete outputs from executed nodes.

Workflow response format:
```yaml
version: 1
id: demo
steps:
  - id: A
    action: text
    inputs:
      value: "hello"
  - id: B
    action: uppercase
    inputs:
      text: "${A}"
```
Final answer with data from {B}.

Do not include execution policy fields in workflow YAML; policies are supplied by the trusted action registry.
Do not include top-level output, outputContract, or per-step outputs fields; the final message template decides what is returned.
Do not add extra fields to text actions; text only accepts value.
If a workflow fails, you receive a tool message with ok, code, message, and retryable fields. Then either return a corrected workflow response or a plain final answer.
"#;
const DEFAULT_MAX_TURNS: u32 = 8;
const MAX_TURNS: u32 = 32;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 60_000;
const MAX_REQUEST_TIMEOUT_MS: u64 = 120_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubagentInput {
    pub prompt: String,
}

#[derive(Clone, Debug)]
pub struct SubagentConfig {
    pub model: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub max_turns: u32,
    pub temperature: f32,
    pub request_timeout_ms: u64,
    pub system_prompt: Option<String>,
    pub action_catalog: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Clone, Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Clone, Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn validate_workflow_message(
        &self,
        yaml: &str,
        final_message_template: &str,
    ) -> Result<(), ActionError>;

    async fn execute_yaml(&self, yaml: &str) -> Result<ToolExecution, ActionError>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolExecution {
    pub output: String,
    #[serde(default, rename = "nodeOutputs")]
    pub node_outputs: BTreeMap<String, String>,
}

impl ToolExecution {
    pub fn from_output(output: String) -> Self {
        Self {
            output,
            node_outputs: BTreeMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct SubagentAction {
    executor: Arc<dyn ToolExecutor>,
    http: Client,
    config: SubagentConfig,
}

impl SubagentAction {
    pub fn new(executor: Arc<dyn ToolExecutor>, config: SubagentConfig) -> Self {
        Self {
            executor,
            http: Client::builder()
                .timeout(Duration::from_millis(DEFAULT_REQUEST_TIMEOUT_MS))
                .build()
                .expect("default HTTP client configuration must be valid"),
            config,
        }
    }
}

#[async_trait]
impl Action for SubagentAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        match self.run(input).await {
            Ok(output) => ActionOutput {
                payload: output,
                error: None,
            },
            Err(error) => ActionOutput {
                payload: Vec::new(),
                error: Some(error),
            },
        }
    }
}

impl SubagentAction {
    async fn run(&self, input: ActionInput) -> Result<Vec<u8>, ActionError> {
        let payload = if input.payload.is_empty() {
            return Err(ActionError::new("subagent input payload is empty"));
        } else {
            input.payload
        };
        let req: SubagentInput = serde_json::from_slice(&payload)
            .map_err(|err| ActionError::new_with("INVALID_INPUT", err.to_string(), false))?;
        if req.prompt.trim().is_empty() {
            return Err(ActionError::new_with(
                "INVALID_INPUT",
                "prompt must not be empty",
                false,
            ));
        }
        if self.config.model.trim().is_empty() || self.config.base_url.trim().is_empty() {
            return Err(ActionError::new_with(
                "INVALID_CONFIG",
                "subagent model and base_url must not be empty",
                false,
            ));
        }
        let mut history = Vec::new();
        history.push(ChatMessage {
            role: "system".to_string(),
            content: build_system_prompt(&self.config),
            name: None,
        });
        history.push(ChatMessage {
            role: "user".to_string(),
            content: req.prompt.clone(),
            name: None,
        });

        let max_turns = if self.config.max_turns == 0 {
            DEFAULT_MAX_TURNS
        } else {
            self.config.max_turns.min(MAX_TURNS)
        };
        for _ in 0..max_turns {
            let assistant = self.call_llm(&history).await?;
            history.push(assistant.clone());

            match parse_workflow_message(&assistant.content) {
                Ok(Some(workflow)) => {
                    if let Err(error) = self
                        .executor
                        .validate_workflow_message(&workflow.yaml, &workflow.final_message_template)
                        .await
                    {
                        let tool_result = json!({
                            "ok": false,
                            "code": error.code,
                            "message": error.message,
                            "retryable": error.retryable,
                        });
                        history.push(ChatMessage {
                            role: "tool".to_string(),
                            content: tool_result.to_string(),
                            name: Some("dispatcher".to_string()),
                        });
                        continue;
                    }

                    match self.executor.execute_yaml(&workflow.yaml).await {
                        Ok(execution) => {
                            let final_message = render_final_message(
                                &workflow.final_message_template,
                                &execution.node_outputs,
                            )
                            .map_err(|message| {
                                ActionError::new_with("FINAL_MESSAGE_TEMPLATE", message, false)
                            })?;
                            return Ok(final_message.into_bytes());
                        }
                        Err(error) => {
                            let tool_result = json!({
                                "ok": false,
                                "code": error.code,
                                "message": error.message,
                                "retryable": error.retryable,
                            });
                            history.push(ChatMessage {
                                role: "tool".to_string(),
                                content: tool_result.to_string(),
                                name: Some("dispatcher".to_string()),
                            });
                        }
                    }
                }
                Ok(None) => return Ok(assistant.content.into_bytes()),
                Err(message) => {
                    let tool_result = json!({
                        "ok": false,
                        "code": "INVALID_WORKFLOW_RESPONSE",
                        "message": message,
                        "retryable": false,
                    });
                    history.push(ChatMessage {
                        role: "tool".to_string(),
                        content: tool_result.to_string(),
                        name: Some("dispatcher".to_string()),
                    });
                }
            }
        }

        Err(ActionError::new_with(
            "MAX_TURNS",
            format!("subagent reached the maximum of {max_turns} turns"),
            false,
        ))
    }

    async fn call_llm(&self, history: &[ChatMessage]) -> Result<ChatMessage, ActionError> {
        let endpoint = normalize_chat_endpoint(&self.config.base_url);
        let mut request = self.http.post(endpoint).json(&ChatRequest {
            model: self.config.model.clone(),
            messages: history.to_vec(),
            temperature: if self.config.temperature > 0.0 {
                Some(self.config.temperature)
            } else {
                None
            },
        });
        if let Some(key) = &self.config.api_key {
            if !key.trim().is_empty() {
                request = request.bearer_auth(key);
            }
        }
        let request_timeout_ms = if self.config.request_timeout_ms == 0 {
            DEFAULT_REQUEST_TIMEOUT_MS
        } else {
            self.config.request_timeout_ms.min(MAX_REQUEST_TIMEOUT_MS)
        };
        let response: ChatResponse = request
            .timeout(Duration::from_millis(request_timeout_ms))
            .send()
            .await
            .map_err(|err| ActionError::new_with("LLM_REQUEST", err.to_string(), true))?
            .error_for_status()
            .map_err(|err| {
                let retryable = err
                    .status()
                    .map(|status| status.is_server_error())
                    .unwrap_or(false);
                ActionError::new_with("LLM_HTTP", err.to_string(), retryable)
            })?
            .json()
            .await
            .map_err(|err| ActionError::new_with("LLM_RESPONSE", err.to_string(), false))?;
        response
            .choices
            .first()
            .map(|choice| choice.message.clone())
            .ok_or_else(|| ActionError::new_with("LLM_RESPONSE", "response has no choices", false))
    }
}

fn build_system_prompt(config: &SubagentConfig) -> String {
    let base = config
        .system_prompt
        .as_deref()
        .unwrap_or(DEFAULT_SYSTEM_PROMPT)
        .trim();
    let catalog = config.action_catalog.trim();
    if catalog.is_empty() {
        return base.to_string();
    }
    format!(
        "{base}\n\nThe following action catalog is authoritative. Only call actions and use input fields listed here:\n\n{catalog}"
    )
}

fn normalize_chat_endpoint(base: &str) -> String {
    let trimmed = base.trim_end_matches('/');
    if trimmed.ends_with("/v1/chat/completions") {
        trimmed.to_string()
    } else if trimmed.ends_with("/v1") {
        format!("{}/chat/completions", trimmed)
    } else {
        format!("{}/v1/chat/completions", trimmed)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct WorkflowMessage {
    yaml: String,
    final_message_template: String,
}

fn parse_workflow_message(content: &str) -> Result<Option<WorkflowMessage>, String> {
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

fn render_final_message(
    template: &str,
    node_outputs: &BTreeMap<String, String>,
) -> Result<String, String> {
    let mut result = String::new();
    let mut cursor = 0usize;
    while let Some(start) = template[cursor..].find('{') {
        let start_index = cursor + start;
        result.push_str(&template[cursor..start_index]);
        let name_start = start_index + 1;
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

#[cfg(test)]
mod tests {
    use super::{SubagentConfig, SubagentInput, build_system_prompt, parse_workflow_message};

    fn config(system_prompt: Option<&str>, action_catalog: &str) -> SubagentConfig {
        SubagentConfig {
            model: "test-model".to_string(),
            base_url: "http://localhost:8080".to_string(),
            api_key: None,
            max_turns: 2,
            temperature: 0.2,
            request_timeout_ms: 1_000,
            system_prompt: system_prompt.map(str::to_string),
            action_catalog: action_catalog.to_string(),
        }
    }

    #[test]
    fn parses_workflow_response_only_when_it_starts_with_fence() {
        let message = parse_workflow_message("```yaml\nversion: 1\nsteps: []\n```\nDone {A}")
            .expect("valid workflow response")
            .expect("workflow expected");
        assert_eq!(message.yaml, "version: 1\nsteps: []");
        assert_eq!(message.final_message_template, "Done {A}");
        assert!(
            parse_workflow_message("version: 1\nsteps: []")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn subagent_input_only_accepts_prompt() {
        let input: SubagentInput =
            serde_json::from_str(r#"{"prompt":"hello"}"#).expect("prompt should be accepted");
        assert_eq!(input.prompt, "hello");

        let error = serde_json::from_str::<SubagentInput>(
            r#"{"prompt":"hello","model":"workflow-controlled"}"#,
        )
        .expect_err("configuration fields must be rejected");
        assert!(error.to_string().contains("unknown field `model`"));
    }

    #[test]
    fn system_prompt_includes_action_catalog() {
        let prompt = build_system_prompt(&config(
            None,
            "device_info({includeHardware?:bool=true}) -> DeviceInfoOutput",
        ));

        assert!(prompt.contains("Only call actions and use input fields listed here"));
        assert!(prompt.contains("device_info({includeHardware?:bool=true})"));
        assert!(prompt.contains("Workflow response format"));
    }

    #[test]
    fn custom_system_prompt_cannot_remove_action_catalog() {
        let prompt = build_system_prompt(&config(
            Some("Answer in Chinese."),
            "network_status({includeDetails?:bool=true}) -> NetworkStatusOutput",
        ));

        assert!(prompt.starts_with("Answer in Chinese."));
        assert!(prompt.contains("network_status({includeDetails?:bool=true})"));
    }
}
