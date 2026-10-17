//! `npm=`, `yarn=` and `pnpm=` lines of a `.nvmrc`.

use thiserror::Error;

use super::{content_lines, trim_space};
use crate::domain::tool::Tool;
use crate::domain::version::Version;

/// The package manager versions a `.nvmrc` pins; each one exact.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Pins {
    pub npm: Option<Version>,
    pub yarn: Option<Version>,
    pub pnpm: Option<Version>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PinError {
    /// `line` is the pin as the `.nvmrc` rules see it.
    #[error("'{line}' is not an exact version (major.minor.patch)")]
    NotExact { line: String },
    #[error("yarn and pnpm are both pinned")]
    YarnAndPnpm,
    /// `key` (`npm`, `yarn` or `pnpm`) is pinned on more than one line.
    #[error("{key} is pinned more than once")]
    Repeated { key: String },
}

impl Pins {
    /// The pins in the order npm, yarn, pnpm.
    pub fn iter(&self) -> impl Iterator<Item = (Tool, Version)> {
        Tool::PACKAGE_MANAGERS
            .into_iter()
            .filter_map(|tool| self.get(tool).map(|version| (tool, version)))
    }

    #[must_use]
    pub fn get(&self, tool: Tool) -> Option<Version> {
        match tool {
            Tool::Node => None,
            Tool::Npm => self.npm,
            Tool::Yarn => self.yarn,
            Tool::Pnpm => self.pnpm,
        }
    }

    /// The pinned yarn or pnpm, the one `package.json` has to agree with.
    #[must_use]
    pub fn package_manager(&self) -> Option<(Tool, Version)> {
        self.iter()
            .find(|(tool, _)| matches!(tool, Tool::Yarn | Tool::Pnpm))
    }
}

/// Reads the pins from `.nvmrc` content. Lines are the ones
/// [`super::process_content`] sees; keys other than `npm`, `yarn` and `pnpm`
/// are not pins and are ignored. A pin may appear once.
pub fn parse_pins(contents: &str) -> Result<Pins, PinError> {
    let mut pins = Pins::default();
    for line in content_lines(contents) {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = trim_space(key);
        let slot = match key {
            "npm" => &mut pins.npm,
            "yarn" => &mut pins.yarn,
            "pnpm" => &mut pins.pnpm,
            _ => continue,
        };
        if slot.is_some() {
            return Err(PinError::Repeated {
                key: key.to_owned(),
            });
        }
        *slot = Some(
            exact_version(trim_space(value)).ok_or_else(|| PinError::NotExact {
                line: line.to_owned(),
            })?,
        );
    }
    if pins.yarn.is_some() && pins.pnpm.is_some() {
        return Err(PinError::YarnAndPnpm);
    }
    Ok(pins)
}

/// `major.minor.patch` of digits only: no `v`, prerelease or range.
pub(crate) fn exact_version(text: &str) -> Option<Version> {
    let parts: Vec<&str> = text.split('.').collect();
    let digits_only = parts
        .iter()
        .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()));
    if parts.len() != 3 || !digits_only {
        return None;
    }
    text.parse().ok()
}
