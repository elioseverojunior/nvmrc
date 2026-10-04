//! Whether a loader line runs nothing but the loader, so that `migrate` may
//! comment the whole line out without losing anything else.
//!
//! The line (without its comment) is split into simple commands at the
//! unquoted operators `;`, `&`, `|`, `(` and `)` outside `$(...)`, `${...}`
//! and backquotes. After its leading reserved words (`if`, `then`, `{`,
//! `builtin`, ...) each one must be empty, a test (`[`, `[[`, `test`), or a
//! `.`, `\.` or `source` of nvm.sh or nvm's completion. So
//! `[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"` and
//! `if [ -s f ]; then . f; fi` qualify, while `export NVM_DIR=...; . nvm.sh`,
//! `. nvm.sh && nvm use 18` or an `alias` holding a loader do not.

use super::rules::{sources_completion, sources_nvm_sh};

/// The words that may stand before a command without being one.
const RESERVED: [&str; 9] = [
    "if", "then", "else", "elif", "fi", "{", "}", "builtin", "command",
];
/// The commands that only test a file.
const GUARDS: [&str; 3] = ["[", "[[", "test"];
/// The commands that read a file into the shell.
const LOADERS: [&str; 3] = [".", "\\.", "source"];
/// What separates simple commands outside quotes and expansions.
const SEPARATORS: [char; 5] = [';', '&', '|', '(', ')'];

/// Whether `code` (a line without its comment) runs only tests and loaders
/// of nvm.sh or its completion (see the module documentation).
pub(super) fn is_standalone(code: &str) -> bool {
    simple_commands(code).into_iter().all(is_allowed)
}

fn is_allowed(command: &str) -> bool {
    let mut words = command
        .split_whitespace()
        .skip_while(|word| RESERVED.contains(word));
    match words.next() {
        None => true,
        Some(word) if GUARDS.contains(&word) => true,
        Some(word) if LOADERS.contains(&word) => {
            sources_nvm_sh(command) || sources_completion(command)
        }
        Some(_) => false,
    }
}

/// `code` cut at every separator outside quotes and expansions.
fn simple_commands(code: &str) -> Vec<&str> {
    let mut splitter = Splitter::default();
    let mut commands = Vec::new();
    let mut start = 0;
    let mut previous = None;
    let mut characters = code.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        let next = characters.peek().map(|(_, next)| *next);
        if splitter.separates(previous, character, next) {
            commands.push(&code[start..index]);
            start = index + character.len_utf8();
        }
        previous = Some(character);
    }
    commands.push(&code[start..]);
    commands
}

/// Where the split stands: inside quotes, after a `\`, inside expansions.
#[derive(Debug, Default)]
struct Splitter {
    single: bool,
    double: bool,
    backquote: bool,
    escaped: bool,
    /// The `$(` and `${` still open.
    depth: usize,
    /// The `(` or `{` of an expansion comes next.
    opening: bool,
}

impl Splitter {
    /// Reads `character`, between `previous` and `next`; true when it
    /// separates two simple commands.
    fn separates(&mut self, previous: Option<char>, character: char, next: Option<char>) -> bool {
        if std::mem::take(&mut self.escaped) || std::mem::take(&mut self.opening) {
            return false;
        }
        if self.single {
            self.single = character != '\'';
            return false;
        }
        match character {
            '\\' => self.escaped = true,
            '"' => self.double = !self.double,
            '\'' if !self.double => self.single = true,
            '`' => self.backquote = !self.backquote,
            '$' if matches!(next, Some('(' | '{')) => {
                self.depth += 1;
                self.opening = true;
            }
            ')' | '}' if self.depth > 0 => self.depth -= 1,
            // `2>&1`, `<&3` and `&>file` are redirections.
            '&' if matches!(previous, Some('>' | '<')) || next == Some('>') => {}
            _ => return self.at_top() && SEPARATORS.contains(&character),
        }
        false
    }

    fn at_top(&self) -> bool {
        !self.double && !self.backquote && self.depth == 0
    }
}
