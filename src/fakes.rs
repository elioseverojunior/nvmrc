//! In-memory implementations of the ports, for unit tests only.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use crate::ports::{Env, FileSystem};

#[derive(Default)]
pub struct FakeFileSystem {
    files: BTreeMap<PathBuf, String>,
}

impl FakeFileSystem {
    #[must_use]
    pub fn with_file(mut self, path: &str, contents: &str) -> Self {
        self.files.insert(PathBuf::from(path), contents.to_owned());
        self
    }
}

impl FileSystem for FakeFileSystem {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn read_dir_names(&self, path: &Path) -> io::Result<Vec<String>> {
        let names: BTreeSet<String> = self
            .files
            .keys()
            .filter_map(|file| file.strip_prefix(path).ok())
            .filter_map(|rest| rest.components().next())
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        if names.is_empty() {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        }
        Ok(names.into_iter().collect())
    }

    fn is_file(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }
}

#[derive(Default)]
pub struct FakeEnv {
    vars: BTreeMap<String, String>,
}

impl FakeEnv {
    #[must_use]
    pub fn with_var(mut self, key: &str, value: &str) -> Self {
        self.vars.insert(key.to_owned(), value.to_owned());
        self
    }
}

impl Env for FakeEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.vars.get(key).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_file_system_lists_direct_children_only() {
        let fs = FakeFileSystem::default()
            .with_file("/d/a/x", "1")
            .with_file("/d/a/y", "2")
            .with_file("/d/b", "3");
        assert_eq!(fs.read_dir_names(Path::new("/d")).unwrap(), ["a", "b"]);
        assert_eq!(fs.read_dir_names(Path::new("/d/a")).unwrap(), ["x", "y"]);
        assert!(fs.read_dir_names(Path::new("/missing")).is_err());
    }

    #[test]
    fn fake_env_returns_the_variables_it_was_given() {
        let env = FakeEnv::default().with_var("HOME", "/home/me");
        assert_eq!(env.var("HOME"), Some("/home/me".to_owned()));
        assert_eq!(env.var("MISSING"), None);
    }

    #[test]
    fn fake_file_system_reads_files() {
        let fs = FakeFileSystem::default().with_file("/f", "hi");
        assert_eq!(fs.read_to_string(Path::new("/f")).unwrap(), "hi");
        assert!(fs.is_file(Path::new("/f")));
        assert!(!fs.is_file(Path::new("/g")));
    }
}
