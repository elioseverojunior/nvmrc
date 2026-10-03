//! How the shell code of a command reaches the calling shell.
//!
//! With `NVMRC_SCRIPT_FD` set to a descriptor number, the code is written to
//! `/dev/fd/<n>` and the streams are untouched. Without it (standalone use,
//! scripts) the code is printed on stdout and the text the command meant for
//! stdout moves to stderr, so `eval "$(nvm use 18)"` works by hand.

use std::path::PathBuf;

use crate::commands::Output;
use crate::context::Context;
use crate::error::NvmExitCode;
use crate::shell::script_descriptor;

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

/// The file a descriptor number in the environment names, if it is one.
fn descriptor_path(context: &Context<'_>) -> Option<PathBuf> {
    script_descriptor(context.env).map(|value| PathBuf::from(format!("/dev/fd/{value}")))
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
    match descriptor_path(context) {
        None => {
            delivery.stderr.push_str(&delivery.stdout);
            delivery.stdout = code;
        }
        Some(path) => {
            if let Err(error) = context.fs.write_file(&path, &code) {
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
