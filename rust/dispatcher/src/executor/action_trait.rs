use async_trait::async_trait;

#[derive(Clone, Debug)]
pub struct ActionInput {
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ActionOutput {
    pub payload: Vec<u8>,
}

#[async_trait]
pub trait Action: Send + Sync {
    async fn execute(&self, input: ActionInput) -> ActionOutput;
}
