//! `nvm install` with no version and no `--lts` installs what `.nvmrc` asks
//! for (digest 9): `lts/*` and `lts/<name>` there are the LTS filter.

use crate::commands::install::flow::{Halt, Step};
use crate::commands::install::options::Options;
use crate::commands::rc_version::{RcVersion, rc_version};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::error::NvmExitCode;

const USAGE: [&str; 3] = [
    "Usage: nvm install [<version>]",
    "  Provide a <version>, or run from a directory containing an .nvmrc file.",
    "  Run `nvm --help` for full help.",
];

/// `given`, with the version of `.nvmrc` when it names neither a version nor
/// an LTS line. The lookup prints `Found ...` on stdout, or why nothing was
/// found on stderr; with no version argument at all, that is followed by the
/// usage and status 127. An empty version argument (`nvm install ""`) goes
/// on with nothing, as in `nvm.sh`.
///
/// # Errors
/// [`Halt::Exit`] with [`NvmExitCode::NotFound`] as said above.
pub(super) fn with_nvmrc_version(
    context: &Context<'_>,
    given: Options,
    transcript: &mut Transcript,
) -> Step<Options> {
    if !given.version.is_empty() || given.lts.is_some() {
        return Ok(given);
    }
    match rc_version(context, false, transcript) {
        RcVersion::Found { version, .. } => Ok(given.with_version(&version)),
        _ if given.version_given => Ok(given),
        _ => {
            USAGE.into_iter().for_each(|line| transcript.err(line));
            Err(Halt::Exit(NvmExitCode::NotFound))
        }
    }
}
