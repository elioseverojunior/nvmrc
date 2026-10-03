//! `nvm run` (digest 7): pick a version like nvm.sh, then hand `node` (or
//! `iojs`) and the remaining arguments to `nvm exec`, as nvm.sh does.
//!
//! DELIBERATE DEVIATION: nvm.sh prints its help when any argument before a
//! `--` is `-h`, `help` or `--help` (so `nvm run 18 -h` shows nvm's help);
//! here every argument after the subcommand reaches `node`.

use crate::commands::Output;
use crate::commands::exec::{fallback_warning, run_after};
use crate::commands::rc_version::{RcVersion, rc_version};
use crate::commands::transcript::Transcript;
use crate::commands::use_version::target::lookup::{Resolution, ensure_installed, nvm_version};
use crate::context::Context;
use crate::domain::valid_version::is_valid_version;
use crate::error::{CliError, NvmExitCode};

const USAGE: [&str; 3] = [
    "Usage: nvm run [<version>] [<args>]",
    "  Provide a <version>, or run from a directory containing an .nvmrc file.",
    "  Run `nvm --help` for full help.",
];

/// `nvm run [--silent] [--lts[=<name>]] [<version>] [<args>]`.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let options = parse(args);
    let mut transcript = Transcript::default();
    match plan(context, &options, &mut transcript)? {
        Plan::Stop(status) => Ok(transcript.finish(status)),
        Plan::Exec(exec_args) => run_after(context, &exec_args, transcript),
    }
}

/// Options up to the first other argument (`--` and unknown `--x`
/// included), which starts the version or the arguments of `node`.
#[derive(Debug, Default)]
struct Options {
    silent: bool,
    /// `--lts` (`*`) or `--lts=<name>`; `--lts=` is no LTS.
    lts: Option<String>,
    rest: Vec<String>,
}

fn parse(args: &[String]) -> Options {
    let mut options = Options::default();
    for (index, arg) in args.iter().enumerate() {
        match arg.as_str() {
            "--silent" => options.silent = true,
            "--lts" => options.lts = Some("*".to_owned()),
            "" => {}
            other => match other.strip_prefix("--lts=") {
                Some(name) => options.lts = Some(name.to_owned()).filter(|name| !name.is_empty()),
                None => {
                    options.rest = args[index..].to_vec();
                    break;
                }
            },
        }
    }
    options
}

enum Plan {
    Stop(NvmExitCode),
    /// The arguments of the `nvm exec` to hand over to.
    Exec(Vec<String>),
}

fn plan(
    context: &Context<'_>,
    options: &Options,
    transcript: &mut Transcript,
) -> Result<Plan, CliError> {
    if let Some(lts) = &options.lts {
        return Ok(exec(options, format!("--lts={lts}"), "node", &options.rest));
    }
    let Some((first, rest)) = options.rest.split_first() else {
        return alone(context, options, transcript);
    };
    let resolution = nvm_version(context, first)?;
    if resolution != Resolution::NotAvailable || is_valid_version(first) {
        return given(context, first, &resolution, (options, rest), transcript);
    }
    // `IS_VERSION_FROM_NVMRC=1` even for the active-version fallback.
    let provided = rc_or_current(context, options.silent, transcript)?;
    let resolution = nvm_version(context, &provided)?;
    if resolution == Resolution::NotAvailable {
        ensure_installed(context, &provided, true, transcript)?;
        return Ok(Plan::Stop(NvmExitCode::Failure));
    }
    Ok(exec(
        options,
        resolution.text(),
        program(&resolution),
        &options.rest,
    ))
}

/// A version was given (`$1` resolves, or looks like a version).
fn given(
    context: &Context<'_>,
    provided: &str,
    resolution: &Resolution,
    (options, rest): (&Options, &[String]),
    transcript: &mut Transcript,
) -> Result<Plan, CliError> {
    if *resolution == Resolution::NotAvailable {
        ensure_installed(context, provided, false, transcript)?;
        return Ok(Plan::Stop(NvmExitCode::Failure));
    }
    Ok(exec(options, resolution.text(), program(resolution), rest))
}

/// No argument: the `.nvmrc` version, or the usage and 127.
fn alone(
    context: &Context<'_>,
    options: &Options,
    transcript: &mut Transcript,
) -> Result<Plan, CliError> {
    if let RcVersion::Found { version, .. } = rc_version(context, options.silent, transcript) {
        let resolution = nvm_version(context, &version)?;
        if resolution != Resolution::NotAvailable {
            return Ok(exec(options, resolution.text(), program(&resolution), &[]));
        }
    }
    USAGE.into_iter().for_each(|line| transcript.err(line));
    Ok(Plan::Stop(NvmExitCode::NotFound))
}

/// The `.nvmrc` version, else (with the `nvm run` warning) the text of the
/// active version.
fn rc_or_current(
    context: &Context<'_>,
    silent: bool,
    transcript: &mut Transcript,
) -> Result<String, CliError> {
    if let RcVersion::Found { version, .. } = rc_version(context, silent, transcript) {
        return Ok(version);
    }
    if !silent {
        fallback_warning("run")
            .into_iter()
            .for_each(|line| transcript.err(line));
    }
    Ok(nvm_version(context, "current")?.text())
}

fn program(resolution: &Resolution) -> &'static str {
    if resolution.text().starts_with("iojs-") {
        "iojs"
    } else {
        "node"
    }
}

/// `nvm exec [--silent] <lead> <program> <rest>`, where `lead` is the
/// version or `--lts=<name>`.
fn exec(options: &Options, lead: String, program: &str, rest: &[String]) -> Plan {
    let silent = options.silent.then(|| "--silent".to_owned());
    let words = [lead, program.to_owned()];
    Plan::Exec(
        silent
            .into_iter()
            .chain(words)
            .chain(rest.iter().cloned())
            .collect(),
    )
}

#[cfg(test)]
mod tests;
