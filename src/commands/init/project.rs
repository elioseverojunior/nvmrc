//! `nvm init --node --pnpm` and the like: the newest allowed release of each
//! tool asked for, or the exact one given, written to the `.nvmrc` of the
//! working directory and, for yarn or pnpm, to `packageManager` in its
//! `package.json`. Every version is chosen and both files are computed
//! before either is written.

use std::path::Path;

use super::candidates::{
    Candidate, Exact, exact, newest_allowed, node_candidates, tool_candidates,
};
use super::request::{ProjectRequest, Wanted};
use crate::commands::Output;
use crate::commands::policy::judge::Judge;
use crate::commands::policy::report::argument_block;
use crate::commands::project_pins::pin_error;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::nvmrc::pins::parse_pins;
use crate::domain::nvmrc::{with_pin, with_version};
use crate::domain::package_json::{package_manager_field, set_package_manager};
use crate::domain::tool::Tool;
use crate::domain::version::{Flavor, Version};
use crate::error::{CliError, NvmExitCode, PinsError, PolicyError};

/// The tools in the order they are chosen, written and reported.
const ORDER: [Tool; 4] = [Tool::Node, Tool::Npm, Tool::Yarn, Tool::Pnpm];

/// The yarn release whose package (`@yarnpkg/cli-dist`) lists the releases
/// the newest search picks from.
const CLI_DIST: Version = Version {
    flavor: Flavor::Node,
    major: 2,
    minor: 0,
    patch: 0,
};

/// Why project mode stopped before writing.
enum Stop {
    /// An error, printed as its message.
    Error(CliError),
    /// A refusal already printed, with its status.
    Refused(NvmExitCode),
}

impl From<CliError> for Stop {
    fn from(error: CliError) -> Self {
        Self::Error(error)
    }
}

impl From<PolicyError> for Stop {
    fn from(error: PolicyError) -> Self {
        Self::Error(error.into())
    }
}

/// The new `package.json` text and the `packageManager` value it holds.
struct PackageEdit {
    text: String,
    changed: bool,
    field: String,
}

/// Sets up the project as `request` asks; whatever stops it is printed on
/// stderr with its status, after any warning the policy gave.
///
/// # Errors
/// None of its own: every failure is in the returned output.
pub fn run_project(context: &Context<'_>, request: &ProjectRequest) -> Result<Output, CliError> {
    let mut transcript = Transcript::default();
    let status = match set_up(context, request, &mut transcript) {
        Ok(()) => NvmExitCode::Success,
        Err(Stop::Refused(status)) => status,
        Err(Stop::Error(error)) => {
            transcript.err(error.to_string());
            error.exit_code()
        }
    };
    Ok(transcript.finish(status))
}

fn set_up(
    context: &Context<'_>,
    request: &ProjectRequest,
    transcript: &mut Transcript,
) -> Result<(), Stop> {
    let chosen = choose(context, request, transcript)?;
    let directory = context.working_directory().ok_or_else(|| {
        CliError::InvalidArgument("nvm init: the working directory cannot be read.".to_owned())
    })?;
    let (nvmrc, package_json) = (directory.join(".nvmrc"), directory.join("package.json"));
    let nvmrc_text = new_nvmrc(context, &nvmrc, &chosen, transcript)?;
    let package_edit = new_package_json(context, &package_json, &chosen)?;
    if let Some(edit) = package_edit.as_ref().filter(|edit| edit.changed) {
        write(context, &package_json, &edit.text)?;
    }
    write(context, &nvmrc, &nvmrc_text)?;
    transcript.out(format!("Wrote {}: {}", nvmrc.display(), listed(&chosen)));
    if let Some(edit) = package_edit {
        let (field, path) = (edit.field, package_json.display());
        transcript.out(format!("Set packageManager to \"{field}\" in {path}"));
    }
    Ok(())
}

/// The version of each requested tool, in [`ORDER`].
fn choose(
    context: &Context<'_>,
    request: &ProjectRequest,
    transcript: &mut Transcript,
) -> Result<Vec<(Tool, Version)>, Stop> {
    let mut judge = Judge::load(context, transcript)?;
    let mut chosen = Vec::new();
    for tool in ORDER {
        if let Some(wanted) = request.get(tool) {
            let version = resolve(context, &mut judge, tool, wanted, transcript)?;
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
    transcript.err(message.to_owned());
    Stop::Refused(status)
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

/// The `.nvmrc` at `path` (empty when absent) with the chosen keys set.
fn new_nvmrc(
    context: &Context<'_>,
    path: &Path,
    chosen: &[(Tool, Version)],
    transcript: &mut Transcript,
) -> Result<String, Stop> {
    let current = existing(context, path)?.unwrap_or_default();
    refuse_switch(&current, path, chosen, transcript)?;
    Ok(chosen
        .iter()
        .fold(current, |text, (tool, version)| match tool {
            Tool::Node => with_version(&text, &tool.show(version)),
            _ => with_pin(&text, *tool, version),
        }))
}

/// Refuses yarn over a pnpm pin, or pnpm over a yarn pin: the switch is the
/// user's to make.
fn refuse_switch(
    current: &str,
    path: &Path,
    chosen: &[(Tool, Version)],
    transcript: &mut Transcript,
) -> Result<(), Stop> {
    let pins = parse_pins(current).map_err(|error| CliError::from(pin_error(error, path)))?;
    for (tool, _) in chosen {
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

/// The `package.json` at `path` with `packageManager` set, when it exists
/// and yarn or pnpm was chosen.
fn new_package_json(
    context: &Context<'_>,
    path: &Path,
    chosen: &[(Tool, Version)],
) -> Result<Option<PackageEdit>, CliError> {
    let manager = chosen
        .iter()
        .find(|(tool, _)| matches!(tool, Tool::Yarn | Tool::Pnpm));
    let Some((tool, version)) = manager else {
        return Ok(None);
    };
    let Some(original) = existing(context, path)? else {
        return Ok(None);
    };
    let field = package_manager_field(*tool, version);
    let text = set_package_manager(&original, &field).map_err(|error| {
        CliError::from(PinsError::UnreadablePackageJson {
            package_json: path.display().to_string(),
            reason: error.to_string(),
        })
    })?;
    let changed = text != original;
    Ok(Some(PackageEdit {
        text,
        changed,
        field,
    }))
}

/// The text of the file at `path`; `None` when there is none.
fn existing(context: &Context<'_>, path: &Path) -> Result<Option<String>, CliError> {
    if !context.fs.is_file(path) {
        return Ok(None);
    }
    let text = context.fs.read_to_string(path);
    text.map(Some).map_err(|error| CliError::io(path, error))
}

fn write(context: &Context<'_>, path: &Path, text: &str) -> Result<(), CliError> {
    context
        .fs
        .write_file(path, text)
        .map_err(|error| CliError::io(path, error))
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
