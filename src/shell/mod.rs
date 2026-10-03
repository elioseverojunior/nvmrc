//! The shell code that the binary prints for the `nvm` function to
//! evaluate: POSIX code, or fish code when the fish function asks for it.

pub mod fish;
pub mod init;

use thiserror::Error;

use crate::ports::Env;

/// The variable through which the `nvm` function asks for the shell code:
/// the number of the descriptor to write it to.
pub const DESCRIPTOR_VARIABLE: &str = "NVMRC_SCRIPT_FD";

/// The variable through which the fish `nvm` function asks for fish code
/// (`NVMRC_SHELL_KIND=fish`); the POSIX functions never set it.
pub const DIALECT_VARIABLE: &str = "NVMRC_SHELL_KIND";

/// The language a [`Script`] is rendered in.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    /// `export`, `unset` and `hash -r`, for every POSIX shell.
    #[default]
    Posix,
    /// `set -gx` and `set -e`, for fish.
    Fish,
}

impl Dialect {
    /// Fish when [`DIALECT_VARIABLE`] is exactly `fish`, POSIX otherwise
    /// (also when the binary runs on its own without it).
    #[must_use]
    pub fn from_env(env: &dyn Env) -> Self {
        match env.var(DIALECT_VARIABLE).as_deref() {
            Some("fish") => Self::Fish,
            _ => Self::Posix,
        }
    }
}

/// The descriptor number the `nvm` function opened for the shell code; `None`
/// when the binary runs on its own (the variable unset, or not a number).
#[must_use]
pub fn script_descriptor(env: &dyn Env) -> Option<String> {
    let value = env.var(DESCRIPTOR_VARIABLE)?;
    let is_number = !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit());
    is_number.then_some(value)
}

/// A programming error while building a script.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ShellError {
    /// The variable name is not `[A-Za-z_][A-Za-z0-9_]*`.
    #[error("invalid shell variable name: {0:?}")]
    InvalidName(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Export(String, String),
    Unset(String),
    HashReset,
}

/// An ordered list of shell statements.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Script {
    steps: Vec<Step>,
}

/// Single-quotes `value` for a POSIX shell (`'` becomes `'\''`).
#[must_use]
pub fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

fn validate(name: &str) -> Result<(), ShellError> {
    let mut chars = name.chars();
    let starts_well = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
    if starts_well && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Ok(())
    } else {
        Err(ShellError::InvalidName(name.to_string()))
    }
}

impl Step {
    fn render(&self, dialect: Dialect) -> String {
        match dialect {
            Dialect::Posix => self.render_posix(),
            Dialect::Fish => fish::render(self),
        }
    }

    fn render_posix(&self) -> String {
        match self {
            Step::Export(name, value) => format!("export {name}={}\n", quote(value)),
            Step::Unset(name) => format!("unset {name}\n"),
            Step::HashReset => "hash -r 2>/dev/null || true\n".to_string(),
        }
    }
}

impl Script {
    /// An empty script.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `export NAME='value'`.
    ///
    /// # Errors
    /// [`ShellError::InvalidName`] when `name` is not a valid variable name.
    pub fn export(self, name: &str, value: &str) -> Result<Self, ShellError> {
        validate(name)?;
        Ok(self.push(Step::Export(name.to_string(), value.to_string())))
    }

    /// Adds `unset NAME`.
    ///
    /// # Errors
    /// [`ShellError::InvalidName`] when `name` is not a valid variable name.
    pub fn unset(self, name: &str) -> Result<Self, ShellError> {
        validate(name)?;
        Ok(self.push(Step::Unset(name.to_string())))
    }

    /// Adds `hash -r 2>/dev/null || true`.
    #[must_use]
    pub fn hash_reset(self) -> Self {
        self.push(Step::HashReset)
    }

    fn push(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    /// Whether the script has no statements.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Appends the statements of `other`.
    #[must_use]
    pub fn append(mut self, other: Script) -> Self {
        self.steps.extend(other.steps);
        self
    }

    /// Renders the script as POSIX code, one statement per line.
    #[must_use]
    pub fn render(&self) -> String {
        self.render_in(Dialect::Posix)
    }

    /// Renders the script in `dialect`, one statement per line.
    #[must_use]
    pub fn render_in(&self, dialect: Dialect) -> String {
        self.steps.iter().map(|step| step.render(dialect)).collect()
    }
}

#[cfg(test)]
mod tests;
