use crate::executor::ExecutionResult;
use crate::runtime::{DiagnosticContext, FailureRecord};

pub struct Diagnoser;

impl Diagnoser {
    pub fn record_failure(context: &mut DiagnosticContext, result: &ExecutionResult) {
        if let Some(message) = &result.error {
            context.history.push(FailureRecord {
                node_id: result.node_id.clone(),
                message: message.clone(),
            });
        }
    }
}
