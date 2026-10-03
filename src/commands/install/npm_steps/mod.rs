//! What an install does with `npm` once the version is in place, in the order
//! of `nvm.sh`: `--latest-npm`, then the default packages. A version without
//! an `npm` skips these steps with a warning (`nvm.sh` would download an `npm`
//! installer from the internet and run it; this port never does).

use std::path::Path;

use crate::commands::install::flow::Step;
use crate::commands::install::options::Options;
use crate::commands::npm::Npm;
use crate::commands::npm::latest::{Node, install_latest};
use crate::commands::npm::packages::{Source, reinstall};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::npm::default_packages;
use crate::domain::version::Version;
use crate::error::NvmExitCode;

/// The steps (`--latest-npm`, the default packages, the packages of
/// `--reinstall-packages-from`), stopping at the first one that does not
/// succeed; its status is the install's.
///
/// # Errors
/// [`crate::commands::install::flow::Halt::Error`] when `$NVM_DIR` is unknown.
pub fn run(
    context: &Context<'_>,
    options: &Options,
    version: &Version,
    version_path: &Path,
    source: Option<&Source>,
    transcript: &mut Transcript,
) -> Step<NvmExitCode> {
    let npm = Npm::in_version(context, version_path);
    if options.latest_npm {
        let status = upgrade(context, npm.as_ref(), version, transcript);
        if status != NvmExitCode::Success {
            return Ok(status);
        }
    }
    if !options.skip_default_packages {
        let status = default_packages(context, npm.as_ref(), version, transcript)?;
        if status != NvmExitCode::Success {
            return Ok(status);
        }
    }
    Ok(source.map_or(NvmExitCode::Success, |source| {
        copy_packages(context, npm.as_ref(), version, source, transcript)
    }))
}

/// `nvm reinstall-packages <source>` into the version just installed.
fn copy_packages(
    context: &Context<'_>,
    npm: Option<&Npm>,
    version: &Version,
    source: &Source,
    transcript: &mut Transcript,
) -> NvmExitCode {
    if *source == Source::Version(*version) {
        transcript.err("Can not reinstall packages from the current version of node.");
        return NvmExitCode::MissingTarget;
    }
    match npm {
        Some(npm) => reinstall(context, source, npm, transcript),
        None => skip(version, "the reinstall of global packages", transcript),
    }
}

fn skip(version: &Version, what: &str, transcript: &mut Transcript) -> NvmExitCode {
    transcript.err(format!("npm was not found in {version}; skipping {what}."));
    NvmExitCode::Success
}

fn upgrade(
    context: &Context<'_>,
    npm: Option<&Npm>,
    version: &Version,
    transcript: &mut Transcript,
) -> NvmExitCode {
    if npm.is_none() {
        return skip(version, "the npm upgrade", transcript);
    }
    let text = version.to_string();
    install_latest(context, npm, &Node::Version(&text), transcript)
}

/// `nvm_install_default_packages`: one `npm install -g --quiet` for the lot.
fn default_packages(
    context: &Context<'_>,
    npm: Option<&Npm>,
    version: &Version,
    transcript: &mut Transcript,
) -> Step<NvmExitCode> {
    let file = context.nvm_dir()?.join("default-packages");
    let Ok(contents) = context.fs.read_to_string(&file) else {
        return Ok(NvmExitCode::Success);
    };
    let shown = file.display().to_string();
    let joined = match default_packages::parse(&contents, &shown) {
        Ok(joined) => joined,
        Err(error) => {
            transcript.err(error.to_string());
            return Ok(NvmExitCode::Failure);
        }
    };
    if joined.is_empty() {
        return Ok(NvmExitCode::Success);
    }
    let Some(npm) = npm else {
        return Ok(skip(version, "the default packages", transcript));
    };
    transcript.out(format!(
        "Installing default global packages from {shown}..."
    ));
    transcript.out(format!("npm install -g --quiet {joined}"));
    let mut args = vec!["install", "-g", "--quiet"];
    args.extend(joined.split_whitespace());
    if npm.run(context, &args, transcript) {
        return Ok(NvmExitCode::Success);
    }
    transcript.err("Failed installing default packages. Please check if your default-packages file or a package in it has problems!");
    Ok(NvmExitCode::Failure)
}

#[cfg(test)]
mod tests;
