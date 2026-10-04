//! The `nvm-exec` executable (digest 8): select a node version from
//! `NODE_VERSION`, else from the `.nvmrc`, and leave the command to the CLI,
//! which runs it with the environment of the selected version.
//!
//! Every failure to select is status 127 (a refused prefix included), and
//! there is no fallback to `default` or to the active version.

use crate::commands::Output;
use crate::commands::exec::child::{self, Environment};
use crate::commands::rc_version::{RcVersion, rc_version};
use crate::commands::transcript::Transcript;
use crate::commands::use_version::apply::Switch;
use crate::commands::use_version::target::lookup::ensure_installed;
use crate::commands::use_version::{self, Target};
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};

const UNABLE_TO_SELECT: [&str; 2] = [
    "nvm-exec: unable to select a node version",
    "  Set `NODE_VERSION` (e.g. `NODE_VERSION=default`), or add an `.nvmrc` file.",
];

/// What `nvm use` made of the request.
struct Selection {
    /// What it printed on stderr.
    stderr: String,
    /// The version it switched to, or `None` when it failed.
    target: Option<Target>,
}

/// `nvm-exec <command> [<args>...]`: the version comes from `NODE_VERSION`
/// (what `nvm use "$NODE_VERSION"` accepts, its stdout discarded and its
/// stderr kept) or, when that is empty or unset, from the `.nvmrc`.
///
/// The command runs as the CLI's child: a leading `--` is dropped and no
/// command is success. `NODE_VERSION` is left as the parent has it.
#[must_use]
pub fn run(context: &Context<'_>, command: &[String]) -> Output {
    let mut transcript = Transcript::default();
    let environment = match select(context, &mut transcript) {
        Ok(Some(environment)) => environment,
        Ok(None) => return transcript.finish(NvmExitCode::NotFound),
        Err(error) => {
            transcript.err(error.to_string());
            return transcript.finish(NvmExitCode::NotFound);
        }
    };
    let output = transcript.finish(NvmExitCode::Success);
    match child::invocation(command, environment.keep_node_version()) {
        Some(invocation) => output.with_spawn(invocation),
        None => output,
    }
}

/// The environment of the selected version, or `None` with the reasons in
/// `transcript`.
fn select(
    context: &Context<'_>,
    transcript: &mut Transcript,
) -> Result<Option<Environment>, CliError> {
    let selection = match context
        .env
        .var("NODE_VERSION")
        .filter(|name| !name.is_empty())
    {
        Some(name) => {
            let selection = switch(context, &[name])?;
            if !selection.stderr.is_empty() {
                transcript.err(selection.stderr.clone());
            }
            selection
        }
        None => from_nvmrc(context, transcript)?,
    };
    selection
        .target
        .map(|target| environment(context, &target))
        .transpose()
}

/// The `.nvmrc` path: the lookup and the install check print as they do in
/// the script, then a muted `nvm use` decides.
fn from_nvmrc(context: &Context<'_>, transcript: &mut Transcript) -> Result<Selection, CliError> {
    if let RcVersion::Found { version, .. } = rc_version(context, false, transcript) {
        // The script goes on whether or not the version is installed.
        ensure_installed(context, &version, false, transcript)?;
    }
    let selection = switch(context, &[])?;
    if selection.target.is_none() {
        UNABLE_TO_SELECT
            .iter()
            .for_each(|line| transcript.err(*line));
    }
    Ok(selection)
}

/// `nvm use <args>`: its stderr and, when it succeeded, the version it chose.
fn switch(context: &Context<'_>, args: &[String]) -> Result<Selection, CliError> {
    let (output, target) = use_version::run_and_target(context, args)?;
    let target = target.filter(|_| output.status == NvmExitCode::Success);
    Ok(Selection {
        stderr: output.stderr,
        target,
    })
}

fn environment(context: &Context<'_>, target: &Target) -> Result<Environment, CliError> {
    match target {
        Target::System(_) => child::system(context).map(|(_, environment)| environment),
        Target::Installed { directory, .. } => {
            let switch = Switch::new(&context.nvm_dir()?, directory);
            child::installed(context, &switch, &switch.path(context)?, "")
        }
    }
}

#[cfg(test)]
mod tests;
