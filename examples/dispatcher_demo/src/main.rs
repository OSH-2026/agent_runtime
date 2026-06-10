use actions::catalog::metadata_for_action;
use actions::{Action, ActionInput, ActionOutput, ActionRegistry};
use async_trait::async_trait;
use dispatcher::scheduler::{Dispatcher, TopoPolicy};
use dispatcher::{
    ActionExecutor, ActionPolicy, Contract, Engine, ExecutionContext, ExecutionPlan, GlobalState,
    InMemoryAuditLog, InMemoryStateStore, Node, NodeConfig, RiskLevel, SideEffectLevel,
    SimpleRecovery,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

struct EchoAction;

#[async_trait]
impl Action for EchoAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        ActionOutput {
            payload: input.payload,
            error: None,
        }
    }
}

#[tokio::main]
async fn main() {
    let mut registry = ActionRegistry::default();
    registry.register_local_with_metadata(
        "echo",
        Arc::new(EchoAction),
        metadata_for_action("echo").expect("echo metadata must exist"),
    );
    let registry = Arc::new(registry);

    let node_a = Node {
        id: "A".to_string(),
        action: "echo".to_string(),
        inputs: None,
        config: NodeConfig {
            retry_budget: 2,
            timeout: Duration::from_secs(5),
            side_effect: SideEffectLevel::Pure,
            policy: ActionPolicy::default().with_risk(RiskLevel::Low),
        },
        contract: Contract {
            schema: "bytes".to_string(),
        },
    };
    let node_b = Node {
        id: "B".to_string(),
        action: "echo".to_string(),
        inputs: None,
        config: NodeConfig {
            retry_budget: 2,
            timeout: Duration::from_secs(5),
            side_effect: SideEffectLevel::Pure,
            policy: ActionPolicy::default()
                .with_risk(RiskLevel::High)
                .with_timeout(10_000)
                .with_retries(3),
        },
        contract: Contract {
            schema: "bytes".to_string(),
        },
    };

    let mut nodes = HashMap::new();
    nodes.insert(node_a.id.clone(), node_a);
    nodes.insert(node_b.id.clone(), node_b);

    let plan = ExecutionPlan {
        id: "demo-plan".to_string(),
        version: 1,
        nodes,
        edges: vec![dispatcher::Edge {
            from: "A".to_string(),
            to: "B".to_string(),
        }],
        output_node: "B".to_string(),
        output_contract: Contract {
            schema: "bytes".to_string(),
        },
    };

    let state = GlobalState::new(&plan);
    let dispatcher = Dispatcher::new(Box::new(TopoPolicy::default()));
    let executor = ActionExecutor::new(Arc::clone(&registry), Arc::new(plan.clone()));
    let recovery = SimpleRecovery::default();

    let mut engine = Engine {
        plan,
        state,
        dispatcher,
        executor: Box::new(executor),
        recovery: Box::new(recovery),
        audit_log: Box::new(InMemoryAuditLog::default()),
        state_store: Box::new(InMemoryStateStore::default()),
        diagnostic: Default::default(),
    };

    let context = ExecutionContext {
        inputs: b"hello world".to_vec(),
    };

    if let Err(error) = engine.run(&context).await {
        eprintln!("engine error: {error}");
        return;
    }

    println!("demo completed");
}
