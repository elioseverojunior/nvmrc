//! Node and io.js versions, and partial patterns such as `20` or `v20.1`.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use crate::error::VersionError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Flavor {
    Node,
    IoJs,
}

/// A fully specified version. Ordering is numeric, with the flavor only
/// breaking ties, so `iojs-v3.0.0` sorts between `v2.9.0` and `v4.0.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Version {
    pub flavor: Flavor,
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

/// A version with optional minor and patch, used to select installed versions.
/// Without an `iojs-` prefix the flavor is `None` and the pattern matches both
/// flavors, as in `nvm.sh` (`nvm ls 3` lists `iojs-v3.0.0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionPattern {
    pub flavor: Option<Flavor>,
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
}

fn parse_parts(input: &str) -> Result<(Option<Flavor>, Vec<u64>), VersionError> {
    let invalid = || VersionError::Invalid(input.to_owned());
    let (flavor, rest) = match input.strip_prefix("iojs-") {
        Some(rest) => (Some(Flavor::IoJs), rest),
        None => (None, input),
    };
    let rest = rest.strip_prefix('v').unwrap_or(rest);
    let parts = rest
        .split('.')
        .map(|part| {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(invalid());
            }
            part.parse::<u64>().map_err(|_| invalid())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() > 3 {
        return Err(invalid());
    }
    Ok((flavor, parts))
}

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match parse_parts(input)? {
            (flavor, parts) if parts.len() == 3 => Ok(Self {
                flavor: flavor.unwrap_or(Flavor::Node),
                major: parts[0],
                minor: parts[1],
                patch: parts[2],
            }),
            _ => Err(VersionError::Invalid(input.to_owned())),
        }
    }
}

impl FromStr for VersionPattern {
    type Err = VersionError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (flavor, parts) = parse_parts(input)?;
        Ok(Self {
            flavor,
            major: parts[0],
            minor: parts.get(1).copied(),
            patch: parts.get(2).copied(),
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self.flavor {
            Flavor::Node => "",
            Flavor::IoJs => "iojs-",
        };
        write!(formatter, "{prefix}{}", self.directory_name())
    }
}

impl Flavor {
    /// The directory under `$NVM_DIR/versions` holding this flavor.
    #[must_use]
    pub fn versions_directory(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::IoJs => "io.js",
        }
    }
}

impl Version {
    /// The on-disk directory name, without the `iojs-` prefix: `v3.0.0`.
    #[must_use]
    pub fn directory_name(&self) -> String {
        format!("v{}.{}.{}", self.major, self.minor, self.patch)
    }

    /// The numeric part, ignoring flavor (used for floor comparison).
    #[must_use]
    pub fn triple(&self) -> (u64, u64, u64) {
        (self.major, self.minor, self.patch)
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.triple()
            .cmp(&other.triple())
            .then(self.flavor.cmp(&other.flavor))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl VersionPattern {
    #[must_use]
    pub fn matches(&self, version: &Version) -> bool {
        self.flavor.is_none_or(|flavor| flavor == version.flavor)
            && version.major == self.major
            && self.minor.is_none_or(|minor| minor == version.minor)
            && self.patch.is_none_or(|patch| patch == version.patch)
    }

    /// The lowest version the pattern covers: `24` is `v24.0.0`.
    #[must_use]
    pub fn lowest(&self) -> Version {
        Version {
            flavor: self.flavor.unwrap_or(Flavor::Node),
            major: self.major,
            minor: self.minor.unwrap_or(0),
            patch: self.patch.unwrap_or(0),
        }
    }

    /// The highest installed version the pattern matches. Unlike [`Ord`], a
    /// Node version wins over an io.js one with the same number, as in
    /// `nvm.sh` (`nvm version 3.0` is `v3.0.0` when both are installed).
    #[must_use]
    pub fn highest_match<'a>(&self, installed: &'a [Version]) -> Option<&'a Version> {
        installed
            .iter()
            .filter(|version| self.matches(version))
            .max_by(|left, right| {
                let node_first = right.flavor.cmp(&left.flavor);
                left.triple().cmp(&right.triple()).then(node_first)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        text.parse().expect("valid version")
    }

    #[test]
    fn parses_with_and_without_the_v_prefix() {
        assert_eq!(version("v20.1.2"), version("20.1.2"));
    }

    #[test]
    fn parses_iojs_versions() {
        let parsed = version("iojs-v3.0.0");
        assert_eq!(parsed.flavor, Flavor::IoJs);
        assert_eq!(parsed.to_string(), "iojs-v3.0.0");
    }

    #[test]
    fn directory_name_drops_the_iojs_prefix() {
        assert_eq!(version("iojs-v3.0.0").directory_name(), "v3.0.0");
        assert_eq!(version("20.1.2").directory_name(), "v20.1.2");
    }

    #[test]
    fn flavors_name_their_versions_directory() {
        assert_eq!(Flavor::Node.versions_directory(), "node");
        assert_eq!(Flavor::IoJs.versions_directory(), "io.js");
    }

    #[test]
    fn displays_node_versions_with_the_v_prefix() {
        assert_eq!(version("20.1.2").to_string(), "v20.1.2");
    }

    #[test]
    fn rejects_malformed_versions() {
        for bad in [
            "", "v", "1.2", "1.2.3.4", "a.b.c", "1..3", "-1.0.0", "v1.0.x",
        ] {
            assert!(bad.parse::<Version>().is_err(), "{bad:?} should be invalid");
        }
    }

    #[test]
    fn orders_numerically_not_lexicographically() {
        assert!(version("v10.0.0") > version("v9.0.0"));
        assert!(version("v1.10.0") > version("v1.9.0"));
    }

    #[test]
    fn pattern_accepts_one_to_three_parts() {
        let major: VersionPattern = "20".parse().unwrap();
        assert_eq!((major.minor, major.patch), (None, None));
        let minor: VersionPattern = "v20.1".parse().unwrap();
        assert_eq!((minor.minor, minor.patch), (Some(1), None));
    }

    #[test]
    fn pattern_lowest_fills_missing_parts_with_zero() {
        let pattern: VersionPattern = "24".parse().unwrap();
        assert_eq!(pattern.lowest(), version("v24.0.0"));
    }

    #[test]
    fn pattern_picks_the_highest_matching_installed_version() {
        let installed = [version("v20.1.0"), version("v20.10.0"), version("v18.9.0")];
        let pattern: VersionPattern = "20".parse().unwrap();
        assert_eq!(pattern.highest_match(&installed), Some(&installed[1]));
        let none: VersionPattern = "16".parse().unwrap();
        assert_eq!(none.highest_match(&installed), None);
    }

    #[test]
    fn a_plain_pattern_matches_both_flavors() {
        let pattern: VersionPattern = "3".parse().unwrap();
        assert!(pattern.matches(&version("iojs-v3.0.0")));
        assert!(pattern.matches(&version("v3.1.0")));
    }

    #[test]
    fn an_iojs_pattern_only_matches_iojs() {
        let pattern: VersionPattern = "iojs-3".parse().unwrap();
        assert!(pattern.matches(&version("iojs-v3.0.0")));
        assert!(!pattern.matches(&version("v3.1.0")));
        assert_eq!(pattern.flavor, Some(Flavor::IoJs));
    }

    #[test]
    fn iojs_sorts_among_node_versions_by_number() {
        let mut versions = [version("v4.0.0"), version("iojs-v3.0.0"), version("v2.9.0")];
        versions.sort();
        let sorted: Vec<String> = versions.iter().map(ToString::to_string).collect();
        assert_eq!(sorted, ["v2.9.0", "iojs-v3.0.0", "v4.0.0"]);
    }

    #[test]
    fn the_highest_match_crosses_flavors_by_number() {
        let installed = [version("v2.9.0"), version("iojs-v3.0.0")];
        let pattern: VersionPattern = "3".parse().unwrap();
        assert_eq!(pattern.highest_match(&installed), Some(&installed[1]));
    }

    #[test]
    fn the_highest_match_prefers_node_on_equal_numbers() {
        let installed = [version("iojs-v3.0.0"), version("v3.0.0")];
        for text in ["3", "3.0", "3.0.0"] {
            let pattern: VersionPattern = text.parse().unwrap();
            assert_eq!(pattern.highest_match(&installed), Some(&installed[1]));
        }
    }

    #[test]
    fn equal_numbers_are_ordered_node_before_iojs() {
        assert!(version("v3.0.0") < version("iojs-v3.0.0"));
    }
}
