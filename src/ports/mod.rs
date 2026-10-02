//! Traits through which the domain and commands reach the outside world.

use std::io;
use std::path::Path;

pub trait FileSystem {
    /// # Errors
    /// Propagates the underlying I/O error.
    fn read_to_string(&self, path: &Path) -> io::Result<String>;

    /// Names (not paths) of the direct children of `path`.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn read_dir_names(&self, path: &Path) -> io::Result<Vec<String>>;

    fn is_file(&self, path: &Path) -> bool;
}

pub trait Env {
    fn var(&self, key: &str) -> Option<String>;
}
