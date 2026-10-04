use std::io;
use std::path::{Path, PathBuf};

/// What a finished child process left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub success: bool,
    pub stdout: String,
}

/// A program to run to completion, with no time limit: `npm install`,
/// `./configure`, `make`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Invocation {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Where to run it; the current directory when `None`.
    pub dir: Option<PathBuf>,
    /// Variables added to the environment it inherits.
    pub env: Vec<(String, String)>,
    /// Variables taken out of the environment it inherits.
    pub env_remove: Vec<String>,
    /// A directory put in front of `PATH`, so a `node` that `npm` starts is
    /// the one next to it.
    pub path_prefix: Option<PathBuf>,
}

impl Invocation {
    #[must_use]
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn args(mut self, args: &[&str]) -> Self {
        self.args.extend(args.iter().map(|arg| (*arg).to_owned()));
        self
    }

    #[must_use]
    pub fn dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.dir = Some(dir.into());
        self
    }

    #[must_use]
    pub fn env(mut self, name: &str, value: &str) -> Self {
        self.env.push((name.to_owned(), value.to_owned()));
        self
    }

    #[must_use]
    pub fn env_remove(mut self, name: &str) -> Self {
        self.env_remove.push(name.to_owned());
        self
    }

    #[must_use]
    pub fn path_prefix(mut self, directory: impl Into<PathBuf>) -> Self {
        self.path_prefix = Some(directory.into());
        self
    }
}

/// What a program that ran to the end printed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completed {
    pub success: bool,
    /// The exit status; `128 + n` for a program killed by signal `n`
    /// (Unix), as a shell's `$?`. `None` only from fakes.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub trait Process {
    /// Runs `invocation` until it ends and returns everything it printed.
    ///
    /// # Errors
    /// Fails when the program cannot be started.
    fn execute(&self, invocation: &Invocation) -> io::Result<Completed>;

    /// Runs `program` with `args` and waits for it.
    ///
    /// # Errors
    /// Fails when the program cannot be started.
    fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput>;

    /// Runs `invocation` with stdin, stdout and stderr inherited from this
    /// process, waits for it, and returns its exit status; a child killed by
    /// a signal gives `128 + signal` (Unix).
    ///
    /// `invocation.env_remove` is taken out of the inherited environment,
    /// `invocation.env` is added to it, and an entry named
    /// `PATH` replaces the `PATH` the child would inherit. `path_prefix` is
    /// then put in front of whichever `PATH` the child ends up with. A
    /// program named without a `/` is looked up on that final `PATH`. No
    /// program gets `NVMRC_SCRIPT_FD`, whatever `invocation` says.
    ///
    /// # Errors
    /// Fails when the program cannot be started (`NotFound` when it does not
    /// exist).
    fn spawn(&self, invocation: &Invocation) -> io::Result<i32>;
}
