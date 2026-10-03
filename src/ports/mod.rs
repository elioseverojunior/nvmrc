//! Traits through which the domain and commands reach the outside world.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::time::Duration;

use thiserror::Error;

/// A direct child of a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
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

pub trait Process {
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
    /// Fetches `url` and returns the body as text.
    ///
    /// # Errors
    /// Fails on a non-success status, on a network error, or when the body is
    /// not text.
    fn get_text(&self, url: &str) -> Result<String, HttpError>;
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
