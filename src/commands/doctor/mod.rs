//! `nvm doctor [--shell <name>]`: reports the startup files that still load
//! nvm.sh or fight nvmrc's `nvm` (plan 8). Read-only: it scans and prints,
//! and never writes a file. Exit status 1 when a conflict exists.

mod fix;
mod render;

#[cfg(test)]
mod fix_tests;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod tests;

use crate::commands::Output;
use crate::commands::conflict::{roots, scan};
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};
use crate::shell::init::Shell;

pub use fix::fix_text;

/// How `nvm doctor` is called.
pub const USAGE: &str = "Usage: nvm doctor [--shell <name>]";

const NO_FILES: &str = "nvm doctor: no shell profile files found";

/// Scans the startup files of the shell named by `--shell` (of every shell
/// with files when none is named) and prints what it found.
///
/// # Errors
/// [`CliError::Usage`] for an unknown shell, a missing shell name or a
/// positional word, and [`CliError::Unsupported`] for any other option.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let shell = parse_shell(args)?;
    let files = roots(context, shell);
    if files.is_empty() {
        return Ok(Output::stdout(NO_FILES));
    }
    let report = scan(context, &files);
    let status = if report.has_conflicts() {
        NvmExitCode::Failure
    } else {
        NvmExitCode::Success
    };
    Ok(Output::stdout(render::render(&report)).with_status(status))
}

/// The shell of `--shell <name>` or `--shell=<name>`; the last one wins.
fn parse_shell(args: &[String]) -> Result<Option<Shell>, CliError> {
    let mut shell = None;
    let mut arguments = args.iter();
    while let Some(argument) = arguments.next() {
        let name = if argument == "--shell" {
            let name = arguments
                .next()
                .ok_or_else(|| CliError::Usage(format!("--shell needs a name.\n{USAGE}")))?;
            Some(name.as_str())
        } else {
            argument.strip_prefix("--shell=")
        };
        match name {
            Some(name) => shell = Some(name.parse()?),
            None => return Err(rejected(argument)),
        }
    }
    Ok(shell)
}

fn rejected(argument: &str) -> CliError {
    if argument.starts_with('-') {
        CliError::Unsupported(format!("Unsupported option \"{argument}\"."))
    } else {
        CliError::Usage(format!("Unexpected argument \"{argument}\".\n{USAGE}"))
    }
}
