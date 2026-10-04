//! Reading the roots and the files they source, breadth first (digest
//! section 6): every root at depth 0 before any file they source, so a root
//! that another root sources stays a root (auto-migratable).
//!
//! A `source`, `.` or `\.` line is followed when its path expands (`~`,
//! `$HOME`, `${ZDOTDIR:-$HOME}`, any variable of the process environment);
//! a relative path is resolved from the directory of the sourcing file as
//! named (the shell would use `$PWD`, which is `$HOME` for a login shell) and
//! noted. Files are told apart by their canonical path, so a cycle or a file
//! sourced twice (or through a link) is read once, under its first name.
//! Comments are not followed; nvmrc's own init block sources nothing.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use super::{FileFindings, Note, NoteReason, Report};
use crate::context::Context;
use crate::domain::conflict::{Expansion, expand_path, scan_text, source_target};

/// How deep sourced files are read: roots are 0, the files they source 1.
pub const MAX_DEPTH: usize = 2;

/// Every root (see [`super::roots`]) and the files they source, down to
/// [`MAX_DEPTH`]: what the rules found, in scan order, and what could not be
/// followed. Never fails: a file that cannot be read becomes a [`Note`].
#[must_use]
pub fn scan(context: &Context<'_>, roots: &[PathBuf]) -> Report {
    let mut walker = Walker {
        context,
        seen: HashSet::new(),
        queue: VecDeque::new(),
        report: Report::default(),
    };
    for root in roots {
        let canonical = context
            .fs
            .canonicalize(root)
            .unwrap_or_else(|_| root.clone());
        walker.enqueue(root.clone(), canonical, 0);
    }
    while let Some(pending) = walker.queue.pop_front() {
        walker.visit(pending);
    }
    walker.report
}

/// A file waiting to be read.
struct Pending {
    path: PathBuf,
    canonical: PathBuf,
    depth: usize,
}

struct Walker<'c, 'a> {
    context: &'c Context<'a>,
    /// The canonical paths already queued.
    seen: HashSet<PathBuf>,
    queue: VecDeque<Pending>,
    report: Report,
}

impl Walker<'_, '_> {
    fn enqueue(&mut self, path: PathBuf, canonical: PathBuf, depth: usize) {
        if self.seen.insert(canonical.clone()) {
            self.queue.push_back(Pending {
                path,
                canonical,
                depth,
            });
        }
    }

    fn visit(&mut self, file: Pending) {
        let text = match self.context.fs.read_to_string(&file.path) {
            Ok(text) => text,
            Err(error) => {
                let reason = NoteReason::Unreadable(error.to_string());
                return self.note(&file.path, None, "", reason);
            }
        };
        for (index, line) in text.lines().enumerate() {
            if let Some(token) = source_target(line) {
                self.follow(&file, index + 1, line, &token);
            }
        }
        self.report.files.push(FileFindings {
            hits: scan_text(&text),
            path: file.path,
            canonical: file.canonical,
            depth: file.depth,
        });
    }

    /// Queues (or notes) the file that line `number` of `from` sources.
    fn follow(&mut self, from: &Pending, number: usize, line: &str, token: &str) {
        let Some(path) = self.resolve(from, number, line, token) else {
            return;
        };
        if !self.context.fs.is_file(&path) {
            return self.note(&from.path, Some(number), line, NoteReason::Missing(path));
        }
        if is_nvm_loader_target(&path) {
            return;
        }
        let canonical = self.context.fs.canonicalize(&path);
        let canonical = canonical.unwrap_or_else(|_| path.clone());
        if from.depth < MAX_DEPTH {
            self.enqueue(path, canonical, from.depth + 1);
        } else if self.seen.insert(canonical) {
            self.note(&from.path, Some(number), line, NoteReason::TooDeep(path));
        }
    }

    /// The path `token` names, expanded, relative paths made absolute (and
    /// noted); `None` (noted) when it cannot be expanded.
    fn resolve(
        &mut self,
        from: &Pending,
        number: usize,
        line: &str,
        token: &str,
    ) -> Option<PathBuf> {
        let environment = self.context.env;
        let lookup = |name: &str| environment.var(name);
        let path = match expand_path(token, &lookup) {
            Expansion::Resolved(path) => PathBuf::from(path),
            Expansion::Unresolvable(why) => {
                self.note(
                    &from.path,
                    Some(number),
                    line,
                    NoteReason::Unresolvable(why),
                );
                return None;
            }
        };
        if path.is_absolute() {
            return Some(path);
        }
        let directory = from.path.parent().unwrap_or_else(|| Path::new("/"));
        let resolved = directory.join(path);
        let reason = NoteReason::Relative(resolved.clone());
        self.note(&from.path, Some(number), line, reason);
        Some(resolved)
    }

    fn note(&mut self, path: &Path, line: Option<usize>, text: &str, reason: NoteReason) {
        self.report.notes.push(Note {
            path: path.to_path_buf(),
            line,
            text: text.to_owned(),
            reason,
        });
    }
}

/// nvm.sh and its completion are what a loader line names, not startup files:
/// reading them would report nvm's own functions and helpers as findings.
fn is_nvm_loader_target(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("nvm.sh" | "bash_completion")
    )
}
