//! The minimum installable version (`NVM_MIN_VERSION`, `$NVM_DIR/min-version`).

use crate::domain::version::{Version, VersionPattern};
use crate::error::FloorError;

/// A valid version floor. An invalid one can never be constructed, so a bad
/// floor can never let an install through unchecked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionFloor(Version);

impl VersionFloor {
    /// # Errors
    /// Returns [`FloorError::Invalid`] when `raw` is not a version.
    pub fn parse(raw: &str) -> Result<Self, FloorError> {
        let pattern: VersionPattern = raw
            .parse()
            .map_err(|_| FloorError::Invalid(raw.to_owned()))?;
        Ok(Self(pattern.lowest()))
    }

    /// The environment value wins over the first line of the file.
    ///
    /// # Errors
    /// Returns [`FloorError::Invalid`] when the chosen value is not a version.
    pub fn from_sources(
        env_value: Option<&str>,
        file_contents: Option<&str>,
    ) -> Result<Option<Self>, FloorError> {
        let first_line = file_contents.and_then(|contents| contents.lines().next());
        env_value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| first_line.map(str::trim).filter(|value| !value.is_empty()))
            .map(Self::parse)
            .transpose()
    }

    /// # Errors
    /// Returns [`FloorError::Below`] when `candidate` is lower than the floor.
    pub fn check(&self, candidate: &Version) -> Result<(), FloorError> {
        if candidate.triple() >= self.0.triple() {
            return Ok(());
        }
        Err(FloorError::Below {
            version: candidate.to_string(),
            floor: Version {
                flavor: candidate.flavor,
                ..self.0
            }
            .to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::VersionFloor;
    use crate::domain::version::Version;

    fn version(text: &str) -> Version {
        text.parse().expect("valid version")
    }

    fn floor(text: &str) -> VersionFloor {
        VersionFloor::parse(text).expect("valid floor")
    }

    #[test]
    fn a_version_below_a_major_only_floor_is_refused() {
        let error = floor("24").check(&version("v20.0.0")).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("v20.0.0") && message.contains("v24.0.0"),
            "{message}"
        );
    }

    #[test]
    fn the_floor_may_be_written_with_a_leading_v() {
        assert!(floor("v24").check(&version("v20.0.0")).is_err());
    }

    #[test]
    fn equal_and_higher_versions_are_allowed() {
        assert!(floor("24").check(&version("v24.0.0")).is_ok());
        assert!(floor("24").check(&version("v25.1.0")).is_ok());
    }

    #[test]
    fn a_full_version_floor_is_honoured_to_the_patch_level() {
        assert!(floor("v24.1.0").check(&version("v24.0.0")).is_err());
    }

    #[test]
    fn an_invalid_floor_is_an_error_naming_the_value() {
        let error = VersionFloor::parse("not-a-version").unwrap_err();
        assert!(error.to_string().contains("not-a-version"));
    }

    #[test]
    fn no_sources_means_no_floor() {
        assert_eq!(VersionFloor::from_sources(None, None), Ok(None));
        assert_eq!(VersionFloor::from_sources(Some(""), Some("")), Ok(None));
    }

    #[test]
    fn the_file_is_used_when_the_env_var_is_unset() {
        let found = VersionFloor::from_sources(None, Some("24\nignored")).unwrap();
        assert_eq!(found, Some(floor("24")));
    }

    #[test]
    fn the_env_var_overrides_the_file() {
        let found = VersionFloor::from_sources(Some("18"), Some("24")).unwrap();
        assert_eq!(found, Some(floor("18")));
    }

    #[test]
    fn an_invalid_value_is_an_error_not_a_silent_pass() {
        assert!(VersionFloor::from_sources(Some("nope"), Some("24")).is_err());
        assert!(VersionFloor::from_sources(None, Some("nope")).is_err());
    }
}
