//! Traits through which the domain and commands reach the outside world.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use thiserror::Error;

/// A direct child of a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// What a path is, following symlinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileInfo {
    pub is_dir: bool,
    pub len: u64,
    /// Any execute permission bit is set (always true off Unix).
    pub executable: bool,
    pub modified: Option<SystemTime>,
}

pub trait FileSystem {
    /// # Errors
    /// Propagates the underlying I/O error.
    fn read_to_string(&self, path: &Path) -> io::Result<String>;

    /// The direct children of `path`, in unspecified order.
    ///
    /// # Errors
    /// Propagates the underlying I/O error (for example when `path` is missing
    /// or is not a directory).
    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>>;

    fn is_file(&self, path: &Path) -> bool;

    /// # Errors
    /// Propagates the underlying I/O error (for example when `path` is missing).
    fn remove_file(&self, path: &Path) -> io::Result<()>;

    /// Removes `path` and everything under it, like `rm -rf`: a path that is
    /// missing is fine.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;

    /// Creates or replaces the file at `path`.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()>;

    /// Like [`Self::write_file`], for contents that are not text.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn write_bytes(&self, path: &Path, contents: &[u8]) -> io::Result<()>;

    /// # Errors
    /// Propagates the underlying I/O error (for example when `path` is missing).
    fn file_info(&self, path: &Path) -> io::Result<FileInfo>;

    /// Moves a file or a directory (with everything in it) to `to`, which must
    /// not exist.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// Creates one directory, failing with `AlreadyExists` when it is there:
    /// the atomic step a lock is made of.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn create_dir(&self, path: &Path) -> io::Result<()>;

    /// Creates `path` and any missing parents; fine if it already exists.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
}

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
    pub fn path_prefix(mut self, directory: impl Into<PathBuf>) -> Self {
        self.path_prefix = Some(directory.into());
        self
    }
}

/// What a program that ran to the end printed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completed {
    pub success: bool,
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
}

/// Why a download failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HttpError {
    #[error("{url}: HTTP {code}")]
    Status { url: String, code: u16 },
    #[error("{url}: {message}")]
    Transport { url: String, message: String },
    /// The server answered, but the body is not usable (too large, or not
    /// text). Asking again would not change that.
    #[error("{url}: {message}")]
    Body { url: String, message: String },
}

pub trait Http {
    /// Fetches `url` and returns the body as it is (an archive, say).
    ///
    /// # Errors
    /// Fails on a non-success status, on a network error, or when the body is
    /// over the size limit.
    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError>;

    /// Fetches `url` and returns the body as text.
    ///
    /// # Errors
    /// Fails on a non-success status, on a network error, or when the body is
    /// not text.
    fn get_text(&self, url: &str) -> Result<String, HttpError>;
}

pub trait Digest {
    /// The SHA-256 of the file at `path`, in lowercase hex.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn sha256_file(&self, path: &Path) -> io::Result<String>;
}

pub trait Archive {
    /// Unpacks the `.tar.gz` at `archive` into the existing directory
    /// `destination`, keeping every path inside it.
    ///
    /// # Errors
    /// Propagates the underlying I/O error, and fails on a file that is not a
    /// gzip-compressed tar.
    fn extract(&self, archive: &Path, destination: &Path) -> io::Result<()>;
}

pub trait Sleeper {
    /// Waits for `duration` (between retries).
    fn sleep(&self, duration: Duration);
}

pub trait Env {
    /// The variable as text; `None` when unset or not valid UTF-8.
    fn var(&self, key: &str) -> Option<String>;

    /// The variable as an OS string, so non-UTF-8 paths survive.
    fn var_os(&self, key: &str) -> Option<OsString>;
}
