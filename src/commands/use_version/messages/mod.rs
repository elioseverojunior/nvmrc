//! The failure messages of `nvm use`, verbatim from nvm.sh.

use crate::domain::version_prefix::with_v_prefix;

/// `nvm use system` when no `node` or `iojs` is left once the nvm entries
/// are stripped from `PATH`.
pub const SYSTEM_NOT_FOUND: &str = "System version of node not found.";

/// `nvm_ensure_version_installed system` without a system `node` or `iojs`.
pub const NO_SYSTEM_VERSION: &str = "N/A: no system version of node/io.js is installed.";

/// `The alias "<provided>" leads to an infinite loop. Aborting.`
#[must_use]
pub fn infinite_loop(provided: &str) -> String {
    format!("The alias \"{provided}\" leads to an infinite loop. Aborting.")
}

/// The three stderr lines of `nvm_ensure_version_installed` for a version
/// that is not installed. `alias_end` is what `nvm_resolve_alias` prints
/// when `provided` is an alias (`v99`, or `∞` for a loop); otherwise the
/// version is shown with its `v` prefix (`99` reads `v99`).
#[must_use]
pub fn not_yet_installed(provided: &str, alias_end: Option<&str>, from_nvmrc: bool) -> [String; 3] {
    let shown = match alias_end {
        Some(end) => format!("{provided} -> {end}"),
        None => with_v_prefix(provided),
    };
    let hint = if provided == "lts" {
        LTS_IS_NOT_AN_ALIAS.to_owned()
    } else if from_nvmrc {
        INSTALL_FROM_NVMRC.to_owned()
    } else {
        format!("You need to run `nvm install {provided}` to install and use it.")
    };
    [
        format!("N/A: version \"{shown}\" is not yet installed."),
        String::new(),
        hint,
    ]
}

const LTS_IS_NOT_AN_ALIAS: &str = "`lts` is not an alias - you may need to run \
`nvm install --lts` to install and `nvm use --lts` to use it.";

const INSTALL_FROM_NVMRC: &str = "You need to run `nvm install` to install and use \
the node version specified in `.nvmrc`.";

#[cfg(test)]
mod tests;
