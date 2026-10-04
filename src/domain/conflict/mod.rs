//! The lines of shell startup files that load nvm.sh, or would fight nvmrc's
//! `nvm` (plan 8; the rule table of the research digest, section 7, written
//! by hand: the crate has no regex engine).
//!
//! [`scan_text`] reads one file. It skips comments (a `#` that starts a word
//! outside quotes, so `foo # . nvm.sh` is no hit), the lines `migrate` already
//! commented out (`# [nvmrc-migrated] ...`) and nvmrc's own init blocks
//! ([`init_blocks`]: a [`BEGIN_MARKER`] up to the next [`END_MARKER`], both
//! included; a begin marker with no end marker after it hides nothing).
//! Every other line gets one [`Hit`] per [`Kind`] it matches, in the rules'
//! order.
//!
//! A loader or completion is reported as [`Kind::LazyLoader`] (manual fix)
//! instead when it is nested (see `nesting.rs`: inside an unclosed block, or
//! on a line that opens or closes one), when it continues the line before or
//! is continued on the next (a `\` at the end), or when the file also
//! defines a lazy stub or unsets `nvm`; and as [`Kind::CompoundLoader`] when
//! its line runs other commands too (see `standalone.rs`). [`source_target`]
//! and [`expand_path`] let the caller follow the files a line sources.

mod expand;
mod kind;
mod lexer;
mod nesting;
mod plugins;
mod rules;
mod runtime;
mod source_path;
mod standalone;

#[cfg(test)]
mod expand_tests;
#[cfg(test)]
mod kind_tests;
#[cfg(test)]
mod lexer_tests;
#[cfg(test)]
mod nesting_tests;
#[cfg(test)]
mod rules_tests;
#[cfg(test)]
mod runtime_tests;
#[cfg(test)]
mod source_path_tests;
#[cfg(test)]
mod standalone_tests;
#[cfg(test)]
mod tests;

pub use expand::{Expansion, expand_path};
pub use kind::{Kind, Severity};
pub use runtime::{RUNTIME_WARNING_ADVICE, RuntimeConflict, runtime_warning};
pub use source_path::source_target;

use lexer::{ends_in_continuation, strip_comment, tokens};
use nesting::BlockTracker;
use plugins::PluginsBlock;
use rules::line_kinds;
use standalone::is_standalone;

/// The first line of nvmrc's init block (the snippet of `nvm init`).
pub const BEGIN_MARKER: &str = "# >>> nvmrc init >>>";
/// The last line of nvmrc's init block.
pub const END_MARKER: &str = "# <<< nvmrc init <<<";
/// What `migrate` puts before a line it disables.
pub const MIGRATED_PREFIX: &str = "# [nvmrc-migrated]";

/// One rule matching one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// The line number, from 1.
    pub line: usize,
    pub kind: Kind,
    /// The whole line, as written (without its line ending).
    pub text: String,
}

/// Every hit of the startup file `text` (see the module documentation).
#[must_use]
pub fn scan_text(text: &str) -> Vec<Hit> {
    let lines: Vec<&str> = text.lines().collect();
    let mut skipped = vec![false; lines.len()];
    for (begin, end) in init_blocks(&lines) {
        skipped[begin..=end].fill(true);
    }
    let mut scanner = Scanner::default();
    for (index, line) in lines.into_iter().enumerate() {
        if skipped[index] || line.trim().starts_with(MIGRATED_PREFIX) {
            continue;
        }
        scanner.observe(index + 1, line);
    }
    scanner.finish()
}

/// The 0-based first and last lines of each init block of `lines`: a
/// [`BEGIN_MARKER`] line and the [`END_MARKER`] line after it, with no other
/// begin marker between them (an earlier, unterminated begin marker is
/// ignored).
#[must_use]
pub fn init_blocks(lines: &[&str]) -> Vec<(usize, usize)> {
    let mut blocks = Vec::new();
    let mut open = None;
    for (index, line) in lines.iter().enumerate() {
        match line.trim() {
            BEGIN_MARKER => open = Some(index),
            END_MARKER => blocks.extend(open.take().map(|begin| (begin, index))),
            _ => {}
        }
    }
    blocks
}

/// The state carried from line to line.
#[derive(Debug, Default)]
struct Scanner {
    hits: Vec<Hit>,
    plugins: PluginsBlock,
    blocks: BlockTracker,
    /// The line before ended in a `\`.
    continued: bool,
}

impl Scanner {
    fn observe(&mut self, number: usize, line: &str) {
        let code = strip_comment(line);
        let words = tokens(code);
        let continues = ends_in_continuation(code);
        let nested = self.blocks.observe(&words) || self.continued || continues;
        self.continued = continues;
        let mut kinds = line_kinds(code);
        if self.plugins.observe(&words) && !kinds.contains(&Kind::OmzPlugin) {
            kinds.push(Kind::OmzPlugin);
        }
        let compound = !nested && !is_standalone(code);
        for kind in kinds {
            let kind = match kind {
                kind if nested => lazy(kind),
                kind if compound && kind.is_auto_migratable() => Kind::CompoundLoader,
                kind => kind,
            };
            self.push(number, kind, line);
        }
    }

    /// Records a hit, once per kind and line.
    fn push(&mut self, line: usize, kind: Kind, text: &str) {
        let duplicate = self
            .hits
            .last()
            .is_some_and(|last| last.line == line && last.kind == kind);
        if !duplicate {
            self.hits.push(Hit {
                line,
                kind,
                text: text.to_string(),
            });
        }
    }

    /// The hits, every loader lazy when the file defines stubs or unsets.
    fn finish(self) -> Vec<Hit> {
        let lazy_file = self
            .hits
            .iter()
            .any(|hit| matches!(hit.kind, Kind::LazyStub | Kind::Unset));
        if !lazy_file {
            return self.hits;
        }
        self.hits
            .into_iter()
            .map(|hit| Hit {
                kind: lazy(hit.kind),
                ..hit
            })
            .collect()
    }
}

/// `kind`, or [`Kind::LazyLoader`] for a loader or completion (one sharing
/// its line included).
fn lazy(kind: Kind) -> Kind {
    if kind.is_auto_migratable() || kind == Kind::CompoundLoader {
        Kind::LazyLoader
    } else {
        kind
    }
}
