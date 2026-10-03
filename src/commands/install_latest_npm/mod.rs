//! `nvm install-latest-npm`: upgrade the `npm` of the node in use.

use crate::commands::Output;
use crate::commands::current;
use crate::commands::npm::Npm;
use crate::commands::npm::latest::{Node, install_latest};
use crate::commands::resolve::system_version;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::current::Current;
use crate::error::CliError;

const USAGE: &str = "Usage: nvm install-latest-npm\n  Run `nvm --help` for full help.";

/// # Errors
/// - [`CliError::Usage`] when given any word.
/// - [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    if !args.is_empty() {
        return Err(CliError::Usage(USAGE.to_owned()));
    }
    let active = current::detect(context)?;
    let system = system_version(context)?;
    let node = match (&active, &system) {
        (Current::Version(version), _) => Some(version.to_string()),
        (Current::System, version) => version.clone(),
        (Current::None, _) => None,
    };
    let npm = Npm::on_path(context);
    let mut transcript = Transcript::default();
    let known = node.as_deref().map_or(Node::None, Node::Version);
    let status = install_latest(context, npm.as_ref(), &known, &mut transcript);
    Ok(transcript.finish(status))
}

#[cfg(test)]
mod tests;
