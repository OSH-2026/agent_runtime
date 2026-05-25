use super::ActionError;

#[derive(Clone, Debug)]
pub struct ActionOutput {
    pub payload: Vec<u8>,
    pub error: Option<ActionError>,
}

impl ActionOutput {
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
}

#[derive(Clone, Debug)]
pub struct ActionResponse {
    pub success: bool,
    pub result: Vec<u8>,
    pub error: Option<ActionError>,
}

impl ActionResponse {
    pub fn into_output(self) -> ActionOutput {
        ActionOutput {
            payload: self.result,
            error: self.error,
        }
    }
}
