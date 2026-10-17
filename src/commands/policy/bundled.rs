//! The npm bundled with a node version, for the npm rules when npm is not
//! pinned.

use super::enforce::{Origin, Requested, index_location, indexed_release};
use crate::commands::install::place::version_path;
use crate::commands::resolve::system_node;
use crate::context::Context;
use crate::domain::version::Version;
use crate::error::PolicyError;

/// The npm bundled with the requested node: the `version` of its
/// `lib/node_modules/npm/package.json` when that node is installed,
/// `npm --version` of the `npm` beside the system `node`, else the `npm`
/// column of the release index.
///
/// # Errors
/// [`PolicyError::ReleaseInfoUnavailable`] when none of these can be read,
/// or offline when the index is needed.
pub fn bundled_npm(context: &Context<'_>, requested: &Requested) -> Result<Version, PolicyError> {
    if requested.origin == Origin::System {
        return system_npm(context);
    }
    if let Some(version) = installed_npm(context, &requested.version) {
        return Ok(version);
    }
    indexed_release(context, requested)?
        .npm
        .ok_or_else(|| PolicyError::ReleaseInfoUnavailable(index_location(context)))
}

fn installed_npm(context: &Context<'_>, node: &Version) -> Option<Version> {
    let manifest = version_path(context, node)
        .ok()?
        .join("lib/node_modules/npm/package.json");
    let text = context.fs.read_to_string(&manifest).ok()?;
    let document: serde_json::Value = serde_json::from_str(&text).ok()?;
    document["version"].as_str()?.parse().ok()
}

fn system_npm(context: &Context<'_>) -> Result<Version, PolicyError> {
    let Some(npm) = system_node(context)
        .ok()
        .flatten()
        .map(|node| node.with_file_name("npm"))
    else {
        return Err(PolicyError::ReleaseInfoUnavailable(
            "the system npm".to_owned(),
        ));
    };
    context
        .process()
        .run(&npm, &["--version"])
        .ok()
        .filter(|output| output.success)
        .and_then(|output| output.stdout.trim().parse().ok())
        .ok_or_else(|| PolicyError::ReleaseInfoUnavailable(npm.display().to_string()))
}
