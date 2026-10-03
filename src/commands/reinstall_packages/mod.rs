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
    let resolved = resolve_installed(context, provided)?;
    let named = match &resolved {
        Resolved::Installed(version) => version.to_string(),
        Resolved::System => "system".to_owned(),
        Resolved::Missing { .. } => "N/A".to_owned(),
    };
    let mut transcript = Transcript::default();
    if *provided == active || named == active {
        transcript.err("Can not reinstall packages from the current version of node.");
        return Ok(transcript.finish(NvmExitCode::MissingTarget));
    }
    let source = match resolved {
        Resolved::Installed(version) => Source::Version(version),
        Resolved::System => Source::System,
        Resolved::Missing { .. } if provided == "system" => Source::System,
        Resolved::Missing { .. } => Source::Missing,
    };
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

#[cfg(test)]
mod tests;
