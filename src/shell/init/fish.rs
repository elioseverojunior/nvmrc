//! The fish `nvm` function, for fish 3.4 or newer.
//!
//! fish runs a command substitution with only its stdout captured: the
//! redirections of an enclosing block do not reach it, so the POSIX
//! `{ code=$(... 3>&1 1>&4) } 4>&1` cannot be written there. The function
//! pipes instead: inside `begin ... end 4>&1` (descriptor 4 is the
//! function's stdout, with the caller's redirections), the binary's
//! descriptor 3 goes into the pipe, its stdout to descriptor 4, and
//! `read -z` takes the whole pipe into a variable in this shell (fish runs
//! builtins in a pipeline in-process). `$pipestatus[1]` is the binary's
//! status. `NVMRC_SHELL_KIND=fish` makes the binary write fish code.
//!
//! The passed variables are exported to the binary with `set -lx` inside
//! the block, so they disappear with it. Variables set with `set -l` are
//! local to the function: nothing needs unsetting. `command nvm` skips the
//! function (and any alias, which fish defines as a function).

use super::{InitOptions, with_exports};

const PREAMBLE: &str = r#"set -gx NVMRC_SHELL 1
test -n "$NVM_DIR"; or set -g NVM_DIR $HOME/.nvm
set -gx NVM_DIR $NVM_DIR
"#;

const FUNCTION: &str = r#"function nvm --description 'Node Version Manager (nvmrc)'
    if not contains -- "$argv[1]" use deactivate install i __auto
        begin
            @EXPORTS@
            command nvm $argv
        end
        return $status
    end
    set -l nvmrc_code
    set -l nvmrc_status
    begin
        @EXPORTS@
        NVMRC_SCRIPT_FD=3 NVMRC_SHELL_KIND=fish command nvm $argv 3>&1 1>&4 4>&- | read -z nvmrc_code
        set nvmrc_status $pipestatus[1]
    end 4>&1
    eval $nvmrc_code
    return $nvmrc_status
end
"#;

fn export_when_set(name: &str) -> String {
    format!("set -q {name}; and set -lx {name} ${name}")
}

/// The preamble, the function and the automatic step.
pub(super) fn code(options: &InitOptions) -> String {
    let auto = options
        .auto_arguments()
        .map(|arguments| format!("nvm {arguments}\n"))
        .unwrap_or_default();
    format!(
        "{PREAMBLE}{}{auto}",
        with_exports(FUNCTION, export_when_set)
    )
}
