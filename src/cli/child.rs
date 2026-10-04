//! The program a command leaves to run (`nvm exec`, `nvm run`), started once
//! the command's messages are out, so they come before anything it prints.

use std::io::{ErrorKind, Write};

use crate::context::Context;
use crate::error::NvmExitCode;
use crate::ports::Invocation;

/// Who starts the program a command leaves to run, and how.
#[derive(Debug, Clone, Copy)]
pub(super) struct Launcher {
    /// The name its messages start with.
    pub tool: &'static str,
    /// Replace this process (`exec`) instead of waiting for a child.
    pub hand_over: bool,
}

/// `nvm exec` and `nvm run`: a child, waited for.
pub(super) const NVM: Launcher = Launcher {
    tool: "nvm",
    hand_over: false,
};
/// `nvm-exec`: `exec "$@"`, as the upstream script ends.
pub(super) const NVM_EXEC: Launcher = Launcher {
    tool: "nvm-exec",
    hand_over: true,
};

/// Runs `invocation` with this process's streams and returns its status;
/// one that cannot start is 127, as in a shell, with the reason on `err`,
/// prefixed by `launcher.tool` (`nvm: <program>: not found`, `nvm-exec:
/// <program>: not found`).
pub(super) fn run_child(
    launcher: Launcher,

    context: &Context<'_>,
    invocation: &Invocation,
    err: &mut dyn Write,
) -> u8 {
    let started = if launcher.hand_over {
        context.process().hand_over(invocation)
    } else {
        context.process().spawn(invocation)
    };
    match started {
        Ok(code) => NvmExitCode::passing_on(Some(code)).code(),
        Err(error) => {
            let program = invocation.program.display();
            let message = if error.kind() == ErrorKind::NotFound {
                format!("{}: {program}: not found\n", launcher.tool)
            } else {
                format!("{}: cannot run {program}: {error}\n", launcher.tool)
            };
            // Nowhere left to report a failed stderr write.
            let _ = super::emit(err, &message);
            NvmExitCode::NotFound.code()
        }
    }
}
