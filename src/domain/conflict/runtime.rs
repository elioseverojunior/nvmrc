//! The warning the init snippet has the binary print when, at shell start,
//! nvm.sh (or a lazy `nvm` stub) is already loaded in the shell. Only the
//! shell can see its functions, so the snippet runs the checks and passes
//! the binary the reason (`nvm __conflict <reason>`); the text lives here.

/// Why the snippet found nvm.sh still loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeConflict {
    /// nvm.sh's helper functions (`nvm_has`) are defined.
    Helpers,
    /// `nvm` was already a function that is not nvmrc's (a lazy stub).
    Function,
}

/// The lines under the first line of the warning, whatever the reason.
pub const RUNTIME_WARNING_ADVICE: &str = "  nvmrc now answers `nvm`, but the old loader keeps running on \
every shell start;\n  run `nvm doctor` to see where, and `nvm migrate` to move it";

/// The first line of the warning, before the reason in parentheses.
const HEADLINE: &str = "nvm: nvm.sh is still loaded in this shell";

impl RuntimeConflict {
    /// Every reason, in the order the snippet checks them.
    pub const ALL: [Self; 2] = [Self::Helpers, Self::Function];

    /// The argument of `nvm __conflict`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Helpers => "helpers",
            Self::Function => "function",
        }
    }

    /// What the snippet saw, in the first line of the warning.
    const fn why(self) -> &'static str {
        match self {
            Self::Helpers => "its helper functions are defined",
            Self::Function => "nvm was already a function",
        }
    }

    /// The reason named `name`, if any.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|reason| reason.name() == name)
    }
}

/// The whole warning, without a trailing newline.
#[must_use]
pub fn runtime_warning(reason: RuntimeConflict) -> String {
    format!("{HEADLINE} ({})\n{RUNTIME_WARNING_ADVICE}", reason.why())
}
