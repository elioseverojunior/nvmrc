//! What `nvm upgrade` starts from: the nearest `.nvmrc`, the versions it
//! pins and the tools the request selects from them. All of it is known
//! before any request.

use std::path::{Path, PathBuf};

use super::request::{Major, ONE_TOOL, UpgradeRequest};
use crate::commands::project::{Stop, current_nvmrc};
use crate::commands::project_pins::pin_error;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::nvmrc::pins::parse_pins;
use crate::domain::nvmrc::{NvmrcContent, find_nvmrc, process_content};
use crate::domain::tool::Tool;
use crate::domain::version::{Version, VersionPattern};
use crate::error::{CliError, NvmExitCode};

/// The status of a missing `.nvmrc` or one that cannot be upgraded.
const FILE_PROBLEM: NvmExitCode = NvmExitCode::InvalidVersion;

/// A tool the `.nvmrc` pins.
#[derive(Debug, Clone)]
pub struct Pinned {
    pub tool: Tool,
    /// The version pinned; for a partial node line, the lowest it covers.
    pub version: Version,
    /// How the version reads: the line as written for a partial node line.
    pub shown: String,
    /// Whether the line names one release; a partial node line does not.
    pub exact: bool,
}

impl Pinned {
    fn exact(tool: Tool, version: Version) -> Self {
        let shown = tool.show(&version);
        Self {
            tool,
            version,
            shown,
            exact: true,
        }
    }
}

/// The nearest `.nvmrc`, its text and what it pins, node first.
pub struct Project {
    pub nvmrc: PathBuf,
    pub text: String,
    pub pinned: Vec<Pinned>,
    /// The node line when it is not a version (`lts/*`); it cannot be
    /// upgraded, so selecting node is refused.
    pub alias: Option<String>,
}

/// Reads the nearest `.nvmrc` from the working directory.
///
/// # Errors
/// [`Stop`] when there is none, it cannot be read or a pin line is invalid.
pub fn read_project(context: &Context<'_>, transcript: &mut Transcript) -> Result<Project, Stop> {
    let found = context
        .working_directory()
        .and_then(|directory| find_nvmrc(context.fs, &directory));
    let Some(nvmrc) = found else {
        let message = "nvm upgrade: no .nvmrc found; run nvm init first.";
        return Err(Stop::refuse(transcript, message.to_owned(), FILE_PROBLEM));
    };
    let text = current_nvmrc(context, &nvmrc)?;
    let pins = parse_pins(&text).map_err(|error| CliError::from(pin_error(error, &nvmrc)))?;
    let (node, alias) = node_pin(&text);
    let others = pins
        .iter()
        .map(|(tool, version)| Pinned::exact(tool, version));
    let pinned = node.into_iter().chain(others).collect();
    Ok(Project {
        nvmrc,
        text,
        pinned,
        alias,
    })
}

/// The bare node line as a pin, or as an alias when it is not a version;
/// neither when there is none. A partial version stands for the lowest
/// release it covers.
fn node_pin(text: &str) -> (Option<Pinned>, Option<String>) {
    let NvmrcContent::Version(line) = process_content(text) else {
        return (None, None);
    };
    let pattern = line.parse::<VersionPattern>().ok();
    let Some(pattern) = pattern.filter(|pattern| pattern.flavor.is_none()) else {
        return (None, Some(line));
    };
    let pinned = Pinned::exact(Tool::Node, pattern.lowest());
    if pattern.minor.is_some() && pattern.patch.is_some() {
        return (Some(pinned), None);
    }
    let partial = Pinned {
        shown: line,
        exact: false,
        ..pinned
    };
    (Some(partial), None)
}

/// The pinned tools `request` names, or all of them.
///
/// # Errors
/// [`Stop`] with exit 3 for an alias node line when node is selected, exit
/// 6 for a tool the `.nvmrc` does not pin, or for
/// `--major=<N>` with a selection that is not exactly one tool.
pub fn select(
    project: &Project,
    request: &UpgradeRequest,
    transcript: &mut Transcript,
) -> Result<Vec<Pinned>, Stop> {
    refuse_alias(project, request, transcript)?;
    let selected: Vec<Pinned> = (project.pinned.iter())
        .filter(|pinned| request.tools.is_empty() || request.tools.contains(&pinned.tool))
        .cloned()
        .collect();
    let unpinned = request
        .tools
        .iter()
        .find(|tool| selected.iter().all(|pinned| pinned.tool != **tool));
    let message = match unpinned {
        Some(tool) => not_pinned(&project.nvmrc, *tool),
        None if matches!(request.major, Major::Exactly(_)) && selected.len() != 1 => {
            format!("nvm upgrade: {ONE_TOOL}")
        }
        None => return Ok(selected),
    };
    Err(Stop::refuse(
        transcript,
        message,
        NvmExitCode::InvalidOptions,
    ))
}

/// Refuses a node line that is not a version when the request selects node.
fn refuse_alias(
    project: &Project,
    request: &UpgradeRequest,
    transcript: &mut Transcript,
) -> Result<(), Stop> {
    let selects_node = request.tools.is_empty() || request.tools.contains(&Tool::Node);
    let Some(line) = project.alias.as_ref().filter(|_| selects_node) else {
        return Ok(());
    };
    let message = format!(
        "nvm upgrade: {} names node as \"{line}\", not a version; \
         use nvm init --node to pin one.",
        project.nvmrc.display()
    );
    Err(Stop::refuse(transcript, message, FILE_PROBLEM))
}

fn not_pinned(nvmrc: &Path, tool: Tool) -> String {
    let name = tool.name();
    format!(
        "nvm upgrade: {} does not pin {name}; use nvm init --{name} to add it.",
        nvmrc.display()
    )
}
