//! `nvm uninstall <version>`: remove an installed version, its leftovers in the
//! cache, and the aliases that name it.

use crate::commands::current;
use crate::commands::install::fetch::Artifact;
use crate::commands::install::place::version_path;
use crate::commands::resolve::{Resolved, resolve_installed};
use crate::commands::transcript::Transcript;
use crate::commands::{Output, unalias};
use crate::context::Context;
use crate::domain::version::{Flavor, Version};
use crate::error::{CliError, NvmExitCode};

const USAGE: &str = "Usage: nvm uninstall <version>\n       nvm uninstall --lts\n       nvm uninstall --lts=<LTS name>\n  Run `nvm --help` for full help.";

/// The alias to resolve for what was asked: `--lts`, `--lts=<name>` and
/// `lts/*` mean the LTS aliases.
fn name_to_resolve(pattern: &str) -> String {
    match pattern {
        "--lts" => "lts/*".to_owned(),
        other => match other.strip_prefix("--lts=") {
            Some(name) => format!("lts/{name}"),
            None => other.to_owned(),
        },
    }
}

/// # Errors
/// - [`CliError::Usage`] unless exactly one word is given.
/// - [`CliError::InvalidArgument`] for the version in use.
/// - [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
///
/// A version that is not installed is only a message on stderr, with status 0,
/// as in `nvm.sh`.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let [pattern] = args else {
        return Err(CliError::Usage(USAGE.to_owned()));
    };
    let resolved = resolve_installed(context, &name_to_resolve(pattern))?;
    let mut transcript = Transcript::default();
    let Resolved::Installed(version) = resolved else {
        return Ok(not_installed(None, pattern, transcript));
    };
    refuse_the_active_version(context, &version, pattern)?;
    let path = version_path(context, &version)?;
    if !is_installed(context, &path) {
        return Ok(not_installed(Some(&version), pattern, transcript));
    }
    remove(context, &version, &path, &mut transcript)?;
    Ok(transcript.finish(NvmExitCode::Success))
}

fn not_installed(version: Option<&Version>, pattern: &str, mut transcript: Transcript) -> Output {
    match version
        .map(ToString::to_string)
        .filter(|text| text != pattern)
    {
        Some(text) => transcript.err(format!(
            "Version '{text}' (inferred from {pattern}) is not installed."
        )),
        None => transcript.err(format!("Version '{pattern}' is not installed.")),
    }
    transcript.finish(NvmExitCode::Success)
}

fn is_installed(context: &Context<'_>, version_path: &std::path::Path) -> bool {
    context
        .fs
        .file_info(&version_path.join("bin/node"))
        .is_ok_and(|node| !node.is_dir && node.executable)
}

fn refuse_the_active_version(
    context: &Context<'_>,
    version: &Version,
    pattern: &str,
) -> Result<(), CliError> {
    if current::detect(context)?.to_string() != version.to_string() {
        return Ok(());
    }
    let kind = match version.flavor {
        Flavor::Node => "node",
        Flavor::IoJs => "io.js",
    };
    Err(CliError::InvalidArgument(format!(
        "nvm: Cannot uninstall currently-active {kind} version, {version} (inferred from {pattern})."
    )))
}

fn remove(
    context: &Context<'_>,
    version: &Version,
    path: &std::path::Path,
    transcript: &mut Transcript,
) -> Result<(), CliError> {
    let cache = context.cache_dir()?;
    let source_slug = match version.flavor {
        Flavor::Node => format!("node-{}", version.directory_name()),
        Flavor::IoJs => format!("iojs-{}", version.directory_name()),
    };
    if let Some(artifact) = Artifact::of(context, version) {
        let _ = context.fs.remove_dir_all(&artifact.files());
    }
    let _ = context
        .fs
        .remove_dir_all(&cache.join("src").join(source_slug).join("files"));
    context
        .fs
        .remove_dir_all(path)
        .map_err(|source| CliError::io(path, source))?;
    let name = match version.flavor {
        Flavor::Node => "node",
        Flavor::IoJs => "io.js",
    };
    transcript.out(format!("Uninstalled {name} {}", version.directory_name()));
    remove_aliases_to(context, version, transcript)
}

/// Every alias file that mentions the version is deleted.
fn remove_aliases_to(
    context: &Context<'_>,
    version: &Version,
    transcript: &mut Transcript,
) -> Result<(), CliError> {
    let directory = context.alias_dir()?;
    let mut names: Vec<String> = context
        .fs
        .read_dir(&directory)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.name)
        .collect();
    names.sort();
    let text = version.to_string();
    for name in names {
        let mentions = context
            .fs
            .read_to_string(&directory.join(&name))
            .is_ok_and(|contents| contents.contains(&text));
        if mentions {
            transcript.absorb(unalias::run(context, &[name])?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
