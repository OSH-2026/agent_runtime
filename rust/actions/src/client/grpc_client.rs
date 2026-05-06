use crate::types::{ActionError, ActionRequest, ActionResponse};

#[derive(Clone, Debug)]
pub struct GrpcClient {
    pub endpoint: String,
}

impl GrpcClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }

    pub async fn call(&self, _request: ActionRequest) -> Result<ActionResponse, ActionError> {
        Err(ActionError::new("grpc client not implemented"))
    }
}
