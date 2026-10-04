use std::io;
use std::path::Path;

pub trait Digest {
    /// The SHA-256 of the file at `path`, in lowercase hex.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn sha256_file(&self, path: &Path) -> io::Result<String>;
}

pub trait Archive {
    /// Unpacks the `.tar.gz` or `.tar.xz` at `archive` into the existing
    /// directory `destination`, keeping every path inside it.
    ///
    /// # Errors
    /// Propagates the underlying I/O error, and fails on a file that is not a
    /// gzip- or xz-compressed tar.
    fn extract(&self, archive: &Path, destination: &Path) -> io::Result<()>;
}
