//! The implicit aliases `node`, `stable`, `unstable` and `iojs`: they have no
//! alias file and are derived from what is installed.

use std::fmt;

use crate::domain::version::{Flavor, Version};

/// The order `nvm alias` lists them in.
pub const IMPLICIT_ALIASES: [&str; 4] = ["iojs", "node", "stable", "unstable"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MajorMinor {
    pub major: u64,
    pub minor: u64,
}

impl fmt::Display for MajorMinor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)
    }
}

impl MajorMinor {
    fn of(version: &Version) -> Self {
        Self {
            major: version.major,
            minor: version.minor,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Implicit {
    pub stable: Option<MajorMinor>,
    pub unstable: Option<MajorMinor>,
    pub iojs: Option<MajorMinor>,
}

/// From 1.0 on every Node release line is stable. Below it, as in the old
/// scheme, even minors are stable and odd minors are unstable. The highest
/// line of each kind wins; io.js has a single kind.
#[must_use]
pub fn derive(installed: &[Version]) -> Implicit {
    let mut implicit = Implicit {
        iojs: lines_of(installed, Flavor::IoJs).into_iter().max(),
        ..Implicit::default()
    };
    for line in lines_of(installed, Flavor::Node) {
        if line.major >= 1 || line.minor % 2 == 0 {
            implicit.stable = Some(line);
        } else {
            implicit.unstable = Some(line);
        }
    }
    implicit
}

/// The release lines of one flavor, ascending and without repeats.
fn lines_of(installed: &[Version], flavor: Flavor) -> Vec<MajorMinor> {
    let mut lines: Vec<MajorMinor> = installed
        .iter()
        .filter(|version| version.flavor == flavor)
        .map(MajorMinor::of)
        .collect();
    lines.sort();
    lines.dedup();
    lines
}

/// The highest installed version of `flavor` on the release line `line`.
#[must_use]
pub fn highest_in(installed: &[Version], flavor: Flavor, line: MajorMinor) -> Option<Version> {
    installed
        .iter()
        .filter(|version| version.flavor == flavor && MajorMinor::of(version) == line)
        .max()
        .copied()
}

/// What `nvm alias <name>` shows as the target of an implicit alias, or `None`
/// when nothing is shown (`stable` before anything is installed).
#[must_use]
pub fn destination(installed: &[Version], name: &str) -> Option<String> {
    let implicit = derive(installed);
    match name {
        "node" => Some("stable".to_owned()),
        "stable" => implicit.stable.map(|line| line.to_string()),
        "unstable" => Some(
            implicit
                .unstable
                .map_or_else(|| "N/A".to_owned(), |line| line.to_string()),
        ),
        "iojs" => Some(
            implicit
                .iojs
                .map_or_else(|| "N/A".to_owned(), |line| format!("iojs-v{line}")),
        ),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn versions(names: &[&str]) -> Vec<Version> {
        names.iter().map(|name| name.parse().unwrap()).collect()
    }

    fn line(major: u64, minor: u64) -> Option<MajorMinor> {
        Some(MajorMinor { major, minor })
    }

    #[test]
    fn every_modern_release_line_is_stable_and_the_highest_wins() {
        let installed = versions(&["v20.1.0", "v20.10.0", "v18.9.0"]);
        let implicit = derive(&installed);
        assert_eq!(implicit.stable, line(20, 10));
        assert_eq!(implicit.unstable, None);
    }

    #[test]
    fn below_1_0_even_minors_are_stable_and_odd_ones_unstable() {
        let installed = versions(&["v0.10.48", "v0.11.16", "v0.12.18", "v4.2.0"]);
        let implicit = derive(&installed);
        assert_eq!(implicit.stable, line(4, 2));
        assert_eq!(implicit.unstable, line(0, 11));
    }

    #[test]
    fn only_old_versions_leave_the_highest_even_minor_stable() {
        let installed = versions(&["v0.10.48", "v0.11.16", "v0.12.18"]);
        let implicit = derive(&installed);
        assert_eq!(implicit.stable, line(0, 12));
        assert_eq!(implicit.unstable, line(0, 11));
    }

    #[test]
    fn iojs_uses_its_highest_release_line() {
        let installed = versions(&["iojs-v2.5.0", "iojs-v3.0.0", "v20.1.0"]);
        assert_eq!(derive(&installed).iojs, line(3, 0));
    }

    #[test]
    fn nothing_installed_derives_nothing() {
        assert_eq!(derive(&[]), Implicit::default());
    }

    #[test]
    fn highest_in_picks_the_newest_patch_of_one_line() {
        let installed = versions(&["v20.1.0", "v20.10.0", "v20.10.3", "v20.2.0"]);
        let found = highest_in(
            &installed,
            Flavor::Node,
            MajorMinor {
                major: 20,
                minor: 10,
            },
        );
        assert_eq!(found, Some("v20.10.3".parse().unwrap()));
    }

    #[test]
    fn destinations_match_nvm_alias_output() {
        let installed = versions(&["v20.1.0", "v20.10.0", "iojs-v3.0.0"]);
        assert_eq!(destination(&installed, "node"), Some("stable".to_owned()));
        assert_eq!(destination(&installed, "stable"), Some("20.10".to_owned()));
        assert_eq!(destination(&installed, "unstable"), Some("N/A".to_owned()));
        assert_eq!(
            destination(&installed, "iojs"),
            Some("iojs-v3.0".to_owned())
        );
        assert_eq!(destination(&installed, "default"), None);
    }

    #[test]
    fn with_nothing_installed_stable_shows_nothing_and_the_rest_show_na() {
        assert_eq!(destination(&[], "stable"), None);
        assert_eq!(destination(&[], "node"), Some("stable".to_owned()));
        assert_eq!(destination(&[], "iojs"), Some("N/A".to_owned()));
        assert_eq!(destination(&[], "unstable"), Some("N/A".to_owned()));
    }
}
