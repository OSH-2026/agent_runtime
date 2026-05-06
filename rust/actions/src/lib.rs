mod registry;

pub mod client;
pub mod protocol;
pub mod types;

pub use registry::ActionRegistry;
pub use types::{ActionError, ActionInput, ActionOutput, ActionRequest, ActionResponse};

use async_trait::async_trait;

#[async_trait]
pub trait Action: Send + Sync {
    async fn execute(&self, input: ActionInput) -> ActionOutput;
}
