use actions::client::RemoteAction;
use actions::{
    Action, ActionClient, ActionError, ActionInput, ActionMetadata, ActionOutput, ActionRegistry,
    ActionRequest, ActionResponse, ActionRisk, ActionSideEffect, ToolExecutor,
};
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

struct FailingAction;

#[async_trait]
impl Action for FailingAction {
    async fn execute(&self, _input: ActionInput) -> ActionOutput {
        ActionOutput {
            payload: Vec::new(),
            error: Some(ActionError::new_with(
                "EXPECTED_FAILURE",
                "failure detail",
                false,
            )),
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
    registry.register_local_with_metadata("echo", Arc::new(EchoAction), safe_metadata());
    registry.register_local_with_metadata("merge", Arc::new(MergeAction), safe_metadata());
    registry.register_local_with_metadata(
        "subagent",
        Arc::new(MockSubagentAction),
        safe_metadata(),
    );

    let grpc = Arc::new(MockGrpcClient);
    registry.register_remote_with_metadata(
        "kotlin_echo",
        RemoteAction::new(grpc, "kotlin_echo"),
        safe_metadata(),
    );

    let executor = DispatcherToolExecutor::new(Arc::new(registry));
    let output = executor.execute_yaml(yaml).await.expect("execution failed");

    assert_eq!(output, "kotlin:hello|subagent:agent kotlin:hello");
}

#[tokio::test]
async fn dispatcher_rejects_actions_without_trusted_metadata() {
    let yaml = r#"
version: 1
id: demo
steps:
  - id: A
    action: echo
"#;
    let mut registry = ActionRegistry::default();
    registry.register_local("echo", Arc::new(EchoAction));

    let error = DispatcherToolExecutor::new(Arc::new(registry))
        .execute_yaml(yaml)
        .await
        .expect_err("untrusted action should be rejected");

    assert_eq!(error.code, "UNTRUSTED_ACTION");
}

#[tokio::test]
async fn dispatcher_preserves_action_failure_details() {
    let yaml = r#"
version: 1
id: demo
steps:
  - id: A
    action: fail
"#;
    let mut registry = ActionRegistry::default();
    registry.register_local_with_metadata("fail", Arc::new(FailingAction), safe_metadata());

    let error = DispatcherToolExecutor::new(Arc::new(registry))
        .execute_yaml(yaml)
        .await
        .expect_err("workflow should fail");

    assert_eq!(error.code, "WORKFLOW_INCOMPLETE");
    assert!(error.message.contains("EXPECTED_FAILURE: failure detail"));
}

#[tokio::test]
async fn dispatcher_rejects_policy_in_generated_yaml() {
    let yaml = r#"
version: 1
id: demo
steps:
  - id: A
    action: confirm
    sideEffect: pure
    policy:
      riskLevel: low
      requiresConfirmation: false
"#;
    let mut registry = ActionRegistry::default();
    registry.register_local_with_metadata("confirm", Arc::new(EchoAction), safe_metadata());

    let error = DispatcherToolExecutor::new(Arc::new(registry))
        .execute_yaml(yaml)
        .await
        .expect_err("generated policy should be rejected");

    assert_eq!(error.code, "POLICY_NOT_ALLOWED");
    assert!(error.message.contains("steps[0]"));
    assert!(error.message.contains("action metadata"));
}

fn safe_metadata() -> ActionMetadata {
    ActionMetadata {
        side_effect: ActionSideEffect::Pure,
        risk: ActionRisk::Low,
        requires_confirmation: false,
        collect_evidence: false,
        timeout_ms: 1_000,
        max_retries: 0,
        callable_by_subagent: true,
    }
}

fn parse_payload(bytes: &[u8]) -> Option<String> {
    let map = parse_object(bytes);
    map.get("payload").cloned()
}

fn parse_object(bytes: &[u8]) -> HashMap<String, String> {
    serde_json::from_slice(bytes).unwrap_or_default()
}
