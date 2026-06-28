use dispatcher::plan::validate_dag;
use dispatcher::{PlanError, load_action_flow_from_str};
use std::fs;
use std::path::PathBuf;

#[test]
fn load_basic_edges_from_references() {
    let yaml = r#"
version: 1
id: demo
steps:
  - id: A
    action: echo
    inputs:
      payload: "hello"
  - id: B
    action: echo
    inputs:
      payload: "${A}"
  - id: C
    action: merge
    inputs:
      left: "${A}"
      right: "${B}"
"#;

    let plan = load_action_flow_from_str(yaml).expect("plan load failed");
    assert_eq!(plan.nodes.len(), 3);
    assert_eq!(plan.edges.len(), 3);
    assert!(plan.edges.iter().any(|e| e.from == "A" && e.to == "B"));
    assert!(plan.edges.iter().any(|e| e.from == "A" && e.to == "C"));
    assert!(plan.edges.iter().any(|e| e.from == "B" && e.to == "C"));
    assert_eq!(plan.output_node.as_deref(), Some("C"));
}

#[test]
fn load_allows_omitted_output_for_multiple_sinks() {
    let yaml = r#"
version: 1
id: demo
steps:
  - id: A
    action: echo
  - id: B
    action: echo
"#;

    let plan = load_action_flow_from_str(yaml).expect("plan load failed");
    assert_eq!(plan.output_node, None);
}

#[test]
fn load_uses_explicit_output_instead_of_step_order() {
    let yaml = r#"
version: 1
id: demo
output: A
steps:
  - id: A
    action: echo
  - id: B
    action: echo
"#;

    let plan = load_action_flow_from_str(yaml).expect("plan load failed");
    assert_eq!(plan.output_node.as_deref(), Some("A"));
}

#[test]
fn load_rejects_duplicate_step_ids() {
    let yaml = r#"
version: 1
id: demo
steps:
  - id: A
    action: echo
  - id: A
    action: echo
"#;

    let err = load_action_flow_from_str(yaml).expect_err("expected duplicate error");
    match err {
        dispatcher::DispatcherError::Plan(PlanError::DuplicateNode(id)) => {
            assert_eq!(id, "A");
        }
        _ => panic!("unexpected error: {err}"),
    }
}

#[test]
fn load_rejects_missing_reference() {
    let yaml = r#"
version: 1
id: demo
steps:
  - id: A
    action: echo
    inputs:
      payload: "${Missing}"
"#;

    let err = load_action_flow_from_str(yaml).expect_err("expected missing ref error");
    match err {
        dispatcher::DispatcherError::Plan(PlanError::MissingNode(id)) => {
            assert_eq!(id, "Missing");
        }
        _ => panic!("unexpected error: {err}"),
    }
}

#[test]
fn load_rejects_invalid_reference_format() {
    let yaml = r#"
version: 1
id: demo
steps:
  - id: A
    action: echo
    inputs:
      payload: "${}"
"#;

    let err = load_action_flow_from_str(yaml).expect_err("expected invalid ref error");
    match err {
        dispatcher::DispatcherError::Plan(PlanError::InvalidReference(_)) => {}
        _ => panic!("unexpected error: {err}"),
    }
}

#[test]
fn validate_detects_cycle() {
    let yaml = r#"
version: 1
id: demo
output: A
steps:
  - id: A
    action: echo
    inputs:
      payload: "${B}"
  - id: B
    action: echo
    inputs:
      payload: "${A}"
"#;

    let plan = load_action_flow_from_str(yaml).expect("plan load failed");
    let err = validate_dag(&plan).expect_err("expected cycle error");
    match err {
        PlanError::Cycle(nodes) => {
            assert!(nodes.contains(&"A".to_string()));
            assert!(nodes.contains(&"B".to_string()));
        }
        _ => panic!("unexpected error: {err}"),
    }
}

#[test]
fn bundled_android_workflows_are_valid_dags() {
    let workflows_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/workflows");
    let workflow_files = [
        "device-health-report.yaml",
        "travel-preflight.yaml",
        "communication-digest.yaml",
        "incident-evidence-capture.yaml",
    ];

    for file_name in workflow_files {
        let path = workflows_dir.join(file_name);
        let yaml = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let plan = load_action_flow_from_str(&yaml)
            .unwrap_or_else(|error| panic!("failed to load {}: {error}", path.display()));
        validate_dag(&plan)
            .unwrap_or_else(|error| panic!("invalid DAG in {}: {error}", path.display()));
        assert!(
            plan.nodes.len() >= 7,
            "{} should remain a non-trivial workflow",
            path.display()
        );
        assert!(
            !plan.edges.is_empty(),
            "{} should contain data dependencies",
            path.display()
        );
    }
}
