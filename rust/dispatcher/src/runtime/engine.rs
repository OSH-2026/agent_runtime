use crate::error::DispatcherError;
use crate::executor::{ExecutionResult, Executor, Outcome};
use crate::plan::{ExecutionPlan, NodeId, SideEffectLevel};
use crate::policy::ActionPolicy;
use crate::recovery::RecoveryStrategy;
use crate::runtime::{DiagnosticContext, ExecutionContext};
use crate::scheduler::{compute_ready_set, Dispatcher};
use crate::state::{GlobalState, NodeState};
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
            let to_run = self.enforce_policy(to_run);
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

    fn enforce_policy(&mut self, nodes: Vec<NodeId>) -> Vec<NodeId> {
        let mut filtered: Vec<NodeId> = Vec::new();
        let mut has_high_risk = false;
        for node_id in nodes {
            let node = match self.plan.nodes.get(&node_id) {
                Some(n) => n,
                None => continue,
            };
            if !node.config.policy.can_execute() {
                if let Some(transition) =
                    self.state.transition(&node_id, NodeState::WaitingHuman)
                {
                    self.record_transition(transition);
                }
                continue;
            }
            if node.config.policy.requires_serial() {
                if has_high_risk {
                    continue;
                }
                has_high_risk = true;
            }
            filtered.push(node_id);
        }
        filtered
    }

    pub fn update_node_policy(
        &mut self,
        node_id: &NodeId,
        policy: ActionPolicy,
    ) -> Result<(), DispatcherError> {
        let node = self
            .plan
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| DispatcherError::Execution(format!("node not found: {node_id}")))?;
        node.config.policy = policy;
        Ok(())
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
