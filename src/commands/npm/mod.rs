//! Running the `npm` of a node version: the one thing `nvm` asks of a version
//! it has installed besides `node` itself. `nvm.sh` has `npm` on its `PATH`
//! because it activated the version; here the version's `bin` directory is put
//! in front of the `PATH` of each run instead.

pub mod latest;

use std::path::PathBuf;

use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::path_search::find_in_path;
use crate::ports::Invocation;

/// An `npm` that exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Npm {
    program: PathBuf,
    /// Its directory, which also holds the `node` it belongs to.
    bin_dir: PathBuf,
}

impl Npm {
    /// The `npm` in `<version_path>/bin`, when the version has one.
    #[must_use]
    pub fn in_version(context: &Context<'_>, version_path: &std::path::Path) -> Option<Self> {
        let bin_dir = version_path.join("bin");
        let program = bin_dir.join("npm");
        let info = context.fs.file_info(&program).ok()?;
        (!info.is_dir).then_some(Self { program, bin_dir })
    }

    /// The first `npm` on `PATH`.
    #[must_use]
    pub fn on_path(context: &Context<'_>) -> Option<Self> {
        let path_variable = context.env.var_os("PATH").unwrap_or_default();
        let program = find_in_path(context.fs, &path_variable, "npm")?;
        let bin_dir = program.parent()?.to_path_buf();
        Some(Self { program, bin_dir })
    }

    fn invocation(&self, args: &[&str]) -> Invocation {
        Invocation::new(&self.program)
            .args(args)
            .path_prefix(&self.bin_dir)
    }

    /// What `npm --version` prints, when it prints something.
    #[must_use]
    pub fn version(&self, context: &Context<'_>) -> Option<String> {
        let done = context
            .process()
            .execute(&self.invocation(&["--version"]))
            .ok()?;
        let text = done.stdout.trim().to_owned();
        (done.success && !text.is_empty()).then_some(text)
    }

    /// Runs `npm` with `args`, its output going on to the transcript like it
    /// goes to the terminal in `nvm.sh`. True when it succeeded.
    pub fn run(&self, context: &Context<'_>, args: &[&str], transcript: &mut Transcript) -> bool {
        match context.process().execute(&self.invocation(args)) {
            Ok(done) => {
                done.stdout.lines().for_each(|line| transcript.out(line));
                done.stderr.lines().for_each(|line| transcript.err(line));
                done.success
            }
            Err(error) => {
                transcript.err(format!("{}: {error}", self.program.display()));
                false
            }
        }
    }

    /// Runs `npm` for its output (`npm list -g`), which is returned instead.
    #[must_use]
    pub fn output(&self, context: &Context<'_>, args: &[&str]) -> Option<String> {
        let done = context.process().execute(&self.invocation(args)).ok()?;
        done.success.then_some(done.stdout)
    }
}

#[cfg(test)]
mod tests;
