//! `nvm install-latest-npm`: bring `npm` to the newest version that works on
//! the `node` it belongs to (`nvm_install_latest_npm`).

use crate::commands::npm::Npm;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::npm::upgrade::{Install, steps};
use crate::domain::version::Version;
use crate::error::NvmExitCode;

/// What is known of the `node` that the `npm` belongs to.
pub enum Node<'a> {
    /// `v20.10.0` (or `iojs-v3.3.1`).
    Version(&'a str),
    /// No `node` is active.
    None,
}

fn triple(text: &str) -> Option<(u64, u64, u64)> {
    let version: Version = text.parse().ok()?;
    Some(version.triple())
}

/// Runs the upgrade, printing as `nvm.sh` does. With `NVM_DEBUG=1` the `npm`
/// commands are printed instead of run.
pub fn install_latest(
    context: &Context<'_>,
    npm: Option<&Npm>,
    node: &Node<'_>,
    transcript: &mut Transcript,
) -> NvmExitCode {
    transcript.out("Attempting to upgrade to the latest working version of npm...");
    let npm_version = npm.and_then(|npm| npm.version(context));
    let node_text = match node {
        Node::Version(text) => Some(text.strip_prefix("iojs-").unwrap_or(text)),
        Node::None => {
            let shown = npm_version.as_deref().unwrap_or_default();
            transcript.out(format!("Detected node version none, npm version v{shown}"));
            None
        }
    };
    let Some(node_text) = node_text.filter(|text| triple(text).is_some()) else {
        transcript.err("Unable to obtain node version.");
        return NvmExitCode::Failure;
    };
    let (Some(npm), Some(npm_version)) = (npm, npm_version) else {
        transcript.err("Unable to obtain npm version.");
        return NvmExitCode::MissingTarget;
    };
    let debug = context.env.var("NVM_DEBUG").as_deref() == Some("1");
    if debug {
        transcript.out(format!(
            "Detected node version {node_text}, npm version v{npm_version}"
        ));
    }
    let plan = steps(
        triple(node_text).unwrap_or_default(),
        triple(&npm_version).unwrap_or_default(),
    );
    for step in plan {
        transcript.out(step.note);
        match step.install {
            Install::Nothing => {}
            Install::Latest => install(context, npm, "npm", debug, transcript),
            Install::Spec(spec) => install(context, npm, spec, debug, transcript),
        }
    }
    let upgraded = npm.version(context).unwrap_or_default();
    transcript.out(format!("* npm upgraded to: v{upgraded}"));
    NvmExitCode::Success
}

fn install(context: &Context<'_>, npm: &Npm, spec: &str, debug: bool, transcript: &mut Transcript) {
    if debug {
        transcript.out(format!("npm install -g {spec}"));
    } else {
        npm.run(context, &["install", "-g", spec], transcript);
    }
}

#[cfg(test)]
mod tests;
