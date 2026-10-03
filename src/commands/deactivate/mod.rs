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
    let mut script = Script::new();
    for change in changes(context, silent, transcript)? {
        script = match change.value {
            Some(value) => script.export(change.name, &value)?,
            None => script.unset(change.name)?,
        };
        if change.name == "PATH" {
            script = script.hash_reset();
        }
    }
    Ok(script)
}

/// A variable deactivating changes: its new value, or `None` to unset it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub name: &'static str,
    pub value: Option<String>,
}

/// What deactivating does to the environment, in nvm.sh's order (`PATH`,
/// `MANPATH`, `NODE_PATH`, then `NVM_BIN` and `NVM_INC` unset), with its
/// messages appended to `transcript`; a variable it leaves alone is absent.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `NVM_DIR` cannot be resolved, after
/// `${NVM_DIR} not set!` went to `transcript`.
pub fn changes(
    context: &Context<'_>,
    silent: bool,
    transcript: &mut Transcript,
) -> Result<Vec<Change>, CliError> {
    let nvm_dir = context
        .nvm_dir()
        .inspect_err(|_| transcript.err("${NVM_DIR} not set!"))?;
    let stripper = Stripper {
        context,
        nvm_dir: nvm_dir.to_string_lossy().into_owned(),
        silent,
    };
    let stripped = [
        stripper.path(transcript),
        stripper.manpath(transcript),
        stripper.node_path(transcript),
    ];
    let unset = ["NVM_BIN", "NVM_INC"].map(|name| Some(Change { name, value: None }));
    Ok(stripped.into_iter().chain(unset).flatten().collect())
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

    fn removed(&self, suffix: &str, variable: &str, transcript: &mut Transcript) {
        if !self.silent {
            transcript.out(format!(
                "{}/*{suffix} removed from ${{{variable}}}",
                self.nvm_dir
            ));
        }
    }

    fn not_found(&self, suffix: &str, variable: &str, transcript: &mut Transcript) {
        if !self.silent {
            transcript.err(format!(
                "Could not find {}/*{suffix} in ${{{variable}}}",
                self.nvm_dir
            ));
        }
    }

    /// The stripped value of `name`, or `None` when stripping changes
    /// nothing.
    fn strip(&self, name: &str, suffix: &str) -> Option<String> {
        let old = self.value(name);
        let new = strip_path(&old, suffix, &self.nvm_dir);
        (new != old).then_some(new)
    }

    fn path(&self, transcript: &mut Transcript) -> Option<Change> {
        let Some(new) = self.strip("PATH", "/bin") else {
            self.not_found("/bin", "PATH", transcript);
            return None;
        };
        self.removed("/bin", "PATH", transcript);
        Some(Change {
            name: "PATH",
            value: Some(new),
        })
    }

    fn manpath(&self, transcript: &mut Transcript) -> Option<Change> {
        if self.value("MANPATH").is_empty() {
            return None;
        }
        let Some(new) = self.strip("MANPATH", "/share/man") else {
            self.not_found("/share/man", "MANPATH", transcript);
            return None;
        };
        self.removed("/share/man", "MANPATH", transcript);
        // `man` treats both of these as unset anyway.
        let value = (!new.is_empty() && new != ":").then_some(new);
        Some(Change {
            name: "MANPATH",
            value,
        })
    }

    fn node_path(&self, transcript: &mut Transcript) -> Option<Change> {
        if self.value("NODE_PATH").is_empty() {
            return None;
        }
        let new = self.strip("NODE_PATH", "/lib/node_modules")?;
        self.removed("/lib/node_modules", "NODE_PATH", transcript);
        Some(Change {
            name: "NODE_PATH",
            value: Some(new),
        })
    }
}

#[cfg(test)]
mod tests;
