//! The nesting heuristic: is a line part of a block, so that commenting it out
//! could empty a body, unbalance the file, or make what follows conditional?
//!
//! The openers are the reserved words `if`, `case`, `while`, `until`, `for`
//! and `select` (closed by `fi`, `esac` and `done`) and the word `{` (closed
//! by `}`). The alphabetic ones count only where a command may start (first
//! on the line, after an operator such as `;` or `&&`, or after another
//! reserved word), so `echo for you` opens nothing; the braces count
//! anywhere outside quotes, as zsh accepts `npm() { nvm; command npm "$@" }`.
//! A line is nested when
//!
//! - a block is open before it, whatever its indentation (an `if` body
//!   written at the `if`'s own indentation is still conditional), or
//! - the line itself opens a block it does not close, or closes one it did
//!   not open (`if x; then . nvm.sh` with the `fi` below).
//!
//! A block opened and closed on the same line (`if [ -s f ]; then . f; fi`,
//! `npm() { ...; }`) leaves the line top level: commenting the whole line out
//! keeps the file valid. Here-documents are not tracked.

use super::lexer::Token;

/// How many blocks are still open.
#[derive(Debug, Default)]
pub(super) struct BlockTracker {
    open: usize,
}

/// What a structural word does to the nesting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    Open,
    Close,
}

impl BlockTracker {
    /// Reads the next line (its `tokens`); true when it is nested (see the
    /// module documentation).
    pub(super) fn observe(&mut self, tokens: &[Token]) -> bool {
        let inside = self.open > 0;
        let mut depth: isize = 0;
        let mut lowest: isize = 0;
        let mut command_position = true;
        for token in tokens {
            let (effect, next_position) = classify(token, command_position);
            command_position = next_position;
            match effect {
                Some(Effect::Open) => {
                    self.open += 1;
                    depth += 1;
                }
                Some(Effect::Close) => {
                    self.open = self.open.saturating_sub(1);
                    depth -= 1;
                    lowest = lowest.min(depth);
                }
                None => {}
            }
        }
        inside || depth > 0 || lowest < 0
    }
}

/// The effect of `token` on the nesting, and whether a command may start
/// after it.
fn classify(token: &Token, command_position: bool) -> (Option<Effect>, bool) {
    let value = match token {
        Token::Operator(_) => return (None, true),
        Token::Word { quoted: true, .. } => return (None, false),
        Token::Word { value, .. } => value.as_str(),
    };
    match value {
        "{" => (Some(Effect::Open), true),
        "}" => (Some(Effect::Close), true),
        _ if !command_position => (None, false),
        "if" | "while" | "until" => (Some(Effect::Open), true),
        // The word after these is a name or a value, not a command.
        "case" | "for" | "select" => (Some(Effect::Open), false),
        "fi" | "esac" | "done" => (Some(Effect::Close), true),
        "then" | "do" | "else" | "elif" | "!" | "time" => (None, true),
        _ => (None, false),
    }
}
