//! How the steps of an install end early.

use crate::error::{CliError, NvmExitCode};

/// Why an install stops before its last step.
#[derive(Debug)]
pub enum Halt {
    /// Stop, printing what the transcript holds, with this status.
    Exit(NvmExitCode),
    /// Stop with an error that the CLI prints.
    Error(CliError),
}

impl From<CliError> for Halt {
    fn from(error: CliError) -> Self {
        Self::Error(error)
    }
}

pub type Step<T> = Result<T, Halt>;
