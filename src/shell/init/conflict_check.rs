//! What the snippet runs first, in an interactive shell only: whether nvm.sh
//! is still loaded (digest 4.2). The binary cannot see shell functions, so
//! the shell checks, once, before the snippet defines its own `nvm`, and
//! only when a check fires runs `nvm __conflict <reason>` (the message lives
//! in [`crate::domain::conflict::runtime_warning`]) on stderr. The checks
//! are builtins (no fork, except `$(...)` as noted); their status and the
//! binary's are ignored (`|| true`), so a `set -eu` shell starts anyway.
//!
//! First `helpers`: nvm.sh's `nvm_has` is defined (`type` is a builtin in
//! every shell; dash returns 127 when it is missing). Then `function`: `nvm`
//! is already a function that is not nvmrc's (a lazy stub, oh-my-zsh's or
//! zsh-nvm's lazy mode). nvmrc's function is recognized by its
//! `NVMRC_SCRIPT_FD`, so a second init in the same shell stays silent. sh
//! and dash cannot print a function's body: they skip this check when
//! `NVMRC_SHELL` is already set (an init already ran, here or in a parent).

use super::Shell;
use crate::domain::conflict::RuntimeConflict;

/// zsh: `nvm` is in the table of functions, and its body is not ours.
const ZSH_FOREIGN_FUNCTION: &str =
    "(( ${+functions[nvm]} )) && [[ ${functions[nvm]} != *NVMRC_SCRIPT_FD* ]]";
/// sh and dash: before any init, `command -v` names a function by its name.
const SH_FOREIGN_FUNCTION: &str =
    "[ -z \"${NVMRC_SHELL+set}\" ] && [ \"$(command -v nvm)\" = nvm ]";

/// bash and ksh: whether `nvm` is a function, then whether its body is ours.
fn by_body(defined: &str, body: &str, warn: &str) -> String {
    format!(
        "    elif {defined} >/dev/null 2>&1; then\n      \
         case \"$({body})\" in\n        *NVMRC_SCRIPT_FD*) ;;\n        \
         *) {warn} ;;\n      esac\n"
    )
}

fn posix_warn(reason: RuntimeConflict) -> String {
    format!("command \\nvm __conflict {} >&2 || true", reason.name())
}

fn posix(shell: Shell) -> String {
    let warn = posix_warn(RuntimeConflict::Function);
    let function_check = match shell {
        Shell::Bash => by_body("declare -F nvm", "declare -f nvm", &warn),
        Shell::Ksh => by_body("typeset -f nvm", "typeset -f nvm", &warn),
        Shell::Zsh => format!("    elif {ZSH_FOREIGN_FUNCTION}; then\n      {warn}\n"),
        _ => format!("    elif {SH_FOREIGN_FUNCTION}; then\n      {warn}\n"),
    };
    format!(
        "case $- in\n  *i*)\n    if type nvm_has >/dev/null 2>&1; then\n      {}\n\
         {function_check}    fi\n    ;;\nesac\n",
        posix_warn(RuntimeConflict::Helpers)
    )
}

fn fish() -> String {
    format!(
        "if status is-interactive\n    if functions -q nvm_has\n        \
         command nvm __conflict {} >&2\n    \
         else if functions -q nvm; and not functions nvm | string match -q '*NVMRC_SCRIPT_FD*'\n        \
         command nvm __conflict {} >&2\n    end\nend\n",
        RuntimeConflict::Helpers.name(),
        RuntimeConflict::Function.name()
    )
}

/// The check for `shell`, newline-terminated.
pub(super) fn conflict_check(shell: Shell) -> String {
    match shell {
        Shell::Fish => fish(),
        _ => posix(shell),
    }
}
