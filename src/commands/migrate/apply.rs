//! Writing the planned edits: an atomic replace whose candidate must pass
//! the shell's syntax check, with a backup beside each file (see
//! `guarded.rs`), then a scan that proves no loader is left (digest section
//! 5).

use std::path::PathBuf;

use super::check::SyntaxCheck;
use super::guarded::{Refusal, replace_with_backup};
use super::plan::Change;
use crate::commands::conflict::{Report, scan};
use crate::commands::transcript::Transcript;
use crate::context::Context;

/// Applies every change in order. Returns the paths that were NOT written
/// (refused or failed); each one is reported on `transcript`.
pub fn apply_all(
    context: &Context<'_>,
    changes: &[Change],
    transcript: &mut Transcript,
) -> Vec<PathBuf> {
    changes
        .iter()
        .filter(|change| !apply(context, change, transcript))
        .map(|change| change.path.clone())
        .collect()
}

/// Writes `change` (see [`replace_with_backup`]); whether it was written.
fn apply(context: &Context<'_>, change: &Change, transcript: &mut Transcript) -> bool {
    let check = SyntaxCheck::new(context.process(), &change.path);
    let written = replace_with_backup(context, change, &|temporary| check.verify(temporary));
    report(change, &check, &written, transcript);
    written.is_ok()
}

/// What happened to one file, on `transcript`.
fn report(
    change: &Change,
    check: &SyntaxCheck<'_>,
    written: &Result<PathBuf, Refusal>,
    transcript: &mut Transcript,
) {
    let path = change.path.display();
    if let Some(command) = check.missing() {
        transcript.err(format!(
            "nvm migrate: {command} not found: the syntax of {path} was not checked"
        ));
    }
    match written {
        Ok(backup) => transcript.out(format!("migrated {path} (backup: {})", backup.display())),
        Err(Refusal::Failed(_)) if check.rejected() => transcript.err(format!(
            "nvm migrate: {path}: the edited file would not pass `{}`; left unchanged",
            check.command_line()
        )),
        Err(refusal) => transcript.err(format!("nvm migrate: {path}: {refusal}")),
    }
}

/// Scans `files` again and reports every top-level loader or completion
/// left outside the `skipped` files; whether one is left. Returns the new
/// scan too.
pub fn verify(
    context: &Context<'_>,
    files: &[PathBuf],
    skipped: &[PathBuf],
    transcript: &mut Transcript,
) -> (Report, bool) {
    let report = scan(context, files);
    let mut left = false;
    for (file, hit) in report.auto_migratable() {
        if !skipped.contains(&file.path) {
            left = true;
            transcript.err(format!(
                "nvm migrate: {}:{}: {} still there after the edit",
                file.path.display(),
                hit.line,
                hit.kind.label()
            ));
        }
    }
    (report, left)
}
