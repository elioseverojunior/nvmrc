use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::ports::{DirEntry, FileInfo, FileSystem};

mod atomic;

pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
        fs::read_dir(path)?
            .map(|entry| {
                let entry = entry?;
                Ok(DirEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    is_dir: entry.path().is_dir(),
                })
            })
            .collect()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
            Ok(_) => fs::remove_file(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()> {
        fs::write(path, contents)
    }

    fn write_bytes(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        fs::write(path, contents)
    }

    fn file_info(&self, path: &Path) -> io::Result<FileInfo> {
        let metadata = fs::metadata(path)?;
        Ok(FileInfo {
            is_dir: metadata.is_dir(),
            len: metadata.len(),
            executable: is_executable(&metadata),
            modified: metadata.modified().ok(),
        })
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        fs::create_dir(path)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    #[cfg(unix)]
    fn symlink(&self, target: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(not(unix))]
    fn symlink(&self, _target: &Path, _link: &Path) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    #[cfg(unix)]
    fn read_link(&self, link: &Path) -> io::Result<PathBuf> {
        fs::read_link(link)
    }

    #[cfg(not(unix))]
    fn read_link(&self, _link: &Path) -> io::Result<PathBuf> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    #[cfg(unix)]
    fn same_directory(&self, a: &Path, b: &Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        match (fs::metadata(a), fs::metadata(b)) {
            (Ok(a), Ok(b)) => a.is_dir() && a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }

    #[cfg(not(unix))]
    fn same_directory(&self, a: &Path, b: &Path) -> bool {
        match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(a), Ok(b)) => a == b && a.is_dir(),
            _ => false,
        }
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        fs::canonicalize(path)
    }

    fn replace_file(
        &self,
        path: &Path,
        contents: &str,
        verify: &dyn Fn(&Path) -> io::Result<()>,
    ) -> io::Result<()> {
        atomic::replace_file(path, contents, verify)
    }
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod atomic_tests;
#[cfg(test)]
mod tests;
