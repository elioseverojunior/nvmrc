//! The writing step of `nvm init`: the new `.nvmrc` and `package.json`
//! texts, computed before either file is written.

use std::path::{Path, PathBuf};

use super::project::Stop;
use crate::context::Context;
use crate::domain::nvmrc::{with_pin, with_version};
use crate::domain::package_json::{
    PackageManagerError, check_package_manager, package_manager_field, set_package_manager,
};
use crate::domain::tool::Tool;
use crate::domain::version::Version;
use crate::error::{CliError, PinsError};

/// The two files `nvm init` writes, in the working directory.
pub struct ProjectFiles {
    pub nvmrc: PathBuf,
    pub package_json: PathBuf,
}

impl ProjectFiles {
    #[must_use]
    pub fn in_directory(directory: &Path) -> Self {
        Self {
            nvmrc: directory.join(".nvmrc"),
            package_json: directory.join("package.json"),
        }
    }
}

/// A change to `package.json`: its new text, the `packageManager` value
/// set and the one it replaced, if any.
pub struct PackageEdit {
    text: String,
    field: String,
    previous: Option<String>,
}

/// The `.nvmrc` at `path`; empty when there is none.
///
/// # Errors
/// [`CliError::Io`] when it exists but cannot be read.
pub fn current_nvmrc(context: &Context<'_>, path: &Path) -> Result<String, CliError> {
    Ok(existing(context, path)?.unwrap_or_default())
}

/// `current` with the chosen keys set.
#[must_use]
pub fn new_nvmrc(current: String, chosen: &[(Tool, Version)]) -> String {
    chosen
        .iter()
        .fold(current, |text, (tool, version)| match tool {
            Tool::Node => with_version(&text, &tool.show(version)),
            _ => with_pin(&text, *tool, version),
        })
}

/// The `package.json` at `path` with `packageManager` set, when it exists,
/// yarn or pnpm was chosen and its value (a corepack hash aside) differs.
///
/// # Errors
/// When it cannot be read, or not as a JSON object.
pub fn new_package_json(
    context: &Context<'_>,
    path: &Path,
    chosen: &[(Tool, Version)],
) -> Result<Option<PackageEdit>, CliError> {
    let manager = chosen
        .iter()
        .find(|(tool, _)| matches!(tool, Tool::Yarn | Tool::Pnpm));
    let Some((tool, version)) = manager else {
        return Ok(None);
    };
    let Some(original) = existing(context, path)? else {
        return Ok(None);
    };
    let previous = match check_package_manager(&original, *tool, version) {
        Ok(()) => return Ok(None),
        Err(PackageManagerError::Different { found }) => Some(found),
        Err(_) => None,
    };
    let field = package_manager_field(*tool, version);
    let text = set_package_manager(&original, &field).map_err(|error| {
        CliError::from(PinsError::UnreadablePackageJson {
            package_json: path.display().to_string(),
            reason: error.to_string(),
        })
    })?;
    Ok(Some(PackageEdit {
        text,
        field,
        previous,
    }))
}

impl PackageEdit {
    /// The line that reports the edit of the `package.json` at `path`.
    #[must_use]
    pub fn reported(&self, path: &Path) -> String {
        let set = format!(
            "Set packageManager to \"{}\" in {}",
            self.field,
            path.display()
        );
        match &self.previous {
            Some(previous) => format!("{set} (was \"{previous}\")"),
            None => set,
        }
    }
}

/// Writes `package.json` (when it changes) and then the `.nvmrc`.
///
/// # Errors
/// [`Stop`] with the failed write.
pub fn save(
    context: &Context<'_>,
    files: &ProjectFiles,
    nvmrc_text: &str,
    package_edit: Option<&PackageEdit>,
) -> Result<(), Stop> {
    if let Some(edit) = package_edit {
        write(context, &files.package_json, &edit.text)?;
    }
    write(context, &files.nvmrc, nvmrc_text)?;
    Ok(())
}

/// The text of the file at `path`; `None` when there is none.
fn existing(context: &Context<'_>, path: &Path) -> Result<Option<String>, CliError> {
    if !context.fs.is_file(path) {
        return Ok(None);
    }
    let text = context.fs.read_to_string(path);
    text.map(Some).map_err(|error| CliError::io(path, error))
}

fn write(context: &Context<'_>, path: &Path, text: &str) -> Result<(), CliError> {
    context
        .fs
        .write_file(path, text)
        .map_err(|error| CliError::io(path, error))
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
