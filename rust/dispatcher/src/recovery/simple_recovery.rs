use crate::executor::ExecutionResult;
use crate::plan::{ExecutionPlan, SideEffectLevel};
use crate::recovery::{Diagnoser, EscalationGuard, RecoveryAction, RecoveryLevel, RecoveryStrategy};
use crate::runtime::DiagnosticContext;

pub struct SimpleRecovery {
    guard: EscalationGuard,
}

impl Default for SimpleRecovery {
    fn default() -> Self {
        Self {
            guard: EscalationGuard::default(),
        }
    }
}

impl RecoveryStrategy for SimpleRecovery {
    fn handle_failure(
        &mut self,
        result: &ExecutionResult,
        plan: &ExecutionPlan,
        diagnostic: &mut DiagnosticContext,
    ) -> RecoveryAction {
        Diagnoser::record_failure(diagnostic, result);
        let node = match plan.nodes.get(&result.node_id) {
            Some(node) => node,
            None => {
                return RecoveryAction {
                    node_id: result.node_id.clone(),
                    level: RecoveryLevel::Replan,
                };
            }
        };
        let level = match node.config.side_effect {
            SideEffectLevel::NonIdempotent => RecoveryLevel::Patch,
            _ => self
                .guard
                .register_failure(&result.node_id, node.config.retry_budget),
        };
        RecoveryAction {
            node_id: result.node_id.clone(),
            level,
        }
    }
}
