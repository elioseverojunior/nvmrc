//! Why project mode stopped.

use crate::commands::transcript::Transcript;
use crate::error::{CliError, NvmExitCode, PolicyError};

/// Why a project command stopped.
pub enum Stop {
    /// An error, printed as its message.
    Error(CliError),
    /// A refusal already printed, with its status.
    Refused(NvmExitCode),
}

impl Stop {
    /// Prints `message` on stderr and stops with `status`.
    pub fn refuse(transcript: &mut Transcript, message: String, status: NvmExitCode) -> Self {
        transcript.err(message);
        Self::Refused(status)
    }

    /// The status to stop with, after printing the error, if it is one.
    #[must_use]
    pub fn printed(self, transcript: &mut Transcript) -> NvmExitCode {
        match self {
            Self::Refused(status) => status,
            Self::Error(error) => {
                transcript.err(error.to_string());
                error.exit_code()
            }
        }
    }
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
