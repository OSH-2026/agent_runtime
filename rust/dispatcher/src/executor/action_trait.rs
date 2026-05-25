use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct ActionInput {
    pub payload: Vec<u8>,
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct ActionOutput {
    pub payload: Vec<u8>,
    pub error: Option<String>,
}

#[async_trait]
pub trait Action: Send + Sync {
    async fn execute(&self, input: ActionInput) -> ActionOutput;
}
