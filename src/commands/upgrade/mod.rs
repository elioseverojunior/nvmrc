//! `nvm upgrade [--node] [--npm] [--yarn] [--pnpm] [--major[=<N>]]`: moves
//! the tools pinned in the nearest `.nvmrc` to newer allowed releases (see
//! [`request`]) and rewrites the files (see [`project`]).

mod pinned;
mod project;
mod request;
mod target;

use crate::commands::Output;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};
use project::upgrade;
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
        Ok(_) => NvmExitCode::Success,
        Err(stop) => stop.printed(&mut transcript),
    };
    Ok(transcript.finish(status))
}
