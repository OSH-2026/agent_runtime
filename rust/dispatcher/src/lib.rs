pub mod error;

pub mod executor;
pub mod plan;
pub mod recovery;
pub mod runtime;
pub mod scheduler;
pub mod state;
pub mod storage;

pub use crate::error::{DispatcherError, PlanError};
pub use crate::executor::{ActionExecutor, ExecutionResult, Executor, Outcome};
pub use crate::plan::{Contract, Edge, ExecutionPlan, Node, NodeConfig, NodeId, SideEffectLevel};
pub use crate::recovery::{RecoveryAction, RecoveryLevel, RecoveryStrategy, SimpleRecovery};
pub use crate::runtime::{DiagnosticContext, Engine, ExecutionContext};
pub use crate::storage::{AuditEvent, AuditLog, InMemoryAuditLog, InMemoryStateStore, StateStore};
pub use crate::state::{GlobalState, NodeState, Transition};
