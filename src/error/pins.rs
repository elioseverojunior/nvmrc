//! The error for a pin or a `package.json` that cannot be used.

use thiserror::Error;

use super::CliError;

/// A pin in `.nvmrc`, or the `package.json` beside it, that cannot be used.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PinsError {
    #[error("nvm: {nvmrc}: '{line}' is not an exact version (major.minor.patch).")]
    NotExact { nvmrc: String, line: String },
    #[error("nvm: {nvmrc} pins both yarn and pnpm; pin at most one.")]
    YarnAndPnpm { nvmrc: String },
    #[error("nvm: {nvmrc} pins {key} more than once; keep one {key}= line.")]
    Repeated { nvmrc: String, key: String },
    #[error(
        "nvm: {package_json} has no packageManager; set it to \"{expected}\" to match {nvmrc}."
    )]
    MissingPackageManager {
        package_json: String,
        nvmrc: String,
        expected: String,
    },
    #[error(
        "nvm: {package_json} has packageManager \"{found}\", but {nvmrc} pins {tool} {version}; set it to \"{expected}\"."
    )]
    DifferentPackageManager {
        package_json: String,
        nvmrc: String,
        found: String,
        tool: String,
        version: String,
        expected: String,
    },
    /// `tool` is the tool and its version as users write them (`pnpm 9.12.0`).
    #[error("nvm: {tool} (from {nvmrc}) is not installed. Run `nvm install`.")]
    NotInstalled { tool: String, nvmrc: String },
    #[error("nvm: {package_json} cannot be read as JSON: {reason}")]
    UnreadablePackageJson {
        package_json: String,
        reason: String,
    },
}

impl From<PinsError> for CliError {
    fn from(error: PinsError) -> Self {
        Self::Pins(Box::new(error))
    }
}
