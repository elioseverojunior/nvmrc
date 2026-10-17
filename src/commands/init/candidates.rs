//! The releases `nvm init` can pick from, and which of them the policy allows.

use crate::commands::package_managers::registry::fetch_packument;
use crate::commands::policy::enforce::index_location;
use crate::commands::policy::judge::Judge;
use crate::commands::remote_index;
use crate::context::Context;
use crate::domain::policy::{ReleaseInfo, Violation};
use crate::domain::registry::published_versions;
use crate::domain::tool::Tool;
use crate::domain::version::{Flavor, Version};
use crate::error::{CliError, PolicyError};

/// A release that exists, with what the policy needs to judge it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub version: Version,
    pub release: ReleaseInfo,
}

/// What the policy says of one requested version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exact {
    Allowed,
    NotPublished,
    NotAllowed(Vec<Violation>),
}

/// The releases of the node index, newest version first.
///
/// # Errors
/// [`PolicyError::ReleaseInfoUnavailable`] when the index cannot be had;
/// [`CliError::NvmDirUnresolved`] when there is no nvm directory.
pub fn node_candidates(context: &Context<'_>) -> Result<Vec<Candidate>, CliError> {
    let unavailable = || PolicyError::ReleaseInfoUnavailable(index_location(context));
    let fetched = remote_index::fetch(context, Flavor::Node)?;
    let releases = fetched.releases.filter(|found| !found.is_empty());
    let mut candidates: Vec<Candidate> = releases
        .ok_or_else(unavailable)?
        .into_iter()
        .map(|release| Candidate {
            version: release.version,
            release: ReleaseInfo {
                lts: release.lts,
                published_day: release.published_day,
            },
        })
        .collect();
    // The index is ordered by date, so a backport can precede a newer line.
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.version));
    Ok(candidates)
}

/// The stable releases of the registry package that ships `tool` at
/// `package_of` (yarn `2.0.0` for the newest search, the exact version
/// otherwise), newest first.
///
/// # Errors
/// [`PolicyError::ReleaseInfoUnavailable`] when the registry cannot be read or
/// its packument is unusable.
pub fn tool_candidates(
    context: &Context<'_>,
    tool: Tool,
    package_of: &Version,
) -> Result<Vec<Candidate>, PolicyError> {
    let (url, text) = fetch_packument(context, tool, package_of)
        .map_err(|error| PolicyError::ReleaseInfoUnavailable(error.location))?;
    let published = published_versions(&text)
        .map_err(|error| PolicyError::ReleaseInfoUnavailable(format!("{url}: {error}")))?;
    Ok(published
        .into_iter()
        .rev()
        .map(|(version, published_day)| Candidate {
            version,
            release: ReleaseInfo {
                lts: None,
                published_day,
            },
        })
        .collect())
}

/// The first candidate with no violation; for node, among those with an LTS
/// name only.
///
/// # Errors
/// [`PolicyError`] when the policy cannot judge a candidate.
pub fn newest_allowed(
    context: &Context<'_>,
    judge: &mut Judge,
    tool: Tool,
    candidates: &[Candidate],
) -> Result<Option<Version>, PolicyError> {
    for candidate in candidates {
        if tool == Tool::Node && candidate.release.lts.is_none() {
            continue;
        }
        let found = judge.violations(context, tool, &candidate.version, &candidate.release)?;
        if found.is_empty() {
            return Ok(Some(candidate.version));
        }
    }
    Ok(None)
}

/// Whether `version` is published and allowed.
///
/// # Errors
/// [`PolicyError`] when the policy cannot judge the version.
pub fn exact(
    context: &Context<'_>,
    judge: &mut Judge,
    tool: Tool,
    candidates: &[Candidate],
    version: &Version,
) -> Result<Exact, PolicyError> {
    let Some(candidate) = candidates
        .iter()
        .find(|candidate| candidate.version == *version)
    else {
        return Ok(Exact::NotPublished);
    };
    let found = judge.violations(context, tool, version, &candidate.release)?;
    Ok(if found.is_empty() {
        Exact::Allowed
    } else {
        Exact::NotAllowed(found)
    })
}

#[cfg(test)]
#[path = "candidates_tests.rs"]
mod tests;
