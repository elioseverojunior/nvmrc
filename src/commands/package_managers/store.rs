//! The shared store of package managers:
//! `$NVM_DIR/package-managers/<tool>/<version>/{package,bin}`, used by every
//! node version.

use std::path::{Path, PathBuf};

use super::registry::fetch_manifest;
use crate::commands::install::place::unpack;
use crate::context::Context;
use crate::domain::integrity::verify;
use crate::domain::registry::Manifest;
use crate::domain::tool::Tool;
use crate::domain::version::Version;
use crate::error::CliError;

/// `$NVM_DIR/package-managers/<tool>/<version>`.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn tool_dir(context: &Context<'_>, tool: Tool, version: &Version) -> Result<PathBuf, CliError> {
    Ok(context
        .nvm_dir()?
        .join("package-managers")
        .join(tool.name())
        .join(tool.show(version)))
}

/// The directory of the commands of an installed tool.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn bin_dir(context: &Context<'_>, tool: Tool, version: &Version) -> Result<PathBuf, CliError> {
    Ok(tool_dir(context, tool, version)?.join("bin"))
}

/// The package and the tool's own command are both there.
#[must_use]
pub fn is_installed(context: &Context<'_>, tool: Tool, version: &Version) -> bool {
    let Ok(directory) = tool_dir(context, tool, version) else {
        return false;
    };
    context.fs.is_file(&directory.join("package/package.json"))
        && context.fs.is_file(&directory.join("bin").join(tool.name()))
}

/// Downloads `tool` at `version` and replaces whatever sits in its store
/// directory. The bytes are checked against the registry's integrity before
/// anything is written; the new tree is built beside the old one and swapped
/// in by a rename. Any failure removes the work area and leaves the store as
/// it was.
///
/// # Errors
/// What went wrong, as a message for stderr.
pub fn install_tool(context: &Context<'_>, tool: Tool, version: &Version) -> Result<(), String> {
    let manifest = fetch_manifest(context, tool, version).map_err(|error| error.to_string())?;
    let directory = tool_dir(context, tool, version).map_err(|error| error.to_string())?;
    let mut partial = directory.clone().into_os_string();
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    let result = build_and_swap(context, &manifest, &directory, &partial);
    let _ = context.fs.remove_dir_all(&partial);
    result
}

fn build_and_swap(
    context: &Context<'_>,
    manifest: &Manifest,
    directory: &Path,
    partial: &Path,
) -> Result<(), String> {
    let fs = context.fs;
    let failed = |error: std::io::Error| error.to_string();
    let location = &manifest.tarball;
    let bytes = context
        .http()
        .get_secure_bytes(location)
        .map_err(|error| format!("{location}: {error}"))?;
    verify(&manifest.integrity, &bytes).map_err(|error| format!("{location}: {error}"))?;
    fs.create_dir_all(partial).map_err(failed)?;
    let tarball = partial.join("download.tgz");
    fs.write_bytes(&tarball, &bytes).map_err(failed)?;
    let top = unpack(context, &tarball, &partial.join("files"))?;
    let out = partial.join("out");
    fs.create_dir_all(&out).map_err(failed)?;
    fs.rename(&top, &out.join("package")).map_err(failed)?;
    link_commands(context, manifest, &out)?;
    fs.remove_dir_all(directory).map_err(failed)?;
    if let Some(parent) = directory.parent() {
        fs.create_dir_all(parent).map_err(failed)?;
    }
    fs.rename(&out, directory).map_err(failed)
}

/// One symlink `bin/<command>` -> `../package/<path>` per command, and the
/// targets made executable.
fn link_commands(context: &Context<'_>, manifest: &Manifest, out: &Path) -> Result<(), String> {
    let fs = context.fs;
    let bin = out.join("bin");
    fs.create_dir_all(&bin).map_err(|error| error.to_string())?;
    for (command, path) in &manifest.commands {
        let relative = Path::new("../package").join(path);
        fs.symlink(&relative, &bin.join(command))
            .map_err(|error| format!("{command}: {error}"))?;
        fs.make_executable(&out.join("package").join(path))
            .map_err(|error| format!("{command}: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
