//! Steps 6 and 10 to 13 of nvm.sh's `use` (digest 4.2): the shell code that
//! switches to the resolved version, the `NVM_SYMLINK_CURRENT` link, and the
//! `Now using ...` line, whose npm suffix comes from the **new** `PATH`, with
//! the prefix checks of step 12 between them.

#[cfg(test)]
mod tests;

use std::ffi::OsStr;
use std::io;
use std::path::Path;

use crate::commands::Output;
use crate::commands::deactivate::deactivate;
use crate::commands::npm::Npm;
use crate::commands::transcript::Transcript;
use crate::commands::use_version::prefix::{self, Refusal};
use crate::commands::use_version::{Options, SystemFlavor, SystemNode, Target};
use crate::context::Context;
use crate::domain::path_edit::{change_path, manpath_with_trailing_colon, strip_path};
use crate::domain::path_search::find_in_path;
use crate::domain::version::{Flavor, Version};
use crate::error::{CliError, NvmExitCode};
use crate::shell::{Script, ShellError};

/// Switches to `target`, with everything already in `transcript` printed
/// first.
pub(super) fn apply(
    context: &Context<'_>,
    options: &Options,
    target: &Target,
    transcript: Transcript,
) -> Result<Output, CliError> {
    match target {
        Target::System(node) => system(context, options, node, transcript),
        Target::Installed {
            version,
            directory,
            provided,
            ..
        } => {
            let command = prefix::command(version, provided.is_some(), options.silent);
            installed(context, options, (version, directory, &command), transcript)
        }
    }
}

/// Steps 10 to 13: the switch, then the prefix checks, then the
/// `Now using ...` line, which a refused prefix leaves out.
fn installed(
    context: &Context<'_>,
    options: &Options,
    (version, directory, command): (&Version, &Path, &str),
    mut transcript: Transcript,
) -> Result<Output, CliError> {
    let nvm_dir = context.nvm_dir()?;
    let switch = Switch {
        nvm_dir: text(&nvm_dir),
        directory: text(directory),
    };
    let path = switch.change(&context.env.var("PATH").unwrap_or_default(), "/bin");
    let script = switch.script(context, &path)?;
    link_current(context, &nvm_dir, directory, &mut transcript);
    let message = (!options.silent).then(|| now_using(version, &npm_suffix(context, &path)));
    let check = prefix::Check {
        nvm_dir: &switch.nvm_dir,
        directory: &switch.directory,
        path: &path,
        command,
        delete: options.delete_prefix,
    };
    if let Err(refusal) = prefix::check(context, &check, &mut transcript) {
        let script = after_refusal(context, refusal, script)?;
        return Ok(transcript.finish_with(NvmExitCode::IncompatiblePrefix, script));
    }
    message.into_iter().for_each(|line| transcript.out(line));
    Ok(transcript.finish_with(NvmExitCode::Success, script))
}

/// Step 12's failure: a refused variable makes nvm.sh run a silent
/// `nvm deactivate` (its messages discarded), so the switch gives way to
/// it; a refused npmrc file leaves the switch applied.
fn after_refusal(
    context: &Context<'_>,
    refusal: Refusal,
    switch: Script,
) -> Result<Script, CliError> {
    match refusal {
        Refusal::KeepSwitch => Ok(switch),
        Refusal::Deactivate => deactivate(context, true, &mut Transcript::default()),
    }
}

/// The directories one switch rewrites `PATH` and `MANPATH` with.
struct Switch {
    nvm_dir: String,
    directory: String,
}

impl Switch {
    fn change(&self, value: &str, suffix: &str) -> String {
        change_path(value, suffix, &self.directory, &self.nvm_dir)
    }

    /// `MANPATH` (only when a `manpath` program is on the new `PATH`), then
    /// `PATH`, `hash -r`, `NVM_BIN` and `NVM_INC`, in nvm.sh's order.
    fn script(&self, context: &Context<'_>, path: &str) -> Result<Script, ShellError> {
        let script = match self.manpath(context, path) {
            Some(manpath) => Script::new().export("MANPATH", &manpath)?,
            None => Script::new(),
        };
        let directory = &self.directory;
        script
            .export("PATH", path)?
            .hash_reset()
            .export("NVM_BIN", &format!("{directory}/bin"))?
            .export("NVM_INC", &format!("{directory}/include/node"))
    }

    fn manpath(&self, context: &Context<'_>, path: &str) -> Option<String> {
        find_in_path(context.fs, OsStr::new(path), "manpath")?;
        let old = context.env.var("MANPATH").unwrap_or_default();
        Some(manpath_with_trailing_colon(
            &self.change(&old, "/share/man"),
        ))
    }
}

/// `NVM_SYMLINK_CURRENT=true` (exactly): `rm -f $NVM_DIR/current && ln -s
/// <directory> $NVM_DIR/current`. A failure is reported and `use` goes on.
fn link_current(
    context: &Context<'_>,
    nvm_dir: &Path,
    directory: &Path,
    transcript: &mut Transcript,
) {
    if context.env.var("NVM_SYMLINK_CURRENT").as_deref() != Some("true") {
        return;
    }
    let link = nvm_dir.join("current");
    let linked = match context.fs.remove_file(&link) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => context.fs.symlink(directory, &link),
    };
    if let Err(error) = linked {
        transcript.err(CliError::io(&link, error).to_string());
    }
}

/// Step 6: a silent deactivate whose messages are discarded, then the
/// version of the system `node` (or `iojs`). The prefix checks never run.
fn system(
    context: &Context<'_>,
    options: &Options,
    node: &SystemNode,
    mut transcript: Transcript,
) -> Result<Output, CliError> {
    let script = deactivate(context, true, &mut Transcript::default())?;
    if !options.silent {
        let nvm_dir = text(&context.nvm_dir()?);
        let path = strip_path(
            &context.env.var("PATH").unwrap_or_default(),
            "/bin",
            &nvm_dir,
        );
        transcript.out(system_now_using(context, node, &path));
    }
    Ok(transcript.finish_with(NvmExitCode::Success, script))
}

fn system_now_using(context: &Context<'_>, node: &SystemNode, path: &str) -> String {
    let name = match node.flavor {
        SystemFlavor::Node => "node",
        SystemFlavor::IoJs => "io.js",
    };
    // `$(node -v 2>/dev/null)`: whatever it printed, nothing if it failed.
    let version = context
        .process()
        .run(&node.binary, &["--version"])
        .map(|output| output.stdout.trim().to_owned())
        .unwrap_or_default();
    let npm = npm_suffix(context, path);
    format!("Now using system version of {name}: {version}{npm}")
}

/// Step 11: `Now using node v18.20.4`, or `Now using io.js v3.3.1`.
fn now_using(version: &Version, npm: &str) -> String {
    match version.flavor {
        Flavor::Node => format!("Now using node {version}{npm}"),
        Flavor::IoJs => format!("Now using io.js {}{npm}", version.directory_name()),
    }
}

/// `nvm_print_npm_version` with `path` as `PATH`: ` (npm v<version>)`, or
/// nothing when there is no `npm` or it prints nothing.
fn npm_suffix(context: &Context<'_>, path: &str) -> String {
    Npm::on_path_value(context, OsStr::new(path))
        .and_then(|npm| npm.version(context))
        .map_or_else(String::new, |version| format!(" (npm v{version})"))
}

fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
