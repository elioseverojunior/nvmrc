//! The rules of npm, yarn and pnpm against the versions in effect: each
//! pinned one, and the npm bundled with node when npm has a rule but no pin.
//! Release dates come from the registry, only for an age rule.

use std::path::PathBuf;

use super::bundled::bundled_npm;
use super::enforce::{Requested, today};
use super::mode::EnforcedPolicy;
use crate::commands::package_managers::registry::{fetch_release_day, registry_base};
use crate::context::Context;
use crate::domain::policy::{ReleaseInfo, ToolPolicy, Violation};
use crate::domain::registry::packument_url;
use crate::domain::tool::Tool;
use crate::domain::version::Version;
use crate::error::PolicyError;

/// Where the version of a package manager came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOrigin {
    Nvmrc(PathBuf),
    /// The npm bundled with this node version.
    Bundled(Version),
}

/// A package manager version in effect and the rules it breaks.
pub type ToolFinding = (Tool, Version, ToolOrigin, Vec<Violation>);

/// What checking one tool gave: what it breaks (`None`: nothing), or why it
/// cannot be known.
pub(super) type ToolOutcome = Result<Option<ToolFinding>, (Tool, PolicyError)>;

/// The package managers in effect that break a rule, npm, yarn and pnpm in
/// that order.
///
/// # Errors
/// The first [`PolicyError`] of a tool whose version or release date cannot
/// be known.
pub fn tool_violations(
    context: &Context<'_>,
    requested: &Requested,
    policy: &EnforcedPolicy,
) -> Result<Vec<ToolFinding>, PolicyError> {
    tool_checks(context, requested, policy)
        .into_iter()
        .filter_map(Result::transpose)
        .collect::<Result<_, _>>()
        .map_err(|(_, error)| error)
}

/// Each package manager with a rule, checked on its own, so one that cannot
/// be known leaves the findings of the others.
pub(super) fn tool_checks(
    context: &Context<'_>,
    requested: &Requested,
    policy: &EnforcedPolicy,
) -> Vec<ToolOutcome> {
    Tool::PACKAGE_MANAGERS
        .into_iter()
        .filter(|tool| !policy.tool(*tool).is_unrestricted())
        .map(|tool| {
            check_tool(context, requested, policy.tool(tool)).map_err(|error| (tool, error))
        })
        .collect()
}

fn check_tool(
    context: &Context<'_>,
    requested: &Requested,
    rules: &ToolPolicy,
) -> Result<Option<ToolFinding>, PolicyError> {
    let tool = rules.tool;
    let Some((version, origin)) = in_effect(context, requested, tool)? else {
        return Ok(None);
    };
    let release = release_info(context, requested.offline, rules, &version)?;
    let violations = rules
        .violations(&version, release.as_ref(), today(context))
        .map_err(|_| unavailable(context, tool, &version))?;
    Ok((!violations.is_empty()).then_some((tool, version, origin, violations)))
}

/// The pinned version, else for npm the one bundled with node; a tool that
/// is neither is not in effect.
fn in_effect(
    context: &Context<'_>,
    requested: &Requested,
    tool: Tool,
) -> Result<Option<(Version, ToolOrigin)>, PolicyError> {
    if let Some(version) = requested.pins.pins.get(tool) {
        let nvmrc = requested.pins.nvmrc.clone().unwrap_or_default();
        return Ok(Some((version, ToolOrigin::Nvmrc(nvmrc))));
    }
    if tool != Tool::Npm {
        return Ok(None);
    }
    let bundled = bundled_npm(context, requested)?;
    Ok(Some((bundled, ToolOrigin::Bundled(requested.version))))
}

/// The release date from the registry, fetched only for an age rule.
fn release_info(
    context: &Context<'_>,
    offline: bool,
    rules: &ToolPolicy,
    version: &Version,
) -> Result<Option<ReleaseInfo>, PolicyError> {
    if !rules.needs_release_info() {
        return Ok(None);
    }
    if offline {
        return Err(PolicyError::ReleaseInfoUnavailable("offline".to_owned()));
    }
    let day = fetch_release_day(context, context.policy_http(), rules.tool, version)
        .map_err(|error| PolicyError::ReleaseInfoUnavailable(error.location))?
        .ok_or_else(|| unavailable(context, rules.tool, version))?;
    Ok(Some(ReleaseInfo {
        lts: None,
        published_day: Some(day),
    }))
}

fn unavailable(context: &Context<'_>, tool: Tool, version: &Version) -> PolicyError {
    let location = registry_base(context).map_or_else(
        |error| error.location,
        |registry| packument_url(&registry, tool, version),
    );
    PolicyError::ReleaseInfoUnavailable(location)
}
