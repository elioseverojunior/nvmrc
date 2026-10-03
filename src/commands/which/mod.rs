//! `nvm which <version>`: the path to the `node` binary of a version.

use std::path::PathBuf;

use crate::commands::Output;
use crate::commands::current;
use crate::commands::install::place::version_path;
use crate::commands::rc_version::{RcVersion, rc_version};
use crate::commands::resolve::{Resolved, resolve_installed, system_node};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::version::Version;
use crate::domain::version_prefix::with_v_prefix;
use crate::error::{CliError, NvmExitCode};

const USAGE: &str = "Usage: nvm which [current | <version>]\n  \
Provide a <version>, or run from a directory containing an .nvmrc file.\n  \
Run `nvm --help` for full help.";

/// `nvm which [--silent] [current | <version>]`: every argument is looked at,
/// the last positional one wins, `--silent` is taken and `--` and any other
/// flag are ignored, as in nvm.sh.
///
/// # Errors
/// As [`run`].
pub fn run_command(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let silent = args.iter().any(|arg| arg == "--silent");
    let name = args
        .iter()
        .rfind(|arg| !arg.starts_with('-') || arg.len() == 1)
        .map(String::as_str);
    run(context, name, silent)
}

/// Without a version the `.nvmrc` is read (`nvm_rc_version`) and its version
/// is resolved like an explicit one; its `Found` line goes on stdout, ahead
/// of the path or of the error that follows.
///
/// # Errors
/// - [`CliError::Usage`] when no version is given and no usable `.nvmrc`
///   gives one (127): the text starts with what the lookup printed (the
///   missing message, or the invalid `.nvmrc` message, which `which` does
///   not follow with the `Please see` line).
/// - [`CliError::Alias`] when the alias chain loops.
/// - [`CliError::VersionNotInstalled`] when the version is not installed, or
///   its `node` binary is missing.
/// - [`CliError::SystemNodeNotFound`] for `system` without a system node.
///
/// A failure after a `.nvmrc` was found is returned as an [`Output`] (its
/// stderr and status) so the `Found` line printed before it is not lost.
pub fn run(context: &Context<'_>, name: Option<&str>, silent: bool) -> Result<Output, CliError> {
    if let Some(name) = name {
        return binary_of(context, name);
    }
    let mut transcript = Transcript::default();
    let RcVersion::Found { version, .. } = rc_version(context, silent, &mut transcript) else {
        let printed = transcript.finish(NvmExitCode::Success).stderr;
        let lead = if printed.is_empty() {
            String::new()
        } else {
            format!("{printed}\n")
        };
        return Err(CliError::Usage(format!("{lead}{USAGE}")));
    };
    Ok(match binary_of(context, &version) {
        Ok(output) => {
            transcript.absorb(output);
            transcript.finish(NvmExitCode::Success)
        }
        Err(error) => {
            transcript.err(error.to_string());
            transcript.finish(error.exit_code())
        }
    })
}

fn binary_of(context: &Context<'_>, name: &str) -> Result<Output, CliError> {
    let lookup = if name == "current" {
        current::detect(context)?.to_string()
    } else {
        name.to_owned()
    };
    match resolve_installed(context, &lookup)? {
        Resolved::Installed(version) => installed_binary(context, name, &version),
        Resolved::System => system_binary(context),
        Resolved::Missing { .. } if name == "system" => Err(CliError::SystemNodeNotFound),
        Resolved::Missing { resolved } => Err(not_installed(name, &resolved)),
    }
}

fn installed_binary(
    context: &Context<'_>,
    name: &str,
    version: &Version,
) -> Result<Output, CliError> {
    let binary = node_binary(context, version)?;
    if context.fs.is_file(&binary) {
        Ok(Output::stdout(binary.to_string_lossy()))
    } else {
        Err(not_installed(name, name))
    }
}

fn system_binary(context: &Context<'_>) -> Result<Output, CliError> {
    system_node(context)?
        .map(|node| Output::stdout(node.to_string_lossy()))
        .ok_or(CliError::SystemNodeNotFound)
}

fn not_installed(name: &str, resolved: &str) -> CliError {
    // `current` keeps its typed name, whatever it was detected as.
    let resolved = if name == "current" { name } else { resolved };
    CliError::VersionNotInstalled(not_installed_message(name, resolved))
}

fn node_binary(context: &Context<'_>, version: &Version) -> Result<PathBuf, CliError> {
    Ok(version_path(context, version)?.join("bin").join("node"))
}

fn not_installed_message(name: &str, resolved: &str) -> String {
    let shown = if resolved == name {
        with_v_prefix(name)
    } else {
        format!("{name} -> {}", with_v_prefix(resolved))
    };
    format!(
        "N/A: version \"{shown}\" is not yet installed.\n\n\
         You need to run `nvm install {name}` to install and use it."
    )
}

#[cfg(test)]
mod tests;
