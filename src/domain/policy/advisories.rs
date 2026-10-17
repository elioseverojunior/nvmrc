//! The Node Security WG vulnerability index: parsing it, and finding which of
//! its advisories affect a version. The index is not ours, so anything in it
//! that cannot be understood is an error, never a silent skip.

use std::collections::BTreeMap;

use nodejs_semver::Range;
use serde::Deserialize;
use serde_json::Value;

use super::document::Severity;
use crate::domain::version::{Flavor, Version};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IndexError {
    #[error("the vulnerability index is not an object of advisories: {0}")]
    NotAnIndex(String),
    #[error("advisory {id} of the vulnerability index is not understood: {reason}")]
    BadEntry { id: String, reason: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Advisory {
    pub id: String,
    pub cves: Vec<String>,
    pub vulnerable: Range,
    pub patched: Option<Range>,
    pub severity: Severity,
    pub environments: Vec<String>,
}

/// One thing a version is affected by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The CVE id, or `advisory <id>` when the advisory has none.
    pub name: String,
    pub severity: Severity,
}

#[derive(Deserialize)]
struct RawEntry {
    #[serde(default)]
    cve: Vec<String>,
    vulnerable: String,
    patched: Option<String>,
    severity: Option<String>,
    #[serde(default, rename = "affectedEnvironments")]
    environments: Vec<String>,
}

pub fn parse_advisories(text: &str) -> Result<Vec<Advisory>, IndexError> {
    let entries: BTreeMap<String, Value> =
        serde_json::from_str(text).map_err(|error| IndexError::NotAnIndex(error.to_string()))?;
    entries
        .into_iter()
        .map(|(id, value)| parse_entry(id, value))
        .collect()
}

fn parse_entry(id: String, value: Value) -> Result<Advisory, IndexError> {
    let bad = |reason: String| IndexError::BadEntry {
        id: id.clone(),
        reason,
    };
    let raw: RawEntry = serde_json::from_value(value).map_err(|error| bad(error.to_string()))?;
    let vulnerable = parse_range(&raw.vulnerable).map_err(bad)?;
    let patched = match raw.patched.as_deref().map(str::trim) {
        None | Some("") => None,
        Some(text) => Some(parse_range(text).map_err(bad)?),
    };
    let severity = parse_severity(raw.severity.as_deref()).map_err(bad)?;
    Ok(Advisory {
        id: id.clone(),
        cves: raw.cve,
        vulnerable,
        patched,
        severity,
        environments: raw.environments,
    })
}

fn parse_range(text: &str) -> Result<Range, String> {
    Range::parse(text.trim()).map_err(|error| format!("range {text:?}: {error}"))
}

/// A severity the index does not give is read as high, never as harmless.
fn parse_severity(text: Option<&str>) -> Result<Severity, String> {
    match text {
        None | Some("unknown") => Ok(Severity::High),
        Some("critical") => Ok(Severity::Critical),
        Some("high") => Ok(Severity::High),
        Some("medium") => Ok(Severity::Medium),
        Some("low") => Ok(Severity::Low),
        Some("none") => Ok(Severity::None),
        Some(other) => Err(format!("severity {other:?}")),
    }
}

/// What the advisories say affects `version`, one finding per CVE id. With no
/// `os`, platform-specific advisories count. io.js versions are never matched.
#[must_use]
pub fn findings(advisories: &[Advisory], version: &Version, os: Option<&str>) -> Vec<Finding> {
    if version.flavor != Flavor::Node {
        return Vec::new();
    }
    let Ok(semver) = format!("{}.{}.{}", version.major, version.minor, version.patch)
        .parse::<nodejs_semver::Version>()
    else {
        return Vec::new();
    };
    advisories
        .iter()
        .filter(|advisory| applies_to_platform(advisory, os) && affects(advisory, &semver))
        .flat_map(findings_of)
        .collect()
}

fn applies_to_platform(advisory: &Advisory, os: Option<&str>) -> bool {
    let Some(os) = os else { return true };
    advisory.environments.is_empty()
        || advisory
            .environments
            .iter()
            .any(|environment| environment == "all" || environment == os)
}

fn affects(advisory: &Advisory, version: &nodejs_semver::Version) -> bool {
    advisory.vulnerable.satisfies(version)
        && !advisory
            .patched
            .as_ref()
            .is_some_and(|patched| patched.satisfies(version))
}

fn findings_of(advisory: &Advisory) -> Vec<Finding> {
    let names = if advisory.cves.is_empty() {
        vec![format!("advisory {}", advisory.id)]
    } else {
        advisory.cves.clone()
    };
    names
        .into_iter()
        .map(|name| Finding {
            name,
            severity: advisory.severity,
        })
        .collect()
}
