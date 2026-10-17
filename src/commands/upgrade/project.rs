//! The rewrite of `nvm upgrade`: a target for each selected tool, and the
//! nearest `.nvmrc` and the `package.json` beside it written only when every
//! one has an allowed target.

use std::path::PathBuf;

use super::pinned::{Pinned, Project, read_project, select};
use super::request::{Major, UpgradeRequest};
use super::target::{Target, target};
use crate::commands::policy::judge::Judge;
use crate::commands::policy::report::newest_block;
use crate::commands::project::{ProjectFiles, Stop, new_nvmrc, new_package_json, save};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::tool::Tool;
use crate::domain::version::Version;
use crate::error::NvmExitCode;

/// What the rewrite did.
#[derive(Debug, PartialEq, Eq)]
pub enum Upgraded {
    /// Every selected tool is on its target.
    Nothing,
    /// The `.nvmrc` rewritten, and `package.json` beside it if it changed.
    Written { nvmrc: PathBuf },
}

/// Moves the tools `request` selects to their targets, printing each change
/// and each tool up to date in node, npm, yarn, pnpm order once the files
/// are written.
///
/// # Errors
/// [`Stop`] for a missing or invalid `.nvmrc` (exit 3), a selection that
/// does not fit it (exit 6), a tool without an allowed target (exit 7, with
/// a block for each) and any failure to know or to write.
pub fn upgrade(
    context: &Context<'_>,
    request: &UpgradeRequest,
    transcript: &mut Transcript,
) -> Result<Upgraded, Stop> {
    let project = read_project(context, transcript)?;
    let selected = select(&project, request, transcript)?;
    let planned = plan_targets(context, &project, selected, request.major, transcript)?;
    refuse_missing(&planned, transcript)?;
    let (lines, moves) = changes(&planned);
    let upgraded = if moves.is_empty() {
        Upgraded::Nothing
    } else {
        write(context, project, &moves, transcript)?
    };
    // Reported only once the files are as the lines say.
    lines.into_iter().for_each(|line| transcript.out(line));
    Ok(upgraded)
}

/// The target of each `selected` tool, judged with the npm bundled with
/// node unless the `.nvmrc` pins npm.
fn plan_targets(
    context: &Context<'_>,
    project: &Project,
    selected: Vec<Pinned>,
    major: Major,
    transcript: &mut Transcript,
) -> Result<Vec<(Pinned, Target)>, Stop> {
    let npm_pinned = project.pinned.iter().any(|pinned| pinned.tool == Tool::Npm);
    let mut judge = Judge::load(context, transcript)?.with_npm_pinned(npm_pinned);
    let mut planned = Vec::new();
    for pinned in selected {
        let found = target(context, &mut judge, pinned.tool, &pinned.version, major)?;
        planned.push((pinned, found));
    }
    Ok(planned)
}

/// Prints the block of every tool without a target and stops with exit 7
/// when there is one.
fn refuse_missing(planned: &[(Pinned, Target)], transcript: &mut Transcript) -> Result<(), Stop> {
    let mut refused = false;
    for (pinned, found) in planned {
        refused |= no_target(pinned.tool, found, transcript);
    }
    if refused {
        return Err(Stop::Refused(NvmExitCode::BelowVersionFloor));
    }
    Ok(())
}

/// Prints why `tool` has no target, when `found` says so; whether it did.
fn no_target(tool: Tool, found: &Target, transcript: &mut Transcript) -> bool {
    let Target::NoneInMajor {
        major,
        newest,
        lowest,
    } = found
    else {
        return false;
    };
    let name = tool.name();
    transcript.err(match major {
        Some(major) => format!("nvm upgrade: no allowed {name} release in major {major}."),
        None => format!("nvm upgrade: no allowed {name} release."),
    });
    let hint = hint(name, *lowest);
    match newest {
        Some((version, refused)) => {
            newest_block(tool, version, *major, refused, &hint, transcript);
        }
        None => transcript.err(hint),
    }
    true
}

/// The way out of a major without an allowed release.
fn hint(name: &str, lowest: Option<u64>) -> String {
    match lowest {
        Some(lowest) => format!(
            "The lowest major with an allowed release is {lowest}. Run \
             `nvm upgrade --{name} --major={lowest}`, or `nvm upgrade --{name} --major` \
             for the newest."
        ),
        None => format!("No major has an allowed {name} release."),
    }
}

/// The line for each change and each tool up to date; the changes.
fn changes(planned: &[(Pinned, Target)]) -> (Vec<String>, Vec<(Tool, Version)>) {
    let (mut lines, mut moves) = (Vec::new(), Vec::new());
    for (pinned, found) in planned {
        let (tool, shown) = (pinned.tool, &pinned.shown);
        let Some(version) = moved_to(pinned, found) else {
            lines.push(format!("{} {shown} is up to date", tool.name()));
            continue;
        };
        lines.push(format!(
            "{} {shown} -> {}",
            tool.name(),
            tool.show(&version)
        ));
        moves.push((tool, version));
    }
    (lines, moves)
}

/// Where `pinned` moves; a partial node line on its target still becomes
/// the exact release.
fn moved_to(pinned: &Pinned, found: &Target) -> Option<Version> {
    match found {
        Target::Move(version) => Some(*version),
        Target::UpToDate if !pinned.exact => Some(pinned.version),
        _ => None,
    }
}

/// Writes the `.nvmrc` with `moves` and, for yarn or pnpm, the
/// `package.json` beside it; both texts are computed first.
fn write(
    context: &Context<'_>,
    project: Project,
    moves: &[(Tool, Version)],
    transcript: &mut Transcript,
) -> Result<Upgraded, Stop> {
    let files = ProjectFiles::beside(&project.nvmrc);
    let nvmrc_text = new_nvmrc(project.text, moves);
    let package_edit = new_package_json(context, &files.package_json, moves)?;
    let edit = package_edit.as_ref();
    save(
        context,
        "nvm upgrade",
        &files,
        &nvmrc_text,
        edit,
        transcript,
    )?;
    Ok(Upgraded::Written { nvmrc: files.nvmrc })
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "bundled_npm_tests.rs"]
mod bundled_npm_tests;
