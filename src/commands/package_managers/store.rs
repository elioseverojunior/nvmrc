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
use crate::ports::FileSystem;

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

/// `directory` with `suffix` appended to its name.
fn sibling(directory: &Path, suffix: &str) -> PathBuf {
    let mut name = directory.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Downloads `tool` at `version` and replaces whatever sits in its store
/// directory. The bytes are checked against the registry's integrity before
/// anything is written; the new tree is built beside the old one and swapped
/// in by renames. Any failure removes the work area and leaves the store as
/// it was.
///
/// # Errors
/// What went wrong, as a message for stderr.
pub fn install_tool(context: &Context<'_>, tool: Tool, version: &Version) -> Result<(), String> {
    let manifest = fetch_manifest(context, tool, version).map_err(|error| error.to_string())?;
    let directory = tool_dir(context, tool, version).map_err(|error| error.to_string())?;
    let (partial, old) = (sibling(&directory, ".partial"), sibling(&directory, ".old"));
    for stale in [&partial, &old] {
        context
            .fs
            .remove_dir_all(stale)
            .map_err(|error| error.to_string())?;
    }
    let result = build(context, tool, &manifest, &directory, &partial);
    let _ = context.fs.remove_dir_all(&partial);
    result
}

fn build(
    context: &Context<'_>,
    tool: Tool,
    manifest: &Manifest,
    directory: &Path,
    partial: &Path,
) -> Result<(), String> {
    let tarball = download(context, manifest, partial)?;
    let top = unpack(context, &tarball, &partial.join("files"))?;
    let out = partial.join("out");
    stage(context, tool, manifest, &top, &out)?;
    swap(context.fs, &out, directory, &sibling(directory, ".old"))
}

/// The tarball, checked against its integrity and written under `partial`.
fn download(context: &Context<'_>, manifest: &Manifest, partial: &Path) -> Result<PathBuf, String> {
    let location = &manifest.tarball;
    let bytes = context
        .http()
        .get_secure_bytes(location)
        .map_err(|error| format!("{location}: {error}"))?;
    verify(&manifest.integrity, &bytes).map_err(|error| format!("{location}: {error}"))?;
    let tarball = partial.join("download.tgz");
    let written = context.fs.create_dir_all(partial);
    written
        .and_then(|()| context.fs.write_bytes(&tarball, &bytes))
        .map_err(|error| error.to_string())?;
    Ok(tarball)
}

/// Builds `out`: the unpacked package, one symlink `bin/<command>` ->
/// `../package/<path>` per command, the targets made executable. Refuses a
/// package that lacks `package.json` or the tool's own command.
fn stage(
    context: &Context<'_>,
    tool: Tool,
    manifest: &Manifest,
    top: &Path,
    out: &Path,
) -> Result<(), String> {
    let fs = context.fs;
    let failed = |error: std::io::Error| error.to_string();
    fs.create_dir_all(&out.join("bin")).map_err(failed)?;
    fs.rename(top, &out.join("package")).map_err(failed)?;
    link_commands(context, manifest, out)?;
    let complete = fs.is_file(&out.join("package/package.json"))
        && fs.is_file(&out.join("bin").join(tool.name()));
    if complete {
        return Ok(());
    }
    Err(format!(
        "the package has no package.json or no `{}` command",
        tool.name()
    ))
}

/// Replaces `directory` with `out`; the old tree is set aside first and put
/// back if the new one cannot take its place.
fn swap(fs: &dyn FileSystem, out: &Path, directory: &Path, old: &Path) -> Result<(), String> {
    let existed = fs.file_info(directory).is_ok();
    if existed {
        fs.rename(directory, old)
            .map_err(|error| error.to_string())?;
    }
    if let Err(error) = fs.rename(out, directory) {
        if existed {
            let _ = fs.rename(old, directory);
        }
        return Err(error.to_string());
    }
    fs.remove_dir_all(old).map_err(|error| error.to_string())
}

fn link_commands(context: &Context<'_>, manifest: &Manifest, out: &Path) -> Result<(), String> {
    let fs = context.fs;
    for (command, path) in &manifest.commands {
        let target = out.join("package").join(path);
        let problem = |error: String| format!("{command}: {error}");
        confine(fs, &out.join("package"), &target).map_err(problem)?;
        let relative = Path::new("../package").join(path);
        fs.symlink(&relative, &out.join("bin").join(command))
            .map_err(|error| problem(error.to_string()))?;
        fs.make_executable(&target)
            .map_err(|error| problem(error.to_string()))?;
    }
    Ok(())
}

/// `target` is a regular file inside `package` once every link is followed:
/// an archive can hold links that point anywhere.
fn confine(fs: &dyn FileSystem, package: &Path, target: &Path) -> Result<(), String> {
    let real = fs.canonicalize(target).map_err(|error| error.to_string())?;
    let root = fs
        .canonicalize(package)
        .map_err(|error| error.to_string())?;
    let is_file = fs.file_info(&real).is_ok_and(|info| !info.is_dir);
    if real.starts_with(&root) && is_file {
        return Ok(());
    }
    Err("the command is not a file inside the package (it points outside)".to_owned())
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
