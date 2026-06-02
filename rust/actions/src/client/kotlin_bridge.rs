use crate::client::ActionClient;
use crate::client::GrpcClient;
use crate::types::{ActionInput, ActionOutput};
use crate::Action;
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Clone)]
pub struct RemoteAction {
    pub client: Arc<dyn ActionClient>,
    pub action_name: String,
}

impl RemoteAction {
    pub fn new(client: Arc<dyn ActionClient>, action_name: impl Into<String>) -> Self {
        Self {
            client,
            action_name: action_name.into(),
        }
    }

    pub fn from_grpc(client: GrpcClient, action_name: impl Into<String>) -> Self {
        Self {
            client: Arc::new(client),
            action_name: action_name.into(),
        }
    }
}

#[async_trait]
impl Action for RemoteAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        let request = input.into_request(&self.action_name);
        match self.client.call(request).await {
            Ok(response) => response.into_output(),
            Err(error) => ActionOutput {
                payload: Vec::new(),
                error: Some(error),
            },
        }
    }
}
