#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeState {
    Pending,
    Ready,
    Running,
    WaitingHuman,
    Blocked,
    Executed,
    FailedRetryable,
    Failed,
    Cancelled,
    Skipped,
}
