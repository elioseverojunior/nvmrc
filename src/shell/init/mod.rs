//! `nvm init <shell>`: the POSIX shell code that defines the `nvm` function
//! around the binary, between two marker lines that tools can find again.
//!
//! The function runs `use`, `deactivate`, `install` (and `i`) and `__auto`
//! with `NVMRC_SCRIPT_FD=3`: the binary writes the shell code for the
//! calling shell to descriptor 3, which the function captures and `eval`s,
//! while stdout and stderr pass straight through (so `nvm use 18 >/dev/null`
//! silences only the messages). The unexported variables the binary reads
//! are passed in. Every other command is the binary, untouched, without
//! descriptor 3. The function keeps its status in `$1` (positional
//! parameters are local to a function in every POSIX shell) so it can unset
//! its temporaries, and a failure inside `$(...)` does not end a `set -e`
//! shell before the code is applied.
//!
//! As sourcing `nvm.sh` does, the snippet ends with the automatic `use` (or
//! `install`), whose status is the status of the whole snippet: `eval
//! "$(nvm init bash)"` is 3 when the `.nvmrc` names a missing version.

use std::str::FromStr;

use crate::error::CliError;

/// The first line of the snippet.
pub const BEGIN_MARKER: &str = "# >>> nvmrc init >>>";
/// The last line of the snippet.
pub const END_MARKER: &str = "# <<< nvmrc init <<<";
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
}

const SHELLS: [Shell; 5] = [Shell::Bash, Shell::Zsh, Shell::Sh, Shell::Dash, Shell::Ksh];

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
        }
    }

    /// How the function starts. zsh reads `function nvm {`, because it
    /// expands an alias named `nvm` in `nvm() {` even after `unalias` in
    /// the same `eval` string; the POSIX form everywhere else.
    fn function_header(self) -> &'static str {
        match self {
            Self::Zsh => "function nvm {",
            Self::Bash | Self::Sh | Self::Dash | Self::Ksh => "nvm() {",
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
    /// The statement run at the end (`\nvm`: a quoted name is never taken
    /// for an alias, and zsh expands the aliases of a whole `eval` string
    /// before running its `unalias`), as `nvm_process_parameters` picks it.
    fn auto_statement(self) -> &'static str {
        match (self.no_use, self.install) {
            (true, _) => "",
            (false, true) => "\\nvm __auto install\n",
            (false, false) => "\\nvm __auto use\n",
        }
    }
}

const PREAMBLE: &str = r#"export NVMRC_SHELL=1
export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
unalias nvm 2>/dev/null || true
"#;

const BODY: &str = r#"
  case "${1-}" in
    use | deactivate | install | i | __auto) ;;
    *)
      command nvm "$@"
      return
      ;;
  esac
  __nvmrc_status=0
  {
    __nvmrc_code=$(
      MANPATH="${MANPATH-}" NODE_PATH="${NODE_PATH-}" \
        NVM_SYMLINK_CURRENT="${NVM_SYMLINK_CURRENT-}" PREFIX="${PREFIX-}" \
        NVMRC_SCRIPT_FD=3 command nvm "$@" 3>&1 1>&4 4>&-
    ) || __nvmrc_status=$?
  } 4>&1
  eval "$__nvmrc_code"
  set -- "$__nvmrc_status"
  unset __nvmrc_code __nvmrc_status
  return "$1"
}
"#;

/// The init snippet for `shell`, newline-terminated.
#[must_use]
pub fn snippet(shell: Shell, options: &InitOptions) -> String {
    let name = shell.name();
    format!(
        "{BEGIN_MARKER}\n\
# The nvm function of nvmrc, from `eval \"$(nvm init {name})\"` in the {name} startup file.\n\
{PREAMBLE}{header}{BODY}{auto}{END_MARKER}\n",
        header = shell.function_header(),
        auto = options.auto_statement()
    )
}

#[cfg(test)]
mod tests;
