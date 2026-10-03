//! `nvm reinstall-packages <version>` (also `copy-packages`): install in the
//! node in use the global packages of another version.

use crate::commands::Output;
use crate::commands::current;
use crate::commands::npm::Npm;
use crate::commands::npm::packages::{Source, reinstall};
use crate::commands::resolve::{Resolved, resolve_installed, system_node};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};

/// # Errors
/// - [`CliError::Usage`] unless exactly one word is given.
/// - [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(context: &Context<'_>, command: &str, args: &[String]) -> Result<Output, CliError> {
    let [provided] = args else {
        let usage = format!("Usage: nvm {command} <version>\n  Run `nvm --help` for full help.");
        return Err(CliError::Usage(usage));
    };
    let active = current::detect(context)?.to_string();
    let source = source_of(provided, resolve_installed(context, provided)?);
    let mut transcript = Transcript::default();
    if *provided == active || source.label() == active {
        transcript.err("Can not reinstall packages from the current version of node.");
        return Ok(transcript.finish(NvmExitCode::MissingTarget));
    }
    if source == Source::System && system_node(context)?.is_none() {
        transcript.err("No system version of node or io.js detected.");
        return Ok(transcript.finish(NvmExitCode::InvalidVersion));
    }
    let Some(destination) = Npm::on_path(context) else {
        transcript
            .err("npm was not found on PATH; there is nowhere to reinstall the global packages.");
        return Ok(transcript.finish(NvmExitCode::Failure));
    };
    let status = reinstall(context, &source, &destination, &mut transcript);
    Ok(transcript.finish(status))
}

/// Where the packages come from: what was asked for, once resolved.
fn source_of(provided: &str, resolved: Resolved) -> Source {
    match resolved {
        Resolved::Installed(version) => Source::Version(version),
        Resolved::System => Source::System,
        Resolved::Missing { .. } if provided == "system" => Source::System,
        Resolved::Missing { .. } => Source::Missing,
    }
}

#[cfg(test)]
mod tests;
