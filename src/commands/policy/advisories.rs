//! The CVE check: the vulnerability index read once, and what it says about a
//! version split by the severity limit.

use super::enforce::Requested;
use super::load::read_location;
use crate::context::Context;
use crate::domain::policy::{
    CvePolicy, Finding, Severity, Violation, finding_list, findings, parse_advisories,
};
use crate::domain::version::Flavor;
use crate::error::PolicyError;

/// What the index says about a version, split by the limit.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Judgement {
    /// At or above the limit: the version is not allowed.
    pub failing: Vec<Finding>,
    /// Below the limit, or every finding when the limit is `none`: a warning.
    pub below: Vec<Finding>,
}

/// The CVE check of the gate: the violation for findings at or above the
/// limit, if any. Findings below it are a warning added to `notes`, whatever
/// the gate decides. io.js versions are not checked, and nothing is fetched.
///
/// # Errors
/// As [`judge`].
pub(super) fn cve_violation(
    context: &Context<'_>,
    requested: &Requested,
    cve: CvePolicy,
    notes: &mut Vec<String>,
) -> Result<Option<Violation>, PolicyError> {
    if requested.version.flavor != Flavor::Node {
        return Ok(None);
    }
    let judgement = judge(context, requested, &cve)?;
    if !judgement.below.is_empty() {
        notes.push(format!(
            "nvm: warning: node {} has known vulnerabilities below the limit {}: {}",
            requested.version,
            cve.limit.value,
            finding_list(&judgement.below)
        ));
    }
    if judgement.failing.is_empty() {
        return Ok(None);
    }
    Ok(Some(Violation::Vulnerable {
        findings: judgement.failing,
        limit: cve.limit,
    }))
}

/// Reads the index named by `cve` and judges `requested` against it.
///
/// # Errors
/// [`PolicyError::ReleaseInfoUnavailable`] for a remote index while offline,
/// before any request; the errors of [`read_location`]; and
/// [`PolicyError::IndexInvalid`] for an index that cannot be understood.
pub(super) fn judge(
    context: &Context<'_>,
    requested: &Requested,
    cve: &CvePolicy,
) -> Result<Judgement, PolicyError> {
    let location = &cve.index.value;
    if requested.offline && location.starts_with("https://") {
        return Err(PolicyError::ReleaseInfoUnavailable("offline".to_owned()));
    }
    let text = read_location(context, location)?;
    let advisories = parse_advisories(&text).map_err(|error| PolicyError::IndexInvalid {
        location: location.clone(),
        reason: error.to_string(),
    })?;
    let os = context.platform().map(|platform| platform.os.slug());
    let found = findings(&advisories, &requested.version, os);
    Ok(split(found, cve.limit.value))
}

/// A finding fails when its severity reaches the limit; `none` fails nothing.
fn split(found: Vec<Finding>, limit: Severity) -> Judgement {
    let fails =
        |finding: &Finding| limit != Severity::None && finding.severity.rank() >= limit.rank();
    let (failing, below) = found.into_iter().partition(fails);
    Judgement { failing, below }
}
