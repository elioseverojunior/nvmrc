//! What every command needs: the ports, plus values derived from them.

use std::path::{Path, PathBuf};

use crate::adapters::fs_alias_store::FsAliasStore;
use crate::adapters::no_archive::NoArchive;
use crate::adapters::no_cpu::NoCpu;
use crate::adapters::no_digest::NoDigest;
use crate::adapters::no_http::NoHttp;
use crate::adapters::no_process::NoProcess;
use crate::adapters::no_sleeper::NoSleeper;
use crate::domain::alias::AliasStore;
use crate::domain::platform::{Os, Platform};
use crate::domain::version::Version;
use crate::error::CliError;
use crate::ports::{Archive, Cpu, Digest, Env, FileSystem, Http, Process, Sleeper};

pub struct Context<'a> {
    pub fs: &'a dyn FileSystem,
    pub env: &'a dyn Env,
    process: &'a dyn Process,
    http: &'a dyn Http,
    digest: &'a dyn Digest,
    archive: &'a dyn Archive,
    sleeper: &'a dyn Sleeper,
    cpu: &'a dyn Cpu,
    platform: Option<Platform>,
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
            archive: &NoArchive,
            sleeper: &NoSleeper,
            cpu: &NoCpu,
            platform: Some(Platform {
                os: Os::Linux,
                arch: "x64".to_owned(),
            }),
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
    pub fn with_cpu(mut self, cpu: &'a dyn Cpu) -> Self {
        self.cpu = cpu;
        self
    }

    #[must_use]
    pub fn cpu(&self) -> &dyn Cpu {
        self.cpu
    }

    /// The machine the binaries are for; `None` when it has no official ones.
    /// A context starts out as Linux on x64.
    #[must_use]
    pub fn with_platform(mut self, platform: Option<Platform>) -> Self {
        self.platform = platform;
        self
    }

    #[must_use]
    pub fn platform(&self) -> Option<&Platform> {
        self.platform.as_ref()
    }

    #[must_use]
    pub fn with_sleeper(mut self, sleeper: &'a dyn Sleeper) -> Self {
        self.sleeper = sleeper;
        self
    }

    #[must_use]
    pub fn sleeper(&self) -> &dyn Sleeper {
        self.sleeper
    }

    #[must_use]
    pub fn with_archive(mut self, archive: &'a dyn Archive) -> Self {
        self.archive = archive;
        self
    }

    #[must_use]
    pub fn archive(&self) -> &dyn Archive {
        self.archive
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

    /// Every version installed under `versions/node` and `versions/io.js`,
    /// plus the old layout directly in `$NVM_DIR` (`$NVM_DIR/v0.10.48`), as
    /// `nvm_ls` searches all three; a version in two layouts counts once.
    /// Only directories with a canonical version name count: `v20.1.0`, never
    /// `20.1.0`, `v020.1.0` or a plain file.
    ///
    /// # Errors
    /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
    pub fn installed_versions(&self) -> Result<Vec<Version>, CliError> {
        let versions_dir = self.nvm_dir()?.join("versions");
        let mut found: Vec<Version> = self.list_versions(&versions_dir.join("node"), "").collect();
        let legacy = self
            .list_versions(&self.nvm_dir()?, "")
            .filter(|version| !found.contains(version))
            .collect::<Vec<_>>();
        found.extend(legacy);
        found.extend(self.list_versions(&versions_dir.join("io.js"), "iojs-"));
        Ok(found)
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
mod tests;
