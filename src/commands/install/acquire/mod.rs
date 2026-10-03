//! Getting a version onto the disk: the third-party hook if there is one,
//! else the prebuilt binary, and from source when the binary cannot be had
//! and `-b` (or `NVM_NO_SOURCE_FALLBACK=1`) did not forbid it.

mod hook;

use std::time::SystemTime;

use crate::commands::install::fetch::{self, Artifact};
use crate::commands::install::flow::{Halt, Step, Target};
use crate::commands::install::lock::{InstallLock, LockRequest, acquire as take};
use crate::commands::install::options::Options;
use crate::commands::install::place;
use crate::commands::install::source::{Build, build};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::platform::binary_available;
use crate::domain::source_build::jobs as plan_jobs;
use crate::domain::version::{Flavor, Version};
use crate::error::NvmExitCode;

const DEFAULT_LOCK_TIMEOUT_SECONDS: u64 = 600;

/// What became of the attempt to install the binary.
enum Binary {
    Installed,
    /// There is no binary for this version on this machine.
    Unavailable,
    Failed,
}

/// Everything from the lock to a version directory that is in place.
///
/// # Errors
/// [`Halt`] with the status the install ends with, after the transcript says
/// why.
pub fn acquire(
    context: &Context<'_>,
    options: &Options,
    target: &Target,
    transcript: &mut Transcript,
) -> Step<()> {
    let version = &target.version;
    if version.flavor == Flavor::Node && version.triple() < (0, 12, 0) {
        transcript.err(format!(
            "Versions before v0.12.0 use the legacy layout, which is not supported: {version}"
        ));
        return Err(Halt::Exit(NvmExitCode::InvalidVersion));
    }
    if let Some(program) = context
        .env
        .var("NVM_INSTALL_THIRD_PARTY_HOOK")
        .filter(|hook| !hook.is_empty())
    {
        return hook::run(context, &program, options, target, transcript);
    }
    let _lock = take_lock(context, version, transcript)?;
    let binary = if options.no_binary {
        None
    } else {
        Some(install_binary(context, target, options.offline, transcript))
    };
    if matches!(binary, Some(Binary::Installed)) {
        return Ok(());
    }
    if options.no_source {
        return Err(without_source(binary, version, transcript));
    }
    if matches!(binary, Some(Binary::Failed)) {
        transcript.err("Binary download failed, trying source.");
    }
    from_source(context, options, target, transcript)
}

/// `-b`: the end of the road, with what `nvm.sh` says about the binary.
fn without_source(binary: Option<Binary>, version: &Version, transcript: &mut Transcript) -> Halt {
    if matches!(binary, Some(Binary::Failed)) {
        transcript.err("Binary download failed. Download from source aborted.");
        return Halt::Exit(NvmExitCode::MissingTarget);
    }
    transcript.err(format!("Binary download is not available for {version}"));
    Halt::Exit(NvmExitCode::InvalidVersion)
}

fn install_binary(
    context: &Context<'_>,
    target: &Target,
    offline: bool,
    transcript: &mut Transcript,
) -> Binary {
    let version = &target.version;
    let has_binaries = context
        .platform()
        .is_some_and(|platform| platform.os.has_binaries());
    let Some(artifact) =
        Artifact::of(context, version).filter(|_| has_binaries && binary_available(version))
    else {
        return Binary::Unavailable;
    };
    let name = match version.flavor {
        Flavor::Node => "node",
        Flavor::IoJs => "io.js",
    };
    transcript.out(format!(
        "Downloading and installing {name} {}...",
        version.directory_name()
    ));
    let Ok(tarball) = fetch::fetch(context, &artifact, version, offline, transcript) else {
        return Binary::Failed;
    };
    match place::place(context, &tarball, &artifact.files(), &target.path) {
        Ok(()) => Binary::Installed,
        Err(message) => {
            transcript.err(message);
            Binary::Failed
        }
    }
}

fn from_source(
    context: &Context<'_>,
    options: &Options,
    target: &Target,
    transcript: &mut Transcript,
) -> Step<()> {
    let jobs = make_jobs(context, target.make_jobs, transcript);
    let job = Build {
        version: &target.version,
        version_path: &target.path,
        jobs,
        extra: &options.extra,
        offline: options.offline,
    };
    build(context, &job, transcript).map_err(|_| Halt::Exit(NvmExitCode::Failure))
}

/// `-j`, or `NVM_MAKE_JOBS` when it is a natural number, or one fewer than
/// the cores, with the messages of `nvm_get_make_jobs` for the last.
fn make_jobs(
    context: &Context<'_>,
    requested: Option<usize>,
    transcript: &mut Transcript,
) -> usize {
    if let Some(jobs) = requested {
        return jobs;
    }
    let from_env = context
        .env
        .var("NVM_MAKE_JOBS")
        .and_then(|text| text.trim().parse::<usize>().ok())
        .filter(|jobs| *jobs > 0);
    if let Some(jobs) = from_env {
        return jobs;
    }
    let planned = plan_jobs(None, context.cpu().cores());
    planned.stdout.iter().for_each(|line| transcript.out(line));
    planned.stderr.iter().for_each(|line| transcript.err(line));
    planned.jobs
}

fn take_lock<'a>(
    context: &'a Context<'_>,
    version: &Version,
    transcript: &mut Transcript,
) -> Step<Option<InstallLock<'a>>> {
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
    let lock = take(context.fs, context.sleeper(), &request, &mut notes);
    for note in notes {
        transcript.err(note);
    }
    Ok(lock?)
}
