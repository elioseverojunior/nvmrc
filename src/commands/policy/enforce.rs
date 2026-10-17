//! The gate a node version passes before it is installed: the merged policy
//! against the version, with the release index consulted only when a rule
//! needs it, and the vulnerability index for every node version.

use std::path::PathBuf;

use super::advisories::cve_violation;
use super::mode::{Mode, resolve_mode};
use crate::commands::remote_index;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::mirror;
use crate::domain::policy::{NodePolicy, ReleaseInfo, Violation};
use crate::domain::version::{Flavor, Version};
use crate::error::{CliError, NvmExitCode, PolicyError};

const SECONDS_PER_DAY: i64 = 86_400;

/// Where the version being checked came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    Nvmrc(PathBuf),
    Argument,
    /// The version already active, when none was asked for.
    Active,
    System,
}

/// The version to check, where it came from and whether the network is off.
#[derive(Debug, Clone)]
pub struct Requested {
    pub version: Version,
    pub origin: Origin,
    pub offline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    Pass,
    Stop(NvmExitCode),
}

/// Checks `requested` against the policy that applies, printing what it
/// found on `transcript`. In an advisory context a finding is a warning and
/// the gate passes.
///
/// # Errors
/// [`CliError::Policy`] when the policy cannot be read or checked, unless the
/// context is advisory.
pub fn enforce_node(
    context: &Context<'_>,
    requested: &Requested,
    transcript: &mut Transcript,
) -> Result<Gate, CliError> {
    match check(context, requested, transcript) {
        Err(error) if context.is_advisory() => {
            let text = error.to_string();
            let text = text.trim_start_matches("nvm: ");
            transcript.err(format!("nvm: warning: {text}"));
            Ok(Gate::Pass)
        }
        Err(error) => Err(error.into()),
        Ok(checked) => {
            let gate = report(context, requested, &checked.violations, transcript);
            checked
                .notes
                .into_iter()
                .for_each(|line| transcript.err(line));
            Ok(gate)
        }
    }
}

/// What the check found: the violations, and the warnings printed after the
/// report so the output reads top-down.
#[derive(Default)]
struct Checked {
    violations: Vec<Violation>,
    notes: Vec<String>,
}

fn check(
    context: &Context<'_>,
    requested: &Requested,
    transcript: &mut Transcript,
) -> Result<Checked, PolicyError> {
    let (mode, warnings) = resolve_mode(context)?;
    warnings.into_iter().for_each(|line| transcript.err(line));
    let Mode::Enforced(policy) = mode else {
        return Ok(Checked::default());
    };
    let mut checked = Checked {
        violations: node_violations(context, requested, &policy.node)?,
        notes: Vec::new(),
    };
    match cve_violation(context, requested, policy.cve, &mut checked.notes) {
        Ok(found) => checked.violations.extend(found),
        // The node rules already decide the gate; the index is a warning.
        Err(error) if !checked.violations.is_empty() => {
            checked.notes.push(unknown_vulnerabilities(&error));
        }
        Err(error) => return Err(error),
    }
    Ok(checked)
}

fn unknown_vulnerabilities(error: &PolicyError) -> String {
    let text = error.to_string();
    let reason = text.trim_start_matches("nvm: cannot check the policy: ");
    format!("nvm: warning: cannot check known vulnerabilities: {reason}")
}

fn node_violations(
    context: &Context<'_>,
    requested: &Requested,
    policy: &NodePolicy,
) -> Result<Vec<Violation>, PolicyError> {
    let release = release_info(context, requested, policy)?;
    let today = context.clock().unix_seconds().div_euclid(SECONDS_PER_DAY);
    policy
        .violations(&requested.version, release.as_ref(), today)
        .map_err(|_| PolicyError::ReleaseInfoUnavailable(index_location(context)))
}

/// The index entry of the version, fetched only for rules that need it.
fn release_info(
    context: &Context<'_>,
    requested: &Requested,
    policy: &NodePolicy,
) -> Result<Option<ReleaseInfo>, PolicyError> {
    if !policy.needs_release_info() || requested.version.flavor != Flavor::Node {
        return Ok(None);
    }
    if requested.offline {
        return Err(PolicyError::ReleaseInfoUnavailable("offline".to_owned()));
    }
    let unavailable = || PolicyError::ReleaseInfoUnavailable(index_location(context));
    let fetching = context.clone().with_http(context.policy_http());
    let fetched = remote_index::fetch(&fetching, Flavor::Node).map_err(|_| unavailable())?;
    fetched
        .releases
        .unwrap_or_default()
        .into_iter()
        .find(|release| release.version == requested.version)
        .map(|release| ReleaseInfo {
            lts: release.lts,
            published_day: release.published_day,
        })
        .map(Some)
        .ok_or_else(unavailable)
}

fn index_location(context: &Context<'_>) -> String {
    mirror::from_env(context.env, Flavor::Node).map_or_else(
        |_| "the release index".to_owned(),
        |mirror| mirror.index_url(),
    )
}

fn report(
    context: &Context<'_>,
    requested: &Requested,
    violations: &[Violation],
    transcript: &mut Transcript,
) -> Gate {
    if violations.is_empty() {
        return Gate::Pass;
    }
    let prefix = if context.is_advisory() {
        "nvm: warning: node"
    } else {
        "nvm: node"
    };
    transcript.err(format!(
        "{prefix} {} {} is not allowed by policy:",
        requested.version,
        origin_text(&requested.origin)
    ));
    for violation in violations {
        transcript.err(format!("  - {violation}"));
    }
    transcript.err(fix_text(&requested.origin));
    if context.is_advisory() {
        Gate::Pass
    } else {
        Gate::Stop(NvmExitCode::BelowVersionFloor)
    }
}

fn origin_text(origin: &Origin) -> String {
    match origin {
        Origin::Nvmrc(path) => format!("(from {})", path.display()),
        Origin::Argument => "(from the command line)".to_owned(),
        Origin::Active => "(the active version)".to_owned(),
        Origin::System => "(system)".to_owned(),
    }
}

fn fix_text(origin: &Origin) -> String {
    match origin {
        Origin::Nvmrc(path) => format!("Raise {} to an allowed version.", path.display()),
        _ => "Use an allowed version.".to_owned(),
    }
}
