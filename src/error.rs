//! Typed errors and the single mapping from errors to process exit codes.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Public exit-code contract, taken from `nvm.sh`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum NvmExitCode {
    #[default]
    Success = 0,
    Failure = 1,
    InvalidVersion = 3,
    BelowVersionFloor = 7,
    AliasLoop = 8,
    /// `nvm alias lts/<name>` for an alias that does not exist.
    NoSuchAlias = 2,
    /// An option `nvm.sh` does not support, or one used in a combination it
    /// does not support.
    UnsupportedOption = 55,
    /// A usage error, or a requested system version that does not exist.
    NotFound = 127,
}

impl NvmExitCode {
    #[must_use]
    pub fn code(self) -> u8 {
        self as u8
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
            | Self::Io { .. } => NvmExitCode::Failure,
            Self::Usage(_) | Self::SystemNodeNotFound => NvmExitCode::NotFound,
            Self::Unsupported(_) => NvmExitCode::UnsupportedOption,
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
        assert_eq!(NvmExitCode::NoSuchAlias.code(), 2);
        assert_eq!(NvmExitCode::UnsupportedOption.code(), 55);
        assert_eq!(NvmExitCode::NotFound.code(), 127);
    }

    #[test]
    fn errors_map_to_their_exit_codes() {
        let floor = FloorError::Invalid("x".into());
        assert_eq!(
            CliError::from(floor).exit_code(),
            NvmExitCode::BelowVersionFloor
        );
        let alias = AliasError::Loop("a".into());
        assert_eq!(CliError::from(alias).exit_code(), NvmExitCode::AliasLoop);
        let version = VersionError::Invalid("x".into());
        assert_eq!(
            CliError::from(version).exit_code(),
            NvmExitCode::InvalidVersion
        );
        assert_eq!(
            CliError::NotInstalled.exit_code(),
            NvmExitCode::InvalidVersion
        );
        assert_eq!(CliError::NvmDirUnresolved.exit_code(), NvmExitCode::Failure);
        let not_installed = CliError::VersionNotInstalled("x".into());
        assert_eq!(not_installed.exit_code(), NvmExitCode::Failure);
        let usage = CliError::Usage("x".into());
        assert_eq!(usage.exit_code(), NvmExitCode::NotFound);
        let no_system_node = CliError::SystemNodeNotFound;
        assert_eq!(no_system_node.exit_code(), NvmExitCode::NotFound);
        let unsupported = CliError::Unsupported("x".into());
        assert_eq!(unsupported.exit_code(), NvmExitCode::UnsupportedOption);
        let invalid = CliError::InvalidArgument("x".into());
        assert_eq!(invalid.exit_code(), NvmExitCode::Failure);
        let source = std::io::Error::from(std::io::ErrorKind::NotFound);
        let io_error = CliError::io(Path::new("/n/alias/work"), source);
        assert_eq!(io_error.exit_code(), NvmExitCode::Failure);
    }

    #[test]
    fn io_errors_name_the_path() {
        let source = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        let message = CliError::io(Path::new("/n/alias/work"), source).to_string();
        assert!(message.starts_with("/n/alias/work: "), "{message}");
    }
}
