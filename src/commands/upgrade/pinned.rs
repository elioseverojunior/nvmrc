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
}

/// Reads the nearest `.nvmrc` from the working directory.
///
/// # Errors
/// [`Stop`] when there is none, it cannot be read, a pin line is invalid or
/// its node line is not a version.
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
    let node = node_pin(&nvmrc, &text, transcript)?;
    let others = pins
        .iter()
        .map(|(tool, version)| Pinned::exact(tool, version));
    let pinned = node.into_iter().chain(others).collect();
    Ok(Project {
        nvmrc,
        text,
        pinned,
    })
}

/// The bare node line; `None` when there is none. A partial version stands
/// for the lowest release it covers.
fn node_pin(nvmrc: &Path, text: &str, transcript: &mut Transcript) -> Result<Option<Pinned>, Stop> {
    let NvmrcContent::Version(line) = process_content(text) else {
        return Ok(None);
    };
    let pattern = line.parse::<VersionPattern>().ok();
    let Some(pattern) = pattern.filter(|pattern| pattern.flavor.is_none()) else {
        let message = format!(
            "nvm upgrade: {} names node as \"{line}\", not a version; \
             use nvm init --node to pin one.",
            nvmrc.display()
        );
        return Err(Stop::refuse(transcript, message, FILE_PROBLEM));
    };
    let pinned = Pinned::exact(Tool::Node, pattern.lowest());
    if pattern.minor.is_some() && pattern.patch.is_some() {
        return Ok(Some(pinned));
    }
    Ok(Some(Pinned {
        shown: line,
        exact: false,
        ..pinned
    }))
}

/// The pinned tools `request` names, or all of them.
///
/// # Errors
/// [`Stop`] with exit 6 for a tool the `.nvmrc` does not pin, or for
/// `--major=<N>` with a selection that is not exactly one tool.
pub fn select(
    project: &Project,
    request: &UpgradeRequest,
    transcript: &mut Transcript,
) -> Result<Vec<Pinned>, Stop> {
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

fn not_pinned(nvmrc: &Path, tool: Tool) -> String {
    let name = tool.name();
    format!(
        "nvm upgrade: {} does not pin {name}; use nvm init --{name} to add it.",
        nvmrc.display()
    )
}
