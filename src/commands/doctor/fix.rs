//! What to do about each kind of finding: one table, the single place where
//! the wording of the advice lives.

use std::path::Path;

use crate::domain::conflict::Kind;

const AUTO: &str = "run `nvm migrate` (comments the line out and adds the nvmrc init line)";
const MANUAL_LOADER: &str = "remove or replace this loader by hand: nvmrc has its own function \
(see `eval \"$(nvmrc init <shell>)\"`)";
const STUB: &str = "the stub redefines `nvm` after the init line: delete it, or move the init \
line after it";
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
        Kind::LazyStub | Kind::Unset => STUB,
        Kind::OmzPlugin => OMZ,
        Kind::ZshNvm => ZSH_NVM,
        Kind::Bass => BASS,
        Kind::HelperCall => HELPER,
        Kind::NvmDirExport => KEPT,
        Kind::NotFollowed => "",
    }
}

/// The one-line patch suggested for a loader fixed by hand, `None` for the
/// kinds that have none.
#[must_use]
pub fn suggested_patch(kind: Kind, depth: usize, path: &Path) -> Option<String> {
    let by_hand = matches!(kind, Kind::LazyLoader) || (depth > 0 && kind == Kind::Loader);
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
