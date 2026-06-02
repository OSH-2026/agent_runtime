use crate::types::{ActionError, ActionInput, ActionOutput};
use crate::Action;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const DEFAULT_SYSTEM_PROMPT: &str = "You are a subagent that can call tools by emitting YAML action flows.\n\nWhen you need a tool, respond with a YAML action flow using this schema:\n\nversion: 1\nid: demo\nsteps:\n  - id: A\n    action: echo\n    inputs:\n      payload: \"hello\"\n  - id: B\n    action: echo\n    inputs:\n      payload: \"${A}\"\n\nOnly output the YAML block when requesting tools.\n\nIf you want to end the subagent, respond with a line containing: tool: return\nThe caller will return the previous message content as the final answer.\n";

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
            http: Client::new(),
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
            .map_err(|err| ActionError::new(err.to_string()))?;
        let mut history = Vec::new();
        history.push(ChatMessage {
            role: "system".to_string(),
            content: req.system_prompt.unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string()),
            name: None,
        });
        history.push(ChatMessage {
            role: "user".to_string(),
            content: req.prompt.clone(),
            name: None,
        });

        let max_turns = if req.max_turns == 0 { 8 } else { req.max_turns };
        for _ in 0..max_turns {
            let assistant = self
                .call_llm(&req, &history)
                .await
                .map_err(|err| ActionError::new(err.to_string()))?;
            history.push(assistant.clone());

            if is_return_tool(&assistant.content) {
                let last = history
                    .iter()
                    .rev()
                    .skip(1)
                    .find(|msg| !msg.content.trim().is_empty())
                    .map(|msg| msg.content.clone())
                    .unwrap_or_default();
                let output = SubagentOutput { text: last, history };
                let bytes = serde_json::to_vec(&output)
                    .map_err(|err| ActionError::new(err.to_string()))?;
                return Ok(bytes);
            }

            if let Some(yaml) = extract_yaml(&assistant.content) {
                let tool_result = self.executor.execute_yaml(&yaml).await?;
                history.push(ChatMessage {
                    role: "tool".to_string(),
                    content: tool_result,
                    name: Some("dispatcher".to_string()),
                });
                continue;
            }

            let output = SubagentOutput {
                text: assistant.content,
                history,
            };
            let bytes = serde_json::to_vec(&output)
                .map_err(|err| ActionError::new(err.to_string()))?;
            return Ok(bytes);
        }

        let output = SubagentOutput {
            text: "subagent max turns reached".to_string(),
            history,
        };
        let bytes = serde_json::to_vec(&output).map_err(|err| ActionError::new(err.to_string()))?;
        Ok(bytes)
    }

    async fn call_llm(
        &self,
        req: &SubagentInput,
        history: &[ChatMessage],
    ) -> Result<ChatMessage, reqwest::Error> {
        let endpoint = normalize_chat_endpoint(&req.base_url);
        let mut request = self.http.post(endpoint).json(&ChatRequest {
            model: req.model.clone(),
            messages: history.to_vec(),
            temperature: if req.temperature > 0.0 { Some(req.temperature) } else { None },
        });
        if let Some(key) = &req.api_key {
            if !key.trim().is_empty() {
                request = request.bearer_auth(key);
            }
        }
        let response: ChatResponse = request.send().await?.json().await?;
        Ok(response
            .choices
            .first()
            .map(|choice| choice.message.clone())
            .unwrap_or(ChatMessage {
                role: "assistant".to_string(),
                content: "".to_string(),
                name: None,
            }))
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

fn is_return_tool(content: &str) -> bool {
    content.lines().any(|line| line.trim() == "tool: return")
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
