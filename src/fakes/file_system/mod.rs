use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::ports::{DirEntry, FileInfo, FileSystem};

#[derive(Default)]
pub struct FakeFileSystem {
    files: RefCell<BTreeMap<PathBuf, Vec<u8>>>,
    dirs: RefCell<BTreeSet<PathBuf>>,
    executables: RefCell<BTreeSet<PathBuf>>,
    modified: RefCell<BTreeMap<PathBuf, SystemTime>>,
}

impl FakeFileSystem {
    #[must_use]
    pub fn with_file(mut self, path: &str, contents: &str) -> Self {
        self.files
            .get_mut()
            .insert(PathBuf::from(path), contents.as_bytes().to_vec());
        self
    }

    /// A file with the execute permission.
    #[must_use]
    pub fn with_executable(mut self, path: &str, contents: &str) -> Self {
        self.executables.get_mut().insert(PathBuf::from(path));
        self.with_file(path, contents)
    }

    /// When the file or directory was last changed.
    #[must_use]
    pub fn with_modified(mut self, path: &str, time: SystemTime) -> Self {
        self.modified.get_mut().insert(PathBuf::from(path), time);
        self
    }

    /// An explicit, possibly empty, directory. Parents of files exist already.
    #[must_use]
    pub fn with_dir(mut self, path: &str) -> Self {
        self.dirs.get_mut().insert(PathBuf::from(path));
        self
    }
}

impl FileSystem for FakeFileSystem {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        let bytes = self.files.borrow().get(path).cloned();
        let bytes = bytes.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
        let mut children: BTreeMap<String, bool> = BTreeMap::new();
        let dirs = self.dirs.borrow();
        let mut exists = dirs.contains(path);
        let files = self.files.borrow();
        let file_paths = files.keys().map(|file| (file, false));
        let dir_paths = dirs.iter().map(|dir| (dir, true));
        for (candidate, is_dir_path) in file_paths.chain(dir_paths) {
            let Ok(rest) = candidate.strip_prefix(path) else {
                continue;
            };
            let mut parts = rest.components();
            let Some(first) = parts.next() else {
                continue;
            };
            exists = true;
            let name = first.as_os_str().to_string_lossy().into_owned();
            *children.entry(name).or_insert(false) |= is_dir_path || parts.next().is_some();
        }
        if !exists {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        }
        Ok(children
            .into_iter()
            .map(|(name, is_dir)| DirEntry { name, is_dir })
            .collect())
    }

    fn is_file(&self, path: &Path) -> bool {
        self.files.borrow().contains_key(path)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.files
            .borrow_mut()
            .remove(path)
            .map(|_| ())
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.files
            .borrow_mut()
            .retain(|file, _| !file.starts_with(path));
        self.dirs.borrow_mut().retain(|dir| !dir.starts_with(path));
        Ok(())
    }

    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()> {
        self.write_bytes(path, contents.as_bytes())
    }

    fn write_bytes(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        self.files
            .borrow_mut()
            .insert(path.to_path_buf(), contents.to_vec());
        Ok(())
    }

    fn file_info(&self, path: &Path) -> io::Result<FileInfo> {
        let modified = self.modified.borrow().get(path).copied();
        if let Some(contents) = self.files.borrow().get(path) {
            return Ok(FileInfo {
                is_dir: false,
                len: contents.len() as u64,
                executable: self.executables.borrow().contains(path),
                modified,
            });
        }
        if self.read_dir(path).is_ok() {
            return Ok(FileInfo {
                is_dir: true,
                len: 0,
                executable: true,
                modified,
            });
        }
        Err(io::Error::from(io::ErrorKind::NotFound))
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        if self.file_info(from).is_err() {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        }
        let moved = |old: &Path| to.join(old.strip_prefix(from).unwrap_or(old));
        let mut files = self.files.borrow_mut();
        let names: Vec<PathBuf> = files
            .keys()
            .filter(|f| f.starts_with(from))
            .cloned()
            .collect();
        for name in names {
            if let Some(contents) = files.remove(&name) {
                files.insert(moved(&name), contents);
            }
        }
        let mut dirs = self.dirs.borrow_mut();
        let names: Vec<PathBuf> = dirs
            .iter()
            .filter(|d| d.starts_with(from))
            .cloned()
            .collect();
        for name in names {
            dirs.remove(&name);
            dirs.insert(moved(&name));
        }
        Ok(())
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        if self.file_info(path).is_ok() {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        self.dirs.borrow_mut().insert(path.to_path_buf());
        Ok(())
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.dirs.borrow_mut().insert(path.to_path_buf());
        Ok(())
    }
}

#[cfg(test)]
mod tests;
