use crate::state::GlobalState;

pub trait StateStore: Send + Sync {
    fn save(&self, state: &GlobalState) -> Result<(), String>;
    fn load(&self) -> Result<GlobalState, String>;
}
