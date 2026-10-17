//! The verdict: which rules a node version breaks.

use std::fmt;

use super::rules::NodePolicy;
use super::source::{Source, Sourced};
use crate::domain::version::{Flavor, Version};

/// What the index says about a release, for the rules that need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseInfo {
    pub lts: Option<String>,
    pub published_day: Option<i64>,
}

/// The version is not in the index although a rule needs its release data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingReleaseInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    BelowMinimum(Sourced<Version>),
    OutsideRange(Sourced<String>),
    Denied(Source),
    NotLts(Source),
    TooRecent {
        age_days: i64,
        required: Sourced<u32>,
    },
    NotNode,
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BelowMinimum(rule) => write!(
                formatter,
                "below the minimum {} (from {})",
                rule.value, rule.source
            ),
            Self::OutsideRange(rule) => write!(
                formatter,
                "outside the allowed range {} (from {})",
                rule.value, rule.source
            ),
            Self::Denied(source) => write!(formatter, "denied (from {source})"),
            Self::NotLts(source) => write!(formatter, "not an LTS release (from {source})"),
            Self::TooRecent { age_days, required } => write!(
                formatter,
                "published {age_days} days ago; at least {} required (from {})",
                required.value, required.source
            ),
            Self::NotNode => formatter.write_str("io.js is not covered by policy"),
        }
    }
}

impl NodePolicy {
    /// Whether judging a version needs its LTS and release-date data.
    #[must_use]
    pub fn needs_release_info(&self) -> bool {
        self.lts_only.is_some() || self.minimum_release_age_days.is_some()
    }

    /// Every rule `version` breaks, in the order minimum, range, deny, LTS,
    /// age. An io.js version breaks only [`Violation::NotNode`].
    ///
    /// # Errors
    /// Returns [`MissingReleaseInfo`] when a rule needs release data that
    /// `release` does not supply.
    pub fn violations(
        &self,
        version: &Version,
        release: Option<&ReleaseInfo>,
        today_day: i64,
    ) -> Result<Vec<Violation>, MissingReleaseInfo> {
        if version.flavor == Flavor::IoJs {
            return Ok(vec![Violation::NotNode]);
        }
        let mut found = self.version_violations(version);
        if self.needs_release_info() {
            found.extend(self.release_violations(release.ok_or(MissingReleaseInfo)?, today_day)?);
        }
        Ok(found)
    }

    fn version_violations(&self, version: &Version) -> Vec<Violation> {
        let mut found = Vec::new();
        if let Some(rule) = self.minimum.as_ref().filter(|rule| *version < rule.value) {
            found.push(Violation::BelowMinimum(rule.clone()));
        }
        let semver = nodejs_semver::Version::parse(format!(
            "{}.{}.{}",
            version.major, version.minor, version.patch
        ));
        for rule in &self.allow {
            let inside = semver
                .as_ref()
                .is_ok_and(|parsed| rule.value.0.satisfies(parsed));
            if !inside {
                found.push(Violation::OutsideRange(Sourced {
                    value: rule.value.1.clone(),
                    source: rule.source.clone(),
                }));
            }
        }
        found.extend(
            self.deny
                .iter()
                .filter(|rule| rule.value == *version)
                .map(|rule| Violation::Denied(rule.source.clone())),
        );
        found
    }

    fn release_violations(
        &self,
        release: &ReleaseInfo,
        today_day: i64,
    ) -> Result<Vec<Violation>, MissingReleaseInfo> {
        let mut found = Vec::new();
        if let Some(source) = self.lts_only.as_ref().filter(|_| release.lts.is_none()) {
            found.push(Violation::NotLts(source.clone()));
        }
        if let Some(required) = &self.minimum_release_age_days {
            let age_days = today_day - release.published_day.ok_or(MissingReleaseInfo)?;
            if age_days < i64::from(required.value) {
                found.push(Violation::TooRecent {
                    age_days,
                    required: required.clone(),
                });
            }
        }
        Ok(found)
    }
}
