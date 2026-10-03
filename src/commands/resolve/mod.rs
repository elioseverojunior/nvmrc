//! Shared by the commands that take a version or alias: resolve the name
//! through the alias files, then match it against the installed versions.

use std::path::PathBuf;

use std::fmt;

use crate::context::Context;
use crate::domain::alias;
use crate::domain::implicit::{derive, highest_in};
use crate::domain::path_search::find_in_dirs;
use crate::domain::version::{Flavor, Version, VersionPattern};
use crate::error::CliError;

#[derive(Debug, PartialEq, Eq)]
pub enum Resolved {
    Installed(Version),
    /// The alias chain ended at `system` and a `node` exists on `PATH`
    /// outside `$NVM_DIR`.
    System,
    /// No installed version matches. `resolved` is where the alias chain
    /// ended, which is the name itself when it is not an alias.
    Missing {
        resolved: String,
    },
}

/// Resolves `name` as `nvm.sh`'s `nvm_version` does (see
/// [`alias::lookup_target`]).
///
/// # Errors
/// - [`CliError::Alias`] when the alias chain loops.
/// - [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn resolve_installed(context: &Context<'_>, name: &str) -> Result<Resolved, CliError> {
    let store = context.alias_store()?;
    let target = alias::lookup_target(&store, name)?;
    if target == "system" && system_node(context)?.is_some() {
        return Ok(Resolved::System);
    }
    let installed = context.installed_versions()?;
    if let Some(version) = find_installed(&target, &installed) {
        return Ok(Resolved::Installed(version));
    }
    // Messages name the plain chain end, as `nvm_resolve_alias` prints it.
    let resolved = alias::resolve(&store, name).unwrap_or_else(|_| "∞".to_owned());
    Ok(Resolved::Missing { resolved })
}

/// The first `node` on `PATH` that does not live under `$NVM_DIR`.
///
/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn system_node(context: &Context<'_>) -> Result<Option<PathBuf>, CliError> {
    let nvm_dir = context.nvm_dir()?;
    let path_variable = context.env.var_os("PATH").unwrap_or_default();
    let outside_nvm = std::env::split_paths(&path_variable)
        .filter(|directory| !directory.starts_with(&nvm_dir))
        .collect::<Vec<_>>();
    Ok(find_in_dirs(context.fs, outside_nvm, "node"))
}

/// The version the system `node` reports, or `None` when there is no system
/// `node` or it does not answer `--version`.
///
/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn system_version(context: &Context<'_>) -> Result<Option<String>, CliError> {
    let Some(node) = system_node(context)? else {
        return Ok(None);
    };
    let version = context
        .process()
        .run(&node, &["--version"])
        .ok()
        .filter(|output| output.success)
        .map(|output| output.stdout.trim().to_owned())
        .filter(|version| !version.is_empty());
    Ok(version)
}

/// `node`, `stable`, `unstable` and `iojs` are implicit aliases for the latest
/// installed release line of their kind; anything else is matched as a version
/// pattern.
fn find_installed(resolved: &str, installed: &[Version]) -> Option<Version> {
    let implicit = derive(installed);
    match resolved {
        "node" | "stable" => highest_in(installed, Flavor::Node, implicit.stable?),
        "unstable" => highest_in(installed, Flavor::Node, implicit.unstable?),
        "iojs" => highest_in(installed, Flavor::IoJs, implicit.iojs?),
        other => other
            .parse::<VersionPattern>()
            .ok()
            .and_then(|pattern| pattern.highest_match(installed).copied()),
    }
}

/// What a name resolves to, as `nvm alias` and `nvm ls` print it.
#[derive(Debug, PartialEq, Eq)]
pub enum Shown {
    Version(Version),
    System,
    /// Nothing installed matches: `N/A`.
    NotAvailable,
    /// The alias chain loops: `∞`.
    Infinite,
}

impl fmt::Display for Shown {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Version(version) => version.fmt(formatter),
            Self::System => formatter.write_str("system"),
            Self::NotAvailable => formatter.write_str("N/A"),
            Self::Infinite => formatter.write_str("∞"),
        }
    }
}

impl Shown {
    /// Resolved names get a trailing ` *` in `nvm alias` and `nvm ls` output.
    #[must_use]
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Version(_) | Self::System)
    }
}

/// Like [`resolve_installed`], but a loop is a value, not an error.
///
/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn shown(context: &Context<'_>, name: &str) -> Result<Shown, CliError> {
    match resolve_installed(context, name) {
        Ok(Resolved::Installed(version)) => Ok(Shown::Version(version)),
        Ok(Resolved::System) => Ok(Shown::System),
        Ok(Resolved::Missing { .. }) => Ok(Shown::NotAvailable),
        Err(CliError::Alias(_)) => Ok(Shown::Infinite),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
