use dispatcher::{load_action_flow_from_str, PlanError};
use dispatcher::plan::validate_dag;

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
