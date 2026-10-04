//! How the shell code of a command reaches the calling shell.
//!
//! In the `nvm` function the context has the function's channel (the
//! descriptor `NVMRC_SCRIPT_FD` named, taken over at startup): the code is
//! sent there and the streams are untouched. Without it (standalone use,
//! scripts) the code is printed on stdout and the text the command meant for
//! stdout moves to stderr, so `eval "$(nvm use 18)"` works by hand.
//!
//! The code is POSIX unless `NVMRC_SHELL_KIND=fish` asks for fish code, as
//! the fish function does (and `NVMRC_SHELL_KIND=fish nvm use 18 |
//! source` by hand).

use crate::commands::Output;
use crate::context::Context;
use crate::error::NvmExitCode;
use crate::shell::Dialect;

/// The final text of each stream, newlines included, and the exit status.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Delivery {
    pub stdout: String,
    pub stderr: String,
    pub status: NvmExitCode,
}

fn line(text: &str) -> String {
    if text.is_empty() {
        String::new()
    } else {
        format!("{text}\n")
    }
}

/// The stdout text as a line, or the empty line the command asked for.
fn stdout_of(output: &Output) -> String {
    if output.stdout.is_empty() && output.blank_stdout {
        "\n".to_owned()
    } else {
        line(&output.stdout)
    }
}

pub(super) fn deliver(output: &Output, context: &Context<'_>) -> Delivery {
    let mut delivery = Delivery {
        stdout: stdout_of(output),
        stderr: line(&output.stderr),
        status: output.status,
    };
    if output.script.is_empty() {
        return delivery;
    }
    let code = output.script.render_in(Dialect::from_env(context.env));
    match context.script_channel() {
        None => {
            delivery.stderr.push_str(&delivery.stdout);
            delivery.stdout = code;
        }
        Some(channel) => {
            if let Err(error) = channel.send(&code) {
                delivery
                    .stderr
                    .push_str(&format!("nvm: cannot hand the shell code over: {error}\n"));
                if delivery.status == NvmExitCode::Success {
                    delivery.status = NvmExitCode::Failure;
                }
            }
        }
    }
    delivery
}
