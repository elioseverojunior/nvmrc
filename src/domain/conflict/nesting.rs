//! The nesting heuristic: is a line part of a block, so that commenting it out
//! could empty a body or unbalance the file?
//!
//! The openers are the unquoted words `{`, `then` and `do`; the closers are
//! `}`, `fi` and `done`; `elif` closes the `then` before it (its own `then`
//! reopens). Every opener remembers the indentation of its line. A line is
//! nested when
//!
//! - its indentation is deeper than the innermost open block's line (so an
//!   indented line with no opener above, such as Homebrew's caveats pasted
//!   with their two spaces, is top level, and so is a body written at the
//!   opener's own indentation), or
//! - the line itself opens a block it does not close, or closes one it did
//!   not open (`if x; then . nvm.sh` with the `fi` below).
//!
//! A block opened and closed on the same line (`if [ -s f ]; then . f; fi`,
//! `npm() { ...; }`) leaves the line top level: commenting the whole line out
//! keeps the file valid. `case` is not tracked.

use super::lexer::Token;

/// The indentations of the lines of the blocks still open, innermost last.
#[derive(Debug, Default)]
pub(super) struct BlockTracker {
    open: Vec<usize>,
}

/// What a structural word does to the nesting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    Open,
    Close,
}

impl BlockTracker {
    /// Reads the next line (its `tokens` and `indentation`); true when it is
    /// nested (see the module documentation).
    pub(super) fn observe(&mut self, tokens: &[Token], indentation: usize) -> bool {
        let inside = self.open.last().is_some_and(|opener| indentation > *opener);
        let mut depth: isize = 0;
        let mut lowest: isize = 0;
        for effect in tokens.iter().filter_map(effect) {
            match effect {
                Effect::Open => {
                    self.open.push(indentation);
                    depth += 1;
                }
                Effect::Close => {
                    self.open.pop();
                    depth -= 1;
                    lowest = lowest.min(depth);
                }
            }
        }
        inside || depth > 0 || lowest < 0
    }
}

/// The effect of `token` on the nesting, when it is a structural word.
fn effect(token: &Token) -> Option<Effect> {
    let Token::Word {
        value,
        quoted: false,
    } = token
    else {
        return None;
    };
    match value.as_str() {
        "{" | "then" | "do" => Some(Effect::Open),
        // `elif` closes the `then` before it; its own `then` reopens.
        "}" | "fi" | "done" | "elif" => Some(Effect::Close),
        _ => None,
    }
}
