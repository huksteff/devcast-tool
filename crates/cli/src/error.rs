//! Errors of `devcast`: exit code 2 for a wrong command line, 1 for a command that failed.

use dc_ops::OpsError;
use dc_project::ProjectError;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CliError {
    /// The command line is wrong: unknown command, bad arguments.
    #[error("{0}")]
    Usage(String),
    /// The command was understood but could not be done.
    #[error("{0}")]
    Failed(String),
}

impl CliError {
    pub fn code(&self) -> u8 {
        match self {
            Self::Usage(_) => 2,
            Self::Failed(_) => 1,
        }
    }
}

impl From<OpsError> for CliError {
    fn from(error: OpsError) -> Self {
        match error {
            OpsError::Usage(message) => Self::Usage(message),
            OpsError::Failed(message) => Self::Failed(message),
        }
    }
}

impl From<ProjectError> for CliError {
    fn from(error: ProjectError) -> Self {
        Self::Failed(error.to_string())
    }
}

impl From<dc_settings::StoreError> for CliError {
    fn from(error: dc_settings::StoreError) -> Self {
        Self::Failed(error.to_string())
    }
}
