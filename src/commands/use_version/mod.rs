//! `nvm use`: the arguments and the version to switch to, resolved and
//! checked as nvm.sh does, with every failure message. The environment change
//! itself is built from the [`Target`] by the caller.

pub mod apply;
pub mod messages;
pub mod options;
pub mod prefix;
pub mod target;

pub use options::Options;
pub use target::{Halt, SystemFlavor, SystemNode, Target};

use crate::commands::Output;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::error::CliError;

/// Parses `args` and resolves the version they ask for.
///
/// # Errors
/// A [`Halt`] whose messages are already in `transcript`.
pub fn plan(
    context: &Context<'_>,
    args: &[String],
    transcript: &mut Transcript,
) -> Result<(Options, Target), Halt> {
    let options = options::parse(args).map_err(|error| Halt::report(&error, transcript))?;
    let target = target::resolve(context, &options, transcript)?;
    Ok((options, target))
}

/// `nvm use`: resolves the version, then returns the shell code that
/// switches to it and the `Now using ...` line. A version that cannot be
/// used is an [`Output`] with its messages and status, and no code.
///
/// # Errors
/// [`CliError::InvalidOptions`] (status 6) when `--save` or `-w` is given
/// twice, and the errors of building the switch (an unresolvable
/// `NVM_DIR`).
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    run_and_target(context, args).map(|(output, _)| output)
}

/// [`run`], also returning the target it switched to (`None` when it stopped
/// before the switch), so that a caller never resolves the version twice.
///
/// # Errors
/// As [`run`].
pub fn run_and_target(
    context: &Context<'_>,
    args: &[String],
) -> Result<(Output, Option<Target>), CliError> {
    let options = options::parse(args)?;
    let mut transcript = Transcript::default();
    match target::resolve(context, &options, &mut transcript) {
        Ok(target) => {
            let output = apply::apply(context, &options, &target, transcript)?;
            Ok((output, Some(target)))
        }
        Err(halt) => Ok((transcript.finish(halt.status), None)),
    }
}

#[cfg(test)]
mod tests;
