//! `nvm_rc_version`: the version a `.nvmrc` in the current directory or above
//! asks for, as `use`, `exec`, `run`, `which` and `install` read it.

use std::path::PathBuf;

use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::nvmrc::{
    NvmrcContent, PLEASE_SEE, find_nvmrc, invalid_message, process_content,
};

const MISSING_MESSAGE: &str = "No version provided and no .nvmrc file found";

/// What the lookup found.
#[derive(Debug, PartialEq, Eq)]
pub enum RcVersion {
    /// The file and the single bare version in it.
    Found { path: PathBuf, version: String },
    /// No `.nvmrc` (or none that could be read, or no `PWD` to start from).
    Missing,
    /// A `.nvmrc` that breaks the content rules; its message was printed.
    Invalid,
}

/// `nvm_rc_version`, starting at the logical working directory (`$PWD`, as
/// nvm.sh does; without it nothing is found).
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
        .env
        .var_os("PWD")
        .and_then(|directory| find_nvmrc(context.fs, &PathBuf::from(directory)))
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
            transcript.err(invalid_message(&parsed).trim_end_matches('\n'));
            RcVersion::Invalid
        }
    }
}

/// The closing stderr line `use` and `install` add after an unusable
/// `.nvmrc`.
pub fn please_see(transcript: &mut Transcript) {
    transcript.err(PLEASE_SEE);
}

#[cfg(test)]
mod tests;
