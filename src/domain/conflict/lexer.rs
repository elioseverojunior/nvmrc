//! A one-line shell lexer, just enough for the rules: where a comment starts,
//! the words and operators of a line, whether it continues on the next one,
//! and word boundaries.
//!
//! Quotes are tracked within one line only (a string spanning lines, like
//! oh-my-zsh's `eval "..."`, is read line by line as unquoted text).

/// A word (its quotes removed) or one of the operators `;&|()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Token {
    /// `quoted` is true when any part of it was quoted or escaped, so it can
    /// never be a reserved word like `{` or `then`.
    Word {
        value: String,
        quoted: bool,
    },
    Operator(char),
}

const OPERATORS: [char; 5] = [';', '&', '|', '(', ')'];

/// Where the scan stands inside quotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quote {
    None,
    Single,
    Double,
}

/// `line` without its comment: a `#` that starts a word outside quotes and
/// everything after it (`${#x}`, `$#` and `a#b` are not comments).
pub(super) fn strip_comment(line: &str) -> &str {
    let mut quote = Quote::None;
    let mut escaped = false;
    let mut previous = None;
    for (index, character) in line.char_indices() {
        let starts_word = previous
            .is_none_or(|before: char| before.is_whitespace() || OPERATORS.contains(&before));
        if character == '#' && quote == Quote::None && !escaped && starts_word {
            return &line[..index];
        }
        (quote, escaped) = step(quote, escaped, character);
        previous = Some(character);
    }
    line
}

/// Where the text of a quote left open at the end of `code` starts (the byte
/// after the opening quote); `None` when every quote is closed.
pub(super) fn open_quote_contents(code: &str) -> Option<usize> {
    let mut quote = Quote::None;
    let mut escaped = false;
    let mut contents = None;
    for (index, character) in code.char_indices() {
        let opening = quote == Quote::None;
        (quote, escaped) = step(quote, escaped, character);
        if opening && quote != Quote::None {
            contents = Some(index + character.len_utf8());
        }
    }
    contents.filter(|_| quote != Quote::None)
}

/// The quote state after `character`; `escaped` is whether it is escaped.
fn step(quote: Quote, escaped: bool, character: char) -> (Quote, bool) {
    if escaped {
        return (quote, false);
    }
    match (quote, character) {
        (Quote::None, '\'') => (Quote::Single, false),
        (Quote::Single, '\'') | (Quote::Double, '"') => (Quote::None, false),
        (Quote::None, '"') => (Quote::Double, false),
        (Quote::None | Quote::Double, '\\') => (quote, true),
        _ => (quote, false),
    }
}

/// The words and operators of `code` (a line without its comment).
pub(super) fn tokens(code: &str) -> Vec<Token> {
    let mut lexer = Lexer::default();
    let mut characters = code.chars();
    while let Some(character) = characters.next() {
        lexer.feed(character, &mut characters);
    }
    lexer.end_word();
    lexer.tokens
}

#[derive(Debug, Default)]
struct Lexer {
    tokens: Vec<Token>,
    word: Option<(String, bool)>,
}

impl Lexer {
    fn feed(&mut self, character: char, rest: &mut std::str::Chars<'_>) {
        match character {
            blank if blank.is_whitespace() => self.end_word(),
            operator if OPERATORS.contains(&operator) => {
                self.end_word();
                self.tokens.push(Token::Operator(operator));
            }
            '\'' => self.quoted_until(rest, '\''),
            '"' => self.quoted_until(rest, '"'),
            '\\' => {
                if let Some(next) = rest.next() {
                    self.push(next, true);
                }
            }
            other => self.push(other, false),
        }
    }

    /// Reads a quoted part up to `close`; inside double quotes `\` escapes
    /// `"`, `\`, `$` and `` ` ``.
    fn quoted_until(&mut self, rest: &mut std::str::Chars<'_>, close: char) {
        self.word.get_or_insert_with(Default::default).1 = true;
        while let Some(character) = rest.next() {
            if character == close {
                return;
            }
            let escapable = |next: &char| matches!(next, '"' | '\\' | '$' | '`');
            let escaped = rest.clone().next().filter(escapable);
            match escaped {
                Some(next) if close == '"' && character == '\\' => {
                    rest.next();
                    self.push(next, true);
                }
                _ => self.push(character, true),
            }
        }
    }

    fn push(&mut self, character: char, quoted: bool) {
        let word = self.word.get_or_insert_with(Default::default);
        word.0.push(character);
        word.1 |= quoted;
    }

    fn end_word(&mut self) {
        if let Some((value, quoted)) = self.word.take() {
            self.tokens.push(Token::Word { value, quoted });
        }
    }
}

/// Whether `code` (a line without its comment, and without its line ending)
/// ends in an unescaped `\`, so the next physical line continues it.
pub(super) fn ends_in_continuation(code: &str) -> bool {
    let trailing = code
        .chars()
        .rev()
        .take_while(|&character| character == '\\');
    trailing.count() % 2 == 1
}

/// A letter, a digit or `_`, as a regex `\w`.
pub(super) fn is_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

/// Whether `text` holds `word` with a word boundary on both sides (regex
/// `\bword\b`, for a `word` that starts and ends with a word character).
pub(super) fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(start, _)| {
        let after = text[start + word.len()..].chars().next();
        boundary_before(text, start) && !after.is_some_and(is_word_character)
    })
}

/// Whether `text` holds `prefix` with a word boundary before it (regex
/// `\bprefix`).
pub(super) fn has_word_start(text: &str, prefix: &str) -> bool {
    text.match_indices(prefix)
        .any(|(start, _)| boundary_before(text, start))
}

/// Whether the character before byte `start` of `text` is not a word character.
pub(super) fn boundary_before(text: &str, start: usize) -> bool {
    !text[..start]
        .chars()
        .next_back()
        .is_some_and(is_word_character)
}
