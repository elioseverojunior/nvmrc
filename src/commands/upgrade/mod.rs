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
        Ok(Upgraded::Written { nvmrc }) => install(context, &nvmrc, &mut transcript)?,
        Ok(Upgraded::Nothing) => NvmExitCode::Success,
        Err(stop) => stop.printed(&mut transcript),
    };
    Ok(transcript.finish(status))
}

/// Installs what the rewritten `.nvmrc` asks for, as `nvm install` would;
/// its output follows the upgrade's and its status is the command's.
fn install(
    context: &Context<'_>,
    nvmrc: &Path,
    transcript: &mut Transcript,
) -> Result<NvmExitCode, CliError> {
    let output = install::run(context, &[])?;
    let status = output.status;
    transcript.absorb(output);
    if status != NvmExitCode::Success {
        transcript.err(format!(
            "nvm upgrade: {} is updated, but the install failed; run nvm install to finish.",
            nvmrc.display()
        ));
    }
    Ok(status)
}

#[cfg(test)]
mod install_tests;
