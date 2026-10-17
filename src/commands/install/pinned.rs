//! The `npm=`, `yarn=` and `pnpm=` pins of `.nvmrc`, installed into the
//! shared store once the node is in place.

use super::flow::{Halt, Step};
use crate::commands::package_managers::store::{install_tool, is_installed};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::nvmrc::pins::Pins;
use crate::domain::tool::Tool;
use crate::domain::version::Version;
use crate::error::NvmExitCode;

/// Installs each pinned tool that is not installed yet. A tool that cannot be
/// had ends the install with status 1; in an advisory context (an automatic
/// path) it is a warning and the install goes on.
pub(super) fn install_pinned(
    context: &Context<'_>,
    pins: &Pins,
    offline: bool,
    transcript: &mut Transcript,
) -> Step<()> {
    for (tool, version) in pins.iter() {
        if let Err(message) = install_one(context, tool, &version, offline, transcript) {
            if !context.is_advisory() {
                transcript.err(message);
                return Err(Halt::Exit(NvmExitCode::Failure));
            }
            let text = message.strip_prefix("nvm: ").unwrap_or(&message);
            transcript.err(format!("nvm: warning: {text}"));
        }
    }
    Ok(())
}

/// One tool; the error is the line to print.
fn install_one(
    context: &Context<'_>,
    tool: Tool,
    version: &Version,
    offline: bool,
    transcript: &mut Transcript,
) -> Result<(), String> {
    let label = format!("{} {}", tool.name(), tool.show(version));
    if is_installed(context, tool, version) {
        transcript.err(format!("{label} is already installed."));
        return Ok(());
    }
    if offline {
        return Err(format!(
            "{label} is not installed, and --offline does not download."
        ));
    }
    transcript.out(format!("Installing {label}..."));
    install_tool(context, tool, version)
        .map_err(|reason| format!("nvm: cannot install {label}: {reason}"))
}
