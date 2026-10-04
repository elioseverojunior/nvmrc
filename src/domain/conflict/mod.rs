//! The lines of shell startup files that load nvm.sh, or would fight nvmrc's
//! `nvm` (plan 8; the rule table of the research digest, section 7, written
//! by hand: the crate has no regex engine).
//!
//! [`scan_text`] reads one file. It skips comments (a `#` that starts a word
//! outside quotes, so `foo # . nvm.sh` is no hit), the lines `migrate` already
//! commented out (`# [nvmrc-migrated] ...`) and nvmrc's own init block
//! ([`BEGIN_MARKER`] to [`END_MARKER`], both included). Every other line gets
//! one [`Hit`] per [`Kind`] it matches, in the rules' order.
//!
//! A loader or completion is reported as [`Kind::LazyLoader`] (manual fix)
//! instead when it is nested (see `nesting.rs`: indented under an unclosed
//! `{`, `then` or `do`, or on a line that opens or closes a block), or when
//! the file also defines a lazy stub or unsets `nvm`. [`source_target`] and
//! [`expand_path`] let the caller follow the files a line sources.

mod expand;
mod kind;
mod lexer;
mod nesting;
mod plugins;
mod rules;
mod runtime;
mod source_path;

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
mod tests;

pub use expand::{Expansion, expand_path};
pub use kind::{Kind, Severity};
pub use runtime::{RUNTIME_WARNING_ADVICE, RuntimeConflict, runtime_warning};
pub use source_path::source_target;

use lexer::{indentation, strip_comment, tokens};
use nesting::BlockTracker;
use plugins::PluginsBlock;
use rules::line_kinds;

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
    let mut scanner = Scanner::default();
    for (index, line) in text.lines().enumerate() {
        scanner.observe(index + 1, line);
    }
    scanner.finish()
}

/// The state carried from line to line.
#[derive(Debug, Default)]
struct Scanner {
    hits: Vec<Hit>,
    inside_init_block: bool,
    plugins: PluginsBlock,
    blocks: BlockTracker,
}

impl Scanner {
    fn observe(&mut self, number: usize, line: &str) {
        if self.is_skipped(line) {
            return;
        }
        let code = strip_comment(line);
        let words = tokens(code);
        let nested = self.blocks.observe(&words, indentation(line));
        let mut kinds = line_kinds(code);
        if self.plugins.observe(&words) && !kinds.contains(&Kind::OmzPlugin) {
            kinds.push(Kind::OmzPlugin);
        }
        for kind in kinds {
            let kind = if nested { lazy(kind) } else { kind };
            self.hits.push(Hit {
                line: number,
                kind,
                text: line.to_string(),
            });
        }
    }

    /// Whether `line` is in (or delimits) the init block, or was migrated.
    fn is_skipped(&mut self, line: &str) -> bool {
        let trimmed = line.trim();
        if self.inside_init_block {
            self.inside_init_block = trimmed != END_MARKER;
            return true;
        }
        self.inside_init_block = trimmed == BEGIN_MARKER;
        self.inside_init_block || trimmed.starts_with(MIGRATED_PREFIX)
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

/// `kind`, or [`Kind::LazyLoader`] for a loader or completion.
fn lazy(kind: Kind) -> Kind {
    if kind.is_auto_migratable() {
        Kind::LazyLoader
    } else {
        kind
    }
}
