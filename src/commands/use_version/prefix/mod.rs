//! Step 12 of `nvm use`, `nvm_die_on_prefix` (digest 4.4): refuse an npm
//! `prefix` that would hide the version, from `PREFIX`, `NPM_CONFIG_PREFIX`
//! in any letter case, or one of the four npmrc files, or delete the npmrc
//! settings with `--delete-prefix`. Its messages print even with `--silent`.

mod npmrc;

#[cfg(test)]
mod tests;

use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::npm::prefix::tree_contains_path;
use crate::domain::version::Version;

/// What the checks need from the switch `use` is making.
#[derive(Debug, Clone, Copy)]
pub struct Check<'a> {
    /// `$NVM_DIR`, normalised.
    pub nvm_dir: &'a str,
    /// `nvm_version_path` of the version switched to.
    pub directory: &'a str,
    /// The new `PATH`, whose `npm` `--delete-prefix` runs.
    pub path: &'a str,
    /// The `nvm use --delete-prefix ...` command the npmrc message suggests.
    pub command: &'a str,
    /// `--delete-prefix`: delete npmrc settings instead of refusing.
    pub delete: bool,
}

/// Why `use` stops (status 11), which decides the environment it leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// `PREFIX` or `NPM_CONFIG_PREFIX`: nvm.sh runs `nvm deactivate`.
    Deactivate,
    /// An npmrc file: the switch stays applied.
    KeepSwitch,
}

/// The command the npmrc message suggests: `nvm use --delete-prefix`, then
/// the resolved `version` when one was named (`PROVIDED_VERSION`, the
/// `.nvmrc` text included), then `--silent`.
#[must_use]
pub fn command(version: &Version, named: bool, silent: bool) -> String {
    let mut command = "nvm use --delete-prefix".to_owned();
    if named {
        command.push_str(&format!(" {version}"));
    }
    if silent {
        command.push_str(" --silent");
    }
    command
}

/// Runs the checks in nvm.sh's order, the first failure reported on stderr.
///
/// # Errors
/// The [`Refusal`] of the first check that fails.
pub fn check(
    context: &Context<'_>,
    check: &Check<'_>,
    transcript: &mut Transcript,
) -> Result<(), Refusal> {
    let refused = prefix_variable(context, check.directory)
        .or_else(|| npm_config_prefix(context, check.nvm_dir));
    if let Some((name, value)) = refused {
        transcript.err(format!(
            "nvm is not compatible with the \"{name}\" environment variable: \
currently set to \"{value}\""
        ));
        transcript.err(format!("Run `unset {name}` to unset it."));
        return Err(Refusal::Deactivate);
    }
    npmrc::check(context, check, transcript)
}

/// `PREFIX`, set and not the version's directory (what `npm exec` sets it
/// to), as a name and value to refuse.
fn prefix_variable(context: &Context<'_>, directory: &str) -> Option<(String, String)> {
    let value = context
        .env
        .var("PREFIX")
        .filter(|value| !value.is_empty())?;
    (value != directory).then(|| ("PREFIX".to_owned(), value))
}

/// The first variable named `NPM_CONFIG_PREFIX` in any letter case, when
/// its value is set and outside `$NVM_DIR`.
fn npm_config_prefix(context: &Context<'_>, nvm_dir: &str) -> Option<(String, String)> {
    let (name, value) = context
        .env
        .vars()
        .into_iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("NPM_CONFIG_PREFIX"))?;
    (!value.is_empty() && !tree_contains_path(nvm_dir, &value)).then_some((name, value))
}
