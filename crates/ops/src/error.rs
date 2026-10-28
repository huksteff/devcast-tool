/// An error for the user or agent: what is wrong and what to try instead, in one message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpsError {
    /// The request itself is wrong: unknown op, bad arguments, malformed plan.
    #[error("{0}")]
    Usage(String),
    /// The request is valid but cannot be done on this project (missing id, out of range …).
    #[error("{0}")]
    Failed(String),
}
