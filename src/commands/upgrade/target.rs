//! Where one tool moves to: the newest allowed release in the requested major.

use crate::commands::policy::judge::{Judge, Refusal};
use crate::commands::project::{
    CLI_DIST, Candidate, YARN_ONE, first_allowed, in_major, majors, node_candidates, refusal,
    tool_candidates,
};
use crate::commands::upgrade::request::Major;
use crate::context::Context;
use crate::domain::tool::Tool;
use crate::domain::version::Version;
use crate::error::{CliError, PolicyError};

/// What an upgrade does with one tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The newest allowed release is the one already pinned.
    UpToDate,
    Move(Version),
    /// No release of the major is allowed.
    NoneInMajor {
        /// The major searched; `None` when every major was.
        major: Option<u64>,
        /// The newest release of the major and why the policy refuses it.
        newest: Option<(Version, Refusal)>,
        /// The lowest major with an allowed release.
        lowest: Option<u64>,
    },
}

/// The newest allowed release of `tool` in the major `major` asks for.
///
/// Yarn reads the package of the major searched (`yarn` for 1,
/// `@yarnpkg/cli-dist` for later ones), or both for any major. A `current`
/// that the policy refuses still moves to the target, even a lower one.
///
/// # Errors
/// [`CliError`] when the releases cannot be had or the policy cannot judge one.
pub fn target(
    context: &Context<'_>,
    judge: &mut Judge,
    tool: Tool,
    current: &Version,
    major: Major,
) -> Result<Target, CliError> {
    let all = candidates(context, tool, current, major)?;
    let wanted = wanted_major(current, major);
    let pool = wanted.map_or_else(|| all.clone(), |number| in_major(&all, number));
    if let Some(found) = first_allowed(context, judge, tool, &pool)? {
        let unchanged = found.version == *current;
        return Ok(if unchanged {
            Target::UpToDate
        } else {
            Target::Move(found.version)
        });
    }
    let newest = newest_refusal(context, judge, tool, &pool)?;
    let scanned = lowest_scan(context, tool, major, all)?;
    let lowest = lowest_allowed_major(context, judge, tool, &scanned)?;
    Ok(Target::NoneInMajor {
        major: wanted,
        newest,
        lowest,
    })
}

/// The major to stay in, or `None` for any.
fn wanted_major(current: &Version, major: Major) -> Option<u64> {
    match major {
        Major::Same => Some(current.major),
        Major::Any => None,
        Major::Exactly(number) => Some(number),
    }
}

fn candidates(
    context: &Context<'_>,
    tool: Tool,
    current: &Version,
    major: Major,
) -> Result<Vec<Candidate>, CliError> {
    let package_of = match (tool, major) {
        (Tool::Node, _) => return node_candidates(context),
        (Tool::Yarn, Major::Any) => return Ok(both_yarn_packages(context)?),
        (_, Major::Same) => *current,
        (_, Major::Any) => CLI_DIST,
        (_, Major::Exactly(number)) => Version {
            major: number,
            ..*current
        },
    };
    Ok(tool_candidates(context, tool, &package_of)?)
}

/// The releases of `@yarnpkg/cli-dist` and of `yarn`, newest first.
fn both_yarn_packages(context: &Context<'_>) -> Result<Vec<Candidate>, PolicyError> {
    let mut found = tool_candidates(context, Tool::Yarn, &CLI_DIST)?;
    found.extend(tool_candidates(context, Tool::Yarn, &YARN_ONE)?);
    found.sort_by_key(|candidate| std::cmp::Reverse(candidate.version));
    found.dedup_by_key(|candidate| candidate.version);
    Ok(found)
}

/// The candidates to scan for the lowest allowed major: for yarn, the
/// releases of the other package are added, unless any major was searched
/// and both are already in.
fn lowest_scan(
    context: &Context<'_>,
    tool: Tool,
    major: Major,
    mut found: Vec<Candidate>,
) -> Result<Vec<Candidate>, PolicyError> {
    if tool != Tool::Yarn || major == Major::Any {
        return Ok(found);
    }
    let from_yarn_package = found.iter().all(|candidate| candidate.version.major < 2);
    let other = if from_yarn_package {
        CLI_DIST
    } else {
        YARN_ONE
    };
    found.extend(tool_candidates(context, tool, &other)?);
    Ok(found)
}

/// The newest candidate of `pool` and why the policy refuses it.
fn newest_refusal(
    context: &Context<'_>,
    judge: &mut Judge,
    tool: Tool,
    pool: &[Candidate],
) -> Result<Option<(Version, Refusal)>, PolicyError> {
    let Some(newest) = pool.first() else {
        return Ok(None);
    };
    let refused = refusal(context, judge, tool, newest)?;
    Ok(Some((newest.version, refused)))
}

/// The lowest major among `candidates` that has an allowed release.
fn lowest_allowed_major(
    context: &Context<'_>,
    judge: &mut Judge,
    tool: Tool,
    candidates: &[Candidate],
) -> Result<Option<u64>, PolicyError> {
    for number in majors(candidates) {
        let pool = in_major(candidates, number);
        if first_allowed(context, judge, tool, &pool)?.is_some() {
            return Ok(Some(number));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
