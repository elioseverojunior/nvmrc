//! The gate a node version passes before it is installed: the merged policy
//! against the version, with the release index consulted only when a rule
//! needs it, the vulnerability index for every node version, and the rules
//! of npm, yarn and pnpm for each of them in effect.

use std::path::PathBuf;

use super::advisories::cve_violation;
use super::mode::{Mode, resolve_mode};
use super::report::{Checked, report};
use super::tools::tool_checks;
use crate::commands::project_pins::ProjectPins;
use crate::commands::remote_index;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::index::Release;
use crate::domain::mirror;
use crate::domain::policy::{ReleaseInfo, ToolPolicy, Violation};
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

/// The version to check, where it came from, whether the network is off,
/// and the pins of the project.
#[derive(Debug, Clone)]
pub struct Requested {
    pub version: Version,
    pub origin: Origin,
    pub offline: bool,
    pub pins: ProjectPins,
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
            let gate = report(context, requested, &checked, transcript);
            checked
                .notes
                .into_iter()
                .for_each(|line| transcript.err(line));
            Ok(gate)
        }
    }
}

/// What could not be known, named as in `cannot check <what>`.
type Unknown = (&'static str, PolicyError);

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
        node: node_violations(context, requested, &policy.node)?,
        ..Checked::default()
    };
    let mut unknown: Vec<Unknown> = Vec::new();
    match cve_violation(context, requested, policy.cve.clone(), &mut checked.notes) {
        Ok(found) => checked.node.extend(found),
        Err(error) => unknown.push(("known vulnerabilities", error)),
    }
    for outcome in tool_checks(context, requested, &policy) {
        match outcome {
            Ok(found) => checked.tools.extend(found),
            Err((tool, error)) => unknown.push((tool.name(), error)),
        }
    }
    settle(checked, unknown)
}

/// What could not be known fails the check when nothing else does; when a
/// violation already decides the gate, it is a warning.
fn settle(mut checked: Checked, unknown: Vec<Unknown>) -> Result<Checked, PolicyError> {
    if checked.is_clean() {
        return match unknown.into_iter().next() {
            Some((_, error)) => Err(error),
            None => Ok(checked),
        };
    }
    let notes = unknown
        .iter()
        .map(|(what, error)| unknown_note(what, error));
    checked.notes.extend(notes);
    Ok(checked)
}

fn unknown_note(what: &str, error: &PolicyError) -> String {
    let text = error.to_string();
    let reason = text.trim_start_matches("nvm: cannot check the policy: ");
    format!("nvm: warning: cannot check {what}: {reason}")
}

/// Days since 1970-01-01, by the clock of `context`.
pub(super) fn today(context: &Context<'_>) -> i64 {
    context.clock().unix_seconds().div_euclid(SECONDS_PER_DAY)
}

fn node_violations(
    context: &Context<'_>,
    requested: &Requested,
    policy: &ToolPolicy,
) -> Result<Vec<Violation>, PolicyError> {
    let release = release_info(context, requested, policy)?;
    policy
        .violations(&requested.version, release.as_ref(), today(context))
        .map_err(|_| PolicyError::ReleaseInfoUnavailable(index_location(context)))
}

/// The index entry of the version, fetched only for rules that need it.
fn release_info(
    context: &Context<'_>,
    requested: &Requested,
    policy: &ToolPolicy,
) -> Result<Option<ReleaseInfo>, PolicyError> {
    if !policy.needs_release_info() || requested.version.flavor != Flavor::Node {
        return Ok(None);
    }
    let release = indexed_release(context, requested)?;
    Ok(Some(ReleaseInfo {
        lts: release.lts,
        published_day: release.published_day,
    }))
}

/// The entry of the requested version in the release index, read with the
/// policy client.
///
/// # Errors
/// [`PolicyError::ReleaseInfoUnavailable`] offline, when the index cannot be
/// read or when it does not list the version.
pub(super) fn indexed_release(
    context: &Context<'_>,
    requested: &Requested,
) -> Result<Release, PolicyError> {
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
        .ok_or_else(unavailable)
}

pub(crate) fn index_location(context: &Context<'_>) -> String {
    mirror::from_env(context.env, Flavor::Node).map_or_else(
        |_| "the release index".to_owned(),
        |mirror| mirror.index_url(),
    )
}
