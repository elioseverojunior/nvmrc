//! The arguments of `nvm use`, scanned as nvm.sh does: every argument is
//! looked at, flags anywhere, the last positional wins.

use crate::error::CliError;

/// What `nvm use` was asked for.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Options {
    /// `--silent`: no messages but the ones nvm.sh prints regardless.
    pub silent: bool,
    /// `--delete-prefix`: remove an npm `prefix` that hides the version.
    pub delete_prefix: bool,
    /// `--save` or `-w`: write the version to `$PWD/.nvmrc`.
    pub save: bool,
    /// `--lts` (`*`) or `--lts=<name>`; `--lts=` is no LTS.
    pub lts: Option<String>,
    /// The last non-empty positional argument.
    pub version: Option<String>,
}

/// Parses the arguments of `nvm use`.
///
/// # Errors
/// Returns [`CliError::InvalidOptions`] (status 6) when `--save` or `-w` is
/// given more than once.
pub fn parse(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    for arg in args {
        options.take(arg)?;
    }
    Ok(options)
}

const SAVE_GIVEN_TWICE: &str = "--save and -w may only be provided once";

impl Options {
    fn take(&mut self, arg: &str) -> Result<(), CliError> {
        match arg {
            "--silent" => self.silent = true,
            "--delete-prefix" => self.delete_prefix = true,
            "--lts" => self.lts = Some("*".to_owned()),
            "--save" | "-w" if self.save => {
                return Err(CliError::InvalidOptions(SAVE_GIVEN_TWICE.to_owned()));
            }
            "--save" | "-w" => self.save = true,
            "" => {}
            // `--lts=` is no LTS; `--` and any other `--flag` are ignored.
            long if long.starts_with("--") => {
                if let Some(name) = long.strip_prefix("--lts=") {
                    self.lts = Some(name.to_owned()).filter(|name| !name.is_empty());
                }
            }
            version => self.version = Some(version.to_owned()),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
