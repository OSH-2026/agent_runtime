use crate::client::GrpcClient;
use crate::types::{ActionInput, ActionOutput};
use crate::Action;
use async_trait::async_trait;

#[derive(Clone)]
pub struct RemoteAction {
    pub client: GrpcClient,
    pub action_name: String,
}

#[async_trait]
impl Action for RemoteAction {
    async fn execute(&self, input: ActionInput) -> ActionOutput {
        let request = input.into_request(&self.action_name);
        match self.client.call(request).await {
            Ok(response) => response.into_output(),
            Err(error) => ActionOutput {
                payload: Vec::new(),
                error: Some(error.message),
            },
        }
    }
}
