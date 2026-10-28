/// An error for the user or agent: what is wrong and what to try instead, in one message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct OpsError(pub String);
