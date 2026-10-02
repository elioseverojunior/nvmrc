//! `nvm alias` and `nvm alias <prefix>`: list aliases.
//!
//! Three groups, each sorted: the alias files, the implicit aliases that have
//! no file (`iojs`, `node`, `stable`, `unstable`), and the `lts/*` files.
//! Output is always plain, as `nvm.sh` prints when stdout is not a terminal.

use std::path::Path;

use crate::commands::Output;
use crate::commands::resolve::shown;
use crate::context::Context;
use crate::domain::alias::AliasStore;
use crate::domain::alias_format::format_line;
use crate::domain::implicit::{IMPLICIT_ALIASES, destination};
use crate::error::{CliError, NvmExitCode};

/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn list(context: &Context<'_>, prefix: Option<&str>) -> Result<Output, CliError> {
    let prefix = prefix.unwrap_or_default();
    if prefix.starts_with("lts/") {
        return lts_target(context, prefix);
    }
    let alias_dir = context.alias_dir()?;
    let mut lines = directory_lines(context, &alias_dir, "", prefix)?;
    lines.extend(implicit_lines(context, &alias_dir, prefix)?);
    lines.extend(directory_lines(
        context,
        &alias_dir.join("lts"),
        "lts/",
        prefix,
    )?);
    Ok(Output::stdout(lines.join("\n")))
}

/// `nvm alias lts/iron` prints the alias file's target as is.
fn lts_target(context: &Context<'_>, name: &str) -> Result<Output, CliError> {
    match context.alias_store()?.target(name) {
        Some(target) => Ok(Output::stdout(target)),
        None => Ok(Output::default()
            .with_stderr("Alias does not exist.")
            .with_status(NvmExitCode::NoSuchAlias)),
    }
}

fn line(
    context: &Context<'_>,
    name: &str,
    target: &str,
    default: bool,
) -> Result<String, CliError> {
    let version = shown(context, target)?;
    let text = version.to_string();
    Ok(format_line(
        name,
        target,
        &text,
        version.is_available(),
        default,
    ))
}

/// The alias files directly inside `directory` whose name starts with
/// `prefix`, named `label` + the file name. Hidden files only match a hidden
/// prefix, like a shell glob.
fn directory_lines(
    context: &Context<'_>,
    directory: &Path,
    label: &str,
    prefix: &str,
) -> Result<Vec<String>, CliError> {
    let store = context.alias_store()?;
    let mut lines = Vec::new();
    for entry in context.fs.read_dir(directory).unwrap_or_default() {
        let hidden = entry.name.starts_with('.') && !prefix.starts_with('.');
        if entry.is_dir || hidden || !entry.name.starts_with(prefix) {
            continue;
        }
        let name = format!("{label}{}", entry.name);
        if let Some(target) = store.target(&name) {
            lines.push(line(context, &name, &target, false)?);
        }
    }
    lines.sort();
    Ok(lines)
}

/// The implicit aliases that have no file of their own, and that match the
/// prefix exactly (a prefix does not select among them).
fn implicit_lines(
    context: &Context<'_>,
    alias_dir: &Path,
    prefix: &str,
) -> Result<Vec<String>, CliError> {
    let installed = context.installed_versions()?;
    let mut lines = Vec::new();
    for name in IMPLICIT_ALIASES {
        let wanted = prefix.is_empty() || prefix == name;
        if !wanted || context.fs.is_file(&alias_dir.join(name)) {
            continue;
        }
        if let Some(target) = destination(&installed, name) {
            lines.push(line(context, name, &target, true)?);
        }
    }
    lines.sort();
    Ok(lines)
}

#[cfg(test)]
mod tests;
