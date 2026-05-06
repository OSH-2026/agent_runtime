use crate::executor::{ExecutionResult, Executor, Outcome};
use crate::plan::ExecutionPlan;
use crate::recovery::RecoveryStrategy;
use crate::runtime::ExecutionContext;
use crate::scheduler::{compute_ready_set, Dispatcher};
use crate::state::GlobalState;

pub struct Engine {
    pub plan: ExecutionPlan,
    pub state: GlobalState,
    pub dispatcher: Dispatcher,
    pub executor: Box<dyn Executor>,
    pub recovery: Box<dyn RecoveryStrategy>,
}

impl Engine {
    pub async fn run(&mut self, context: &ExecutionContext) {
        loop {
            let ready = compute_ready_set(&self.state, &self.plan);
            if ready.is_empty() {
                break;
            }
            let to_run = self.dispatcher.dispatch_ready(ready);
            let results = self.executor.execute_batch(to_run, context).await;
            for result in results {
                self.apply_transition(result);
            }
        }
    }

    fn apply_transition(&mut self, result: ExecutionResult) {
        match result.outcome {
            Outcome::Success => self.state.mark_executed(&result.node_id),
            Outcome::Failure => {
                self.recovery.handle_failure(&result);
                self.state.mark_failed(&result.node_id);
            }
            Outcome::Retry => {
                self.state.mark_retryable(&result.node_id);
            }
        }
    }
}
