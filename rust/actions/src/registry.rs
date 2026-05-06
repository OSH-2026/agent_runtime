use crate::client::RemoteAction;
use crate::Action;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
pub struct ActionRegistry {
    local: HashMap<String, Arc<dyn Action>>,
    remote: HashMap<String, RemoteAction>,
}

impl ActionRegistry {
    pub fn register_local(&mut self, name: impl Into<String>, action: Arc<dyn Action>) {
        self.local.insert(name.into(), action);
    }

    pub fn register_remote(&mut self, name: impl Into<String>, action: RemoteAction) {
        self.remote.insert(name.into(), action);
    }

    pub fn get(&self, name: &str) -> Option<ActionHandle> {
        if let Some(action) = self.local.get(name) {
            return Some(ActionHandle::Local(Arc::clone(action)));
        }
        self.remote.get(name).cloned().map(ActionHandle::Remote)
    }
}

pub enum ActionHandle {
    Local(Arc<dyn Action>),
    Remote(RemoteAction),
}

impl ActionHandle {
    pub async fn execute(&self, input: crate::types::ActionInput) -> crate::types::ActionOutput {
        match self {
            ActionHandle::Local(action) => action.execute(input).await,
            ActionHandle::Remote(action) => action.execute(input).await,
        }
    }
}
