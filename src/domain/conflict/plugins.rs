//! oh-my-zsh's `plugins=( ... )` array, which may span many lines: the word
//! `nvm` inside it loads omz's nvm plugin (digest 3.1).

use super::lexer::Token;

/// Whether the scan is inside a `plugins=(` (or `plugins+=(`) array.
#[derive(Debug, Default)]
pub(super) struct PluginsBlock {
    open: bool,
}

impl PluginsBlock {
    /// Reads the next line's `tokens`; true when one of its array elements is
    /// exactly `nvm` (quoted or not; `zsh-nvm` and `pnvm` are not).
    pub(super) fn observe(&mut self, tokens: &[Token]) -> bool {
        let mut found = false;
        let mut previous = None;
        for token in tokens {
            if self.open {
                match token {
                    Token::Operator(')') => self.open = false,
                    Token::Word { value, .. } => found |= value == "nvm",
                    Token::Operator(_) => {}
                }
            } else {
                self.open = opens_array(previous, token);
            }
            previous = Some(token);
        }
        found
    }
}

/// `plugins=` (or `plugins+=`) followed by `(`.
fn opens_array(previous: Option<&Token>, token: &Token) -> bool {
    let assigns = matches!(
        previous,
        Some(Token::Word { value, quoted: false }) if value == "plugins=" || value == "plugins+="
    );
    assigns && *token == Token::Operator('(')
}
