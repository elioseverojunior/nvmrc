//! Aliases stored as files: `$NVM_DIR/alias/<name>`, first line is the target.

use std::path::{Component, Path, PathBuf};

use crate::domain::alias::AliasStore;
use crate::ports::FileSystem;

pub struct FsAliasStore<'a> {
    fs: &'a dyn FileSystem,
    alias_dir: PathBuf,
}

impl<'a> FsAliasStore<'a> {
    #[must_use]
    pub fn new(fs: &'a dyn FileSystem, nvm_dir: &Path) -> Self {
        Self {
            fs,
            alias_dir: nvm_dir.join("alias"),
        }
    }
}

/// Alias names may contain `/` (`lts/iron`) but never `..` or absolute paths.
fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && Path::new(name)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

impl AliasStore for FsAliasStore<'_> {
    fn target(&self, name: &str) -> Option<String> {
        if !is_safe_name(name) {
            return None;
        }
        let contents = self.fs.read_to_string(&self.alias_dir.join(name)).ok()?;
        first_meaningful_line(&contents)
    }
}

/// Like `nvm_print_alias_file`: everything after a `#` is a comment, blank
/// lines are skipped, and the first line left is the target.
fn first_meaningful_line(contents: &str) -> Option<String> {
    contents
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .find(|line| !line.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::fakes::FakeFileSystem;

    use super::*;

    fn store(fs: &FakeFileSystem) -> FsAliasStore<'_> {
        FsAliasStore::new(fs, Path::new("/nvm"))
    }

    #[test]
    fn reads_the_first_line_of_the_alias_file() {
        let fs = FakeFileSystem::default().with_file("/nvm/alias/default", "v20.0.0\n");
        assert_eq!(store(&fs).target("default"), Some("v20.0.0".to_owned()));
    }

    #[test]
    fn supports_nested_alias_names() {
        let fs = FakeFileSystem::default().with_file("/nvm/alias/lts/iron", "v20.1.0");
        assert_eq!(store(&fs).target("lts/iron"), Some("v20.1.0".to_owned()));
    }

    #[test]
    fn a_missing_or_empty_alias_has_no_target() {
        let fs = FakeFileSystem::default().with_file("/nvm/alias/empty", "\n");
        assert_eq!(store(&fs).target("nope"), None);
        assert_eq!(store(&fs).target("empty"), None);
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let fs = FakeFileSystem::default()
            .with_file("/nvm/alias/commented", "v20 # the current LTS\n")
            .with_file("/nvm/alias/blank-first", "\n\n  v18  \nv16\n")
            .with_file("/nvm/alias/only-comment", "# nothing here\n\n");
        assert_eq!(store(&fs).target("commented"), Some("v20".to_owned()));
        assert_eq!(store(&fs).target("blank-first"), Some("v18".to_owned()));
        assert_eq!(store(&fs).target("only-comment"), None);
    }

    #[test]
    fn rejects_path_traversal_in_alias_names() {
        let fs = FakeFileSystem::default().with_file("/nvm/secret", "v1.0.0");
        assert_eq!(store(&fs).target("../secret"), None);
        assert_eq!(store(&fs).target("/nvm/secret"), None);
    }
}
