//! POSIX shell code that the binary prints for the `nvm` function to `eval`.

use thiserror::Error;

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
    fn render(&self) -> String {
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

    /// Renders the script, one statement per line.
    #[must_use]
    pub fn render(&self) -> String {
        self.steps.iter().map(Step::render).collect()
    }
}

#[cfg(test)]
mod tests;
