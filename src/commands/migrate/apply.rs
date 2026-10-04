//! Writing the planned edits: a backup beside each file, then an atomic
//! replace whose candidate must pass the shell's syntax check, then a scan
//! that proves no loader is left (digest section 5).

use std::cell::Cell;
use std::io;
use std::path::{Path, PathBuf};

use super::plan::Change;
use crate::commands::conflict::{Report, scan};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::migration::{backup_name, syntax_check};
use crate::ports::Process;

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

/// Backs `change.path` up and writes it; whether it was written.
fn apply(context: &Context<'_>, change: &Change, transcript: &mut Transcript) -> bool {
    let path = change.path.display();
    let backup = backup_path(context, &change.path);
    if let Err(error) = context.fs.write_file(&backup, &change.old) {
        let backup = backup.display();
        transcript.err(format!(
            "nvm migrate: {path}: cannot write {backup}: {error}"
        ));
        return false;
    }
    let check = SyntaxCheck::new(context.process(), &change.path);
    let written = context
        .fs
        .replace_file(&change.path, &change.new, &|temporary| {
            check.verify(temporary)
        });
    report(change, &backup, &check, &written, transcript);
    written.is_ok()
}

/// What happened to one file, on `transcript`.
fn report(
    change: &Change,
    backup: &Path,
    check: &SyntaxCheck<'_>,
    written: &io::Result<()>,
    transcript: &mut Transcript,
) {
    let path = change.path.display();
    if let Some(command) = check.missing() {
        transcript.err(format!(
            "nvm migrate: {command} not found: the syntax of {path} was not checked"
        ));
    }
    match written {
        Ok(()) => transcript.out(format!("migrated {path} (backup: {})", backup.display())),
        Err(_) if check.rejected.get() => transcript.err(format!(
            "nvm migrate: {path}: the edited file would not pass `{}`; left unchanged",
            check.command_line()
        )),
        Err(error) => transcript.err(format!("nvm migrate: {path}: {error}")),
    }
}

/// `<file name>.nvmrc-backup-<now>` in the directory of `path` as named (a
/// symbolic link's directory, not its target's).
fn backup_path(context: &Context<'_>, path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let directory = path.parent().unwrap_or_else(|| Path::new("/"));
    directory.join(backup_name(&name, context.clock().unix_seconds()))
}

/// The syntax checker of one file, run on the candidate before it replaces
/// the file, remembering what happened.
struct SyntaxCheck<'a> {
    process: &'a dyn Process,
    command: Option<(&'static str, Vec<&'static str>)>,
    missing: Cell<bool>,
    rejected: Cell<bool>,
}

impl<'a> SyntaxCheck<'a> {
    fn new(process: &'a dyn Process, path: &Path) -> Self {
        Self {
            process,
            command: syntax_check(&path.display().to_string()),
            missing: Cell::new(false),
            rejected: Cell::new(false),
        }
    }

    /// Runs the checker on `temporary`: a failure or a non-success status is
    /// an error; a checker that is not installed is skipped.
    fn verify(&self, temporary: &Path) -> io::Result<()> {
        let Some((program, flags)) = &self.command else {
            return Ok(());
        };
        let temporary = temporary.to_string_lossy();
        let mut arguments = flags.clone();
        arguments.push(&temporary);
        match self.process.run(Path::new(program), &arguments) {
            Ok(output) if output.success => Ok(()),
            Ok(_) => {
                self.rejected.set(true);
                Err(io::Error::other("the syntax check failed"))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.missing.set(true);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// The checker's program, when it was not found.
    fn missing(&self) -> Option<&'static str> {
        self.command
            .as_ref()
            .filter(|_| self.missing.get())
            .map(|(program, _)| *program)
    }

    /// `bash -n`, as the messages show it.
    fn command_line(&self) -> String {
        self.command
            .as_ref()
            .map(|(program, flags)| format!("{program} {}", flags.join(" ")))
            .unwrap_or_default()
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
