//! `nvm migrate --undo`: each startup file gets back the content of its
//! latest backup, written through a symbolic link with the same atomic
//! replace; the backups stay.

use std::path::{Path, PathBuf};

use super::args::Options;
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
    latest_backup(&name, &names).map(|backup| directory.join(backup))
}

/// The restores that change something; a file or backup that cannot be read
/// is reported and makes the second value `true`.
fn restores(
    context: &Context<'_>,
    backups: Vec<(PathBuf, PathBuf)>,
    transcript: &mut Transcript,
) -> (Vec<Restore>, bool) {
    let mut found = Vec::new();
    let mut failed = false;
    for (path, backup) in backups {
        let read = |path: &Path| context.fs.read_to_string(path);
        match read(&path).and_then(|old| Ok((old, read(&backup)?))) {
            Ok((old, new)) if old == new => {}
            Ok((old, new)) => found.push(Restore {
                change: Change { path, old, new },
                backup,
            }),
            Err(error) => {
                transcript.err(format!("nvm migrate: {}: {error}", path.display()));
                failed = true;
            }
        }
    }
    (found, failed)
}

/// Writes one restore through the link; whether it was written.
fn write(context: &Context<'_>, restore: &Restore, transcript: &mut Transcript) -> bool {
    let change = &restore.change;
    let path = change.path.display();
    match context
        .fs
        .replace_file(&change.path, &change.new, &|_| Ok(()))
    {
        Ok(()) => {
            transcript.out(format!("restored {path} from {}", restore.backup.display()));
            true
        }
        Err(error) => {
            transcript.err(format!("nvm migrate: {path}: {error}"));
            false
        }
    }
}
