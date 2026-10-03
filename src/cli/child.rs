//! The program a command leaves to run (`nvm exec`, `nvm run`), started once
//! the command's messages are out, so they come before anything it prints.

use std::io::{ErrorKind, Write};

use crate::context::Context;
use crate::error::NvmExitCode;
use crate::ports::Invocation;

/// Runs `invocation` with this process's streams and returns its status;
/// one that cannot start is 127, as in a shell, with the reason on `err`,
/// prefixed by `tool` (`nvm: <program>: not found`, `nvm-exec: <program>:
/// not found`).
pub(super) fn run_child(
    tool: &str,
    context: &Context<'_>,
    invocation: &Invocation,
    err: &mut dyn Write,
) -> u8 {
    match context.process().spawn(invocation) {
        Ok(code) => NvmExitCode::passing_on(Some(code)).code(),
        Err(error) => {
            let program = invocation.program.display();
            let message = if error.kind() == ErrorKind::NotFound {
                format!("{tool}: {program}: not found\n")
            } else {
                format!("{tool}: cannot run {program}: {error}\n")
            };
            // Nowhere left to report a failed stderr write.
            let _ = super::emit(err, &message);
            NvmExitCode::NotFound.code()
        }
    }
}
