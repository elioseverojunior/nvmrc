//! `nvm init --node --pnpm` and the like: the newest allowed release of each
//! tool asked for, or the exact one given, written to the `.nvmrc` of the
//! working directory and, for yarn or pnpm, to `packageManager` in its
//! `package.json`. Every version is chosen and both files are computed
//! before either is written.

use std::path::Path;

use super::request::{ProjectRequest, Wanted};
use crate::commands::Output;
use crate::commands::policy::judge::Judge;
use crate::commands::policy::report::argument_block;
use crate::commands::project::{
    CLI_DIST, Candidate, Exact, ProjectFiles, Stop, current_nvmrc, exact, new_nvmrc,
    new_package_json, newest_allowed, node_candidates, save, tool_candidates,
};
use crate::commands::project_pins::pin_error;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::nvmrc::pins::{Pins, parse_pins};
use crate::domain::tool::Tool;
use crate::domain::version::Version;
use crate::error::{CliError, NvmExitCode};

/// The tools in the order they are chosen, written and reported.
const ORDER: [Tool; 4] = [Tool::Node, Tool::Npm, Tool::Yarn, Tool::Pnpm];

/// Sets up the project as `request` asks; whatever stops it is printed on
/// stderr with its status, after any warning the policy gave.
///
/// # Errors
/// None of its own: every failure is in the returned output.
pub fn run_project(context: &Context<'_>, request: &ProjectRequest) -> Result<Output, CliError> {
    let mut transcript = Transcript::default();
    let status = match set_up(context, request, &mut transcript) {
        Ok(()) => NvmExitCode::Success,
        Err(stop) => stop.printed(&mut transcript),
    };
    Ok(transcript.finish(status))
}

fn set_up(
    context: &Context<'_>,
    request: &ProjectRequest,
    transcript: &mut Transcript,
) -> Result<(), Stop> {
    let directory = context.working_directory().ok_or_else(|| {
        CliError::InvalidArgument("nvm init: the working directory cannot be read.".to_owned())
    })?;
    let files = ProjectFiles::in_directory(&directory);
    let current = current_nvmrc(context, &files.nvmrc)?;
    let pins =
        parse_pins(&current).map_err(|error| CliError::from(pin_error(error, &files.nvmrc)))?;
    refuse_switch(&pins, &files.nvmrc, request, transcript)?;
    let npm_pinned = pins.npm.is_some() || request.get(Tool::Npm).is_some();
    let chosen = choose(context, request, npm_pinned, transcript)?;
    let nvmrc_text = new_nvmrc(current, &chosen);
    let package_edit = new_package_json(context, &files.package_json, &chosen)?;
    save(
        context,
        "nvm init",
        &files,
        &nvmrc_text,
        package_edit.as_ref(),
        transcript,
    )?;
    let nvmrc = files.nvmrc.display();
    transcript.out(format!("Wrote {nvmrc}: {}", listed(&chosen)));
    if let Some(edit) = package_edit {
        transcript.out(edit.reported(&files.package_json));
    }
    Ok(())
}

/// The version of each requested tool, in [`ORDER`]; whether npm is pinned
/// once the files are written decides whether the npm bundled with node is
/// judged.
fn choose(
    context: &Context<'_>,
    request: &ProjectRequest,
    npm_pinned: bool,
    transcript: &mut Transcript,
) -> Result<Vec<(Tool, Version)>, Stop> {
    let mut judge = Judge::load(context, transcript)?.with_npm_pinned(npm_pinned);
    let mut chosen = Vec::new();
    for tool in ORDER {
        if let Some(wanted) = request.get(tool) {
            let version = resolve(context, &mut judge, tool, wanted, transcript)?;
            if tool == Tool::Node {
                let warning = judge.below_limit_warning(context, &version)?;
                warning.into_iter().for_each(|line| transcript.err(line));
            }
            chosen.push((tool, version));
        }
    }
    Ok(chosen)
}

fn resolve(
    context: &Context<'_>,
    judge: &mut Judge,
    tool: Tool,
    wanted: &Wanted,
    transcript: &mut Transcript,
) -> Result<Version, Stop> {
    let Wanted::Exact(version) = wanted else {
        let candidates = candidates_of(context, tool, &CLI_DIST)?;
        let newest = newest_allowed(context, judge, tool, &candidates)?;
        let status = NvmExitCode::BelowVersionFloor;
        return newest.ok_or_else(|| refuse(transcript, &no_allowed_release(tool), status));
    };
    let candidates = candidates_of(context, tool, version)?;
    match exact(context, judge, tool, &candidates, version)? {
        Exact::Allowed => Ok(*version),
        Exact::NotPublished => {
            let message = not_published(tool, version);
            Err(refuse(transcript, &message, NvmExitCode::InvalidVersion))
        }
        Exact::NotAllowed(violations) => {
            argument_block(tool, version, &violations, transcript);
            Err(Stop::Refused(NvmExitCode::BelowVersionFloor))
        }
    }
}

/// The node index for node; for the others, the registry package that ships
/// `tool` at `package_of`.
fn candidates_of(
    context: &Context<'_>,
    tool: Tool,
    package_of: &Version,
) -> Result<Vec<Candidate>, CliError> {
    match tool {
        Tool::Node => node_candidates(context),
        _ => Ok(tool_candidates(context, tool, package_of)?),
    }
}

fn refuse(transcript: &mut Transcript, message: &str, status: NvmExitCode) -> Stop {
    Stop::refuse(transcript, message.to_owned(), status)
}

fn no_allowed_release(tool: Tool) -> String {
    match tool {
        Tool::Node => "nvm init: no allowed node LTS release.".to_owned(),
        _ => format!("nvm init: no allowed {} release.", tool.name()),
    }
}

fn not_published(tool: Tool, version: &Version) -> String {
    let (name, shown) = (tool.name(), tool.show(version));
    match tool {
        Tool::Node => format!("nvm init: node {shown} is not a node release."),
        _ => format!("nvm init: {name} {shown} is not a published release."),
    }
}

/// `node v22.11.0, pnpm 9.12.0`.
fn listed(chosen: &[(Tool, Version)]) -> String {
    let shown: Vec<String> = chosen
        .iter()
        .map(|(tool, version)| format!("{} {}", tool.name(), tool.show(version)))
        .collect();
    shown.join(", ")
}

/// Refuses yarn over a pnpm pin or pnpm over a yarn pin: the switch is the
/// user's to make. Like a malformed pin, it is known before any request.
fn refuse_switch(
    pins: &Pins,
    path: &Path,
    request: &ProjectRequest,
    transcript: &mut Transcript,
) -> Result<(), Stop> {
    for tool in ORDER
        .into_iter()
        .filter(|tool| request.get(*tool).is_some())
    {
        let other = match tool {
            Tool::Yarn => Tool::Pnpm,
            Tool::Pnpm => Tool::Yarn,
            _ => continue,
        };
        if pins.get(other).is_some() {
            let (name, other) = (tool.name(), other.name());
            let message = format!(
                "nvm init: {} pins {other}; remove its {other}= line to pin {name}.",
                path.display()
            );
            return Err(refuse(transcript, &message, NvmExitCode::InvalidVersion));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
