//! What `nvm current` reports: which node the shell would run.

use std::fmt;
use std::path::Path;

use crate::domain::version::Version;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Current {
    /// No `node` on `PATH`.
    None,
    /// A `node` outside `$NVM_DIR`.
    System,
    Version(Version),
}

impl fmt::Display for Current {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str("none"),
            Self::System => formatter.write_str("system"),
            Self::Version(version) => version.fmt(formatter),
        }
    }
}

/// Classifies the `node` found first on `PATH`. Unlike `nvm.sh` this reads the
/// version from the path instead of running `node --version`, so a path under
/// `$NVM_DIR` that names no version (such as the `current` symlink) is `None`.
#[must_use]
pub fn classify(node_path: &Path, nvm_dir: &Path) -> Current {
    let Ok(relative) = node_path.strip_prefix(nvm_dir) else {
        return Current::System;
    };
    let parts: Vec<&str> = relative
        .components()
        .filter_map(|part| part.as_os_str().to_str())
        .collect();
    let version: Option<Version> = match parts.as_slice() {
        ["versions", "node", name, ..] => name.parse().ok(),
        ["versions", "io.js", name, ..] => format!("iojs-{name}").parse().ok(),
        // The legacy layout: $NVM_DIR/vX.Y.Z/bin/node.
        [name, ..] => name.parse().ok(),
        [] => None,
    };
    version.map_or(Current::None, Current::Version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify_path(path: &str) -> String {
        classify(Path::new(path), Path::new("/n")).to_string()
    }

    #[test]
    fn a_node_outside_nvm_dir_is_system() {
        assert_eq!(classify_path("/usr/bin/node"), "system");
    }

    #[test]
    fn a_directory_that_only_shares_the_prefix_is_still_system() {
        assert_eq!(
            classify_path("/nvm-other/versions/node/v1.0.0/bin/node"),
            "system"
        );
    }

    #[test]
    fn a_node_version_directory_names_the_version() {
        assert_eq!(
            classify_path("/n/versions/node/v20.1.0/bin/node"),
            "v20.1.0"
        );
    }

    #[test]
    fn an_iojs_version_directory_gets_the_iojs_prefix() {
        assert_eq!(
            classify_path("/n/versions/io.js/v3.0.0/bin/node"),
            "iojs-v3.0.0"
        );
    }

    #[test]
    fn the_legacy_layout_names_the_version() {
        assert_eq!(classify_path("/n/v0.10.48/bin/node"), "v0.10.48");
    }

    #[test]
    fn a_path_under_nvm_dir_without_a_version_is_none() {
        assert_eq!(classify_path("/n/current/bin/node"), "none");
        assert_eq!(
            classify_path("/n/versions/node/not-a-version/bin/node"),
            "none"
        );
    }

    #[test]
    fn display_of_none_and_system() {
        assert_eq!(Current::None.to_string(), "none");
        assert_eq!(Current::System.to_string(), "system");
    }
}
