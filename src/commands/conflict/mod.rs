//! Finding the lines of the startup files that load nvm.sh or fight nvmrc's
//! `nvm` (plan 8, digest sections 5 and 6): [`roots`] names the startup files
//! of a shell, [`scan`] reads them with the rules of
//! [`crate::domain::conflict`] and follows the files they source, two levels
//! deep, into a [`Report`] that `nvm doctor` prints and `nvm migrate` edits.

mod roots;
mod walk;

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod report_tests;
#[cfg(test)]
mod roots_tests;
#[cfg(test)]
mod walk_tests;

use std::fmt;
use std::path::PathBuf;

use crate::domain::conflict::Hit;

pub use roots::roots;
pub use walk::{MAX_DEPTH, scan};

/// One scanned file and what the rules found in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFindings {
    /// The path as named: the root, or the expanded path of the `source`
    /// line that reached it (a symbolic link stays a link).
    pub path: PathBuf,
    /// The file [`Self::path`] resolves to, links followed.
    pub canonical: PathBuf,
    /// 0 for a root, 1 for a file a root sources, 2 for the next level.
    pub depth: usize,
    /// Every hit, in line order (none for a clean file).
    pub hits: Vec<Hit>,
}

/// Why a [`Note`] was written: all of them are Info, never a conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteReason {
    /// The sourced path cannot be expanded without a shell; the text says
    /// why (`unset $FOO`, `command substitution $(brew --prefix nvm)`).
    Unresolvable(String),
    /// The sourced path is relative: it was resolved from the directory of
    /// the sourcing file (the shell resolves it from `$PWD`) to this path.
    Relative(PathBuf),
    /// The sourced path, expanded, names no file.
    Missing(PathBuf),
    /// The file exists but cannot be read as text; the I/O error.
    Unreadable(String),
    /// The sourced file is deeper than [`MAX_DEPTH`]: not scanned.
    TooDeep(PathBuf),
}

impl fmt::Display for NoteReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unresolvable(why) => write!(formatter, "not followed: {why}"),
            Self::Relative(path) => write!(
                formatter,
                "relative path resolved from the file's directory: {}",
                path.display()
            ),
            Self::Missing(path) => write!(formatter, "file not found: {}", path.display()),
            Self::Unreadable(error) => write!(formatter, "not read: {error}"),
            Self::TooDeep(_) => {
                write!(formatter, "not scanned: deeper than {MAX_DEPTH} levels")
            }
        }
    }
}

/// Something the scan could not do, or did on a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// The file the note is about (the sourcing file for a `source` line).
    pub path: PathBuf,
    /// The `source` line's number, from 1; `None` for the whole file.
    pub line: Option<usize>,
    /// The `source` line as written; empty for the whole file.
    pub text: String,
    pub reason: NoteReason,
}

/// Everything [`scan`] found: the files in scan order, then the notes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub files: Vec<FileFindings>,
    pub notes: Vec<Note>,
}

impl Report {
    /// Every hit that is a conflict (not Info, not Hint), with its file.
    pub fn conflicts(&self) -> impl Iterator<Item = (&FileFindings, &Hit)> {
        self.hits().filter(|(_, hit)| hit.kind.is_conflict())
    }

    /// Whether `nvm doctor` should fail: any hit is a conflict.
    #[must_use]
    pub fn has_conflicts(&self) -> bool {
        self.conflicts().next().is_some()
    }

    /// The hits `migrate` comments out on its own: top-level loaders and
    /// completions of a root file (depth 0). One in a sourced file is still
    /// a conflict, fixed by hand.
    pub fn auto_migratable(&self) -> impl Iterator<Item = (&FileFindings, &Hit)> {
        self.hits()
            .filter(|(file, hit)| file.depth == 0 && hit.kind.is_auto_migratable())
    }

    /// The files that could not be read (not text, not readable), with the
    /// error: they may load nvm.sh, so a scan with any is not clean.
    pub fn unreadable(&self) -> impl Iterator<Item = (&PathBuf, &str)> {
        self.notes.iter().filter_map(|note| match &note.reason {
            NoteReason::Unreadable(error) => Some((&note.path, error.as_str())),
            _ => None,
        })
    }

    fn hits(&self) -> impl Iterator<Item = (&FileFindings, &Hit)> {
        self.files
            .iter()
            .flat_map(|file| file.hits.iter().map(move |hit| (file, hit)))
    }
}
