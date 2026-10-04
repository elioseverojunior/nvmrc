//! The path a line sources (digest section 6): the token after the command
//! word `source`, `.` or `\.`, which starts the line or follows a blank or one
//! of `;&|{(`, and is followed by a blank.
//!
//! The token is a `"..."`, `'...'` or unquoted run up to a blank or one of
//! `;&|)`, its parts concatenated with the quotes removed (`"$NVM_DIR"/nvm.sh`
//! is `$NVM_DIR/nvm.sh`); `$(...)`, `${...}` and `` `...` `` are kept whole,
//! blanks included. A command word inside a quoted string counts too (an
//! `eval "... source \$NVM_DIR/nvm.sh ..."`), as the digest's rule table does.

use std::iter::Peekable;
use std::str::Chars;

use super::lexer::strip_comment;

/// The command words that read a file into the shell.
const COMMANDS: [&str; 3] = ["source", "\\.", "."];
/// What may stand right before a command word.
const COMMAND_PREFIXES: [char; 5] = [';', '&', '|', '{', '('];
/// What ends an unquoted token besides a blank.
const TOKEN_ENDS: [char; 4] = [';', '&', '|', ')'];

type Characters<'a> = Peekable<Chars<'a>>;

/// The path the first `source`, `.` or `\.` of `line` reads, quotes removed;
/// `None` when the line sources nothing outside its comment.
#[must_use]
pub fn source_target(line: &str) -> Option<String> {
    source_targets(strip_comment(line)).into_iter().next()
}

/// Every path `code` (a line without its comment) sources, in order.
pub(super) fn source_targets(code: &str) -> Vec<String> {
    code.char_indices()
        .filter(|(start, _)| starts_command(code, *start))
        .filter_map(|(start, _)| command_end(code, start))
        .filter_map(|end| read_token(&code[end..]))
        .collect()
}

/// Whether a command word may start at byte `start` of `code`.
fn starts_command(code: &str, start: usize) -> bool {
    code[..start]
        .chars()
        .next_back()
        .is_none_or(|before| before.is_whitespace() || COMMAND_PREFIXES.contains(&before))
}

/// The end of the command word starting at `start`, when one does and a
/// blank follows it.
fn command_end(code: &str, start: usize) -> Option<usize> {
    let rest = &code[start..];
    COMMANDS.iter().find_map(|command| {
        let after = rest.strip_prefix(command)?;
        after
            .starts_with(char::is_whitespace)
            .then_some(start + command.len())
    })
}

/// The token at the start of `text` (after its blanks), quotes removed.
fn read_token(text: &str) -> Option<String> {
    let mut characters = text.trim_start().chars().peekable();
    let mut token = String::new();
    let mut consumed = false;
    while let Some(character) = characters.next_if(|next| !ends_token(*next)) {
        consumed = true;
        match character {
            '\'' => copy_until(&mut characters, &mut token, '\''),
            '"' => read_double_quoted(&mut characters, &mut token),
            '\\' => token.extend(characters.next()),
            '$' if matches!(characters.peek(), Some('(' | '{')) => {
                copy_expansion(&mut characters, &mut token);
            }
            '`' => {
                token.push('`');
                copy_until(&mut characters, &mut token, '`');
                token.push('`');
            }
            other => token.push(other),
        }
    }
    consumed.then_some(token)
}

fn ends_token(character: char) -> bool {
    character.is_whitespace() || TOKEN_ENDS.contains(&character)
}

/// Copies up to `close`, which is consumed but not copied.
fn copy_until(characters: &mut Characters<'_>, token: &mut String, close: char) {
    token.extend(
        characters
            .by_ref()
            .take_while(|character| *character != close),
    );
}

/// The rest of a `"..."`: `\` escapes `"`, `\`, `$` and `` ` ``.
fn read_double_quoted(characters: &mut Characters<'_>, token: &mut String) {
    while let Some(character) = characters.next() {
        match character {
            '"' => return,
            '\\' => match characters.next_if(|next| matches!(next, '"' | '\\' | '$' | '`')) {
                Some(escaped) => token.push(escaped),
                None => token.push('\\'),
            },
            '$' if matches!(characters.peek(), Some('(' | '{')) => {
                copy_expansion(characters, token);
            }
            other => token.push(other),
        }
    }
}

/// Copies a `$(...)` or `${...}` whole (the `$` already read), nesting
/// included.
fn copy_expansion(characters: &mut Characters<'_>, token: &mut String) {
    token.push('$');
    let Some(open) = characters.next() else {
        return;
    };
    let close = if open == '(' { ')' } else { '}' };
    token.push(open);
    let mut depth = 1;
    for character in characters.by_ref() {
        token.push(character);
        if character == open {
            depth += 1;
        } else if character == close {
            depth -= 1;
            if depth == 0 {
                return;
            }
        }
    }
}
