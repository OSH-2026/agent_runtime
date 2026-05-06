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

    pub fn get(&self, name: &str) -> Option<ActionHandle<'_>> {
        if let Some(action) = self.local.get(name) {
            return Some(ActionHandle::Local(action));
        }
        self.remote.get(name).map(ActionHandle::Remote)
    }
}

pub enum ActionHandle<'a> {
    Local(&'a Arc<dyn Action>),
    Remote(&'a RemoteAction),
}
