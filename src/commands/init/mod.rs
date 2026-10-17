//! `nvm init <shell> [--no-use] [--install]`: prints the shell code that
//! defines the `nvm` function (see [`crate::shell::init`]). With
//! `--node`, `--npm`, `--yarn` or `--pnpm` instead of a shell, it sets up
//! the project (see [`request`]).

mod request;

use crate::commands::Output;
use crate::context::Context;
use crate::error::CliError;
use crate::shell::init::snippet;
use request::{InitMode, parse};

/// Prints the snippet for the shell named in `args`, or sets up the project.
///
/// # Errors
/// See [`request::parse`]; project mode is not available yet.
pub fn run(_context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    match parse(args)? {
        InitMode::Shell(shell, options) => {
            let text = snippet(shell, &options);
            Ok(Output::stdout(text.trim_end_matches('\n')))
        }
        InitMode::Project(_) => Err(CliError::Unsupported(
            "nvm init: project mode is not available yet.".to_owned(),
        )),
    }
}
