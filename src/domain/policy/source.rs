//! Where a rule came from, so messages can name the source that decided.

use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Flag(&'static str),
    Variable(&'static str),
    CentralPolicy(String),
    RepositoryPolicy(PathBuf),
    MinVersionFile(PathBuf),
}

impl Source {
    /// Everything but the central policy is an override of it.
    #[must_use]
    pub fn is_override(&self) -> bool {
        !matches!(self, Self::CentralPolicy(_))
    }
}

impl fmt::Display for Source {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Flag(name) | Self::Variable(name) => formatter.write_str(name),
            Self::CentralPolicy(location) => write!(formatter, "policy {location}"),
            Self::RepositoryPolicy(path) => {
                write!(formatter, "repository policy {}", path.display())
            }
            Self::MinVersionFile(path) => write!(formatter, "{}", path.display()),
        }
    }
}

/// A value together with the source that supplied it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sourced<T> {
    pub value: T,
    pub source: Source,
}
