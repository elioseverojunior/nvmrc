use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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

    /// Creates a symbolic link at `link` pointing to `target`, failing with
    /// `AlreadyExists` when `link` is there. `ErrorKind::Unsupported` where
    /// the platform has no symlinks.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn symlink(&self, target: &Path, link: &Path) -> io::Result<()>;

    /// Where the symbolic link `link` points.
    ///
    /// # Errors
    /// Propagates the underlying I/O error (for example when `link` is
    /// missing or is not a symbolic link).
    fn read_link(&self, link: &Path) -> io::Result<PathBuf>;

    /// Whether `a` and `b` are one existing directory, symlinks followed
    /// (the same device and inode on Unix).
    fn same_directory(&self, a: &Path, b: &Path) -> bool;

    /// The absolute path `path` names, every symbolic link resolved.
    ///
    /// # Errors
    /// `NotFound` when `path`, or the end of a dangling link, is missing;
    /// otherwise propagates the underlying I/O error.
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;

    /// Atomically replaces the file `path` resolves to with `contents`: a
    /// symbolic link at `path` is followed and SURVIVES. The contents go to a
    /// unique temporary file in the target's directory, with the target's
    /// permissions (a new file gets `0o644` on Unix), synced, then
    /// `verify(temporary file)` runs, and only then the temporary file is
    /// renamed onto the target. Any failure removes the temporary file and
    /// leaves the target untouched.
    ///
    /// # Errors
    /// `NotFound` for a dangling link; the error of `verify`; otherwise
    /// propagates the underlying I/O error.
    fn replace_file(
        &self,
        path: &Path,
        contents: &str,
        verify: &dyn Fn(&Path) -> io::Result<()>,
    ) -> io::Result<()>;
}
