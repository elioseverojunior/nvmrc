//! The aliases `ls-remote` shows next to a release, as `nvm_get_remote_aliases`
//! collects them.

use crate::context::Context;
use crate::domain::alias::resolve;
use crate::error::CliError;

/// `(target, name)` for every alias file directly in `$NVM_DIR/alias`, by name.
/// The built-in names, aliases that loop and values with control characters
/// are left out.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn collect(context: &Context<'_>) -> Result<Vec<(String, String)>, CliError> {
    let directory = context.alias_dir()?;
    let store = context.alias_store()?;
    let mut names: Vec<String> = context
        .fs
        .read_dir(&directory)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.name)
        .filter(|name| !matches!(name.as_str(), "node" | "iojs" | "stable" | "unstable"))
        .filter(|name| !name.chars().any(char::is_control))
        .collect();
    names.sort();
    Ok(names
        .into_iter()
        .filter_map(|name| {
            let target = resolve(&store, &name).ok()?;
            let usable = !target.is_empty() && !target.chars().any(char::is_control);
            usable.then_some((target, name))
        })
        .collect())
}
