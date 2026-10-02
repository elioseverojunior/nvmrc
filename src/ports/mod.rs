//! Traits through which the domain and commands reach the outside world.

use std::ffi::OsString;
use std::io;
use std::path::Path;

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
}

pub trait Env {
    /// The variable as text; `None` when unset or not valid UTF-8.
    fn var(&self, key: &str) -> Option<String>;

    /// The variable as an OS string, so non-UTF-8 paths survive.
    fn var_os(&self, key: &str) -> Option<OsString>;
}
