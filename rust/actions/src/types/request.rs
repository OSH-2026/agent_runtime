#[derive(Clone, Debug)]
pub struct ActionInput {
    pub payload: Vec<u8>,
}

impl ActionInput {
    pub fn into_request(self, action_name: &str) -> ActionRequest {
        ActionRequest {
            action_name: action_name.to_string(),
            payload: self.payload,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ActionRequest {
    pub action_name: String,
    pub payload: Vec<u8>,
}
