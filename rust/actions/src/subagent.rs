use crate::Action;
use crate::types::{ActionError, ActionInput, ActionOutput};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_SYSTEM_PROMPT: &str = "You are a subagent that can call tools by emitting YAML action flows.\n\nWhen you need a tool, respond with a YAML action flow using this schema:\n\nversion: 1\nid: demo\noutput: B\nsteps:\n  - id: A\n    action: echo\n    inputs:\n      payload: \"hello\"\n  - id: B\n    action: echo\n    inputs:\n      payload: \"${A}\"\n\nOnly output the YAML block when requesting tools. Tool results are JSON objects with ok, output, code, message, and retryable fields. Do not include execution policy fields in tool YAML; policies are supplied by the trusted action registry.\n\nAny response that is not a YAML action flow is treated as the final answer.\n";
const DEFAULT_MAX_TURNS: u32 = 8;
const MAX_TURNS: u32 = 32;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 60_000;
const MAX_REQUEST_TIMEOUT_MS: u64 = 120_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubagentInput {
    pub prompt: String,
    pub model: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub max_turns: u32,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub request_timeout_ms: u64,
    #[serde(default)]
    pub system_prompt: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubagentOutput {
    pub text: String,
    pub history: Vec<ChatMessage>,
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
    async fn execute_yaml(&self, yaml: &str) -> Result<String, ActionError>;
}

#[derive(Clone)]
pub struct SubagentAction {
    executor: Arc<dyn ToolExecutor>,
    http: Client,
}

impl SubagentAction {
    pub fn new(executor: Arc<dyn ToolExecutor>) -> Self {
        Self {
            executor,
            http: Client::builder()
                .timeout(Duration::from_millis(DEFAULT_REQUEST_TIMEOUT_MS))
                .build()
                .expect("default HTTP client configuration must be valid"),
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
        if req.prompt.trim().is_empty()
            || req.model.trim().is_empty()
            || req.base_url.trim().is_empty()
        {
            return Err(ActionError::new_with(
                "INVALID_INPUT",
                "prompt, model, and base_url must not be empty",
                false,
            ));
        }
        let mut history = Vec::new();
        history.push(ChatMessage {
            role: "system".to_string(),
            content: req
                .system_prompt
                .clone()
                .unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string()),
            name: None,
        });
        history.push(ChatMessage {
            role: "user".to_string(),
            content: req.prompt.clone(),
            name: None,
        });

        let max_turns = if req.max_turns == 0 {
            DEFAULT_MAX_TURNS
        } else {
            req.max_turns.min(MAX_TURNS)
        };
        for _ in 0..max_turns {
            let assistant = self.call_llm(&req, &history).await?;
            history.push(assistant.clone());

            if let Some(yaml) = extract_yaml(&assistant.content) {
                let tool_result = match self.executor.execute_yaml(&yaml).await {
                    Ok(output) => json!({
                        "ok": true,
                        "output": output,
                    }),
                    Err(error) => json!({
                        "ok": false,
                        "code": error.code,
                        "message": error.message,
                        "retryable": error.retryable,
                    }),
                };
                history.push(ChatMessage {
                    role: "tool".to_string(),
                    content: tool_result.to_string(),
                    name: Some("dispatcher".to_string()),
                });
                continue;
            }

            let output = SubagentOutput {
                text: assistant.content,
                history,
            };
            let bytes =
                serde_json::to_vec(&output).map_err(|err| ActionError::new(err.to_string()))?;
            return Ok(bytes);
        }

        Err(ActionError::new_with(
            "MAX_TURNS",
            format!("subagent reached the maximum of {max_turns} turns"),
            false,
        ))
    }

    async fn call_llm(
        &self,
        req: &SubagentInput,
        history: &[ChatMessage],
    ) -> Result<ChatMessage, ActionError> {
        let endpoint = normalize_chat_endpoint(&req.base_url);
        let mut request = self.http.post(endpoint).json(&ChatRequest {
            model: req.model.clone(),
            messages: history.to_vec(),
            temperature: if req.temperature > 0.0 {
                Some(req.temperature)
            } else {
                None
            },
        });
        if let Some(key) = &req.api_key {
            if !key.trim().is_empty() {
                request = request.bearer_auth(key);
            }
        }
        let request_timeout_ms = if req.request_timeout_ms == 0 {
            DEFAULT_REQUEST_TIMEOUT_MS
        } else {
            req.request_timeout_ms.min(MAX_REQUEST_TIMEOUT_MS)
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

fn extract_yaml(content: &str) -> Option<String> {
    if let Some(block) = extract_fenced_yaml(content) {
        return Some(block);
    }
    if content.trim_start().starts_with("version:") && content.contains("steps:") {
        return Some(content.trim().to_string());
    }
    None
}

fn extract_fenced_yaml(content: &str) -> Option<String> {
    let mut lines = content.lines();
    while let Some(line) = lines.next() {
        if line.trim_start().starts_with("```yaml") {
            let mut yaml = Vec::new();
            for inner in lines.by_ref() {
                if inner.trim_start().starts_with("```") {
                    break;
                }
                yaml.push(inner);
            }
            return Some(yaml.join("\n").trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::extract_yaml;

    #[test]
    fn extracts_fenced_yaml() {
        let yaml = extract_yaml("```yaml\nversion: 1\nsteps: []\n```").expect("yaml expected");
        assert_eq!(yaml, "version: 1\nsteps: []");
    }
}
