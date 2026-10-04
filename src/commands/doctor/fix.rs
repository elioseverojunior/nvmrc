//! What to do about each kind of finding: one table, the single place where
//! the wording of the advice lives.

use std::path::Path;

use crate::domain::conflict::Kind;

const AUTO: &str = "run `nvm migrate` (comments the line out and adds the nvmrc init line)";
const MANUAL_LOADER: &str = "remove or replace this loader by hand: nvmrc has its own function \
(see `eval \"$(nvmrc init <shell>)\"`)";
const COMPOUND: &str = "the line also runs other commands: move them to their own line(s), \
then run `nvm migrate` (or delete only the loader by hand)";
const STUB: &str = "the stub loads nvm.sh on first use, and an `nvm` stub after the init line \
hides nvmrc: delete it";
const UNSET: &str = "it removes a lazy stub once nvm.sh is loaded: delete it with the stub";
const DELETE_COMPLETION: &str = "delete the line: nvm's bash_completion needs nvm.sh";
const OMZ: &str = "remove `nvm` from `plugins=(...)`; it does nothing while the nvmrc binary \
is on PATH but depends on it";
const ZSH_NVM: &str = "remove zsh-nvm";
const BASS: &str = "remove the bass line";
const HELPER: &str = "this hook calls an nvm.sh helper that does not exist under nvmrc: use \
`nvm` subcommands";
const KEPT: &str = "kept by `nvm migrate`";

/// The advice for a finding of `kind` in a file `depth` levels from a root.
/// A loader or completion that `migrate` cannot comment out (inside a
/// sourced file) is fixed by hand, as is a lazy loader.
#[must_use]
pub fn fix_text(kind: Kind, depth: usize) -> &'static str {
    match kind {
        Kind::Loader | Kind::Completion if depth == 0 => AUTO,
        Kind::Loader | Kind::Completion | Kind::LazyLoader => MANUAL_LOADER,
        Kind::CompoundLoader => COMPOUND,
        Kind::LazyStub => STUB,
        Kind::Unset => UNSET,
        Kind::OmzPlugin => OMZ,
        Kind::ZshNvm => ZSH_NVM,
        Kind::Bass => BASS,
        Kind::HelperCall => HELPER,
        Kind::NvmDirExport => KEPT,
        Kind::NotFollowed => "",
    }
}

/// The one-line patch suggested for a loader fixed by hand (`text` is its
/// line), `None` for the kinds that have none. A completion without nvm.sh
/// on its line is deleted: nvmrc has no completion to put in its place.
#[must_use]
pub fn suggested_patch(kind: Kind, depth: usize, path: &Path, text: &str) -> Option<String> {
    let loader_or_completion = matches!(kind, Kind::Loader | Kind::Completion);
    let by_hand = kind == Kind::LazyLoader || (depth > 0 && loader_or_completion);
    if by_hand && !text.contains("nvm.sh") {
        return Some(DELETE_COMPLETION.to_owned());
    }
    by_hand.then(|| {
        let shell = shell_of(path);
        let line = if shell == "fish" {
            "nvmrc init fish | source".to_owned()
        } else {
            format!("eval \"$(nvmrc init {shell})\"")
        };
        format!("replace the line with: {line}")
    })
}

/// The shell a file is written for, from its name: `zsh` in the name,
/// `.fish`, `ksh` in the name, bash otherwise.
fn shell_of(path: &Path) -> &'static str {
    let name = path.to_string_lossy();
    if name.ends_with(".fish") {
        "fish"
    } else if name.contains("zsh") {
        "zsh"
    } else if name.contains("ksh") {
        "ksh"
    } else {
        "bash"
    }
}
