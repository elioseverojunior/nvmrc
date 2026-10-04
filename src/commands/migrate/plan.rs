//! Which startup files `migrate` edits, and how.

use std::path::PathBuf;

use crate::commands::conflict::Report;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::migration::{file_shell, migrate_text, unified_diff};
use crate::shell::init::{SHELLS, Shell};

/// The planned new content of one file, named as the scan named it (a
/// symbolic link stays a link).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: PathBuf,
    pub old: String,
    pub new: String,
}

impl Change {
    /// The unified diff from the current content to the new one.
    pub fn diff(&self) -> String {
        unified_diff(&self.path.display().to_string(), &self.old, &self.new)
    }
}

/// The edits of the root files (depth 0) of `report` that change: the ones
/// with top-level loaders or completions, or with an outdated init block.
/// The block is for [`block_shell_of`] the file. A root that cannot be read
/// is reported on `transcript` and makes the second value `true`.
pub fn planned(
    context: &Context<'_>,
    report: &Report,
    shell: Option<Shell>,
    transcript: &mut Transcript,
) -> (Vec<Change>, bool) {
    let mut changes = Vec::new();
    let mut failed = false;
    for file in report.files.iter().filter(|file| file.depth == 0) {
        match context.fs.read_to_string(&file.path) {
            Ok(old) => changes.extend(change_of(file.path.clone(), old, shell)),
            Err(error) => {
                transcript.err(format!("nvm migrate: {}: {error}", file.path.display()));
                failed = true;
            }
        }
    }
    (changes, failed)
}

/// Warns about each of the `roots` the scan could not read (not text, not
/// readable): it was not migrated and may still load nvm.sh. Whether one
/// was found.
pub fn unread_roots(report: &Report, roots: &[PathBuf], transcript: &mut Transcript) -> bool {
    let mut found = false;
    for (path, error) in report.unreadable() {
        if roots.iter().any(|root| root == path) {
            transcript.err(format!(
                "nvm migrate: warning: {}: not read ({error}); not migrated",
                path.display()
            ));
            found = true;
        }
    }
    found
}

/// The edit of the file `path` whose content is `old`, when it changes.
fn change_of(path: PathBuf, old: String, shell: Option<Shell>) -> Option<Change> {
    let shell = block_shell_of(&path.display().to_string(), &old, shell);
    let new = migrate_text(&old, shell).text;
    (new != old).then_some(Change { path, old, new })
}

/// The shell of the init block for the file `path` holding `text`, `chosen`
/// by `--shell` or not. A file written for one shell ([`file_shell`]) takes
/// `chosen`, else the shell of a current block already in it (a block the
/// user chose stays), else its own. A POSIX file (`.profile`, `$ENV`), which
/// `/bin/sh` may read, only takes `sh`, `dash` or `ksh` that way, and `sh`
/// otherwise: never `$SHELL`, which may be zsh or fish.
fn block_shell_of(path: &str, text: &str, chosen: Option<Shell>) -> Shell {
    let preferred = chosen.or_else(|| current_block_shell(text));
    match file_shell(path) {
        Some(own) => preferred.unwrap_or(own),
        None => preferred
            .filter(|shell| matches!(shell, Shell::Sh | Shell::Dash | Shell::Ksh))
            .unwrap_or(Shell::Sh),
    }
}

/// The shell of a current init line already in `text`.
fn current_block_shell(text: &str) -> Option<Shell> {
    text.lines().find_map(|line| {
        SHELLS
            .into_iter()
            .find(|shell| line.trim() == shell.load_line())
    })
}
