//! What every command needs: the ports, plus values derived from them.

use std::path::PathBuf;

use crate::domain::version::Version;
use crate::ports::{Env, FileSystem};

pub struct Context<'a> {
    pub fs: &'a dyn FileSystem,
    pub env: &'a dyn Env,
}

impl Context<'_> {
    /// `$NVM_DIR` without a trailing slash, defaulting to `$HOME/.nvm`.
    #[must_use]
    pub fn nvm_dir(&self) -> PathBuf {
        let configured = self.env.var("NVM_DIR").filter(|dir| !dir.is_empty());
        let dir = configured
            .unwrap_or_else(|| format!("{}/.nvm", self.env.var("HOME").unwrap_or_default()));
        match dir.trim_end_matches('/') {
            "" => PathBuf::from("/"),
            trimmed => PathBuf::from(trimmed),
        }
    }

    /// Every version found under `versions/node` and `versions/io.js`.
    /// Directories whose names are not versions are ignored.
    #[must_use]
    pub fn installed_versions(&self) -> Vec<Version> {
        let versions_dir = self.nvm_dir().join("versions");
        let node = self.list_versions(&versions_dir.join("node"), "");
        let iojs = self.list_versions(&versions_dir.join("io.js"), "iojs-");
        node.chain(iojs).collect()
    }

    fn list_versions<'s>(
        &'s self,
        directory: &std::path::Path,
        prefix: &'s str,
    ) -> impl Iterator<Item = Version> + 's {
        self.fs
            .read_dir_names(directory)
            .unwrap_or_default()
            .into_iter()
            .filter_map(move |name| format!("{prefix}{name}").parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    #[test]
    fn nvm_dir_removes_the_trailing_slash() {
        let (fs, env) = (
            FakeFileSystem::default(),
            FakeEnv::default().with_var("NVM_DIR", "/n/"),
        );
        let context = Context { fs: &fs, env: &env };
        assert_eq!(context.nvm_dir(), PathBuf::from("/n"));
    }

    #[test]
    fn nvm_dir_keeps_the_root_directory() {
        let (fs, env) = (
            FakeFileSystem::default(),
            FakeEnv::default().with_var("NVM_DIR", "/"),
        );
        let context = Context { fs: &fs, env: &env };
        assert_eq!(context.nvm_dir(), PathBuf::from("/"));
    }

    #[test]
    fn nvm_dir_defaults_to_home_dot_nvm() {
        let (fs, env) = (
            FakeFileSystem::default(),
            FakeEnv::default().with_var("HOME", "/home/me"),
        );
        let context = Context { fs: &fs, env: &env };
        assert_eq!(context.nvm_dir(), PathBuf::from("/home/me/.nvm"));
    }

    #[test]
    fn installed_versions_reads_node_and_iojs_and_skips_junk() {
        let fs = FakeFileSystem::default()
            .with_file("/n/versions/node/v20.1.0/bin/node", "")
            .with_file("/n/versions/node/not-a-version/x", "")
            .with_file("/n/versions/io.js/v3.0.0/bin/iojs", "");
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let context = Context { fs: &fs, env: &env };
        let found: Vec<String> = context
            .installed_versions()
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(found, ["v20.1.0", "iojs-v3.0.0"]);
    }
}
