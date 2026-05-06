mod registry;

pub mod client;
pub mod protocol;
pub mod types;

pub use registry::ActionRegistry;

use async_trait::async_trait;
use types::{ActionInput, ActionOutput};

#[async_trait]
pub trait Action: Send + Sync {
    async fn execute(&self, input: ActionInput) -> ActionOutput;
}
