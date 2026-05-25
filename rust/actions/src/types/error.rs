#[derive(Clone, Debug)]
pub struct ActionError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl ActionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            code: "INTERNAL".to_string(),
            message: message.into(),
            retryable: false,
        }
    }

    pub fn new_with(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}
