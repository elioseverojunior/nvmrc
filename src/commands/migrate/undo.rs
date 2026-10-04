//! `nvm migrate --undo`: each startup file gets back the content of its
//! latest backup, written through a symbolic link with the same guarded
//! replace as `migrate`, which first backs the current content up: the
//! backups stay, and a second `--undo` undoes the first.

use std::path::{Path, PathBuf};

use super::args::Options;
use super::guarded::replace_with_backup;
use super::plan::Change;
use super::{review, status_of};
use crate::commands::Output;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::migration::latest_backup;
use crate::error::NvmExitCode;

/// A change back to a backup.
struct Restore {
    change: Change,
    backup: PathBuf,
}

/// Restores the latest backup of each of `files`.
pub fn run(context: &Context<'_>, files: &[PathBuf], options: Options) -> Output {
    let mut transcript = Transcript::default();
    let backups: Vec<(PathBuf, PathBuf)> = files
        .iter()
        .filter_map(|path| Some((path.clone(), latest(context, path)?)))
        .collect();
    if backups.is_empty() {
        transcript.err("nvm migrate: no backup found");
        return transcript.finish(NvmExitCode::Failure);
    }
    let (restores, mut failed) = restores(context, backups, &mut transcript);
    if restores.is_empty() && !failed {
        transcript.out("nvm migrate: nothing to restore");
        return transcript.finish(NvmExitCode::Success);
    }
    let diffs: String = restores
        .iter()
        .map(|restore| restore.change.diff())
        .collect();
    if let Some(status) = review(context, &diffs, options, &mut transcript) {
        return transcript.finish(status);
    }
    for restore in &restores {
        failed |= !write(context, restore, &mut transcript);
    }
    transcript.finish(status_of(failed))
}

/// The latest backup of `path` in the directory of `path` as named.
fn latest(context: &Context<'_>, path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let directory = path.parent()?;
    let names: Vec<String> = context
        .fs
        .read_dir(directory)
        .ok()?
        .into_iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.name)
        .collect();
    latest_backup(&name, &names, context.clock().unix_seconds())
        .map(|backup| directory.join(backup))
}

/// The restores that change something; a file or backup that cannot be read,
/// or an empty backup, is reported and makes the second value `true`.
fn restores(
    context: &Context<'_>,
    backups: Vec<(PathBuf, PathBuf)>,
    transcript: &mut Transcript,
) -> (Vec<Restore>, bool) {
    let mut found = Vec::new();
    let mut failed = false;
    for (path, backup) in backups {
        match restore_of(context, path, backup) {
            Ok(Some(restore)) => found.push(restore),
            Ok(None) => {}
            Err(message) => {
                transcript.err(message);
                failed = true;
            }
        }
    }
    (found, failed)
}

/// The restore of `path` from `backup`; `None` when both hold the same text.
/// An error message when either cannot be read, or when the backup is empty
/// and the file is not (a backup cut short by a crash).
fn restore_of(
    context: &Context<'_>,
    path: PathBuf,
    backup: PathBuf,
) -> Result<Option<Restore>, String> {
    let read = |file: &Path| context.fs.read_to_string(file);
    let failure = |error: std::io::Error| format!("nvm migrate: {}: {error}", path.display());
    let old = read(&path).map_err(failure)?;
    let new = read(&backup).map_err(failure)?;
    if old == new {
        return Ok(None);
    }
    if new.is_empty() {
        return Err(format!(
            "nvm migrate: {}: the backup is empty; {} was not restored",
            backup.display(),
            path.display()
        ));
    }
    Ok(Some(Restore {
        change: Change { path, old, new },
        backup,
    }))
}

/// Writes one restore through the link, backing the current content up
/// first (so the undo can itself be undone); whether it was written.
fn write(context: &Context<'_>, restore: &Restore, transcript: &mut Transcript) -> bool {
    let path = restore.change.path.display();
    match replace_with_backup(context, &restore.change, &|_| Ok(())) {
        Ok(backup) => {
            transcript.out(format!(
                "restored {path} from {} (backup: {})",
                restore.backup.display(),
                backup.display()
            ));
            true
        }
        Err(refusal) => {
            transcript.err(format!("nvm migrate: {path}: {refusal}"));
            false
        }
    }
}
