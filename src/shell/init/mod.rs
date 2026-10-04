//! `nvm init <shell>`: the shell code that defines the `nvm` function
//! around the binary, between two marker lines that tools can find again:
//! one POSIX function for bash, zsh, sh, dash and ksh (`posix.rs`), and one
//! fish function for fish 3.4 or newer (`fish.rs`).
//!
//! The function runs `use`, `deactivate`, `install` (and `i`),
//! `set-colors` and `__auto` with `NVMRC_SCRIPT_FD=3`: the binary writes
//! the shell code for the calling shell to descriptor 3, which the function
//! captures and evaluates, while stdout and stderr pass straight through (so `nvm use 18
//! >/dev/null` silences only the messages); the function returns the
//! binary's status. Every other command is the binary, untouched, without
//! descriptor 3. Both export the variables the binary reads that a shell
//! may hold unexported (`MANPATH`, `NODE_PATH`, `NVM_COLORS`,
//! `NVM_HAS_COLORS`, `NVM_NO_COLORS`, `NVM_SYMLINK_CURRENT`, `PREFIX`) only
//! when they are set, so `nvm exec` links `current` and checks the prefix as
//! `use` does, the colors follow the shell's settings as nvm.sh (a shell
//! function) sees them, and an unset one never reaches a program the binary
//! starts as an empty variable (a `make` run by `nvm install` seeing
//! `PREFIX=`).
//!
//! As sourcing `nvm.sh` does, the snippet ends with the automatic `use` (or
//! `install`), whose status is the status of the whole snippet: `eval
//! "$(nvm init bash)"` (and `nvm init fish | source`) is 3 when the
//! `.nvmrc` names a missing version.

mod fish;
mod posix;

use std::str::FromStr;

use crate::error::CliError;

/// The first and last lines of the snippet (defined with the conflict rules,
/// which skip the block).
pub use crate::domain::conflict::{BEGIN_MARKER, END_MARKER};
/// How `nvm init` is called.
pub const USAGE: &str = "Usage: nvm init <shell> [--no-use] [--install]";

/// The shells whose `nvm` function `nvm init` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Sh,
    Dash,
    Ksh,
    Fish,
}

/// Every shell, in the order `nvm init` lists them.
pub const SHELLS: [Shell; 6] = [
    Shell::Bash,
    Shell::Zsh,
    Shell::Sh,
    Shell::Dash,
    Shell::Ksh,
    Shell::Fish,
];

impl Shell {
    /// The name `nvm init` takes for the shell.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Sh => "sh",
            Self::Dash => "dash",
            Self::Ksh => "ksh",
            Self::Fish => "fish",
        }
    }

    /// The line of the startup file that loads the snippet.
    fn load_line(self) -> String {
        match self {
            Self::Fish => "nvm init fish | source".to_owned(),
            _ => format!("eval \"$(nvm init {})\"", self.name()),
        }
    }
}

impl FromStr for Shell {
    type Err = CliError;

    /// Case-sensitive, as the shells name themselves.
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        SHELLS
            .into_iter()
            .find(|shell| shell.name() == name)
            .ok_or_else(|| {
                let supported: Vec<&str> = SHELLS.iter().map(|shell| shell.name()).collect();
                CliError::Usage(format!(
                    "Unsupported shell: \"{name}\" (supported shells: {}).\n{USAGE}",
                    supported.join(", ")
                ))
            })
    }
}

/// The options of `nvm init`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InitOptions {
    /// `--no-use`: define the function only (wins over `install`).
    pub no_use: bool,
    /// `--install`: install the default version at start instead of using it.
    pub install: bool,
}

impl InitOptions {
    /// The arguments of the `nvm` call run at the end, as
    /// `nvm_process_parameters` picks it; `None` with `--no-use`.
    fn auto_arguments(self) -> Option<&'static str> {
        match (self.no_use, self.install) {
            (true, _) => None,
            (false, true) => Some("__auto install"),
            (false, false) => Some("__auto use"),
        }
    }
}

/// The variables the binary reads that a shell may hold unexported.
const PASSED: [&str; 7] = [
    "MANPATH",
    "NODE_PATH",
    "NVM_COLORS",
    "NVM_HAS_COLORS",
    "NVM_NO_COLORS",
    "NVM_SYMLINK_CURRENT",
    "PREFIX",
];

/// `template` with each `@EXPORTS@` line replaced by `export(name)` for
/// every [`PASSED`] variable, at the same indentation.
fn with_exports(template: &str, export: fn(&str) -> String) -> String {
    template
        .lines()
        .map(|line| match line.strip_suffix("@EXPORTS@") {
            Some(indent) => PASSED
                .iter()
                .map(|name| format!("{indent}{}\n", export(name)))
                .collect(),
            None => format!("{line}\n"),
        })
        .collect()
}

/// The init snippet for `shell`, newline-terminated.
#[must_use]
pub fn snippet(shell: Shell, options: &InitOptions) -> String {
    let code = match shell {
        Shell::Fish => fish::code(options),
        _ => posix::code(shell, options),
    };
    format!(
        "{BEGIN_MARKER}\n\
# The nvm function of nvmrc, from `{load}` in the {name} startup file.\n\
{code}{END_MARKER}\n",
        load = shell.load_line(),
        name = shell.name(),
    )
}

#[cfg(test)]
mod tests;
