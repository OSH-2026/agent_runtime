use actions::{Action, ActionClient, ActionError, ActionInput, ActionOutput, ActionRegistry, ActionRequest, ActionResponse, ToolExecutor};
use actions::client::RemoteAction;
use async_trait::async_trait;
use dispatcher::DispatcherToolExecutor;
use std::collections::HashMap;
use std::sync::Arc;

struct MockGrpcClient;

#[async_trait]
impl ActionClient for MockGrpcClient {
    async fn call(&self, request: ActionRequest) -> Result<ActionResponse, ActionError> {
        let payload = parse_payload(&request.payload).unwrap_or_default();
        let result = format!("kotlin:{}", payload).into_bytes();
        Ok(ActionResponse {
            success: true,
            result,
            error: None,
        })
    }
}

struct EchoAction;

#[async_trait]
impl Action for EchoAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        let payload = parse_payload(&input.payload).unwrap_or_default();
        ActionOutput {
            payload: payload.into_bytes(),
            error: None,
        }
    }
}

struct MergeAction;

#[async_trait]
impl Action for MergeAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        let map = parse_object(&input.payload);
        let left = map.get("left").cloned().unwrap_or_default();
        let right = map.get("right").cloned().unwrap_or_default();
        ActionOutput {
            payload: format!("{}|{}", left, right).into_bytes(),
            error: None,
        }
    }
}

struct MockSubagentAction;

#[async_trait]
impl Action for MockSubagentAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        let map = parse_object(&input.payload);
        let prompt = map.get("prompt").cloned().unwrap_or_default();
        ActionOutput {
            payload: format!("subagent:{}", prompt).into_bytes(),
            error: None,
        }
    }
}

#[tokio::test]
async fn dispatcher_runs_workflow_with_kotlin_and_subagent() {
    let yaml = r#"
version: 1
id: demo
steps:
  - id: A
    action: echo
    inputs:
      payload: "hello"
  - id: B
    action: kotlin_echo
    inputs:
      payload: "${A}"
  - id: C
    action: subagent
    inputs:
      prompt: "agent ${B}"
  - id: D
    action: merge
    inputs:
      left: "${B}"
      right: "${C}"
"#;

    let mut registry = ActionRegistry::default();
    registry.register_local("echo", Arc::new(EchoAction));
    registry.register_local("merge", Arc::new(MergeAction));
    registry.register_local("subagent", Arc::new(MockSubagentAction));

    let grpc = Arc::new(MockGrpcClient);
    registry.register_remote("kotlin_echo", RemoteAction::new(grpc, "kotlin_echo"));

    let executor = DispatcherToolExecutor::new(Arc::new(registry));
    let output = executor.execute_yaml(yaml).await.expect("execution failed");

    assert_eq!(output, "kotlin:hello|subagent:agent kotlin:hello");
}

fn parse_payload(bytes: &[u8]) -> Option<String> {
    let map = parse_object(bytes);
    map.get("payload").cloned()
}

fn parse_object(bytes: &[u8]) -> HashMap<String, String> {
    serde_json::from_slice(bytes).unwrap_or_default()
}
