//! The arguments of `nvm migrate`.

use super::USAGE;
use crate::error::CliError;
use crate::shell::init::Shell;

/// How `nvm migrate` was called.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// `--dry-run`: print the diffs and write nothing.
    pub dry_run: bool,
    /// `--yes` or `-y`: apply without asking.
    pub yes: bool,
    /// `--undo`: restore the latest backups.
    pub undo: bool,
    /// `--shell <name>`: the shell whose files are scanned, and whose init
    /// line goes in the block; every shell when `None`.
    pub shell: Option<Shell>,
}

/// The options in `args`; a repeated `--shell` takes the last name.
///
/// # Errors
/// [`CliError::Usage`] for an unknown shell, a missing shell name or a
/// positional word, and [`CliError::Unsupported`] for any other option.
pub fn parse(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    let mut arguments = args.iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--dry-run" => options.dry_run = true,
            "--yes" | "-y" => options.yes = true,
            "--undo" => options.undo = true,
            "--shell" => {
                let name = arguments
                    .next()
                    .ok_or_else(|| CliError::Usage(format!("--shell needs a name.\n{USAGE}")))?;
                options.shell = Some(name.parse()?);
            }
            other => match other.strip_prefix("--shell=") {
                Some(name) => options.shell = Some(name.parse()?),
                None => return Err(rejected(other)),
            },
        }
    }
    Ok(options)
}

fn rejected(argument: &str) -> CliError {
    if argument.starts_with('-') {
        CliError::Unsupported(format!("Unsupported option \"{argument}\"."))
    } else {
        CliError::Usage(format!("Unexpected argument \"{argument}\".\n{USAGE}"))
    }
}
