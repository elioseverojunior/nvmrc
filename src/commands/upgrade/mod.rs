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
use crate::commands::install::pinned::install_pins;
use crate::commands::project::current_nvmrc;
use crate::commands::project_pins::pin_error;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::nvmrc::pins::{Pins, parse_pins};
use crate::domain::nvmrc::{NvmrcContent, process_content};
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

/// Installs what the rewritten `.nvmrc` asks for, as `nvm install` would,
/// or only its pins when it has no node line; its output follows the
/// upgrade's and its status is the command's. An error of the install is
/// printed like its output; the files are written either way, and the
/// report says so.
fn install_written(
    context: &Context<'_>,
    nvmrc: &Path,
    transcript: &mut Transcript,
) -> NvmExitCode {
    let status = match pins_only(context, nvmrc) {
        Ok(Some(pins)) => install_pins(context, &pins, transcript),
        Ok(None) => install_all(context, transcript),
        Err(error) => printed(error, transcript),
    };
    if status != NvmExitCode::Success {
        transcript.err(format!(
            "nvm upgrade: {} is updated, but the install failed; run nvm install to finish.",
            nvmrc.display()
        ));
    }
    status
}

/// The pins of the `.nvmrc` at `nvmrc` when it has no node line.
fn pins_only(context: &Context<'_>, nvmrc: &Path) -> Result<Option<Pins>, CliError> {
    let text = current_nvmrc(context, nvmrc)?;
    if matches!(process_content(&text), NvmrcContent::Version(_)) {
        return Ok(None);
    }
    let pins = parse_pins(&text).map_err(|error| CliError::from(pin_error(error, nvmrc)))?;
    Ok(Some(pins))
}

/// `nvm install` with no argument.
fn install_all(context: &Context<'_>, transcript: &mut Transcript) -> NvmExitCode {
    match install::run(context, &[]) {
        Ok(output) => {
            let status = output.status;
            transcript.absorb(output);
            status
        }
        Err(error) => printed(error, transcript),
    }
}

fn printed(error: CliError, transcript: &mut Transcript) -> NvmExitCode {
    transcript.err(error.to_string());
    error.exit_code()
}

#[cfg(test)]
mod install_tests;
