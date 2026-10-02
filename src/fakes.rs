//! In-memory implementations of the ports, for unit tests only.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use crate::ports::{DirEntry, Env, FileSystem};

#[derive(Default)]
pub struct FakeFileSystem {
    files: RefCell<BTreeMap<PathBuf, String>>,
    dirs: RefCell<BTreeSet<PathBuf>>,
}

impl FakeFileSystem {
    #[must_use]
    pub fn with_file(mut self, path: &str, contents: &str) -> Self {
        self.files
            .get_mut()
            .insert(PathBuf::from(path), contents.to_owned());
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
        self.files
            .borrow()
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
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

    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()> {
        self.files
            .borrow_mut()
            .insert(path.to_path_buf(), contents.to_owned());
        Ok(())
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.dirs.borrow_mut().insert(path.to_path_buf());
        Ok(())
    }
}

#[derive(Default)]
pub struct FakeEnv {
    vars: BTreeMap<String, OsString>,
}

impl FakeEnv {
    #[must_use]
    pub fn with_var(self, key: &str, value: &str) -> Self {
        self.with_var_os(key, OsString::from(value))
    }

    #[must_use]
    pub fn with_var_os(mut self, key: &str, value: OsString) -> Self {
        self.vars.insert(key.to_owned(), value);
        self
    }
}

impl Env for FakeEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.vars
            .get(key)
            .and_then(|value| value.to_str().map(str::to_owned))
    }

    fn var_os(&self, key: &str) -> Option<OsString> {
        self.vars.get(key).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, is_dir: bool) -> DirEntry {
        DirEntry {
            name: name.to_owned(),
            is_dir,
        }
    }

    #[test]
    fn fake_file_system_lists_direct_children_with_their_kind() {
        let fs = FakeFileSystem::default()
            .with_file("/d/a/x", "1")
            .with_file("/d/a/y", "2")
            .with_file("/d/b", "3");
        let root = fs.read_dir(Path::new("/d")).unwrap();
        assert_eq!(root, [entry("a", true), entry("b", false)]);
        let nested = fs.read_dir(Path::new("/d/a")).unwrap();
        assert_eq!(nested, [entry("x", false), entry("y", false)]);
    }

    #[test]
    fn fake_file_system_models_empty_directories() {
        let fs = FakeFileSystem::default().with_dir("/d/empty");
        assert_eq!(fs.read_dir(Path::new("/d/empty")).unwrap(), []);
        assert_eq!(
            fs.read_dir(Path::new("/d")).unwrap(),
            [entry("empty", true)]
        );
    }

    #[test]
    fn fake_file_system_fails_on_missing_paths_and_on_files() {
        let fs = FakeFileSystem::default().with_file("/d/f", "x");
        assert!(fs.read_dir(Path::new("/missing")).is_err());
        assert!(fs.read_dir(Path::new("/d/f")).is_err());
    }

    #[test]
    fn fake_file_system_reads_files() {
        let fs = FakeFileSystem::default().with_file("/f", "hi");
        assert_eq!(fs.read_to_string(Path::new("/f")).unwrap(), "hi");
        assert!(fs.read_to_string(Path::new("/g")).is_err());
        assert!(fs.is_file(Path::new("/f")));
        assert!(!fs.is_file(Path::new("/g")));
    }

    #[test]
    fn fake_file_system_removes_files() {
        let fs = FakeFileSystem::default().with_file("/f", "hi");
        fs.remove_file(Path::new("/f")).unwrap();
        assert!(!fs.is_file(Path::new("/f")));
        assert!(fs.remove_file(Path::new("/f")).is_err());
    }

    #[test]
    fn fake_file_system_writes_and_overwrites_files() {
        let fs = FakeFileSystem::default();
        fs.write_file(Path::new("/f"), "one").unwrap();
        fs.write_file(Path::new("/f"), "two").unwrap();
        assert_eq!(fs.read_to_string(Path::new("/f")).unwrap(), "two");
    }

    #[test]
    fn fake_file_system_creates_directories() {
        let fs = FakeFileSystem::default();
        fs.create_dir_all(Path::new("/d/e")).unwrap();
        fs.create_dir_all(Path::new("/d/e")).unwrap();
        assert_eq!(fs.read_dir(Path::new("/d/e")).unwrap(), []);
    }

    #[test]
    fn fake_env_returns_the_variables_it_was_given() {
        let env = FakeEnv::default().with_var("HOME", "/home/me");
        assert_eq!(env.var("HOME"), Some("/home/me".to_owned()));
        assert_eq!(env.var_os("HOME"), Some(OsString::from("/home/me")));
        assert_eq!(env.var("MISSING"), None);
    }

    #[cfg(unix)]
    #[test]
    fn fake_env_keeps_non_utf8_values_for_var_os_only() {
        use std::os::unix::ffi::OsStringExt;
        let raw = OsString::from_vec(b"/n\xff".to_vec());
        let env = FakeEnv::default().with_var_os("NVM_DIR", raw.clone());
        assert_eq!(env.var("NVM_DIR"), None);
        assert_eq!(env.var_os("NVM_DIR"), Some(raw));
    }
}
