//! What the alias rows of `nvm alias` and `nvm ls` are painted with: the
//! version in use, the palette of `NVM_COLORS` and whether colors are on.

use crate::commands::color_policy::{self, ColorPolicy};
use crate::commands::current;
use crate::commands::resolve::Shown;
use crate::context::Context;
use crate::domain::alias_format::colored::{AliasKind, AliasRow, VersionState, paint_line};
use crate::domain::colors::Palette;
use crate::error::CliError;

/// Built once per command: `nvm_ls_current` and `nvm_get_colors` are the
/// same for every row.
#[derive(Debug, Clone)]
pub struct AliasPainter {
    current: String,
    palette: Palette,
    colors: bool,
    warning: Option<String>,
}

impl AliasPainter {
    /// # Errors
    /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
    pub fn new(context: &Context<'_>, policy: ColorPolicy) -> Result<Self, CliError> {
        let (palette, warning) = color_policy::palette(context);
        Ok(Self {
            current: current::detect(context)?.to_string(),
            palette,
            colors: policy.enabled,
            warning,
        })
    }

    /// The row of the alias `name` whose `target` resolves to `shown`.
    #[must_use]
    pub fn line(&self, name: &str, target: &str, shown: &Shown, kind: AliasKind) -> String {
        let version = shown.to_string();
        let row = AliasRow {
            alias: name,
            target,
            version: &version,
            state: self.state(shown, &version),
            kind,
        };
        paint_line(&row, &self.palette, self.colors)
    }

    /// The single `Invalid color code: <x>` line, when `NVM_COLORS` has an
    /// invalid or missing role.
    #[must_use]
    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    /// The version in use first (`system` too), then installed, then `N/A`
    /// or `∞`; `system` not in use is plain (`nvm_is_version_installed`
    /// fails for it).
    fn state(&self, shown: &Shown, version: &str) -> VersionState {
        if version == self.current {
            return VersionState::Current;
        }
        match shown {
            Shown::Version(_) => VersionState::Installed,
            Shown::System => VersionState::Other,
            Shown::NotAvailable | Shown::Infinite => VersionState::Missing,
        }
    }
}
