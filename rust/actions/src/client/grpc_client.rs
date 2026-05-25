use crate::protocol::grpc::action_service_client::ActionServiceClient;
use crate::protocol::grpc::{ActionError as ProtoError, ActionRequest as ProtoRequest};
use crate::types::{ActionError, ActionRequest, ActionResponse};
use tonic::transport::Endpoint;

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

    pub async fn call(&self, request: ActionRequest) -> Result<ActionResponse, ActionError> {
        let endpoint = normalize_endpoint(&self.endpoint);
        let channel = Endpoint::from_shared(endpoint)
            .map_err(|err| ActionError::new(err.to_string()))?
            .connect()
            .await
            .map_err(|err| ActionError::new(err.to_string()))?;
        let mut client = ActionServiceClient::new(channel);
        let proto = ProtoRequest {
            action_name: request.action_name,
            payload: request.payload,
            metadata: request.metadata,
        };
        let response = client
            .execute(proto)
            .await
            .map_err(|err| ActionError::new(err.to_string()))?
            .into_inner();
        let error = response.error.map(from_proto_error);
        Ok(ActionResponse {
            success: response.success,
            result: response.result,
            error,
        })
    }
}

fn normalize_endpoint(raw: &str) -> String {
    if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else {
        format!("http://{}", raw)
    }
}

fn from_proto_error(error: ProtoError) -> ActionError {
    ActionError::new_with(error.code, error.message, error.retryable)
}
