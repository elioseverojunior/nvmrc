//! Finding an executable the way a shell does: the first match along `PATH`.

use std::ffi::OsStr;
use std::path::PathBuf;

use crate::ports::FileSystem;

/// The first `PATH` entry that contains a file called `name`. Empty entries
/// are skipped rather than read as the current directory.
#[must_use]
pub fn find_in_path(fs: &dyn FileSystem, path_variable: &OsStr, name: &str) -> Option<PathBuf> {
    std::env::split_paths(path_variable)
        .filter(|directory| !directory.as_os_str().is_empty())
        .map(|directory| directory.join(name))
        .find(|candidate| fs.is_file(candidate))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::fakes::FakeFileSystem;

    #[test]
    fn finds_the_first_match_in_path_order() {
        let fs = FakeFileSystem::default()
            .with_file("/b/node", "")
            .with_file("/a/node", "");
        let found = find_in_path(&fs, OsStr::new("/a:/b"), "node");
        assert_eq!(found.as_deref(), Some(Path::new("/a/node")));
        let reversed = find_in_path(&fs, OsStr::new("/b:/a"), "node");
        assert_eq!(reversed.as_deref(), Some(Path::new("/b/node")));
    }

    #[test]
    fn skips_directories_without_the_file() {
        let fs = FakeFileSystem::default().with_file("/b/node", "");
        let found = find_in_path(&fs, OsStr::new("/a:/b"), "node");
        assert_eq!(found.as_deref(), Some(Path::new("/b/node")));
    }

    #[test]
    fn ignores_empty_entries() {
        let fs = FakeFileSystem::default().with_file("node", "");
        assert_eq!(find_in_path(&fs, OsStr::new(":"), "node"), None);
    }

    #[test]
    fn returns_none_when_nothing_matches() {
        let fs = FakeFileSystem::default();
        assert_eq!(find_in_path(&fs, OsStr::new("/a:/b"), "node"), None);
    }
}
