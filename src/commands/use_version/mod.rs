//! `nvm use`: the arguments and the version to switch to, resolved and
//! checked as nvm.sh does, with every failure message. The environment change
//! itself is built from the [`Target`] by the caller.

pub mod messages;
pub mod options;
pub mod target;

pub use options::Options;
pub use target::{Halt, SystemFlavor, SystemNode, Target};

use crate::commands::transcript::Transcript;
use crate::context::Context;

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
