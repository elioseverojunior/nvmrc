//! `nvm install`: the version a description stands for, from the mirror, into
//! `$NVM_DIR/versions`. Only prebuilt binaries (`.tar.gz`) are installed; the
//! install is not activated, which is the shell's job (`nvm use`).

mod defaults;
pub mod fetch;
mod flow;
pub mod lock;
pub mod options;
pub mod place;

use std::time::SystemTime;

use crate::commands::Output;
use crate::commands::transcript::Transcript;
use crate::commands::version_remote::lookup;
use crate::context::Context;
use crate::domain::floor::VersionFloor;
use crate::domain::platform::binary_available;
use crate::domain::remote::Query;
use crate::domain::version::{Flavor, Version};
use crate::error::{CliError, NvmExitCode};
use flow::{Halt, Step};
use lock::{LockRequest, acquire};
use options::Options;

const USAGE: &str = "No version provided and no .nvmrc file found\n\
Usage: nvm install [<version>]\n  \
Provide a <version>, or run from a directory containing an .nvmrc file.\n  \
Run `nvm --help` for full help.";
const DEFAULT_LOCK_TIMEOUT_SECONDS: u64 = 600;

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

fn install(context: &Context<'_>, options: &Options, transcript: &mut Transcript) -> Step<()> {
    announce(options, transcript)?;
    let version = resolve(context, options, transcript)?;
    check_floor(context, &version, transcript)?;
    let version_path = place::version_path(context, &version)?;
    if place::is_valid_install(context, &version_path) {
        transcript.err(format!("{version} is already installed."));
        defaults::ensure_default(context, &alias_target(options), transcript)?;
        return apply_alias(context, options, transcript);
    }
    install_binary(context, &version, &version_path, transcript)?;
    apply_alias(context, options, transcript)?;
    if !place::is_valid_install(context, &version_path) {
        let message = format!(
            "The install of {version} reported success but failed verification; not activating it."
        );
        transcript.err(message);
        return Err(Halt::Exit(NvmExitCode::Failure));
    }
    defaults::ensure_default(context, &alias_target(options), transcript)
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

fn resolve(context: &Context<'_>, options: &Options, transcript: &mut Transcript) -> Step<Version> {
    let query = Query {
        pattern: Some(options.version.clone()).filter(|text| !text.is_empty()),
        lts: options.lts.clone(),
    };
    let found = lookup(context, query)?;
    for warning in found.warnings {
        transcript.err(warning);
    }
    found.version.ok_or_else(|| {
        transcript.err(not_found_message(options));
        Halt::Exit(NvmExitCode::InvalidVersion)
    })
}

fn not_found_message(options: &Options) -> String {
    let version = &options.version;
    match options.lts.as_deref() {
        Some("*") => format!(
            "Version '{version}' (with LTS filter) not found - try `nvm ls-remote --lts` to browse available versions."
        ),
        Some(lts) if version.is_empty() => format!(
            "Version with LTS filter '{lts}' not found - try `nvm ls-remote --lts={lts}` to browse available versions."
        ),
        Some(lts) => format!(
            "Version '{version}' (with LTS filter '{lts}') not found - try `nvm ls-remote --lts={lts}` to browse available versions."
        ),
        None => format!(
            "Version '{version}' not found - try `nvm ls-remote` to browse available versions."
        ),
    }
}

fn check_floor(context: &Context<'_>, version: &Version, transcript: &mut Transcript) -> Step<()> {
    let from_file = context
        .fs
        .read_to_string(&context.nvm_dir()?.join("min-version"))
        .ok();
    let from_env = context.env.var("NVM_MIN_VERSION");
    let floor = VersionFloor::from_sources(from_env.as_deref(), from_file.as_deref())
        .and_then(|floor| floor.map_or(Ok(()), |floor| floor.check(version)));
    let Err(error) = floor else {
        return Ok(());
    };
    transcript.err(error.to_string());
    if matches!(error, crate::error::FloorError::Below { .. }) {
        transcript
            .err("Lower or unset NVM_MIN_VERSION (or edit $NVM_DIR/min-version) to install it.");
    }
    Err(Halt::Exit(NvmExitCode::BelowVersionFloor))
}

/// Everything from the lock to the unpacked directory.
fn install_binary(
    context: &Context<'_>,
    version: &Version,
    version_path: &std::path::Path,
    transcript: &mut Transcript,
) -> Step<()> {
    let unavailable = !binary_available(version)
        || context.platform().is_none()
        || (version.flavor == Flavor::Node && version.triple() < (0, 12, 0));
    if unavailable {
        transcript.err(format!("Binary download is not available for {version}"));
        return Err(Halt::Exit(NvmExitCode::InvalidVersion));
    }
    let _lock = take_lock(context, version, transcript)?;
    let name = match version.flavor {
        Flavor::Node => "node",
        Flavor::IoJs => "io.js",
    };
    transcript.out(format!(
        "Downloading and installing {name} {}...",
        version.directory_name()
    ));
    let tarball =
        fetch::fetch(context, version, transcript).map_err(|_| binary_failed(transcript))?;
    let artifact =
        fetch::Artifact::of(context, version).ok_or_else(|| binary_failed(transcript))?;
    place::place(context, &tarball, &artifact.files(), version_path).map_err(|message| {
        transcript.err(message);
        binary_failed(transcript)
    })
}

fn binary_failed(transcript: &mut Transcript) -> Halt {
    transcript.err("Binary download failed. Download from source aborted.");
    Halt::Exit(NvmExitCode::MissingTarget)
}

fn take_lock<'a>(
    context: &'a Context<'_>,
    version: &Version,
    transcript: &mut Transcript,
) -> Step<Option<lock::InstallLock<'a>>> {
    let number = |name: &str, default: u64| {
        context
            .env
            .var(name)
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(default)
    };
    let root = context.cache_dir()?.join("locks");
    let text = version.to_string();
    let request = LockRequest {
        root: &root,
        version: &text,
        timeout_seconds: number("NVM_INSTALL_LOCK_TIMEOUT", DEFAULT_LOCK_TIMEOUT_SECONDS),
        stale_minutes: number("NVM_INSTALL_LOCK_STALE", 0),
        now: SystemTime::now(),
    };
    let mut notes = Vec::new();
    let lock = acquire(context.fs, context.sleeper(), &request, &mut notes);
    for note in notes {
        transcript.err(note);
    }
    Ok(lock?)
}

#[cfg(test)]
mod tests;
