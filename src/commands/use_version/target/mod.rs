//! What `nvm use` switches to, or why it stops: steps 1 to 9 of nvm.sh's
//! `use` (digest 4.2), with its messages in the same order per stream.

pub mod lookup;

use std::path::PathBuf;

use lookup::{Resolution, ensure_installed, is_installed, match_version, nvm_version, system_node};

use crate::commands::install::place::version_path;
use crate::commands::nvmrc_file;
use crate::commands::rc_version::{RcVersion, please_see, rc_version};
use crate::commands::transcript::Transcript;
use crate::commands::use_version::Options;
use crate::commands::use_version::messages::{SYSTEM_NOT_FOUND, infinite_loop};
use crate::context::Context;
use crate::domain::version::Version;
use crate::error::{CliError, NvmExitCode};

/// The version `nvm use` switches to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// An installed version whose `bin/node` can run.
    Installed {
        version: Version,
        /// `IS_VERSION_FROM_NVMRC`: the version came from a `.nvmrc`.
        from_nvmrc: bool,
        /// `PROVIDED_VERSION`: what was asked for (the `.nvmrc` text
        /// included); `None` for `--lts` alone. The prefix-check command
        /// names the version only when this is set.
        provided: Option<String>,
        /// `nvm_version_path`: where the version lives.
        directory: PathBuf,
    },
    /// The `node` or `iojs` left once the nvm entries leave `PATH`.
    System(SystemNode),
}

/// Which program the system version is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemFlavor {
    Node,
    IoJs,
}

/// A `node` (or `iojs`) outside `$NVM_DIR`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemNode {
    pub flavor: SystemFlavor,
    pub binary: PathBuf,
}

/// `nvm use` stops: its messages are already in the transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Halt {
    pub status: NvmExitCode,
}

impl Halt {
    #[must_use]
    pub fn new(status: NvmExitCode) -> Self {
        Self { status }
    }

    /// Prints `error` on stderr and stops with its status.
    pub fn report(error: &CliError, transcript: &mut Transcript) -> Self {
        transcript.err(error.to_string());
        Self::new(error.exit_code())
    }
}

/// Why resolution stopped: already reported, or an error still to print.
enum Stop {
    Halt(Halt),
    Error(CliError),
}

impl From<Halt> for Stop {
    fn from(halt: Halt) -> Self {
        Self::Halt(halt)
    }
}

impl From<CliError> for Stop {
    fn from(error: CliError) -> Self {
        Self::Error(error)
    }
}

/// What was asked for, and what `nvm_version` made of it (`VERSION`).
struct Request {
    provided: Option<String>,
    from_nvmrc: bool,
    resolution: Resolution,
}

/// Resolves the version `options` ask for, in nvm.sh's order: `--lts`, the
/// version given, the `.nvmrc`; `--save` writes it before it is checked.
///
/// # Errors
/// A [`Halt`] with its messages in `transcript`: 127 for no usable version
/// or no system `node`, 8 for an alias loop, 3 for a version that is not
/// installed, 1 when `$NVM_DIR` cannot be found.
pub fn resolve(
    context: &Context<'_>,
    options: &Options,
    transcript: &mut Transcript,
) -> Result<Target, Halt> {
    match settle(context, options, transcript) {
        Ok(target) => Ok(target),
        Err(Stop::Halt(halt)) => Err(halt),
        Err(Stop::Error(error)) => Err(Halt::report(&error, transcript)),
    }
}

fn settle(
    context: &Context<'_>,
    options: &Options,
    transcript: &mut Transcript,
) -> Result<Target, Stop> {
    let request = request(context, options, transcript)?;
    if options.save {
        save(context, &request.resolution, options.silent, transcript)?;
    }
    decide(context, request, options.silent, transcript)
}

fn request(
    context: &Context<'_>,
    options: &Options,
    transcript: &mut Transcript,
) -> Result<Request, Stop> {
    let asked = match (&options.lts, &options.version) {
        (Some(lts), _) => format!("lts/{lts}"),
        (None, Some(version)) => version.clone(),
        (None, None) => return from_nvmrc(context, options.silent, transcript),
    };
    Ok(Request {
        provided: options.version.clone(),
        from_nvmrc: false,
        resolution: match_version(context, &asked)?,
    })
}

/// Step 2: the `.nvmrc`, resolved with `nvm_version` (so `system` needs a
/// system `node` here). Nothing usable is `Please see ...` and 127.
fn from_nvmrc(
    context: &Context<'_>,
    silent: bool,
    transcript: &mut Transcript,
) -> Result<Request, Stop> {
    let RcVersion::Found { version, .. } = rc_version(context, silent, transcript) else {
        please_see(transcript);
        return Err(Halt::new(NvmExitCode::NotFound).into());
    };
    let resolution = nvm_version(context, &version)?;
    Ok(Request {
        provided: Some(version),
        from_nvmrc: true,
        resolution,
    })
}

/// Step 5, `nvm_write_nvmrc`: only what resolves again to an installed
/// version or an existing system is written; a failed write is a warning.
fn save(
    context: &Context<'_>,
    resolution: &Resolution,
    silent: bool,
    transcript: &mut Transcript,
) -> Result<(), CliError> {
    let writable = match resolution {
        Resolution::Version(version) => is_installed(context, version)?,
        Resolution::System => system_node(context)?.is_some(),
        Resolution::Loop | Resolution::NotAvailable | Resolution::Inactive => false,
    };
    if writable {
        // nvm.sh ignores the status: a failed write never stops `use`.
        let _status = nvmrc_file::write(context, &resolution.text(), silent, transcript);
    }
    Ok(())
}

/// Steps 6 to 9.
fn decide(
    context: &Context<'_>,
    request: Request,
    silent: bool,
    transcript: &mut Transcript,
) -> Result<Target, Stop> {
    let provided = request.provided.as_deref().unwrap_or_default();
    match &request.resolution {
        Resolution::System => system(context, silent, transcript),
        Resolution::Loop => {
            if !silent {
                transcript.err(infinite_loop(provided));
            }
            Err(Halt::new(NvmExitCode::AliasLoop).into())
        }
        Resolution::NotAvailable => {
            if !silent {
                ensure_installed(context, provided, request.from_nvmrc, transcript)?;
            }
            Err(Halt::new(NvmExitCode::InvalidVersion).into())
        }
        Resolution::Version(_) | Resolution::Inactive => installed(context, request, transcript),
    }
}

fn system(
    context: &Context<'_>,
    silent: bool,
    transcript: &mut Transcript,
) -> Result<Target, Stop> {
    if let Some(node) = system_node(context)? {
        return Ok(Target::System(node));
    }
    if !silent {
        transcript.err(SYSTEM_NOT_FOUND);
    }
    Err(Halt::new(NvmExitCode::NotFound).into())
}

/// Step 9: the resolved version must have a `bin/node` that can run; its
/// messages print even with `--silent`, as in nvm.sh.
///
/// DELIBERATE DEVIATION: nvm.sh returns 0 here after the `N/A` message (it
/// returns the status of a `!` test, e.g. `nvm use current` with nothing
/// active); this returns 3, the status of every other "not installed".
fn installed(
    context: &Context<'_>,
    request: Request,
    transcript: &mut Transcript,
) -> Result<Target, Stop> {
    let shown = request.resolution.text();
    let installed = ensure_installed(context, &shown, request.from_nvmrc, transcript)?;
    let Resolution::Version(version) = request.resolution else {
        return Err(Halt::new(NvmExitCode::InvalidVersion).into());
    };
    if !installed {
        return Err(Halt::new(NvmExitCode::InvalidVersion).into());
    }
    Ok(Target::Installed {
        directory: version_path(context, &version)?,
        version,
        from_nvmrc: request.from_nvmrc,
        provided: request.provided,
    })
}

#[cfg(test)]
mod tests;
