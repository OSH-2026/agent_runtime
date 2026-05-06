use crate::error::DispatcherError;
use crate::executor::{ExecutionResult, Executor, Outcome};
use crate::plan::{ExecutionPlan, NodeId, SideEffectLevel};
use crate::recovery::RecoveryStrategy;
use crate::runtime::{DiagnosticContext, ExecutionContext};
use crate::scheduler::{compute_ready_set, Dispatcher};
use crate::state::GlobalState;
use crate::storage::{AuditEvent, AuditLog, StateStore};
use crate::plan::validate_dag;

pub struct Engine {
    pub plan: ExecutionPlan,
    pub state: GlobalState,
    pub dispatcher: Dispatcher,
    pub executor: Box<dyn Executor>,
    pub recovery: Box<dyn RecoveryStrategy>,
    pub audit_log: Box<dyn AuditLog>,
    pub state_store: Box<dyn StateStore>,
    pub diagnostic: DiagnosticContext,
}

impl Engine {
    pub async fn run(&mut self, context: &ExecutionContext) -> Result<(), DispatcherError> {
        validate_dag(&self.plan)?;
        loop {
            let transitions = self.state.refresh_ready(&self.plan);
            self.record_transitions(transitions);
            let ready = compute_ready_set(&self.state, &self.plan);
            if ready.is_empty() {
                break;
            }
            let to_run = self.dispatcher.dispatch_ready(ready);
            let to_run = self.enforce_side_effects(to_run);
            for node_id in &to_run {
                if let Some(transition) = self.state.mark_running(node_id) {
                    self.record_transition(transition);
                }
            }
            let results = self.executor.execute_batch(to_run, context).await;
            for result in results {
                self.apply_transition(result);
            }
        }
        Ok(())
    }

    fn apply_transition(&mut self, result: ExecutionResult) {
        match result.outcome {
            Outcome::Success => {
                if let Some(transition) = self.state.mark_executed(&result.node_id) {
                    self.record_transition(transition);
                }
            }
            Outcome::Failure => {
                let action = self
                    .recovery
                    .handle_failure(&result, &self.plan, &mut self.diagnostic);
                match action.level {
                    crate::recovery::RecoveryLevel::Retry => {
                        if let Some(transition) = self.state.mark_retryable(&result.node_id) {
                            self.record_transition(transition);
                        }
                    }
                    crate::recovery::RecoveryLevel::Patch | crate::recovery::RecoveryLevel::Replan => {
                        if let Some(transition) = self.state.mark_failed(&result.node_id) {
                            self.record_transition(transition);
                        }
                    }
                }
            }
            Outcome::Retry => {
                if let Some(transition) = self.state.mark_retryable(&result.node_id) {
                    self.record_transition(transition);
                }
            }
        }
    }

    fn enforce_side_effects(&self, nodes: Vec<NodeId>) -> Vec<NodeId> {
        let mut non_idempotent = None;
        for node_id in &nodes {
            if let Some(node) = self.plan.nodes.get(node_id) {
                if node.config.side_effect == SideEffectLevel::NonIdempotent {
                    non_idempotent = Some(node_id.clone());
                    break;
                }
            }
        }
        if let Some(node_id) = non_idempotent {
            vec![node_id]
        } else {
            nodes
        }
    }

    fn record_transitions(&mut self, transitions: Vec<crate::state::Transition>) {
        for transition in transitions {
            self.record_transition(transition);
        }
    }

    fn record_transition(&mut self, transition: crate::state::Transition) {
        let event = AuditEvent {
            node_id: transition.node_id,
            from: transition.from,
            to: transition.to,
            plan_version: self.state.plan_version,
        };
        self.audit_log.record(event);
        let _ = self.state_store.save(&self.state);
    }
}
