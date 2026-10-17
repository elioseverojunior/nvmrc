//! `nvm exec` (digest 6): pick a version like nvm.sh, say what runs, and
//! leave the command to the CLI, which runs it once the messages are out,
//! with the environment `nvm-exec`'s `nvm use` would give it.
//!
//! DELIBERATE DEVIATION: nvm.sh prints its help when any argument before a
//! `--` is `-h`, `help` or `--help` (so `nvm exec 18 node -h` shows nvm's
//! help); here every argument after the subcommand reaches the command.

pub(crate) mod child;
mod launch;
mod options;
mod select;

use crate::commands::Output;
use crate::commands::policy::enforce::Origin;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::error::CliError;

/// `nvm exec [--silent] [--lts[=<name>]] [<version>] [<command> [<args>]]`.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    run_after(context, args, Transcript::default())
}

/// `nvm exec` after the messages already in `transcript`, as `nvm run`
/// calls it.
///
/// # Errors
/// As [`run`].
pub fn run_after(
    context: &Context<'_>,
    args: &[String],
    transcript: Transcript,
) -> Result<Output, CliError> {
    run_after_from(context, args, transcript, None)
}

/// [`run_after`], with the policy origin of the version when the caller
/// already resolved it (`nvm run` hands over the version it picked).
///
/// # Errors
/// As [`run`].
pub fn run_after_from(
    context: &Context<'_>,
    args: &[String],
    mut transcript: Transcript,
    origin: Option<Origin>,
) -> Result<Output, CliError> {
    let options = match options::parse(args) {
        Ok(options) => options,
        Err(error) => {
            transcript.err(error.to_string());
            return Ok(transcript.finish(error.exit_code()));
        }
    };
    let mut selection = select::select(context, &options, &mut transcript)?;
    if let Some(origin) = origin {
        selection.origin = origin;
    }
    launch::launch(context, &options, selection, transcript)
}

/// The three stderr lines `nvm exec` and `nvm run` print before falling
/// back to the active version; `command` is `exec` or `run`.
#[must_use]
pub fn fallback_warning(command: &str) -> [String; 3] {
    [
        format!(
            "WARNING: `nvm {command}` was invoked without a version argument and without an \
.nvmrc file."
        ),
        "  Falling back to the active node version; this will become an error in a future \
release."
            .to_owned(),
        format!(
            "  Pass `current` explicitly (e.g. `nvm {command} current ...`) to silence this \
warning."
        ),
    ]
}

/// The lab the tests of `exec` and `run` share.
#[cfg(test)]
pub(crate) mod tests;
