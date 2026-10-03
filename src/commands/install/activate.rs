//! What an install does in the `nvm` function (its channel open): it
//! switches the shell to the version, as `nvm.sh` does with `nvm use`
//! (digest 9). Standalone, nothing is activated: the binary cannot change the
//! shell that runs it, so it behaves as a plain install.

use crate::commands::current::detect;
use crate::commands::install::flow::Step;
use crate::commands::transcript::Transcript;
use crate::commands::use_version;
use crate::context::Context;
use crate::domain::current::Current;
use crate::domain::version::Version;
use crate::error::NvmExitCode;

/// `nvm use <version>` for a version that was already installed: always,
/// even when it is the one in use, so `Now using ...` is printed again.
pub(super) fn use_always(
    context: &Context<'_>,
    version: &Version,
    transcript: &mut Transcript,
) -> NvmExitCode {
    if context.script_channel().is_none() {
        return NvmExitCode::Success;
    }
    use_installed(context, version, transcript)
}

/// `nvm_use_if_needed`: `nvm use <version>` after a fresh install, unless
/// that version is the one in use already.
///
/// # Errors
/// [`crate::commands::install::flow::Halt::Error`] when `$NVM_DIR` is unknown.
pub(super) fn use_if_needed(
    context: &Context<'_>,
    version: &Version,
    transcript: &mut Transcript,
) -> Step<NvmExitCode> {
    if context.script_channel().is_none() {
        return Ok(NvmExitCode::Success);
    }
    if matches!(detect(context)?, Current::Version(current) if current == *version) {
        return Ok(NvmExitCode::Success);
    }
    Ok(use_installed(context, version, transcript))
}

/// What `nvm use` prints and the shell code it makes go into `transcript`;
/// its status is returned.
fn use_installed(
    context: &Context<'_>,
    version: &Version,
    transcript: &mut Transcript,
) -> NvmExitCode {
    match use_version::run(context, &[version.to_string()]) {
        Ok(output) => {
            let status = output.status;
            transcript.absorb(output);
            status
        }
        Err(error) => {
            transcript.err(error.to_string());
            error.exit_code()
        }
    }
}
