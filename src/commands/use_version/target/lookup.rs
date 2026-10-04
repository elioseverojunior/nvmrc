//! The nvm.sh helpers `nvm use` resolves with: `nvm_match_version`,
//! `nvm_version`, `nvm_resolve_alias`, `nvm_has_system_node` /
//! `nvm_has_system_iojs`, `nvm_is_version_installed` and
//! `nvm_ensure_version_installed`.

use super::{SystemFlavor, SystemNode};
use crate::commands::current;
use crate::commands::install::place::version_path;
use crate::commands::resolve::{Resolved, path_without_nvm, resolve_installed};
use crate::commands::transcript::Transcript;
use crate::commands::use_version::messages::{NO_SYSTEM_VERSION, not_yet_installed};
use crate::context::Context;
use crate::domain::alias;
use crate::domain::current::Current;
use crate::domain::path_search::find_in_path;
use crate::domain::version::Version;
use crate::domain::version_prefix::with_v_prefix;
use crate::error::CliError;

/// What `nvm_version` prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Version(Version),
    /// `system`.
    System,
    /// `∞`: the alias chain loops.
    Loop,
    /// `N/A`.
    NotAvailable,
    /// `none`: `current` with no `node` on `PATH`.
    Inactive,
}

impl Resolution {
    /// The text nvm.sh holds in `VERSION`.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Version(version) => version.to_string(),
            Self::System => "system".to_owned(),
            Self::Loop => "∞".to_owned(),
            Self::NotAvailable => "N/A".to_owned(),
            Self::Inactive => "none".to_owned(),
        }
    }
}

/// `nvm_match_version`: `system` stays `system` whether or not one exists,
/// `io.js` is `iojs`, anything else is [`nvm_version`].
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn match_version(context: &Context<'_>, name: &str) -> Result<Resolution, CliError> {
    match name {
        "system" => Ok(Resolution::System),
        "io.js" => nvm_version(context, "iojs"),
        other => nvm_version(context, other),
    }
}

/// `nvm_version`: `current` (or nothing) is the active version, an alias
/// chain ending at `system` needs a system `node`, anything else must match
/// an installed version.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn nvm_version(context: &Context<'_>, name: &str) -> Result<Resolution, CliError> {
    if name.is_empty() || name == "current" {
        return active(context);
    }
    let store = context.alias_store()?;
    match alias::lookup_target(&store, name) {
        Err(_) => Ok(Resolution::Loop),
        Ok(end) if end == "system" => Ok(match system_node(context)? {
            Some(_) => Resolution::System,
            None => Resolution::NotAvailable,
        }),
        Ok(_) => Ok(match resolve_installed(context, name)? {
            Resolved::Installed(version) => Resolution::Version(version),
            Resolved::System | Resolved::Missing { .. } => Resolution::NotAvailable,
        }),
    }
}

fn active(context: &Context<'_>) -> Result<Resolution, CliError> {
    Ok(match current::detect(context)? {
        Current::None => Resolution::Inactive,
        Current::System => Resolution::System,
        Current::Version(version) => Resolution::Version(version),
    })
}

/// The `node`, else the `iojs`, that `PATH` finds once `nvm deactivate` has
/// stripped the nvm entries from it.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn system_node(context: &Context<'_>) -> Result<Option<SystemNode>, CliError> {
    let stripped = path_without_nvm(context)?;
    let find = |name: &str| find_in_path(context.fs, &stripped, name);
    let node = find("node").map(|binary| SystemNode {
        flavor: SystemFlavor::Node,
        binary,
    });
    Ok(node.or_else(|| {
        find("iojs").map(|binary| SystemNode {
            flavor: SystemFlavor::IoJs,
            binary,
        })
    }))
}

/// `nvm_is_version_installed`: the version's `bin/node` can run.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn is_installed(context: &Context<'_>, version: &Version) -> Result<bool, CliError> {
    let node = version_path(context, version)?.join("bin").join("node");
    Ok(context
        .fs
        .file_info(&node)
        .is_ok_and(|info| !info.is_dir && info.executable))
}

/// `nvm_resolve_alias`: where an alias leads (`∞` for a loop), or `None`
/// when `name` is not an alias.
fn alias_end(context: &Context<'_>, name: &str) -> Result<Option<String>, CliError> {
    let store = context.alias_store()?;
    Ok(match alias::resolve(&store, name) {
        Err(_) => Some("∞".to_owned()),
        Ok(end) if end != name => Some(with_v_prefix(&end)),
        Ok(_) => None,
    })
}

/// `nvm_ensure_version_installed`: true when `provided` resolves to a
/// version whose `node` can run (or, for `system`, when there is a system
/// `node`); otherwise its messages go to stderr and the answer is false.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn ensure_installed(
    context: &Context<'_>,
    provided: &str,
    from_nvmrc: bool,
    transcript: &mut Transcript,
) -> Result<bool, CliError> {
    if provided == "system" {
        let found = system_node(context)?.is_some();
        if !found {
            transcript.err(NO_SYSTEM_VERSION);
        }
        return Ok(found);
    }
    if let Resolution::Version(version) = nvm_version(context, provided)? {
        if is_installed(context, &version)? {
            return Ok(true);
        }
    }
    let end = alias_end(context, provided)?;
    for line in not_yet_installed(provided, end.as_deref(), from_nvmrc) {
        transcript.err(line);
    }
    Ok(false)
}
