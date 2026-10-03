//! `nvm deactivate`: take the nvm entries out of `PATH`, `MANPATH` and
//! `NODE_PATH`, and forget `NVM_BIN` and `NVM_INC`.
//!
//! The binary cannot change its parent shell, so the work is returned as a
//! [`Script`] for the `nvm` function to eval, next to the messages.

use crate::commands::Output;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::path_edit::strip_path;
use crate::error::{CliError, NvmExitCode};
use crate::shell::Script;

/// `nvm deactivate [--silent]`. Every other argument is ignored, and the
/// status is always 0, as in nvm.sh.
///
/// # Errors
/// As [`deactivate`].
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let silent = args.iter().any(|argument| argument == "--silent");
    let mut transcript = Transcript::default();
    let script = deactivate(context, silent, &mut transcript)?;
    Ok(transcript.finish_with(NvmExitCode::Success, script))
}

/// The deactivation shared with `use system` and the prefix checks: appends
/// nvm.sh's messages to `transcript` (the `*` and `${PATH}` in them are
/// literal text) and returns the shell code that applies it.
///
/// Deliberate deviation: when `NVM_DIR` cannot be resolved, nvm.sh's
/// `nvm_strip_path` prints `${NVM_DIR} not set!` and yields an empty string,
/// which its deactivate takes for a change and so empties `PATH`. Here the
/// same message goes to `transcript` and the call fails with
/// [`CliError::NvmDirUnresolved`] (status 1), producing no script, so nothing
/// is exported or unset.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `NVM_DIR` cannot be resolved, and
/// [`CliError::Shell`] only if the script cannot be built, which the fixed
/// variable names make a programming error.
pub fn deactivate(
    context: &Context<'_>,
    silent: bool,
    transcript: &mut Transcript,
) -> Result<Script, CliError> {
    let nvm_dir = context
        .nvm_dir()
        .inspect_err(|_| transcript.err("${NVM_DIR} not set!"))?;
    let nvm_dir = nvm_dir.to_string_lossy().into_owned();
    let stripper = Stripper {
        context,
        nvm_dir,
        silent,
    };
    let script = stripper.path(Script::new(), transcript)?;
    let script = stripper.manpath(script, transcript)?;
    let script = stripper.node_path(script, transcript)?;
    Ok(script.unset("NVM_BIN")?.unset("NVM_INC")?)
}

struct Stripper<'a, 'b> {
    context: &'a Context<'b>,
    nvm_dir: String,
    silent: bool,
}

impl Stripper<'_, '_> {
    fn value(&self, name: &str) -> String {
        self.context.env.var(name).unwrap_or_default()
    }

    fn shown_dir(&self) -> &str {
        &self.nvm_dir
    }

    fn removed(&self, suffix: &str, variable: &str, transcript: &mut Transcript) {
        if !self.silent {
            transcript.out(format!(
                "{}/*{suffix} removed from ${{{variable}}}",
                self.shown_dir()
            ));
        }
    }

    fn not_found(&self, suffix: &str, variable: &str, transcript: &mut Transcript) {
        if !self.silent {
            transcript.err(format!(
                "Could not find {}/*{suffix} in ${{{variable}}}",
                self.shown_dir()
            ));
        }
    }

    fn path(&self, script: Script, transcript: &mut Transcript) -> Result<Script, CliError> {
        let old = self.value("PATH");
        let new = strip_path(&old, "/bin", &self.nvm_dir);
        if new == old {
            self.not_found("/bin", "PATH", transcript);
            return Ok(script);
        }
        self.removed("/bin", "PATH", transcript);
        Ok(script.export("PATH", &new)?.hash_reset())
    }

    fn manpath(&self, script: Script, transcript: &mut Transcript) -> Result<Script, CliError> {
        let old = self.value("MANPATH");
        if old.is_empty() {
            return Ok(script);
        }
        let new = strip_path(&old, "/share/man", &self.nvm_dir);
        if new == old {
            self.not_found("/share/man", "MANPATH", transcript);
            return Ok(script);
        }
        self.removed("/share/man", "MANPATH", transcript);
        // `man` treats both of these as unset anyway.
        if new.is_empty() || new == ":" {
            Ok(script.unset("MANPATH")?)
        } else {
            Ok(script.export("MANPATH", &new)?)
        }
    }

    fn node_path(&self, script: Script, transcript: &mut Transcript) -> Result<Script, CliError> {
        let old = self.value("NODE_PATH");
        if old.is_empty() {
            return Ok(script);
        }
        let new = strip_path(&old, "/lib/node_modules", &self.nvm_dir);
        if new == old {
            return Ok(script);
        }
        self.removed("/lib/node_modules", "NODE_PATH", transcript);
        Ok(script.export("NODE_PATH", &new)?)
    }
}

#[cfg(test)]
mod tests;
