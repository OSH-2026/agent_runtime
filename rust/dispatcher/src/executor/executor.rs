use crate::executor::ExecutionResult;
use crate::plan::NodeId;
use crate::runtime::ExecutionContext;
use async_trait::async_trait;

#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute_batch(
        &self,
        nodes: Vec<NodeId>,
        context: &ExecutionContext,
    ) -> Vec<ExecutionResult>;
}
