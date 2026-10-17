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
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::nvmrc::pins::Pins;
use crate::domain::nvmrc::{NvmrcContent, process_content};
use crate::domain::tool::Tool;
use crate::domain::version::Version;
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
        Ok(Upgraded::Written { nvmrc, moved }) => {
            install_written(context, &nvmrc, &moved, &mut transcript)
        }
        Ok(Upgraded::Nothing) => NvmExitCode::Success,
        Err(stop) => stop.printed(&mut transcript),
    };
    Ok(transcript.finish(status))
}

/// Installs what the rewritten `.nvmrc` asks for, as `nvm install` would,
/// or, when it has no node line, only the pins this run `moved` (the
/// policy allowed them; the others are left alone); its output follows the
/// upgrade's and its status is the command's. An error of the install is
/// printed like its output; the files are written either way, and the
/// report says so.
fn install_written(
    context: &Context<'_>,
    nvmrc: &Path,
    moved: &[(Tool, Version)],
    transcript: &mut Transcript,
) -> NvmExitCode {
    let status = match has_node_line(context, nvmrc) {
        Ok(false) => install_pins(context, &moved_pins(moved), transcript),
        Ok(true) => install_all(context, transcript),
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

/// Whether the `.nvmrc` at `nvmrc` has a node line.
fn has_node_line(context: &Context<'_>, nvmrc: &Path) -> Result<bool, CliError> {
    let text = current_nvmrc(context, nvmrc)?;
    Ok(matches!(process_content(&text), NvmrcContent::Version(_)))
}

/// The package managers among `moved`, as pins.
fn moved_pins(moved: &[(Tool, Version)]) -> Pins {
    let mut pins = Pins::default();
    for (tool, version) in moved {
        let slot = match tool {
            Tool::Npm => &mut pins.npm,
            Tool::Yarn => &mut pins.yarn,
            Tool::Pnpm => &mut pins.pnpm,
            Tool::Node => continue,
        };
        *slot = Some(*version);
    }
    pins
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
