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
    links: RefCell<BTreeMap<PathBuf, PathBuf>>,
    unwritable: RefCell<BTreeSet<PathBuf>>,
}

impl FakeFileSystem {
    /// Makes every write to `path` fail with a permission error.
    #[must_use]
    pub fn with_unwritable(self, path: &str) -> Self {
        self.unwritable.borrow_mut().insert(PathBuf::from(path));
        self
    }

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

    /// Gives an existing file the execute permission.
    pub fn set_executable(&self, path: &Path) {
        self.executables.borrow_mut().insert(path.to_path_buf());
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

/// The paths of `all` that are `from` or inside it.
fn under<'a>(all: impl Iterator<Item = &'a PathBuf>, from: &Path) -> Vec<PathBuf> {
    all.filter(|path| path.starts_with(from)).cloned().collect()
}

/// Renames, in a set of paths, those that are `from` or inside it.
fn move_paths(set: &mut BTreeSet<PathBuf>, from: &Path, moved: &dyn Fn(&Path) -> PathBuf) {
    for name in under(set.iter(), from) {
        set.remove(&name);
        set.insert(moved(&name));
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
        if self.links.borrow_mut().remove(path).is_some() {
            return Ok(());
        }
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
        if self.unwritable.borrow().contains(path) {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
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
        for name in under(files.keys(), from) {
            if let Some(contents) = files.remove(&name) {
                files.insert(moved(&name), contents);
            }
        }
        move_paths(&mut self.executables.borrow_mut(), from, &moved);
        move_paths(&mut self.dirs.borrow_mut(), from, &moved);
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

    fn symlink(&self, target: &Path, link: &Path) -> io::Result<()> {
        if self.file_info(link).is_ok() || self.links.borrow().contains_key(link) {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        self.links
            .borrow_mut()
            .insert(link.to_path_buf(), target.to_path_buf());
        Ok(())
    }

    fn read_link(&self, link: &Path) -> io::Result<PathBuf> {
        let found = self.links.borrow().get(link).cloned();
        found.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}

#[cfg(test)]
mod tests;
