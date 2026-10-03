//! How the steps of an install end early.

use std::path::PathBuf;

use crate::commands::npm::packages::Source;
use crate::domain::version::Version;
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

/// The version an install is about.
pub struct Target {
    pub version: Version,
    /// `$NVM_DIR/versions/<flavor>/<version>`.
    pub path: PathBuf,
    /// Where `--reinstall-packages-from` takes packages from.
    pub source: Option<Source>,
    /// `make -j`, from `-j` when it is a natural number.
    pub make_jobs: Option<usize>,
}
