//! Which version an install is about: the one the mirror says a description
//! stands for, and whether it may be installed (the version floor).

use crate::commands::install::flow::{Halt, Step};
use crate::commands::install::offline::cached_version;
use crate::commands::install::options::Options;
use crate::commands::resolve::{Resolved, resolve_installed};
use crate::commands::transcript::Transcript;
use crate::commands::version_remote::lookup;
use crate::context::Context;
use crate::domain::alias::{AliasStore, resolve as resolve_alias};
use crate::domain::floor::VersionFloor;
use crate::domain::remote::Query;
use crate::domain::version::Version;
use crate::error::NvmExitCode;

pub(super) fn resolve(
    context: &Context<'_>,
    options: &Options,
    transcript: &mut Transcript,
) -> Step<Version> {
    if options.offline {
        return resolve_offline(context, options, transcript);
    }
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

/// `--offline`: an installed version first, then the newest cached archive.
fn resolve_offline(
    context: &Context<'_>,
    options: &Options,
    transcript: &mut Transcript,
) -> Step<Version> {
    let pattern = offline_pattern(context, options, transcript)?;
    if let Resolved::Installed(version) = resolve_installed(context, &pattern)? {
        return Ok(version);
    }
    cached_version(context, &pattern).ok_or_else(|| {
        transcript.err(not_found_message(options));
        Halt::Exit(NvmExitCode::InvalidVersion)
    })
}

/// What to look for offline: the version as given, or what the local `lts/*`
/// alias stands for, as nothing can be asked of the mirror.
fn offline_pattern(
    context: &Context<'_>,
    options: &Options,
    transcript: &mut Transcript,
) -> Step<String> {
    let Some(lts) = options.lts.as_deref() else {
        return Ok(options.version.clone());
    };
    let name = if lts == "*" {
        "lts/*".to_owned()
    } else {
        format!("lts/{lts}")
    };
    let store = context.alias_store()?;
    let resolved = store
        .target(&name)
        .and_then(|_| resolve_alias(&store, &name).ok());
    resolved.ok_or_else(|| {
        transcript.err(format!(
            "LTS alias '{lts}' not found locally. Run `nvm ls-remote --lts` first to populate LTS aliases."
        ));
        Halt::Exit(NvmExitCode::InvalidVersion)
    })
}

/// What `nvm.sh` says when nothing matches, and where to look instead.
fn not_found_message(options: &Options) -> String {
    let version = &options.version;
    let (filter, command) = match options.lts.as_deref() {
        Some("*") => (
            " (with LTS filter)".to_owned(),
            "nvm ls-remote --lts".to_owned(),
        ),
        Some(lts) if version.is_empty() => {
            return format!(
                "Version with LTS filter '{lts}' not found - try `nvm ls-remote --lts={lts}` to browse available versions."
            );
        }
        Some(lts) => (
            format!(" (with LTS filter '{lts}')"),
            format!("nvm ls-remote --lts={lts}"),
        ),
        None if options.offline => (String::new(), "nvm ls".to_owned()),
        None => (String::new(), "nvm ls-remote".to_owned()),
    };
    let wherever = if options.offline {
        " locally or in cache"
    } else {
        ""
    };
    format!(
        "Version '{version}'{filter} not found{wherever} - try `{command}` to browse available versions."
    )
}

/// How the floor messages name the file before the real path replaces it.
const MIN_VERSION_FILE: &str = "$NVM_DIR/min-version";

pub(super) fn check_floor(
    context: &Context<'_>,
    version: &Version,
    transcript: &mut Transcript,
) -> Step<()> {
    let file = context.nvm_dir()?.join("min-version");
    let from_file = context.fs.read_to_string(&file).ok();
    let from_env = context.env.var("NVM_MIN_VERSION");
    let floor = VersionFloor::from_sources(from_env.as_deref(), from_file.as_deref())
        .and_then(|floor| floor.map_or(Ok(()), |floor| floor.check(version)));
    let Err(error) = floor else {
        return Ok(());
    };
    let shown = file.display().to_string();
    transcript.err(error.to_string().replace(MIN_VERSION_FILE, &shown));
    if matches!(error, crate::error::FloorError::Below { .. }) {
        transcript.err(format!(
            "Lower or unset NVM_MIN_VERSION (or edit {shown}) to install it."
        ));
    }
    Err(Halt::Exit(NvmExitCode::BelowVersionFloor))
}
