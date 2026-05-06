#[derive(Clone, Debug)]
pub struct ActionError {
    pub message: String,
}

impl ActionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
