mod registry;

pub mod client;
pub mod protocol;
pub mod subagent;
pub mod types;

pub use registry::ActionRegistry;
pub use subagent::{SubagentAction, SubagentInput, SubagentOutput, ToolExecutor};
pub use types::{ActionError, ActionInput, ActionOutput, ActionRequest, ActionResponse};

use async_trait::async_trait;

#[async_trait]
pub trait Action: Send + Sync {
    async fn execute(&self, input: ActionInput) -> ActionOutput;
}
