//! Unpacking an archive and putting the result where nvm keeps a version:
//! `nvm_install_binary_extract` and `nvm_validate_install`.

use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::domain::version::Version;
use crate::error::CliError;

/// `$NVM_DIR/versions/node/v20.10.0`, or `.../io.js/v3.3.1`.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn version_path(context: &Context<'_>, version: &Version) -> Result<PathBuf, CliError> {
    Ok(context
        .nvm_dir()?
        .join("versions")
        .join(version.flavor.versions_directory())
        .join(version.directory_name()))
}

/// The version's directory holds a `bin/node` that is not empty and can run.
#[must_use]
pub fn is_valid_install(context: &Context<'_>, version_path: &Path) -> bool {
    context
        .fs
        .file_info(&version_path.join("bin/node"))
        .is_ok_and(|node| !node.is_dir && node.len > 0 && node.executable)
}

/// Unpacks `tarball` into `files`, then replaces `version_path` with the one
/// directory the archive holds, in a single rename. A half-installed
/// directory is never visible, and a broken one that was there is gone.
///
/// # Errors
/// What went wrong, as a message for stderr.
pub fn place(
    context: &Context<'_>,
    tarball: &Path,
    files: &Path,
    version_path: &Path,
) -> Result<(), String> {
    let fs = context.fs;
    fs.remove_dir_all(files)
        .map_err(|error| error.to_string())?;
    fs.create_dir_all(files)
        .map_err(|error| error.to_string())?;
    context
        .archive()
        .extract(tarball, files)
        .map_err(|error| error.to_string())?;
    let top = single_directory(context, files)?;
    fs.remove_dir_all(version_path)
        .map_err(|error| error.to_string())?;
    if let Some(parent) = version_path.parent() {
        fs.create_dir_all(parent)
            .map_err(|error| error.to_string())?;
    }
    fs.rename(&top, version_path)
        .map_err(|error| error.to_string())?;
    let _ = fs.remove_dir_all(files);
    Ok(())
}

/// The only entry of `directory`, which must be a directory: the archive's
/// top-level `node-vX.Y.Z-<os>-<arch>` folder.
fn single_directory(context: &Context<'_>, directory: &Path) -> Result<PathBuf, String> {
    let entries = context
        .fs
        .read_dir(directory)
        .map_err(|error| error.to_string())?;
    match entries.as_slice() {
        [only] if only.is_dir => Ok(directory.join(&only.name)),
        _ => Err("the archive does not hold exactly one top-level directory".to_owned()),
    }
}

#[cfg(test)]
mod tests;
