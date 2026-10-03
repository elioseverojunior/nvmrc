//! Typed errors and the single mapping from errors to process exit codes.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::shell::ShellError;

/// Public exit-code contract, taken from `nvm.sh`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum NvmExitCode {
    #[default]
    Success,
    Failure,
    InvalidVersion,
    BelowVersionFloor,
    AliasLoop,
    /// Something that was asked for does not exist: an `lts/<name>` alias, or
    /// the archive of a version whose download failed.
    MissingTarget,
    /// `nvm install --reinstall-packages-from` the very version being
    /// installed.
    SameVersion,
    /// `nvm install --reinstall-packages-from` a version that is not
    /// installed.
    SourceNotInstalled,
    /// Options that cannot be combined, or given twice.
    InvalidOptions,
    /// `nvm use` found an npm `prefix` setting that breaks nvm.
    IncompatiblePrefix,
    /// `NVM_INSTALL_THIRD_PARTY_HOOK` succeeded and installed nothing.
    HookClaimedSuccess,
    /// An option `nvm.sh` does not support, or one used in a combination it
    /// does not support.
    UnsupportedOption,
    /// A usage error, or a requested system version that does not exist.
    NotFound,
    /// The status of a program that `nvm` ran and passes on, such as the
    /// third-party install hook.
    Passed(u8),
}

impl NvmExitCode {
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
            Self::MissingTarget => 2,
            Self::InvalidVersion => 3,
            Self::SameVersion => 4,
            Self::SourceNotInstalled => 5,
            Self::InvalidOptions => 6,
            Self::BelowVersionFloor => 7,
            Self::AliasLoop => 8,
            Self::IncompatiblePrefix => 11,
            Self::HookClaimedSuccess => 33,
            Self::UnsupportedOption => 55,
            Self::NotFound => 127,
            Self::Passed(code) => code,
        }
    }

    /// The status of a program that ran: 1 when it has none (a signal).
    #[must_use]
    pub fn passing_on(code: Option<i32>) -> Self {
        match code.and_then(|code| u8::try_from(code).ok()) {
            Some(0) => Self::Success,
            Some(code) => Self::Passed(code),
            None => Self::Failure,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VersionError {
    #[error("invalid version: {0}")]
    Invalid(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FloorError {
    #[error("Invalid minimum version '{0}' (from NVM_MIN_VERSION or $NVM_DIR/min-version).")]
    Invalid(String),
    #[error("Version {version} is below the minimum allowed version {floor}.")]
    Below { version: String, floor: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AliasError {
    #[error("The alias \"{0}\" leads to an infinite loop. Aborting.")]
    Loop(String),
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Version(#[from] VersionError),
    #[error(transparent)]
    Floor(#[from] FloorError),
    #[error(transparent)]
    Alias(#[from] AliasError),
    #[error(transparent)]
    Shell(#[from] ShellError),
    #[error("N/A")]
    NotInstalled,
    #[error("Neither NVM_DIR nor HOME is set; cannot locate the nvm directory.")]
    NvmDirUnresolved,
    /// A multi-line usage message, printed as is.
    #[error("{0}")]
    Usage(String),
    /// The full "not yet installed" message, printed as is.
    #[error("{0}")]
    VersionNotInstalled(String),
    #[error("System version of node not found.")]
    SystemNodeNotFound,
    /// A rejected argument, with the message to print.
    #[error("{0}")]
    InvalidArgument(String),
    /// An unsupported option, with the message to print.
    #[error("{0}")]
    Unsupported(String),
    /// Options that cannot be combined, with the message to print.
    #[error("{0}")]
    InvalidOptions(String),
    /// A failed file-system change, naming the path it was made on.
    #[error("{}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl CliError {
    /// A [`CliError::Io`] for a failed operation on `path`, keeping the
    /// underlying error as its source.
    #[must_use]
    pub fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }

    #[must_use]
    pub fn exit_code(&self) -> NvmExitCode {
        match self {
            Self::Version(_) | Self::NotInstalled => NvmExitCode::InvalidVersion,
            Self::NvmDirUnresolved
            | Self::VersionNotInstalled(_)
            | Self::InvalidArgument(_)
            | Self::Shell(_)
            | Self::Io { .. } => NvmExitCode::Failure,
            Self::Usage(_) | Self::SystemNodeNotFound => NvmExitCode::NotFound,
            Self::Unsupported(_) => NvmExitCode::UnsupportedOption,
            Self::InvalidOptions(_) => NvmExitCode::InvalidOptions,
            Self::Floor(_) => NvmExitCode::BelowVersionFloor,
            Self::Alias(_) => NvmExitCode::AliasLoop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_the_nvm_contract() {
        assert_eq!(NvmExitCode::Success.code(), 0);
        assert_eq!(NvmExitCode::InvalidVersion.code(), 3);
        assert_eq!(NvmExitCode::BelowVersionFloor.code(), 7);
        assert_eq!(NvmExitCode::AliasLoop.code(), 8);
        assert_eq!(NvmExitCode::IncompatiblePrefix.code(), 11);
        assert_eq!(NvmExitCode::MissingTarget.code(), 2);
        assert_eq!(NvmExitCode::SameVersion.code(), 4);
        assert_eq!(NvmExitCode::SourceNotInstalled.code(), 5);
        assert_eq!(NvmExitCode::InvalidOptions.code(), 6);
        assert_eq!(NvmExitCode::HookClaimedSuccess.code(), 33);
        assert_eq!(NvmExitCode::Passed(7).code(), 7);
        assert_eq!(NvmExitCode::UnsupportedOption.code(), 55);
        assert_eq!(NvmExitCode::NotFound.code(), 127);
    }

    #[test]
    fn a_program_that_ran_passes_its_status_on() {
        assert_eq!(NvmExitCode::passing_on(Some(7)), NvmExitCode::Passed(7));
        assert_eq!(NvmExitCode::passing_on(Some(0)), NvmExitCode::Success);
        assert_eq!(NvmExitCode::passing_on(None), NvmExitCode::Failure);
        assert_eq!(NvmExitCode::passing_on(Some(-1)), NvmExitCode::Failure);
        assert_eq!(NvmExitCode::passing_on(Some(300)), NvmExitCode::Failure);
    }

    fn assert_exit_codes<const N: usize>(cases: [(CliError, NvmExitCode); N]) {
        for (error, code) in cases {
            assert_eq!(error.exit_code(), code, "{error}");
        }
    }

    #[test]
    fn wrapped_errors_map_to_their_exit_codes() {
        let source = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert_exit_codes([
            (
                FloorError::Invalid("x".into()).into(),
                NvmExitCode::BelowVersionFloor,
            ),
            (AliasError::Loop("a".into()).into(), NvmExitCode::AliasLoop),
            (
                VersionError::Invalid("x".into()).into(),
                NvmExitCode::InvalidVersion,
            ),
            (
                CliError::io(Path::new("/n/alias/work"), source),
                NvmExitCode::Failure,
            ),
        ]);
    }

    #[test]
    fn errors_of_the_commands_map_to_their_exit_codes() {
        assert_exit_codes([
            (CliError::NotInstalled, NvmExitCode::InvalidVersion),
            (CliError::NvmDirUnresolved, NvmExitCode::Failure),
            (
                CliError::VersionNotInstalled("x".into()),
                NvmExitCode::Failure,
            ),
            (CliError::Usage("x".into()), NvmExitCode::NotFound),
            (CliError::SystemNodeNotFound, NvmExitCode::NotFound),
            (
                CliError::Unsupported("x".into()),
                NvmExitCode::UnsupportedOption,
            ),
            (
                CliError::InvalidOptions("x".into()),
                NvmExitCode::InvalidOptions,
            ),
            (CliError::InvalidArgument("x".into()), NvmExitCode::Failure),
        ]);
    }

    #[test]
    fn io_errors_name_the_path() {
        let source = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let message = CliError::io(Path::new("/n/alias/work"), source).to_string();
        assert!(message.starts_with("/n/alias/work: "), "{message}");
    }
}
