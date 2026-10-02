//! Node and io.js versions, and partial patterns such as `20` or `v20.1`.

use std::fmt;
use std::str::FromStr;

use crate::error::VersionError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Flavor {
    Node,
    IoJs,
}

/// A fully specified version. Ordering is by flavor, then numerically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version {
    pub flavor: Flavor,
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

/// A version with optional minor and patch, used to select installed versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionPattern {
    pub flavor: Flavor,
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
}

fn parse_parts(input: &str) -> Result<(Flavor, Vec<u64>), VersionError> {
    let invalid = || VersionError::Invalid(input.to_owned());
    let (flavor, rest) = match input.strip_prefix("iojs-") {
        Some(rest) => (Flavor::IoJs, rest),
        None => (Flavor::Node, input),
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
                flavor,
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
        write!(
            formatter,
            "{prefix}v{}.{}.{}",
            self.major, self.minor, self.patch
        )
    }
}

impl Version {
    /// The numeric part, ignoring flavor (used for floor comparison).
    #[must_use]
    pub fn triple(&self) -> (u64, u64, u64) {
        (self.major, self.minor, self.patch)
    }
}

impl VersionPattern {
    #[must_use]
    pub fn matches(&self, version: &Version) -> bool {
        version.flavor == self.flavor
            && version.major == self.major
            && self.minor.is_none_or(|minor| minor == version.minor)
            && self.patch.is_none_or(|patch| patch == version.patch)
    }

    /// The lowest version the pattern covers: `24` is `v24.0.0`.
    #[must_use]
    pub fn lowest(&self) -> Version {
        Version {
            flavor: self.flavor,
            major: self.major,
            minor: self.minor.unwrap_or(0),
            patch: self.patch.unwrap_or(0),
        }
    }

    #[must_use]
    pub fn highest_match<'a>(&self, installed: &'a [Version]) -> Option<&'a Version> {
        installed
            .iter()
            .filter(|version| self.matches(version))
            .max()
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
    fn pattern_does_not_match_across_flavors() {
        let pattern: VersionPattern = "3".parse().unwrap();
        assert!(!pattern.matches(&version("iojs-v3.0.0")));
    }
}
