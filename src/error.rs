//! Typed errors and the single mapping from errors to process exit codes.

use thiserror::Error;

/// Public exit-code contract, taken from `nvm.sh`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NvmExitCode {
    Success = 0,
    Failure = 1,
    InvalidVersion = 3,
    BelowVersionFloor = 7,
    AliasLoop = 8,
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
}

impl CliError {
    #[must_use]
    pub fn exit_code(&self) -> NvmExitCode {
        match self {
            Self::Version(_) | Self::NotInstalled => NvmExitCode::InvalidVersion,
            Self::NvmDirUnresolved => NvmExitCode::Failure,
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
    }
}
