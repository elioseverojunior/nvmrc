//! The policy gate for commands that pick an installed or system node (`use`,
//! `exec`, `run`): what they have chosen, checked before anything switches or
//! runs.

use super::enforce::{Gate, Origin, Requested, enforce_node};
use super::load::policy_mode_requested;
use crate::commands::project_pins::ProjectPins;
use crate::commands::resolve::system_version;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::nvmrc::find_nvmrc;
use crate::domain::version::Version;
use crate::error::{CliError, PolicyError};

/// The `.nvmrc` the version came from when `from_nvmrc`, else the command
/// line.
#[must_use]
pub fn origin_of(context: &Context<'_>, from_nvmrc: bool) -> Origin {
    if !from_nvmrc {
        return Origin::Argument;
    }
    context
        .working_directory()
        .and_then(|directory| find_nvmrc(context.fs, &directory))
        .map_or(Origin::Argument, Origin::Nvmrc)
}

/// Checks an installed `version` and the package managers in effect with
/// it; the release index is fetched only when a rule needs it.
///
/// # Errors
/// As [`enforce_node`].
pub fn gate_installed(
    context: &Context<'_>,
    version: &Version,
    origin: Origin,
    pins: &ProjectPins,
    transcript: &mut Transcript,
) -> Result<Gate, CliError> {
    let requested = Requested {
        version: *version,
        origin,
        offline: false,
        pins: pins.clone(),
    };
    enforce_node(context, &requested, transcript)
}

/// Checks the system node, whose version is read from `node --version`.
///
/// # Errors
/// [`CliError::Policy`] when a policy applies and the version cannot be
/// read, unless the context is advisory (then a warning and a pass).
pub fn gate_system(
    context: &Context<'_>,
    pins: &ProjectPins,
    transcript: &mut Transcript,
) -> Result<Gate, CliError> {
    if !policy_mode_requested(context) {
        return Ok(Gate::Pass);
    }
    let version = match system_version(context) {
        Ok(text) => text.and_then(|text| text.parse::<Version>().ok()),
        Err(error) => return unknown_system(context, error, transcript),
    };
    match version {
        Some(version) => gate_installed(context, &version, Origin::System, pins, transcript),
        None => unknown_system(context, unreadable(), transcript),
    }
}

fn unreadable() -> CliError {
    CliError::Policy(PolicyError::SystemVersionUnreadable)
}

/// The system version could not be known: an error, or in an advisory
/// context a warning and a pass.
fn unknown_system(
    context: &Context<'_>,
    error: CliError,
    transcript: &mut Transcript,
) -> Result<Gate, CliError> {
    if !context.is_advisory() {
        return Err(error);
    }
    let text = error.to_string();
    transcript.err(format!(
        "nvm: warning: {}",
        text.trim_start_matches("nvm: ")
    ));
    Ok(Gate::Pass)
}
