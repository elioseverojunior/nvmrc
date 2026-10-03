//! What every command needs: the ports, plus values derived from them.

use std::path::{Path, PathBuf};

use crate::adapters::fs_alias_store::FsAliasStore;
use crate::adapters::no_digest::NoDigest;
use crate::adapters::no_http::NoHttp;
use crate::adapters::no_process::NoProcess;
use crate::domain::alias::AliasStore;
use crate::domain::version::Version;
use crate::error::CliError;
use crate::ports::{Digest, Env, FileSystem, Http, Process};

pub struct Context<'a> {
    pub fs: &'a dyn FileSystem,
    pub env: &'a dyn Env,
    process: &'a dyn Process,
    http: &'a dyn Http,
    digest: &'a dyn Digest,
}

impl<'a> Context<'a> {
    /// A context that cannot run programs or reach the network; add those with
    /// [`Self::with_process`] and [`Self::with_http`].
    #[must_use]
    pub fn new(fs: &'a dyn FileSystem, env: &'a dyn Env) -> Self {
        Self {
            fs,
            env,
            process: &NoProcess,
            http: &NoHttp,
            digest: &NoDigest,
        }
    }

    #[must_use]
    pub fn with_process(mut self, process: &'a dyn Process) -> Self {
        self.process = process;
        self
    }

    #[must_use]
    pub fn process(&self) -> &dyn Process {
        self.process
    }

    #[must_use]
    pub fn with_digest(mut self, digest: &'a dyn Digest) -> Self {
        self.digest = digest;
        self
    }

    #[must_use]
    pub fn digest(&self) -> &dyn Digest {
        self.digest
    }

    #[must_use]
    pub fn with_http(mut self, http: &'a dyn Http) -> Self {
        self.http = http;
        self
    }

    #[must_use]
    pub fn http(&self) -> &dyn Http {
        self.http
    }
}

impl Context<'_> {
    /// `$NVM_DIR` normalised (no trailing slash), defaulting to `$HOME/.nvm`.
    /// Read as an OS string, so a non-UTF-8 path is honoured, never ignored.
    ///
    /// # Errors
    /// Returns [`CliError::NvmDirUnresolved`] when neither variable is set.
    pub fn nvm_dir(&self) -> Result<PathBuf, CliError> {
        let configured = self.env.var_os("NVM_DIR").filter(|dir| !dir.is_empty());
        let dir = match configured {
            Some(dir) => PathBuf::from(dir),
            None => {
                let home = self.env.var_os("HOME").filter(|home| !home.is_empty());
                PathBuf::from(home.ok_or(CliError::NvmDirUnresolved)?).join(".nvm")
            }
        };
        Ok(dir.components().collect())
    }

    /// `$NVM_DIR/alias`, where alias files live.
    ///
    /// # Errors
    /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
    pub fn alias_dir(&self) -> Result<PathBuf, CliError> {
        Ok(self.nvm_dir()?.join("alias"))
    }

    /// `$NVM_DIR/.cache`, where downloads are kept.
    ///
    /// # Errors
    /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
    pub fn cache_dir(&self) -> Result<PathBuf, CliError> {
        Ok(self.nvm_dir()?.join(".cache"))
    }

    /// The alias files under `$NVM_DIR/alias`.
    ///
    /// # Errors
    /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
    pub fn alias_store(&self) -> Result<impl AliasStore, CliError> {
        Ok(FsAliasStore::new(self.fs, &self.nvm_dir()?))
    }

    /// Every version installed under `versions/node` and `versions/io.js`.
    /// Only directories with a canonical version name count: `v20.1.0`, never
    /// `20.1.0`, `v020.1.0` or a plain file.
    ///
    /// # Errors
    /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
    pub fn installed_versions(&self) -> Result<Vec<Version>, CliError> {
        let versions_dir = self.nvm_dir()?.join("versions");
        let node = self.list_versions(&versions_dir.join("node"), "");
        let iojs = self.list_versions(&versions_dir.join("io.js"), "iojs-");
        Ok(node.chain(iojs).collect())
    }

    fn list_versions<'s>(
        &'s self,
        directory: &Path,
        prefix: &'s str,
    ) -> impl Iterator<Item = Version> + 's {
        self.fs
            .read_dir(directory)
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| entry.is_dir)
            .filter_map(move |entry| {
                let full_name = format!("{prefix}{}", entry.name);
                let version: Version = full_name.parse().ok()?;
                (version.to_string() == full_name).then_some(version)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeDigest, FakeEnv, FakeFileSystem, FakeHttp};

    fn nvm_dir_for(env: &FakeEnv) -> Result<PathBuf, CliError> {
        let fs = FakeFileSystem::default();
        Context::new(&fs, env).nvm_dir()
    }

    fn installed(fs: &FakeFileSystem) -> Vec<String> {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let context = Context::new(fs, &env);
        let mut found: Vec<String> = context
            .installed_versions()
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect();
        found.sort();
        found
    }

    #[test]
    fn nvm_dir_removes_the_trailing_slash() {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n/");
        assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from("/n"));
    }

    #[test]
    fn nvm_dir_keeps_the_root_directory() {
        let env = FakeEnv::default().with_var("NVM_DIR", "/");
        assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from("/"));
    }

    #[test]
    fn nvm_dir_defaults_to_home_dot_nvm() {
        let env = FakeEnv::default().with_var("HOME", "/home/me");
        assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from("/home/me/.nvm"));
    }

    #[test]
    fn an_empty_nvm_dir_falls_back_to_home() {
        let env = FakeEnv::default()
            .with_var("NVM_DIR", "")
            .with_var("HOME", "/home/me");
        assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from("/home/me/.nvm"));
    }

    #[test]
    fn nvm_dir_is_an_error_without_nvm_dir_and_home() {
        let result = nvm_dir_for(&FakeEnv::default());
        assert!(matches!(result, Err(CliError::NvmDirUnresolved)));
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_nvm_dir_is_honoured_not_ignored() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        let raw = OsString::from_vec(b"/n\xff".to_vec());
        let env = FakeEnv::default()
            .with_var_os("NVM_DIR", raw.clone())
            .with_var("HOME", "/home/me");
        assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from(raw));
    }

    #[test]
    fn a_context_cannot_reach_the_network_until_given_an_http() {
        let fs = FakeFileSystem::default();
        let env = FakeEnv::default();
        assert!(
            Context::new(&fs, &env)
                .http()
                .get_text("http://x/")
                .is_err()
        );
        let http = FakeHttp::default().with_body("http://x/", "ok");
        let context = Context::new(&fs, &env).with_http(&http);
        assert_eq!(context.http().get_text("http://x/").unwrap(), "ok");
    }

    #[test]
    fn a_context_hashes_nothing_until_it_is_given_a_digest() {
        let fs = FakeFileSystem::default();
        let env = FakeEnv::default();
        assert!(
            Context::new(&fs, &env)
                .digest()
                .sha256_file(Path::new("/f"))
                .is_err()
        );
        let digest = FakeDigest::default().with_digest("/f", "abc");
        let context = Context::new(&fs, &env).with_digest(&digest);
        assert_eq!(
            context.digest().sha256_file(Path::new("/f")).unwrap(),
            "abc"
        );
    }

    #[test]
    fn alias_dir_is_under_nvm_dir() {
        let fs = FakeFileSystem::default();
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let context = Context::new(&fs, &env);
        assert_eq!(context.alias_dir().unwrap(), PathBuf::from("/n/alias"));
    }

    #[test]
    fn alias_store_reads_the_alias_directory() {
        let fs = FakeFileSystem::default().with_file("/n/alias/default", "v20");
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let context = Context::new(&fs, &env);
        let store = context.alias_store().unwrap();
        assert_eq!(store.target("default"), Some("v20".to_owned()));
    }

    #[test]
    fn installed_versions_reads_node_and_iojs_directories() {
        let fs = FakeFileSystem::default()
            .with_file("/n/versions/node/v20.1.0/bin/node", "")
            .with_file("/n/versions/io.js/v3.0.0/bin/iojs", "");
        assert_eq!(installed(&fs), ["iojs-v3.0.0", "v20.1.0"]);
    }

    #[test]
    fn installed_versions_ignores_non_canonical_names_and_files() {
        let fs = FakeFileSystem::default()
            .with_file("/n/versions/node/v20.1.0/bin/node", "")
            .with_file("/n/versions/node/20.2.0/bin/node", "")
            .with_file("/n/versions/node/v020.3.0/bin/node", "")
            .with_file("/n/versions/node/not-a-version/x", "")
            .with_file("/n/versions/node/v22.0.0", "a plain file");
        assert_eq!(installed(&fs), ["v20.1.0"]);
    }

    #[test]
    fn an_empty_version_directory_still_counts_as_installed() {
        let fs = FakeFileSystem::default().with_dir("/n/versions/node/v18.0.0");
        assert_eq!(installed(&fs), ["v18.0.0"]);
    }
}
