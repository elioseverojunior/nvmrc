//! Expanding a sourced path without a shell (digest section 6), in order:
//!
//! 1. a leading `~` or `~/` becomes `$HOME`;
//! 2. `$HOME` and `${HOME}`;
//! 3. `${ZDOTDIR:-$HOME}` and `${ZDOTDIR:-${HOME}}`: `ZDOTDIR` when it is set
//!    and not empty, else `HOME`;
//! 4. any other `$VAR` or `${VAR}` from the environment.
//!
//! Anything else is unresolvable, never guessed: a `$(...)` or `` `...` ``, a
//! `${...}` of another form (`${0:A:h}`, `${VAR:-x}`, `${#VAR}`), a special
//! parameter (`$0`, `$@`), an unset variable, `~user`, or a glob character
//! (`*`, `?`, `[`) in the result. A relative path stays relative (the caller
//! resolves it). The values from the environment are not expanded again.

/// The outcome of [`expand_path`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expansion {
    /// The path, every expansion done.
    Resolved(String),
    /// Why the path cannot be followed, such as `unset $FOO`.
    Unresolvable(String),
}

/// Lookup of an environment variable.
type Environment<'a> = &'a dyn Fn(&str) -> Option<String>;

/// `token` (a [`source_target`](super::source_target)) with `~` and the
/// variables of `environment` expanded (see the module documentation).
#[must_use]
pub fn expand_path(token: &str, environment: &dyn Fn(&str) -> Option<String>) -> Expansion {
    match expand(token, environment) {
        Ok(path) if path.contains(['*', '?', '[']) => {
            Expansion::Unresolvable(format!("glob {path}"))
        }
        Ok(path) => Expansion::Resolved(path),
        Err(reason) => Expansion::Unresolvable(reason),
    }
}

fn expand(token: &str, environment: Environment<'_>) -> Result<String, String> {
    let (mut path, mut rest) = expand_tilde(token, environment)?;
    while let Some(position) = rest.find(['$', '`']) {
        path.push_str(&rest[..position]);
        let expression = &rest[position..];
        if expression.starts_with('`') {
            return Err(format!("command substitution {expression}"));
        }
        let (value, length) = expand_dollar(expression, environment)?;
        path.push_str(&value);
        rest = &expression[length..];
    }
    path.push_str(rest);
    Ok(path)
}

/// The expanded leading `~` (or nothing) and the rest of `token`.
fn expand_tilde<'a>(
    token: &'a str,
    environment: Environment<'_>,
) -> Result<(String, &'a str), String> {
    let Some(after) = token.strip_prefix('~') else {
        return Ok((String::new(), token));
    };
    if after.is_empty() || after.starts_with('/') {
        Ok((lookup("HOME", environment)?, after))
    } else {
        Err(format!("unsupported {token}"))
    }
}

/// The value of the `$` expression at the start of `expression`, and its
/// length.
fn expand_dollar(
    expression: &str,
    environment: Environment<'_>,
) -> Result<(String, usize), String> {
    let after = &expression[1..];
    if after.starts_with('(') {
        let length = closing(expression, '(', ')').unwrap_or(expression.len());
        return Err(format!("command substitution {}", &expression[..length]));
    }
    if after.starts_with('{') {
        return expand_braced(expression, environment);
    }
    let name_length = identifier_length(after);
    if name_length == 0 {
        return Err(format!("unsupported {expression}"));
    }
    let value = lookup(&after[..name_length], environment)?;
    Ok((value, 1 + name_length))
}

/// A `${...}` at the start of `expression`.
fn expand_braced(
    expression: &str,
    environment: Environment<'_>,
) -> Result<(String, usize), String> {
    let length =
        closing(expression, '{', '}').ok_or_else(|| format!("unsupported {expression}"))?;
    let inner = &expression[2..length - 1];
    let value = match inner {
        "ZDOTDIR:-$HOME" | "ZDOTDIR:-${HOME}" => zdotdir_or_home(environment)?,
        name if identifier_length(name) == name.len() && !name.is_empty() => {
            lookup(name, environment)?
        }
        _ => return Err(format!("unsupported {}", &expression[..length])),
    };
    Ok((value, length))
}

fn zdotdir_or_home(environment: Environment<'_>) -> Result<String, String> {
    match environment("ZDOTDIR").filter(|value| !value.is_empty()) {
        Some(zdotdir) => Ok(zdotdir),
        None => lookup("HOME", environment),
    }
}

fn lookup(name: &str, environment: Environment<'_>) -> Result<String, String> {
    environment(name).ok_or_else(|| format!("unset ${name}"))
}

/// The length of the shell identifier at the start of `text` (0 when none).
fn identifier_length(text: &str) -> usize {
    let starts_identifier = text
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
    if !starts_identifier {
        return 0;
    }
    text.find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .unwrap_or(text.len())
}

/// The length of `$` plus the `open ... close` group after it, nesting
/// included; `None` when it never closes.
fn closing(expression: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    for (index, character) in expression.char_indices().skip(1) {
        if character == open {
            depth += 1;
        } else if character == close {
            depth -= 1;
            if depth == 0 {
                return Some(index + 1);
            }
        }
    }
    None
}
