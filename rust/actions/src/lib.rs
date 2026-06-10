mod registry;

pub mod catalog;
pub mod client;
pub mod protocol;
pub mod subagent;
pub mod types;

pub use client::ActionClient;
pub use registry::{ActionMetadata, ActionRegistry, ActionRisk, ActionSideEffect};
pub use subagent::{SubagentAction, SubagentInput, ToolExecutor};
pub use types::{ActionError, ActionInput, ActionOutput, ActionRequest, ActionResponse};

use async_trait::async_trait;

#[async_trait]
pub trait Action: Send + Sync {
    async fn execute(&self, input: ActionInput) -> ActionOutput;
}
