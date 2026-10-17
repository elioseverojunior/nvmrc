//! `nvm upgrade [--node] [--npm] [--yarn] [--pnpm] [--major[=<N>]]`: moves
//! the tools pinned in the nearest `.nvmrc` to newer allowed releases (see
//! [`request`]) and rewrites the files (see [`project`]).

mod pinned;
mod project;
mod request;
mod target;

use std::path::Path;

use crate::commands::Output;
use crate::commands::install;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};
use project::{Upgraded, upgrade};
use request::parse;

/// Upgrades the pinned tools named in `args`, or all of them.
///
/// # Errors
/// See [`request::parse`]; the upgrade reports its own failures in the
/// output, with their status.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let request = parse(args)?;
    let mut transcript = Transcript::default();
    let status = match upgrade(context, &request, &mut transcript) {
        Ok(Upgraded::Written { nvmrc }) => install_written(context, &nvmrc, &mut transcript),
        Ok(Upgraded::Nothing) => NvmExitCode::Success,
        Err(stop) => stop.printed(&mut transcript),
    };
    Ok(transcript.finish(status))
}

/// Installs what the rewritten `.nvmrc` asks for, as `nvm install` would;
/// its output follows the upgrade's and its status is the command's. An
/// error of the install is printed like its output; the files are written
/// either way, and the report says so.
fn install_written(
    context: &Context<'_>,
    nvmrc: &Path,
    transcript: &mut Transcript,
) -> NvmExitCode {
    let status = match install::run(context, &[]) {
        Ok(output) => {
            let status = output.status;
            transcript.absorb(output);
            status
        }
        Err(error) => {
            transcript.err(error.to_string());
            error.exit_code()
        }
    };
    if status != NvmExitCode::Success {
        transcript.err(format!(
            "nvm upgrade: {} is updated, but the install failed; run nvm install to finish.",
            nvmrc.display()
        ));
    }
    status
}

#[cfg(test)]
mod install_tests;
