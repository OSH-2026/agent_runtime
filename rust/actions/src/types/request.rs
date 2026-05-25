use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ActionInput {
    pub payload: Vec<u8>,
    pub metadata: HashMap<String, String>,
}

impl Default for ActionInput {
    fn default() -> Self {
        Self {
            payload: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

impl ActionInput {
    pub fn into_request(self, action_name: &str) -> ActionRequest {
        ActionRequest {
            action_name: action_name.to_string(),
            payload: self.payload,
            metadata: self.metadata,
        }
    }

    pub fn new(payload: Vec<u8>) -> Self {
        Self {
            payload,
            metadata: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ActionRequest {
    pub action_name: String,
    pub payload: Vec<u8>,
    pub metadata: HashMap<String, String>,
}
