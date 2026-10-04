//! `nvm migrate [--dry-run] [--yes] [--undo] [--shell <name>]`: moves the
//! shell startup files from nvm.sh to nvmrc's init line (plan 8, digest
//! section 5). The top-level nvm.sh loader and completion lines of each root
//! file become `# [nvmrc-migrated]` comments under the init block; the diffs
//! are shown, and only `--yes` or a confirmation writes them: an atomic
//! replace through the link that the shell's syntax check must pass and
//! that is refused when the file changed since the diff, with a backup of
//! the old content (beside the LINK for a symbolic link, with the file's
//! permissions) taken just before the rename, then a scan proving no loader
//! is left. `--undo` restores the latest backups, backing the current
//! content up the same way.
//! `$NVM_DIR` and nvm.sh are never touched; paths are shown as named.

mod apply;
mod args;
mod check;
mod guarded;
mod plan;
mod undo;

#[cfg(test)]
mod apply_tests;
#[cfg(test)]
mod args_tests;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod output_tests;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod undo_tests;
#[cfg(test)]
mod write_tests;

use std::io;
use std::path::PathBuf;

use crate::commands::Output;
use crate::commands::conflict::{Report, roots, scan};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::printable::printable;
use crate::error::{CliError, NvmExitCode};

use args::Options;

/// How `nvm migrate` is called.
pub const USAGE: &str = "Usage: nvm migrate [--dry-run] [--yes] [--undo] [--shell <name>]";

const QUESTION: &str = "Apply these changes? [y/N] ";

/// Migrates (or with `--undo` restores) the startup files of the shell named
/// by `--shell`, of every shell when none is named.
///
/// # Errors
/// [`CliError::Usage`] for an unknown shell, a missing shell name or a
/// positional word, and [`CliError::Unsupported`] for any other option.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let options = args::parse(args)?;
    let files = roots(context, options.shell);
    if options.undo {
        return Ok(undo::run(context, &files, options));
    }
    Ok(migrate(context, &files, options))
}

/// The forward migration of `files`.
fn migrate(context: &Context<'_>, files: &[PathBuf], options: Options) -> Output {
    let mut transcript = Transcript::default();
    let report = scan(context, files);
    let (changes, mut unreadable) = plan::planned(context, &report, options.shell, &mut transcript);
    unreadable |= plan::unread_roots(&report, files, &mut transcript);
    if changes.is_empty() {
        transcript.out("nvm migrate: nothing to migrate");
        manual_line(&report, &mut transcript);
        return transcript.finish(status_of(unreadable));
    }
    let diffs: String = changes.iter().map(plan::Change::diff).collect();
    if let Some(status) = review(context, &diffs, options, &mut transcript) {
        return transcript.finish(status);
    }
    let skipped = apply::apply_all(context, &changes, &mut transcript);
    let (report, left) = apply::verify(context, files, &skipped, &mut transcript);
    let failed = unreadable || left || !skipped.is_empty();
    if !failed {
        transcript.out("nvm migrate: done");
    }
    manual_line(&report, &mut transcript);
    transcript.finish(status_of(failed))
}

fn status_of(failed: bool) -> NvmExitCode {
    if failed {
        NvmExitCode::Failure
    } else {
        NvmExitCode::Success
    }
}

/// `nvm migrate: N manual finding(s) left: ...` when the scan has conflicts
/// that `migrate` does not fix.
fn manual_line(report: &Report, transcript: &mut Transcript) {
    let manual = report
        .conflicts()
        .filter(|(file, hit)| file.depth != 0 || !hit.kind.is_auto_migratable())
        .count();
    if manual > 0 {
        transcript.out(format!(
            "nvm migrate: {manual} manual finding(s) left: run `nvm doctor`"
        ));
    }
}

/// Shows `diffs` (control characters in caret notation) and decides whether
/// to write them: `None` to go on, the
/// status to stop with otherwise. `--dry-run` stops after the diffs, `--yes`
/// goes on, else the prompt asks with the diffs in its question (so they
/// appear before it); without a terminal the diffs go to stdout and it
/// stops.
fn review(
    context: &Context<'_>,
    diffs: &str,
    options: Options,
    transcript: &mut Transcript,
) -> Option<NvmExitCode> {
    let diffs = &printable(diffs);
    if options.dry_run || options.yes {
        transcript.out(diffs.trim_end());
        return options.dry_run.then_some(NvmExitCode::Success);
    }
    let refusal = match context.prompt().confirm(&format!("{diffs}{QUESTION}")) {
        Ok(true) => return None,
        Ok(false) => return refused(transcript, "nvm migrate: aborted"),
        Err(error) if error.kind() == io::ErrorKind::Unsupported => {
            "nvm migrate: confirmation needs a terminal: pass --yes".to_owned()
        }
        Err(error) => format!("nvm migrate: {error}"),
    };
    transcript.out(diffs.trim_end());
    refused(transcript, refusal)
}

fn refused(transcript: &mut Transcript, message: impl Into<String>) -> Option<NvmExitCode> {
    transcript.err(message);
    Some(NvmExitCode::Failure)
}
