//! The CVE check: the vulnerability index read once, and what it says about a
//! version split by the severity limit.

use super::enforce::Requested;
use super::load::{is_https, read_location_with};
use crate::context::Context;
use crate::domain::policy::{
    Advisory, CvePolicy, DEFAULT_INDEX, Finding, Severity, Sourced, Violation, finding_list,
    findings, parse_advisories,
};
use crate::domain::version::{Flavor, Version};
use crate::error::PolicyError;
use crate::ports::Http;

/// The CVE check of the gate: the violation for findings at or above the
/// limit, if any. Findings below it are a warning added to `notes`, whatever
/// the gate decides. io.js versions are not checked, and nothing is fetched.
///
/// # Errors
/// [`PolicyError::ReleaseInfoUnavailable`] for a remote index while offline,
/// before any request; the errors of [`load_advisories`].
pub(super) fn cve_violation(
    context: &Context<'_>,
    requested: &Requested,
    cve: CvePolicy,
    notes: &mut Vec<String>,
) -> Result<Option<Violation>, PolicyError> {
    if requested.version.flavor != Flavor::Node {
        return Ok(None);
    }
    if requested.offline && is_https(&cve.index.value) {
        return Err(PolicyError::ReleaseInfoUnavailable("offline".to_owned()));
    }
    let advisories = load_advisories(context, &cve)?;
    let (violation, below) = vulnerable(context, &advisories, &requested.version, &cve.limit);
    notes.extend(below_limit_warning(
        &requested.version,
        &cve.limit.value,
        &below,
    ));
    Ok(violation)
}

/// The warning for the findings of node `version` below the `limit`; none
/// when there are none.
#[must_use]
pub(super) fn below_limit_warning(
    version: &Version,
    limit: &Severity,
    below: &[Finding],
) -> Option<String> {
    (!below.is_empty()).then(|| {
        format!(
            "nvm: warning: node {version} has known vulnerabilities below the limit {limit}: {}",
            finding_list(below)
        )
    })
}

/// What `advisories` say about `version`, split by the limit: the violation
/// for the findings at or above it, and the findings below it (every finding
/// when the limit is `none`).
pub(super) fn vulnerable(
    context: &Context<'_>,
    advisories: &[Advisory],
    version: &Version,
    limit: &Sourced<Severity>,
) -> (Option<Violation>, Vec<Finding>) {
    let os = context.platform().map(|platform| platform.os.slug());
    let fails = |finding: &Finding| {
        limit.value != Severity::None && finding.severity.rank() >= limit.value.rank()
    };
    let (failing, below): (Vec<_>, Vec<_>) = findings(advisories, version, os)
        .into_iter()
        .partition(fails);
    let violation = (!failing.is_empty()).then(|| Violation::Vulnerable {
        findings: failing,
        limit: limit.clone(),
    });
    (violation, below)
}

/// Reads and parses the index `cve` names.
///
/// # Errors
/// The errors of [`read_location_with`], and [`PolicyError::IndexInvalid`]
/// for an index that cannot be understood.
pub(super) fn load_advisories(
    context: &Context<'_>,
    cve: &CvePolicy,
) -> Result<Vec<Advisory>, PolicyError> {
    let location = &cve.index.value;
    let text = read_location_with(context, index_client(context, location), location)?;
    parse_advisories(&text).map_err(|error| PolicyError::IndexInvalid {
        location: location.clone(),
        reason: error.to_string(),
    })
}

/// The built-in index is a third party's, so it is read without
/// `NVM_AUTH_HEADER`; an index a policy names is read like the policy.
fn index_client<'a>(context: &Context<'a>, location: &str) -> &'a dyn Http {
    if location == DEFAULT_INDEX {
        context.third_party_http()
    } else {
        context.policy_http()
    }
}
