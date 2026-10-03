//! The POSIX `nvm` function, for bash, zsh, sh, dash and ksh.
//!
//! The binary is always `command \nvm`: a quoted name is never taken for an
//! alias, even after ksh93's `alias command='command '`. The function keeps
//! its status in `$1` (positional parameters are local to a function in
//! every POSIX shell) so it can unset its temporaries, and a failure inside
//! `$(...)` does not end a `set -e` shell before the code is applied.

use super::{InitOptions, Shell, with_exports};

const PREAMBLE: &str = r#"export NVMRC_SHELL=1
export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
unalias nvm 2>/dev/null || true
"#;

const BODY: &str = r#"
  case "${1-}" in
    use | deactivate | install | i | __auto) ;;
    *)
      (
        @EXPORTS@
        command \nvm "$@"
      )
      return
      ;;
  esac
  __nvmrc_status=0
  {
    __nvmrc_code=$(
      @EXPORTS@
      NVMRC_SCRIPT_FD=3 command \nvm "$@" 3>&1 1>&4 4>&-
    ) || __nvmrc_status=$?
  } 4>&1
  eval "$__nvmrc_code"
  set -- "$__nvmrc_status"
  unset __nvmrc_code __nvmrc_status
  return "$1"
}
"#;

/// How the function starts. zsh reads `function nvm {`, because it expands
/// an alias named `nvm` in `nvm() {` even after `unalias` in the same `eval`
/// string; the POSIX form everywhere else.
fn function_header(shell: Shell) -> &'static str {
    match shell {
        Shell::Zsh => "function nvm {",
        _ => "nvm() {",
    }
}

fn export_when_set(name: &str) -> String {
    format!("if [ -n \"${{{name}+set}}\" ]; then export {name}; fi")
}

/// The preamble, the function and the automatic step (`\nvm`: a quoted
/// name is never taken for an alias, and zsh expands the aliases of a whole
/// `eval` string before running its `unalias`).
pub(super) fn code(shell: Shell, options: &InitOptions) -> String {
    let auto = options
        .auto_arguments()
        .map(|arguments| format!("\\nvm {arguments}\n"))
        .unwrap_or_default();
    format!(
        "{PREAMBLE}{}{}{auto}",
        function_header(shell),
        with_exports(BODY, export_when_set)
    )
}
