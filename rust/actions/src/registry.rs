use crate::Action;
use crate::client::RemoteAction;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionSideEffect {
    Pure,
    Idempotent,
    NonIdempotent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionRisk {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug)]
pub struct ActionMetadata {
    pub side_effect: ActionSideEffect,
    pub risk: ActionRisk,
    pub requires_confirmation: bool,
    pub collect_evidence: bool,
    pub timeout_ms: u64,
    pub max_retries: u32,
    pub callable_by_subagent: bool,
}

impl Default for ActionMetadata {
    fn default() -> Self {
        Self {
            side_effect: ActionSideEffect::NonIdempotent,
            risk: ActionRisk::High,
            requires_confirmation: true,
            collect_evidence: true,
            timeout_ms: 30_000,
            max_retries: 0,
            callable_by_subagent: false,
        }
    }
}

#[derive(Default)]
pub struct ActionRegistry {
    local: HashMap<String, Arc<dyn Action>>,
    remote: HashMap<String, RemoteAction>,
    metadata: HashMap<String, ActionMetadata>,
}

impl ActionRegistry {
    pub fn register_local(&mut self, name: impl Into<String>, action: Arc<dyn Action>) {
        self.local.insert(name.into(), action);
    }

    pub fn register_local_with_metadata(
        &mut self,
        name: impl Into<String>,
        action: Arc<dyn Action>,
        metadata: ActionMetadata,
    ) {
        let name = name.into();
        self.local.insert(name.clone(), action);
        self.metadata.insert(name.clone(), metadata);
    }

    pub fn register_remote(&mut self, name: impl Into<String>, action: RemoteAction) {
        self.remote.insert(name.into(), action);
    }

    pub fn register_remote_with_metadata(
        &mut self,
        name: impl Into<String>,
        action: RemoteAction,
        metadata: ActionMetadata,
    ) {
        let name = name.into();
        self.remote.insert(name.clone(), action);
        self.metadata.insert(name.clone(), metadata);
    }

    pub fn trusted_metadata(&self, name: &str) -> Option<&ActionMetadata> {
        self.metadata.get(name)
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
