//! Why project mode stopped.

use crate::error::{CliError, NvmExitCode, PolicyError};

/// Why a project command stopped.
pub enum Stop {
    /// An error, printed as its message.
    Error(CliError),
    /// A refusal already printed, with its status.
    Refused(NvmExitCode),
}

impl From<CliError> for Stop {
    fn from(error: CliError) -> Self {
        Self::Error(error)
    }
}

impl From<PolicyError> for Stop {
    fn from(error: PolicyError) -> Self {
        Self::Error(error.into())
    }
}
