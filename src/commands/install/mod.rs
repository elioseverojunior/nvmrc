//! `nvm install`: the version a description stands for, from the mirror, into
//! `$NVM_DIR/versions`. Prebuilt binaries (`.tar.xz` or `.tar.gz`) are installed; the
//! install is not activated, which is the shell's job (`nvm use`).

mod acquire;
mod defaults;
pub mod fetch;
mod flow;
pub mod lock;
mod npm_steps;
mod offline;
pub mod options;
pub mod place;
mod resolve;
mod source;

use crate::commands::Output;
use crate::commands::npm::packages::Source;
use crate::commands::nvmrc_file;
use crate::commands::resolve::{Resolved, resolve_installed};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::source_build::natural_jobs;
use crate::domain::version::Version;
use crate::domain::version_prefix::with_v_prefix;
use crate::error::{CliError, NvmExitCode};
use flow::{Halt, Step, Target};
use options::Options;
use resolve::{check_floor, resolve};

const USAGE: &str = "No version provided and no .nvmrc file found\n\
Usage: nvm install [<version>]\n  \
Provide a <version>, or run from a directory containing an .nvmrc file.\n  \
Run `nvm --help` for full help.";

/// # Errors
/// As [`options::parse`], [`CliError::Usage`] without a version, and
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let options = options::parse(args)?;
    let mut transcript = Transcript::default();
    match install(context, &options, &mut transcript) {
        Ok(()) => Ok(transcript.finish(NvmExitCode::Success)),
        Err(Halt::Exit(status)) => Ok(transcript.finish(status)),
        Err(Halt::Error(error)) => Err(error),
    }
}

fn install(context: &Context<'_>, given: &Options, transcript: &mut Transcript) -> Step<()> {
    let make_jobs = requested_jobs(given, transcript);
    let options = &no_source_fallback(context, given)?;
    announce(options, transcript)?;
    let version = resolve(context, options, transcript)?;
    check_floor(context, &version, transcript)?;
    let path = place::version_path(context, &version)?;
    let source = reinstall_source(context, options, &version, transcript)?;
    let target = Target {
        version,
        path,
        source,
        make_jobs,
    };
    if place::is_valid_install(context, &target.path) {
        already_installed(context, options, &target, transcript)
    } else {
        fresh_install(context, options, &target, transcript)
    }
}

/// A valid install is left as it is; the steps after the install still run.
/// The status of the last one is the status of the command: `--save`, which
/// writes `.nvmrc`, replaces that of the `npm` steps, as in `nvm.sh`.
fn already_installed(
    context: &Context<'_>,
    options: &Options,
    target: &Target,
    transcript: &mut Transcript,
) -> Step<()> {
    transcript.err(format!("{} is already installed.", target.version));
    let mut status = npm_steps::run(context, options, target, transcript)?;
    defaults::ensure_default(context, &alias_target(options), transcript)?;
    if options.save {
        status = nvmrc_file::write(context, &target.version.to_string(), false, transcript);
    }
    if status == NvmExitCode::Success {
        apply_alias(context, options, transcript)?;
    }
    end_with(status)
}

/// Unlike `nvm.sh`, which forgets `--save` on a fresh install, `.nvmrc` is
/// written here too, after everything else.
fn fresh_install(
    context: &Context<'_>,
    options: &Options,
    target: &Target,
    transcript: &mut Transcript,
) -> Step<()> {
    acquire::acquire(context, options, target, transcript)?;
    apply_alias(context, options, transcript)?;
    if !place::is_valid_install(context, &target.path) {
        let message = format!(
            "The install of {} reported success but failed verification; not activating it.",
            target.version
        );
        transcript.err(message);
        return Err(Halt::Exit(NvmExitCode::Failure));
    }
    defaults::ensure_default(context, &alias_target(options), transcript)?;
    let mut status = npm_steps::run(context, options, target, transcript)?;
    if options.save && status == NvmExitCode::Success {
        status = nvmrc_file::write(context, &target.version.to_string(), false, transcript);
    }
    end_with(status)
}

/// The version `--reinstall-packages-from` names, which must be installed and
/// must not be the one being installed.
fn reinstall_source(
    context: &Context<'_>,
    options: &Options,
    version: &Version,
    transcript: &mut Transcript,
) -> Step<Option<Source>> {
    let Some(provided) = &options.reinstall_from else {
        return Ok(None);
    };
    if with_v_prefix(provided) == version.to_string() {
        transcript.err(
            "You can't reinstall global packages from the same version of node you're installing.",
        );
        return Err(Halt::Exit(NvmExitCode::SameVersion));
    }
    match resolve_installed(context, provided)? {
        Resolved::Installed(from) => Ok(Some(Source::Version(from))),
        Resolved::System => Ok(Some(Source::System)),
        Resolved::Missing { .. } => {
            transcript.err(
                "If --reinstall-packages-from is provided, it must point to an installed version of node.",
            );
            Err(Halt::Exit(NvmExitCode::SourceNotInstalled))
        }
    }
}

/// `-j`: what `nvm.sh` says when the option is read, and the number if it is
/// a natural one.
fn requested_jobs(options: &Options, transcript: &mut Transcript) -> Option<usize> {
    let text = options.make_jobs.as_deref()?;
    let jobs = natural_jobs(text);
    match jobs {
        Some(jobs) => transcript.out(format!("number of `make` jobs: {jobs}")),
        None if !text.is_empty() => transcript.err(format!(
            "{text} is invalid for number of `make` jobs, must be a natural number"
        )),
        None => {}
    }
    jobs
}

/// `NVM_NO_SOURCE_FALLBACK=1` is `-b` for every install, and is at odds with
/// `-s`.
fn no_source_fallback(context: &Context<'_>, given: &Options) -> Step<Options> {
    let mut options = given.clone();
    if context.env.var("NVM_NO_SOURCE_FALLBACK").as_deref() == Some("1") && !options.no_source {
        if options.no_binary {
            let message = "-s cannot be combined with NVM_NO_SOURCE_FALLBACK=1 since that would skip install from both binary and source";
            return Err(Halt::Error(CliError::InvalidOptions(message.to_owned())));
        }
        options.no_source = true;
    }
    Ok(options)
}

/// The install ends with the status of its last step.
fn end_with(status: NvmExitCode) -> Step<()> {
    match status {
        NvmExitCode::Success => Ok(()),
        failed => Err(Halt::Exit(failed)),
    }
}

/// What an alias made for this install points at.
fn alias_target(options: &Options) -> String {
    match (&options.lts, options.version.is_empty()) {
        (Some(lts), true) => format!("lts/{}", lts.to_lowercase()),
        _ => options.version.clone(),
    }
}

/// `--default` and `--alias=<name>`.
fn apply_alias(context: &Context<'_>, options: &Options, transcript: &mut Transcript) -> Step<()> {
    match &options.alias {
        Some(name) => defaults::make_alias(context, name, &alias_target(options), transcript),
        None => Ok(()),
    }
}

fn announce(options: &Options, transcript: &mut Transcript) -> Step<()> {
    if !options.version_given && options.lts.is_none() {
        return Err(Halt::Error(CliError::Usage(USAGE.to_owned())));
    }
    match (options.announce_lts, options.lts.as_deref()) {
        (true, Some("*")) => transcript.out("Installing latest LTS version."),
        (true, Some(name)) => {
            transcript.out(format!(
                "Installing with latest version of LTS line: {name}"
            ));
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests;
