use crate::state::GlobalState;
use std::sync::Mutex;

pub trait StateStore: Send + Sync {
    fn save(&self, state: &GlobalState) -> Result<(), String>;
    fn load(&self) -> Result<GlobalState, String>;
}

pub struct InMemoryStateStore {
    state: Mutex<Option<GlobalState>>, 
}

impl InMemoryStateStore {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(None),
        }
    }
}

impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl StateStore for InMemoryStateStore {
    fn save(&self, state: &GlobalState) -> Result<(), String> {
        let mut guard = self.state.lock().map_err(|_| "state lock poisoned".to_string())?;
        *guard = Some(state.clone());
        Ok(())
    }

    fn load(&self) -> Result<GlobalState, String> {
        let guard = self.state.lock().map_err(|_| "state lock poisoned".to_string())?;
        guard.clone().ok_or_else(|| "state not initialized".to_string())
    }
}
