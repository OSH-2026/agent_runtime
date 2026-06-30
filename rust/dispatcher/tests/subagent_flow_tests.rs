use actions::client::RemoteAction;
use actions::{
    Action, ActionClient, ActionError, ActionInput, ActionMetadata, ActionOutput, ActionRegistry,
    ActionRequest, ActionResponse, ActionRisk, ActionSideEffect, ToolExecutor,
};
use async_trait::async_trait;
use dispatcher::{
    ActionRegistryFactory, ConfirmationHandler, ConfirmationRequest, DispatcherToolExecutor,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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

struct TestRegistryFactory {
    build: fn(Arc<dyn ToolExecutor>) -> ActionRegistry,
}

struct FixedConfirmationHandler {
    approved: bool,
    requests: AtomicUsize,
}

#[async_trait]
impl ConfirmationHandler for FixedConfirmationHandler {
    async fn confirm(&self, _request: ConfirmationRequest) -> Result<bool, ActionError> {
        self.requests.fetch_add(1, Ordering::SeqCst);
        Ok(self.approved)
    }
}

impl ActionRegistryFactory for TestRegistryFactory {
    fn create_registry(&self, tools: Arc<dyn ToolExecutor>) -> Result<ActionRegistry, ActionError> {
        Ok((self.build)(tools))
    }
}

fn executor(build: fn(Arc<dyn ToolExecutor>) -> ActionRegistry) -> DispatcherToolExecutor {
    DispatcherToolExecutor::new(Arc::new(TestRegistryFactory { build }))
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

    let executor = executor(full_registry);
    let output = executor.execute_yaml(yaml).await.expect("execution failed");

    assert_eq!(
        output.output.as_str(),
        "kotlin:hello|subagent:agent kotlin:hello"
    );
    assert_eq!(
        output.node_outputs.get("B").map(String::as_str),
        Some("kotlin:hello")
    );
}

#[tokio::test]
async fn dispatcher_resumes_after_confirmation_is_approved() {
    let yaml = r#"
version: 1
id: confirm-demo
steps:
  - id: A
    action: guarded_echo
    inputs:
      payload: "approved"
"#;
    let handler = Arc::new(FixedConfirmationHandler {
        approved: true,
        requests: AtomicUsize::new(0),
    });
    let factory = Arc::new(TestRegistryFactory {
        build: confirmation_registry,
    });

    let output = DispatcherToolExecutor::with_confirmation_handler(factory, handler.clone())
        .execute_yaml(yaml)
        .await
        .expect("approved workflow should resume");

    assert_eq!(output.output.as_str(), "approved");
    assert_eq!(handler.requests.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn dispatcher_stops_after_confirmation_is_rejected() {
    let yaml = r#"
version: 1
id: confirm-demo
steps:
  - id: A
    action: guarded_echo
    inputs:
      payload: "rejected"
"#;
    let handler = Arc::new(FixedConfirmationHandler {
        approved: false,
        requests: AtomicUsize::new(0),
    });
    let factory = Arc::new(TestRegistryFactory {
        build: confirmation_registry,
    });

    let error = DispatcherToolExecutor::with_confirmation_handler(factory, handler.clone())
        .execute_yaml(yaml)
        .await
        .expect_err("rejected workflow must stop");

    assert_eq!(error.code, "USER_REJECTED");
    assert_eq!(handler.requests.load(Ordering::SeqCst), 1);
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
    let error = executor(untrusted_registry)
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
    let error = executor(failing_registry)
        .execute_yaml(yaml)
        .await
        .expect_err("workflow should fail");

    assert_eq!(error.code, "WORKFLOW_INCOMPLETE");
    assert!(error.message.contains("EXPECTED_FAILURE: failure detail"));
}

#[tokio::test]
async fn dispatcher_rejects_final_template_before_execution() {
    let yaml = r#"
version: 1
id: demo
steps:
  - id: A
    action: echo
    inputs:
      payload: "hello"
"#;
    let error = executor(full_registry)
        .validate_workflow_message(yaml, "Done ${Missing}")
        .await
        .expect_err("unknown final-message node should be rejected");

    assert_eq!(error.code, "FINAL_MESSAGE_TEMPLATE");
    assert!(error.message.contains("unknown node 'Missing'"));

    executor(full_registry)
        .validate_workflow_message(yaml, "Done {A}")
        .await
        .expect("bare braces should be treated as literal text");
}

#[tokio::test]
async fn dispatcher_allows_multiple_terminal_nodes_without_output() {
    let yaml = r#"
version: 1
id: demo
steps:
  - id: A
    action: echo
    inputs:
      payload: "left"
  - id: B
    action: echo
    inputs:
      payload: "right"
"#;
    let executor = executor(full_registry);

    executor
        .validate_workflow_message(yaml, "Done ${A} and ${B}")
        .await
        .expect("final template should validate against all nodes");
    let output = executor.execute_yaml(yaml).await.expect("execution failed");

    assert_eq!(output.output, "");
    assert_eq!(
        output.node_outputs.get("A").map(String::as_str),
        Some("left")
    );
    assert_eq!(
        output.node_outputs.get("B").map(String::as_str),
        Some("right")
    );
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
    let error = executor(confirm_registry)
        .execute_yaml(yaml)
        .await
        .expect_err("generated policy should be rejected");

    assert_eq!(error.code, "POLICY_NOT_ALLOWED");
    assert!(error.message.contains("steps[0]"));
    assert!(error.message.contains("action metadata"));
}

struct RecursiveAction {
    tools: Arc<dyn ToolExecutor>,
}

#[async_trait]
impl Action for RecursiveAction {
    async fn execute(&self, _input: ActionInput) -> ActionOutput {
        let yaml = r#"
version: 1
id: inner
steps:
  - id: result
    action: echo
    inputs:
      payload: "nested"
"#;
        match self.tools.execute_yaml(yaml).await {
            Ok(output) => ActionOutput {
                payload: output.output.into_bytes(),
                error: None,
            },
            Err(error) => ActionOutput {
                payload: Vec::new(),
                error: Some(error),
            },
        }
    }
}

struct RecursiveRegistryFactory {
    creations: Arc<AtomicUsize>,
}

impl ActionRegistryFactory for RecursiveRegistryFactory {
    fn create_registry(&self, tools: Arc<dyn ToolExecutor>) -> Result<ActionRegistry, ActionError> {
        self.creations.fetch_add(1, Ordering::SeqCst);
        let mut registry = ActionRegistry::default();
        registry.register_local_with_metadata("echo", Arc::new(EchoAction), safe_metadata());
        registry.register_local_with_metadata(
            "recurse",
            Arc::new(RecursiveAction { tools }),
            safe_metadata(),
        );
        Ok(registry)
    }
}

#[tokio::test]
async fn recursive_tools_create_an_independent_registry_per_level() {
    let yaml = r#"
version: 1
id: outer
steps:
  - id: result
    action: recurse
"#;
    let creations = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(RecursiveRegistryFactory {
        creations: Arc::clone(&creations),
    });

    let output = DispatcherToolExecutor::new(factory)
        .execute_yaml(yaml)
        .await
        .expect("recursive execution should succeed");

    assert_eq!(output.output.as_str(), "nested");
    assert_eq!(creations.load(Ordering::SeqCst), 2);
}

fn full_registry(_tools: Arc<dyn ToolExecutor>) -> ActionRegistry {
    let mut registry = ActionRegistry::default();
    registry.register_local_with_metadata("echo", Arc::new(EchoAction), safe_metadata());
    registry.register_local_with_metadata("merge", Arc::new(MergeAction), safe_metadata());
    registry.register_local_with_metadata(
        "subagent",
        Arc::new(MockSubagentAction),
        safe_metadata(),
    );
    registry.register_remote_with_metadata(
        "kotlin_echo",
        RemoteAction::new(Arc::new(MockGrpcClient), "kotlin_echo"),
        safe_metadata(),
    );
    registry
}

fn untrusted_registry(_tools: Arc<dyn ToolExecutor>) -> ActionRegistry {
    let mut registry = ActionRegistry::default();
    registry.register_local("echo", Arc::new(EchoAction));
    registry
}

fn failing_registry(_tools: Arc<dyn ToolExecutor>) -> ActionRegistry {
    let mut registry = ActionRegistry::default();
    registry.register_local_with_metadata("fail", Arc::new(FailingAction), safe_metadata());
    registry
}

fn confirm_registry(_tools: Arc<dyn ToolExecutor>) -> ActionRegistry {
    let mut registry = ActionRegistry::default();
    registry.register_local_with_metadata("confirm", Arc::new(EchoAction), safe_metadata());
    registry
}

fn confirmation_registry(_tools: Arc<dyn ToolExecutor>) -> ActionRegistry {
    let mut registry = ActionRegistry::default();
    let mut metadata = safe_metadata();
    metadata.risk = ActionRisk::High;
    metadata.requires_confirmation = true;
    registry.register_local_with_metadata("guarded_echo", Arc::new(EchoAction), metadata);
    registry
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
