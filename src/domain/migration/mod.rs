//! Planning `nvm migrate` (plan 8, digest section 5), pure: the new text of
//! a startup file whose top-level nvm.sh loader and completion lines become
//! comments under nvmrc's init block, the diff shown before writing, the
//! backup names, and the program that syntax-checks the candidate.
//!
//! [`migrate_text`] never deletes a line: each [`Kind::Loader`] or
//! [`Kind::Completion`] line [`scan_text`] reports becomes
//! `# [nvmrc-migrated] <line>` (install.sh's duplicate guard, a `grep` for
//! `/nvm.sh`, still matches it and so never appends the loader again, digest
//! finding 0.8). `export NVM_DIR=...` lines stay (finding 0.7), and so do the
//! lines that need a manual fix (lazy loaders, stubs, plugins). The init
//! block goes before the first migrated line (keeping the order relative to
//! the `PATH` setup), or replaces the block already in the file.
//!
//! [`Kind::Loader`]: crate::domain::conflict::Kind::Loader
//! [`Kind::Completion`]: crate::domain::conflict::Kind::Completion

mod backup;
mod diff;

#[cfg(test)]
mod backup_tests;
#[cfg(test)]
mod diff_tests;
#[cfg(test)]
mod tests;

pub use backup::{backup_name, latest_backup};
pub use diff::unified_diff;

use crate::domain::conflict::{BEGIN_MARKER, END_MARKER, MIGRATED_PREFIX, scan_text};
use crate::shell::init::Shell;

/// The planned edit of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    /// The whole new content (equal to the input when nothing changes).
    pub text: String,
    /// The lines of the ORIGINAL file turned into comments, from 1.
    pub migrated_lines: Vec<usize>,
    /// What happened to the init block.
    pub block: BlockChange,
}

/// What [`migrate_text`] did with the init block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockChange {
    /// Added before line `before_line` of the original file (from 1).
    Inserted { before_line: usize },
    /// An existing block (same markers) was rewritten with the current one.
    Replaced,
    /// No block was added, or the existing one is already current.
    Unchanged,
}

/// The line of the init block that loads nvmrc ([`Shell::load_line`]): it
/// runs the `nvmrc` binary, never the `nvm` function (digest finding 0.1).
#[must_use]
pub fn load_line(shell: Shell) -> String {
    shell.load_line()
}

/// The init block `migrate` writes: the begin marker, [`load_line`] and the
/// end marker, each ending in `\n`.
#[must_use]
pub fn init_block(shell: Shell) -> String {
    format!("{BEGIN_MARKER}\n{}\n{END_MARKER}\n", load_line(shell))
}

/// The migration of the startup file `text` for `shell` (see the module
/// documentation). The original line endings are kept: a file without final
/// newline stays without, and the block's lines take the file's line ending
/// (`\r\n` when its first line ends so). A file with nothing to migrate and no
/// block is returned as is ([`BlockChange::Unchanged`]); a second run over
/// the result changes nothing.
#[must_use]
pub fn migrate_text(text: &str, shell: Shell) -> Migration {
    let plan = Plan::new(text, shell);
    let new_text = plan.render();
    if new_text == text {
        return Migration {
            text: new_text,
            migrated_lines: Vec::new(),
            block: BlockChange::Unchanged,
        };
    }
    Migration {
        text: new_text,
        block: plan.block_change(),
        migrated_lines: plan.targets,
    }
}

/// What [`migrate_text`] found in the file and will write.
struct Plan<'a> {
    /// The lines of the file as `(content, line ending)`.
    lines: Vec<(&'a str, &'a str)>,
    /// The lines to comment out, from 1.
    targets: Vec<usize>,
    /// The 0-based lines of the block already in the file.
    existing: Option<(usize, usize)>,
    /// The line (from 1) the new block goes before, when none exists.
    anchor: Option<usize>,
    /// The current block's lines, without endings.
    block: [String; 3],
    /// The line ending the block's lines take.
    ending: &'a str,
}

impl<'a> Plan<'a> {
    fn new(text: &'a str, shell: Shell) -> Self {
        let lines = split_lines(text);
        let targets = migratable_lines(text);
        let existing = existing_block(&lines);
        Self {
            anchor: existing
                .is_none()
                .then(|| targets.first().copied())
                .flatten(),
            ending: file_ending(&lines),
            block: [
                BEGIN_MARKER.to_owned(),
                load_line(shell),
                END_MARKER.to_owned(),
            ],
            lines,
            targets,
            existing,
        }
    }

    /// The new text of the file.
    fn render(&self) -> String {
        let mut text = String::new();
        for (index, line) in self.lines.iter().enumerate() {
            if self.anchor == Some(index + 1) {
                self.push_block(&mut text, self.ending);
            }
            match self.existing {
                Some((begin, end)) if index == begin => {
                    self.push_block(&mut text, self.lines[end].1)
                }
                Some((begin, end)) if (begin..=end).contains(&index) => {}
                _ if self.targets.contains(&(index + 1)) => push_migrated(&mut text, *line),
                _ => push_line(&mut text, line.0, line.1),
            }
        }
        text
    }

    /// The block, its last line ending in `last_ending`.
    fn push_block(&self, text: &mut String, last_ending: &str) {
        push_line(text, &self.block[0], self.ending);
        push_line(text, &self.block[1], self.ending);
        push_line(text, &self.block[2], last_ending);
    }

    /// What happens to the block (the text does change).
    fn block_change(&self) -> BlockChange {
        match (self.anchor, self.existing) {
            (Some(before_line), _) => BlockChange::Inserted { before_line },
            (None, Some((begin, end)))
                if self.lines[begin..=end]
                    .iter()
                    .map(|line| line.0)
                    .ne(self.block.iter().map(String::as_str)) =>
            {
                BlockChange::Replaced
            }
            _ => BlockChange::Unchanged,
        }
    }
}

/// The line numbers (from 1) of the auto-migratable hits, each once.
fn migratable_lines(text: &str) -> Vec<usize> {
    let mut lines: Vec<usize> = scan_text(text)
        .into_iter()
        .filter(|hit| hit.kind.is_auto_migratable())
        .map(|hit| hit.line)
        .collect();
    lines.dedup();
    lines
}

/// Each line of `text` as `(content, line ending)`; the ending is `\r\n`,
/// `\n`, or empty for a last line without newline.
fn split_lines(text: &str) -> Vec<(&str, &str)> {
    text.split_inclusive('\n')
        .map(|segment| {
            let content = segment
                .strip_suffix("\r\n")
                .or_else(|| segment.strip_suffix('\n'))
                .unwrap_or(segment);
            (content, &segment[content.len()..])
        })
        .collect()
}

/// The line ending of the file: the first one, `\n` when there is none.
fn file_ending<'a>(lines: &[(&str, &'a str)]) -> &'a str {
    lines
        .iter()
        .map(|line| line.1)
        .find(|ending| !ending.is_empty())
        .unwrap_or("\n")
}

/// The 0-based lines of the first complete init block (markers included).
fn existing_block(lines: &[(&str, &str)]) -> Option<(usize, usize)> {
    let begin = lines
        .iter()
        .position(|line| line.0.trim() == BEGIN_MARKER)?;
    let length = lines[begin..]
        .iter()
        .position(|line| line.0.trim() == END_MARKER)?;
    Some((begin, begin + length))
}

/// `line` commented out by `migrate`, indentation kept after the prefix.
fn push_migrated(text: &mut String, line: (&str, &str)) {
    text.push_str(MIGRATED_PREFIX);
    text.push(' ');
    push_line(text, line.0, line.1);
}

fn push_line(text: &mut String, content: &str, ending: &str) {
    text.push_str(content);
    text.push_str(ending);
}

/// The program (and its arguments) that syntax-checks a candidate for the
/// startup file `path`, chosen by its file name: `zsh` in the name (or
/// `.zprofile`, `.zlogin`, `.zlogout`) `zsh -n`; `bash` `bash -n`; `.fish`
/// `fish --no-execute`; `ksh` (`.kshrc`) `ksh -n`; any other name (`.profile`,
/// an `$ENV` file) `sh -n`. `None` when `path` names no file. The caller
/// appends the path of the temp file to the arguments.
#[must_use]
pub fn syntax_check(path: &str) -> Option<(&'static str, Vec<&'static str>)> {
    const ZSH_FILES: [&str; 3] = [".zprofile", ".zlogin", ".zlogout"];
    let name = path.rsplit('/').next().unwrap_or_default();
    let (program, flag) = if name.is_empty() {
        return None;
    } else if name.contains("zsh") || ZSH_FILES.contains(&name) {
        ("zsh", "-n")
    } else if name.contains("bash") {
        ("bash", "-n")
    } else if name.ends_with(".fish") {
        ("fish", "--no-execute")
    } else if name.contains("ksh") {
        ("ksh", "-n")
    } else {
        ("sh", "-n")
    };
    Some((program, vec![flag]))
}
