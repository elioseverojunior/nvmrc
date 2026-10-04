//! `nvm_rc_version`: the version a `.nvmrc` in the current directory or above
//! asks for, as `use`, `exec`, `run`, `which` and `install` read it.

use std::path::PathBuf;

use crate::commands::color_policy;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::nvmrc::{
    NvmrcContent, PLEASE_SEE, colored_invalid_message, find_nvmrc, invalid_message, process_content,
};

const MISSING_MESSAGE: &str = "No version provided and no .nvmrc file found";

/// What the lookup found.
#[derive(Debug, PartialEq, Eq)]
pub enum RcVersion {
    /// The file and the single bare version in it.
    Found { path: PathBuf, version: String },
    /// No `.nvmrc` (or none that could be read, or no directory to start
    /// from).
    Missing,
    /// A `.nvmrc` that breaks the content rules; its message was printed.
    Invalid,
}

/// `nvm_rc_version`, starting at the logical working directory
/// ([`Context::working_directory`]: `$PWD` as bash validates it).
///
/// Not silent: a find prints `Found '<path>' with version <<version>>` on
/// stdout, and a miss prints `No version provided and no .nvmrc file found`
/// on stderr. An unreadable file is a miss with the same message. An invalid
/// file always prints its message on stderr (silent or not), as the bytes
/// `nvm.sh` prints: the message without its closing newline, which the CLI
/// adds. The extra [`please_see`] line is NOT part of it: `nvm which` does not
/// print it, while `use` and `install` call [`please_see`] after an unusable
/// result.
pub fn rc_version(context: &Context<'_>, silent: bool, transcript: &mut Transcript) -> RcVersion {
    let content = context
        .working_directory()
        .and_then(|directory| find_nvmrc(context.fs, &directory))
        .and_then(|path| Some((context.fs.read_to_string(&path).ok()?, path)));
    let Some((text, path)) = content else {
        if !silent {
            transcript.err(MISSING_MESSAGE);
        }
        return RcVersion::Missing;
    };
    match process_content(&text) {
        NvmrcContent::Version(version) => {
            if !silent {
                let shown = path.display();
                transcript.out(format!("Found '{shown}' with version <{version}>"));
            }
            RcVersion::Found { path, version }
        }
        NvmrcContent::Invalid { parsed } => {
            transcript.err(invalid_report(context, &parsed));
            RcVersion::Invalid
        }
    }
}

/// The invalid-file message: colored only when the exported
/// `NVM_HAS_COLORS=1` forces it, since nvm.sh wraps it inside a command
/// substitution where the terminal check always fails.
fn invalid_report(context: &Context<'_>, parsed: &[String]) -> String {
    if color_policy::forced(context) {
        return colored_invalid_message(parsed);
    }
    invalid_message(parsed).trim_end_matches('\n').to_owned()
}

/// The closing stderr line `use` and `install` add after an unusable
/// `.nvmrc`.
pub fn please_see(transcript: &mut Transcript) {
    transcript.err(PLEASE_SEE);
}

#[cfg(test)]
mod tests;
