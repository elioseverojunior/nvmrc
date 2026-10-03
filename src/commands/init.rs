//! `nvm init <shell> [--no-use] [--install]`: prints the shell code that
//! defines the `nvm` function (see [`crate::shell::init`]).

use crate::commands::Output;
use crate::error::CliError;
use crate::shell::init::{InitOptions, Shell, USAGE, snippet};

/// Prints the snippet for the shell named in `args`. Of `--no-use` and
/// `--install`, the last one given wins, as in `nvm_process_parameters`.
///
/// # Errors
/// [`CliError::Usage`] without a shell or for one that is not supported,
/// and [`CliError::Unsupported`] for an unknown option or a second shell.
pub fn run(args: &[String]) -> Result<Output, CliError> {
    let mut options = InitOptions::default();
    let mut shell_name: Option<&str> = None;
    for arg in args {
        match arg.as_str() {
            "--no-use" => options = auto_step(true, false),
            "--install" => options = auto_step(false, true),
            word if word.starts_with('-') || shell_name.is_some() => {
                let message = format!("nvm init: unsupported argument \"{word}\".\n{USAGE}");
                return Err(CliError::Unsupported(message));
            }
            word => shell_name = Some(word),
        }
    }
    let shell: Shell = shell_name
        .ok_or_else(|| CliError::Usage(USAGE.to_owned()))?
        .parse()?;
    let text = snippet(shell, &options);
    Ok(Output::stdout(text.trim_end_matches('\n')))
}

fn auto_step(no_use: bool, install: bool) -> InitOptions {
    InitOptions { no_use, install }
}
