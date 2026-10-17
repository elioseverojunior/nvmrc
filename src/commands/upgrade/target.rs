//! Where one tool moves to: the newest allowed release in the requested major.

use crate::commands::policy::judge::Judge;
use crate::commands::project::{
    Candidate, first_allowed, in_major, majors, node_candidates, tool_candidates,
};
use crate::commands::upgrade::request::Major;
use crate::context::Context;
use crate::domain::policy::Violation;
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
        major: u64,
        /// The newest release of the major and why the policy refuses it.
        newest: Option<(Version, Vec<Violation>)>,
        /// The lowest major with an allowed release.
        lowest: Option<u64>,
    },
}

/// The newest allowed release of `tool` in the major `major` asks for.
///
/// Yarn reads the `yarn` package for the major it already uses and
/// `@yarnpkg/cli-dist` for any other. A `current` that the policy refuses
/// still moves to the target, even a lower one.
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
    let newest = newest_violations(context, judge, tool, &pool)?;
    let lowest = lowest_allowed_major(context, judge, tool, &all)?;
    Ok(Target::NoneInMajor {
        major: wanted.unwrap_or(current.major),
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
    if tool == Tool::Node {
        return node_candidates(context);
    }
    let package_of = match major {
        Major::Same => *current,
        Major::Any => Version {
            major: 2,
            ..*current
        },
        Major::Exactly(number) => Version {
            major: number,
            ..*current
        },
    };
    Ok(tool_candidates(context, tool, &package_of)?)
}

/// The newest candidate of `pool` and the rules it breaks.
fn newest_violations(
    context: &Context<'_>,
    judge: &mut Judge,
    tool: Tool,
    pool: &[Candidate],
) -> Result<Option<(Version, Vec<Violation>)>, PolicyError> {
    let Some(newest) = pool.first() else {
        return Ok(None);
    };
    let found = judge.violations(context, tool, &newest.version, &newest.release)?;
    Ok(Some((newest.version, found)))
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
