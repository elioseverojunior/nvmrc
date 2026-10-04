//! What a rule found, how serious it is, and whether `migrate` may edit it.

use std::fmt;

/// How a finding weighs in `nvm doctor`'s verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Worth knowing, never a conflict (an `NVM_DIR` export, a `source` whose
    /// path cannot be followed).
    Info,
    /// Not a conflict, but worth a manual fix: a call of an nvm.sh helper
    /// (breaks once nvm.sh is gone), or oh-my-zsh's `nvm` plugin (it returns
    /// at once while nvmrc's `nvm` binary is on `PATH`, digest finding 0.5).
    Hint,
    /// nvm.sh (or something loading it) competes with nvmrc's `nvm`.
    Conflict,
}

/// The kind of a line the rules recognised (digest section 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A top-level `.`, `\.` or `source` of `nvm.sh`.
    Loader,
    /// A top-level source of nvm's `bash_completion`.
    Completion,
    /// A loader or completion inside a block, on a continued line, or in a
    /// file that defines lazy stubs: never edited automatically.
    LazyLoader,
    /// A top-level loader or completion sharing its line with other commands
    /// (`export NVM_DIR=...; . nvm.sh`): commenting the line out would lose
    /// them, so it is fixed by hand.
    CompoundLoader,
    /// A function named like a node command (`nvm() {`, `function npm {`).
    LazyStub,
    /// `unset -f nvm`, `unfunction nvm`: a self-removing stub.
    Unset,
    /// `nvm` in oh-my-zsh's `plugins=( ... )`, or its `zstyle`: a hint, since
    /// the plugin does nothing while nvmrc's binary is on `PATH`.
    OmzPlugin,
    /// lukechilds/zsh-nvm.
    ZshNvm,
    /// fish's `bass source .../nvm.sh`.
    Bass,
    /// A call of an nvm.sh internal (`nvm_find_nvmrc`, `nvm_ls`, ...).
    HelperCall,
    /// `export NVM_DIR=...`: kept, nvmrc reads it.
    NvmDirExport,
    /// A `source` whose path holds something that cannot be expanded.
    NotFollowed,
}

impl Kind {
    /// How serious the finding is.
    #[must_use]
    pub const fn severity(self) -> Severity {
        match self {
            Self::NvmDirExport | Self::NotFollowed => Severity::Info,
            Self::HelperCall | Self::OmzPlugin => Severity::Hint,
            _ => Severity::Conflict,
        }
    }

    /// Whether the finding counts as a conflict (`nvm doctor` exits 1).
    #[must_use]
    pub const fn is_conflict(self) -> bool {
        matches!(self.severity(), Severity::Conflict)
    }

    /// Whether `migrate` comments the line out on its own.
    #[must_use]
    pub const fn is_auto_migratable(self) -> bool {
        matches!(self, Self::Loader | Self::Completion)
    }

    /// The name shown in reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Loader => "nvm.sh loader",
            Self::Completion => "nvm bash_completion",
            Self::LazyLoader => "lazy loader",
            Self::CompoundLoader => "loader sharing its line",
            Self::LazyStub => "lazy stub",
            Self::Unset => "nvm unset",
            Self::OmzPlugin => "oh-my-zsh nvm plugin",
            Self::ZshNvm => "zsh-nvm",
            Self::Bass => "bass nvm.sh",
            Self::HelperCall => "nvm.sh helper call",
            Self::NvmDirExport => "NVM_DIR export",
            Self::NotFollowed => "source not followed",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}
