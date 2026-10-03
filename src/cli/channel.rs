//! How the shell code of a command reaches the calling shell.
//!
//! In the `nvm` function the context has the function's channel (the
//! descriptor `NVMRC_SCRIPT_FD` named, taken over at startup): the code is
//! sent there and the streams are untouched. Without it (standalone use,
//! scripts) the code is printed on stdout and the text the command meant for
//! stdout moves to stderr, so `eval "$(nvm use 18)"` works by hand.

use crate::commands::Output;
use crate::context::Context;
use crate::error::NvmExitCode;

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

pub(super) fn deliver(output: &Output, context: &Context<'_>) -> Delivery {
    let mut delivery = Delivery {
        stdout: line(&output.stdout),
        stderr: line(&output.stderr),
        status: output.status,
    };
    if output.script.is_empty() {
        return delivery;
    }
    let code = output.script.render();
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
