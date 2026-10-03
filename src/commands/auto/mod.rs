//! `nvm __auto <use|install|none>`: what `nvm.sh` does at the end of being
//! sourced (`nvm_auto`, digest 10), for the init snippet to run when the
//! shell starts.
//!
//! `use`: with no node or the system node first on `PATH`, the version the
//! `default` alias resolves to (a `default` that resolves to nothing usable
//! ends here, without reading `.nvmrc`), else the `.nvmrc` version; with a
//! version of `$NVM_DIR` first on `PATH`, that version again. `install`: the
//! raw target of `default` when it is a version, else the `.nvmrc` version.
//! Both run silently: the stdout of `use` and `install` is dropped, their
//! stderr, shell code and status are kept.

use crate::commands::current::detect;
use crate::commands::rc_version::{RcVersion, rc_version};
use crate::commands::resolve::shown;
use crate::commands::transcript::Transcript;
use crate::commands::{Output, install, use_version};
use crate::context::Context;
use crate::domain::alias::AliasStore;
use crate::domain::current::Current;
use crate::domain::valid_version::is_valid_version;
use crate::error::CliError;

const DEFAULT: &str = "default";
const INVALID_MODE: &str = "Invalid auto mode supplied.";

/// `nvm_auto <mode>`.
///
/// # Errors
/// [`CliError::InvalidArgument`] (status 1) for a mode other than `use`,
/// `install` and `none`; [`CliError::NvmDirUnresolved`] when `$NVM_DIR`
/// cannot be found.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    match args.first().map(String::as_str) {
        Some("use") => auto_use(context),
        Some("install") => auto_install(context),
        Some("none") => Ok(Output::default()),
        _ => Err(CliError::InvalidArgument(INVALID_MODE.to_owned())),
    }
}

fn auto_use(context: &Context<'_>) -> Result<Output, CliError> {
    match detect(context)? {
        Current::None | Current::System => use_default_or_nvmrc(context),
        Current::Version(version) => Ok(use_silently(context, Some(version.to_string()))),
    }
}

/// `nvm_resolve_local_alias default`, then the `.nvmrc`.
fn use_default_or_nvmrc(context: &Context<'_>) -> Result<Output, CliError> {
    if context.alias_store()?.target(DEFAULT).is_some() {
        let version = shown(context, DEFAULT)?.to_string();
        if version != "N/A" && is_valid_version(&version) {
            return Ok(use_silently(context, Some(version)));
        }
        return Ok(Output::default());
    }
    if has_rc_version(context) {
        return Ok(use_silently(context, None));
    }
    Ok(Output::default())
}

fn auto_install(context: &Context<'_>) -> Result<Output, CliError> {
    let target = context.alias_store()?.target(DEFAULT).unwrap_or_default();
    if target != "N/A" && is_valid_version(&target) {
        return Ok(without_stdout(install::run(context, &[target])));
    }
    if has_rc_version(context) {
        return Ok(without_stdout(install::run(context, &[])));
    }
    Ok(Output::default())
}

/// `nvm_rc_version 3>/dev/null >/dev/null 2>&1` succeeds.
fn has_rc_version(context: &Context<'_>) -> bool {
    let mut discarded = Transcript::default();
    matches!(
        rc_version(context, true, &mut discarded),
        RcVersion::Found { .. }
    )
}

/// `nvm use --silent [version] >/dev/null`.
fn use_silently(context: &Context<'_>, version: Option<String>) -> Output {
    let args: Vec<String> = std::iter::once("--silent".to_owned())
        .chain(version)
        .collect();
    without_stdout(use_version::run(context, &args))
}

/// The result of a command with its stdout sent to `/dev/null`; a failure
/// keeps its message on stderr (`N/A` is stdout, so it goes) and its status.
fn without_stdout(result: Result<Output, CliError>) -> Output {
    match result {
        Ok(output) => Output {
            stdout: String::new(),
            ..output
        },
        Err(CliError::NotInstalled) => {
            Output::default().with_status(CliError::NotInstalled.exit_code())
        }
        Err(error) => Output::default()
            .with_stderr(error.to_string())
            .with_status(error.exit_code()),
    }
}

#[cfg(test)]
mod tests;
