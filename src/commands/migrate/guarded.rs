//! The write shared by `migrate` and `--undo`: the planned content replaces
//! the file atomically (through a symbolic link) only when the candidate
//! passes its check AND the file still holds what the diff was made from;
//! the backup of that content is taken right then, just before the rename,
//! so a refused or failed write never leaves a backup behind.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use super::plan::Change;
use crate::context::Context;
use crate::domain::migration::backup_name;

/// How many sequence numbers are tried within one second.
const BACKUP_ATTEMPTS: u32 = 100;

/// Why a guarded write did not happen.
#[derive(Debug)]
pub enum Refusal {
    /// The file no longer holds `change.old`: someone wrote it meanwhile.
    Changed,
    /// The check, the backup or the replace failed.
    Failed(io::Error),
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Changed => formatter
                .write_str("changed since the diff was shown; left unchanged: run it again"),
            Self::Failed(error) => write!(formatter, "{error}"),
        }
    }
}

/// Writes `change.new` over `change.path` once `check` accepts the
/// candidate and the file still holds `change.old`, keeping a backup of
/// `change.old`; returns the backup's path.
///
/// # Errors
/// [`Refusal::Changed`] or [`Refusal::Failed`]; the file is untouched and no
/// backup is left.
pub fn replace_with_backup(
    context: &Context<'_>,
    change: &Change,
    check: &dyn Fn(&Path) -> io::Result<()>,
) -> Result<PathBuf, Refusal> {
    let backup = RefCell::new(None);
    let changed = Cell::new(false);
    let written = context
        .fs
        .replace_file(&change.path, &change.new, &|temporary| {
            check(temporary)?;
            if context.fs.read_to_string(&change.path)? != change.old {
                changed.set(true);
                return Err(io::Error::other("changed since the diff was shown"));
            }
            backup.replace(Some(create_backup(context, &change.path, &change.old)?));
            Ok(())
        });
    settle(context, written, backup.into_inner(), changed.get())
}

/// The outcome of the replace; a backup taken before a failed rename is
/// removed (nothing was migrated, so `--undo` must not pick it).
fn settle(
    context: &Context<'_>,
    written: io::Result<()>,
    backup: Option<PathBuf>,
    changed: bool,
) -> Result<PathBuf, Refusal> {
    match (written, backup) {
        (Ok(()), Some(backup)) => Ok(backup),
        (Ok(()), None) => Err(Refusal::Failed(io::Error::other("no backup was taken"))),
        (Err(error), backup) => {
            if let Some(backup) = backup {
                let _ = context.fs.remove_file(&backup);
            }
            Err(if changed {
                Refusal::Changed
            } else {
                Refusal::Failed(error)
            })
        }
    }
}

/// A new `<file name>.nvmrc-backup-<now>[-<n>]` holding `contents` in the
/// directory of `path` as named (a symbolic link's directory, not its
/// target's), with the permissions of the file `path` resolves to.
fn create_backup(context: &Context<'_>, path: &Path, contents: &str) -> io::Result<PathBuf> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let directory = path.parent().unwrap_or_else(|| Path::new("/"));
    let now = context.clock().unix_seconds();
    for sequence in 0..BACKUP_ATTEMPTS {
        let backup = directory.join(backup_name(&name, now, sequence));
        match context.fs.create_new_file(&backup, contents, path) {
            Ok(()) => return Ok(backup),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(cannot_write(&backup, &error)),
        }
    }
    let error = io::Error::from(io::ErrorKind::AlreadyExists);
    Err(cannot_write(
        &directory.join(backup_name(&name, now, 0)),
        &error,
    ))
}

fn cannot_write(backup: &Path, error: &io::Error) -> io::Error {
    io::Error::new(
        error.kind(),
        format!("cannot write {}: {error}", backup.display()),
    )
}
