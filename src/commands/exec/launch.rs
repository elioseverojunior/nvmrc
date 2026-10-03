//! Digest 6.3: check the version is installed, print the `Running ...`
//! line, and plan the command with the environment of the version.

use super::child;
use super::options::Options;
use super::select::Selection;
use crate::commands::Output;
use crate::commands::install::place::version_path;
use crate::commands::transcript::Transcript;
use crate::commands::use_version::apply::{Switch, link_current, npm_suffix};
use crate::commands::use_version::prefix::{self, Check};
use crate::commands::use_version::target::lookup::{Resolution, ensure_installed, nvm_version};
use crate::context::Context;
use crate::domain::version::Version;
use crate::error::{CliError, NvmExitCode};

pub(super) fn launch(
    context: &Context<'_>,
    options: &Options,
    selection: Selection,
    mut transcript: Transcript,
) -> Result<Output, CliError> {
    if !ensure_installed(context, &selection.provided, false, &mut transcript)? {
        return Ok(transcript.finish(NvmExitCode::Failure));
    }
    match nvm_version(context, &selection.version)? {
        Resolution::Version(version) => {
            installed(context, options, &selection, &version, transcript)
        }
        Resolution::System => system(context, options, selection, transcript),
        Resolution::Loop | Resolution::NotAvailable | Resolution::Inactive => {
            Ok(transcript.finish(NvmExitCode::Failure))
        }
    }
}

/// An installed version: the `Running ...` line (whose npm suffix comes
/// from a silent `nvm use`, so a refused prefix leaves it out and reports
/// the refusal), then `nvm-exec`'s own `nvm use`, whose refusal is 127.
fn installed(
    context: &Context<'_>,
    options: &Options,
    selection: &Selection,
    version: &Version,
    mut transcript: Transcript,
) -> Result<Output, CliError> {
    let nvm_dir = context.nvm_dir()?;
    let directory = version_path(context, version)?;
    let switch = Switch::new(&nvm_dir, &directory);
    let path = switch.path(context);
    let prefixes = Prefixes {
        context,
        version,
        nvm_dir: nvm_dir.to_string_lossy().into_owned(),
        directory: directory.to_string_lossy().into_owned(),
        path: &path,
    };
    link_current(context, &nvm_dir, &directory, &mut transcript);
    if !options.silent {
        let npm = prefixes.npm_suffix(&mut transcript);
        let resolved = version.to_string();
        transcript.out(running(options, &selection.version, &resolved, &npm));
    }
    if !prefixes.accept(false, &mut transcript) {
        return Ok(transcript.finish(NvmExitCode::NotFound));
    }
    let environment = child::installed(context, &switch, &path, &selection.version)?;
    Ok(planned(transcript, &selection.command, environment))
}

/// The prefix checks of an `nvm use` of `version` with the new `path`.
struct Prefixes<'a, 'b> {
    context: &'a Context<'b>,
    version: &'a Version,
    nvm_dir: String,
    directory: String,
    path: &'a str,
}

impl Prefixes<'_, '_> {
    /// Whether an `nvm use` (`--silent` or not, which only changes the
    /// command its npmrc message suggests) accepts the prefix settings; a
    /// refusal is reported in `transcript`.
    fn accept(&self, silent: bool, transcript: &mut Transcript) -> bool {
        let command = prefix::command(self.version, true, silent);
        let check = Check {
            nvm_dir: &self.nvm_dir,
            directory: &self.directory,
            path: self.path,
            command: &command,
            delete: false,
        };
        prefix::check(self.context, &check, transcript).is_ok()
    }

    /// `$(nvm use --silent VERSION && nvm_print_npm_version)`: nothing when
    /// that `nvm use` refuses a prefix.
    fn npm_suffix(&self, transcript: &mut Transcript) -> String {
        if self.accept(true, transcript) {
            npm_suffix(self.context, self.path)
        } else {
            String::new()
        }
    }
}

/// `system`: no prefix checks, and the environment of `nvm deactivate`.
fn system(
    context: &Context<'_>,
    options: &Options,
    selection: Selection,
    mut transcript: Transcript,
) -> Result<Output, CliError> {
    let (path, environment) = child::system(context)?;
    if !options.silent {
        let npm = npm_suffix(context, &path);
        transcript.out(running(options, &selection.version, "system", &npm));
    }
    Ok(planned(transcript, &selection.command, environment))
}

/// The command to run with `environment`, or nothing to run without one.
fn planned(transcript: Transcript, command: &[String], environment: child::Environment) -> Output {
    let output = transcript.finish(NvmExitCode::Success);
    match child::invocation(command, environment) {
        Some(invocation) => output.with_spawn(invocation),
        None => output,
    }
}

/// The `Running ...` line; `version` is `VERSION`, `resolved` what
/// `nvm_version` makes of it.
fn running(options: &Options, version: &str, resolved: &str, npm: &str) -> String {
    match options.lts.as_deref() {
        Some("*") => format!("Running node latest LTS -> {resolved}{npm}"),
        Some(name) => format!("Running node LTS \"{name}\" -> {resolved}{npm}"),
        None => match version.strip_prefix("iojs-") {
            Some(bare) => format!("Running io.js {bare}{npm}"),
            None => format!("Running node {version}{npm}"),
        },
    }
}
