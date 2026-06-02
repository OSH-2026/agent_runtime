mod grpc_client;
mod kotlin_bridge;

use async_trait::async_trait;
use crate::types::{ActionError, ActionRequest, ActionResponse};

#[async_trait]
pub trait ActionClient: Send + Sync {
	async fn call(&self, request: ActionRequest) -> Result<ActionResponse, ActionError>;
}

pub use grpc_client::GrpcClient;
pub use kotlin_bridge::RemoteAction;
