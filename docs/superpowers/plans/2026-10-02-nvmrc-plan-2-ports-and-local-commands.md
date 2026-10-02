# nvmrc Plan 2: Port Hardening and Local Commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden the `FileSystem` and `Env` ports, then add the local commands
`current`, `which`, `unalias` and `alias <name> <target>` (plus `version`
without an argument) to both binaries.

**Architecture:** Same layering as Plan 1. Commands return an `Output`
(`stdout` and `stderr` text) or a typed `CliError`; the CLI prints stderr, then
stdout, and maps errors to the nvm exit codes. Version and alias resolution
moves into one shared `commands::resolve` module that `version`, `which` and
`alias` all use.

**Tech Stack:** Rust 2024 edition (MSRV 1.88), `clap`, `thiserror`, `tempfile`
(dev only). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-02-nvmrc-design.md` (sections 3, 4,
6 and 11). This plan follows `2026-10-02-nvmrc-plan-1-foundation-resolution.md`
and starts from its final state (branch `feat/plan-1-foundation-resolution`).

**Revised roadmap:** Plan 3 `ls` and `alias` listing with colors and the
remaining built-in aliases (`stable`, `unstable`, `lts/*`); Plan 4 network
(`ls-remote`, `cache`, mirror, checksum); Plan 5 `install` and `uninstall`;
Plan 6 shell (`use`, `deactivate`, `exec`, `run`, `init`, `nvm-exec`, `.nvmrc`
semantics); Plan 7 `doctor` and `migrate`; Plan 8 compatibility contract and CI.

**Carried over from the Plan 1 final review:** this plan fixes the
`read_dir` entry kinds and canonical-name filter, `Env::var_os` for a
non-UTF-8 `NVM_DIR`, a missing `HOME` becoming an error, the alias store
reached through `Context`, `nvm version` without an argument meaning
`current`, and the vacuous stderr-failure test.

## Global Constraints

- Rust edition 2024, `rust-version = "1.88.0"`; no APIs newer than 1.88. The
  development toolchain is pinned separately by `rust-toolchain.toml`.
- Run cargo through the rustup proxies (Homebrew's `rust` formula ships a
  `cargo` that ignores `rust-toolchain.toml`).
- `.cargo/config.toml` sets `-D warnings` for every workspace build, test and
  check, so a dead-code or unused-import warning in any intermediate task is a
  build error. `Cargo.toml` must not declare any `[profile.*]` table.
- No async I/O, no workspace, no plugin system, no new dependencies.
- Files under 300 lines (tests included), functions under 30 lines, cyclomatic
  complexity under 10, meaningful names without abbreviations.
- Errors use `thiserror` in the library; only the binaries touch
  `std::process::ExitCode`. Exit codes: 0 success, 1 generic failure, 3
  invalid or unknown version, 7 below the version floor, 8 alias loop, 127
  usage error or missing system node.
- Commands return `Output { stdout, stderr }` or a `CliError`; the CLI prints
  stderr first, then stdout, adds a newline to each non-empty stream, and a
  failed stdout write exits 1.
- Layout compatible with nvm: `$NVM_DIR/versions/node/vX.Y.Z`,
  `$NVM_DIR/versions/io.js/vX.Y.Z` (the `iojs-` prefix is stripped on disk),
  `$NVM_DIR/alias/<name>` (first line is the target, written as `<target>\n`).
- Path-traversal safety: alias names may contain `/` when read, but alias
  creation and deletion reject `/`, `#`, `.`, `..` and the empty name.
- Commits: Conventional Commits, GPG-signed with `git commit -S`, and no AI
  attribution or `Co-Authored-By` trailer in the message.
- Do not keep test scripts or test-output directories in the repository.

Each task below lists its steps in TDD order. Where a file already exists, the
code is shown as a unified diff against the previous task's final state; where
a file is new, the full code is shown. Apply diffs by hand (hunk headers are
informative, line numbers may drift), then run `cargo fmt`.

---

### Task 1: Harden the ports (entry kinds, OS-string env, NVM_DIR errors)

**Files:**

- Modify: `src/ports/mod.rs`, `src/fakes.rs`, `src/adapters/std_fs.rs`,
  `src/adapters/std_env.rs`, `src/context.rs`, `src/error.rs`,
  `src/commands/version.rs`

**Interfaces:**

- Produces: `ports::DirEntry { name: String, is_dir: bool }`;
  `FileSystem::read_dir(&self, &Path) -> io::Result<Vec<DirEntry>>` (replaces
  `read_dir_names`, order unspecified); `Env::var_os(&self, &str) ->
  Option<OsString>`; test-only `FakeFileSystem::with_dir`,
  `FakeEnv::with_var_os`; `CliError::NvmDirUnresolved` (exit 1);
  `Context::nvm_dir() -> Result<PathBuf, CliError>` and
  `Context::installed_versions() -> Result<Vec<Version>, CliError>`.
- Behaviour: only directories with a canonical name (`v20.1.0`, never `20.1.0`,
  `v020.1.0` or a plain file) count as installed; `NVM_DIR` is read with
  `var_os` and normalised through `Path::components` (so `/` stays `/`);
  without `NVM_DIR` and `HOME` the nvm directory cannot be located and that is
  an error, not `/.nvm`.
- [ ] **Step 1: Write the failing tests**

Apply to the test module of `src/context.rs`:

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -3,49 +3,90 @@
     use super::*;
     use crate::fakes::{FakeEnv, FakeFileSystem};
 
+    fn nvm_dir_for(env: &FakeEnv) -> Result<PathBuf, CliError> {
+        let fs = FakeFileSystem::default();
+        Context { fs: &fs, env }.nvm_dir()
+    }
+
+    fn installed(fs: &FakeFileSystem) -> Vec<String> {
+        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
+        let context = Context { fs, env: &env };
+        let mut found: Vec<String> = context
+            .installed_versions()
+            .unwrap()
+            .iter()
+            .map(ToString::to_string)
+            .collect();
+        found.sort();
+        found
+    }
+
     #[test]
     fn nvm_dir_removes_the_trailing_slash() {
-        let (fs, env) = (
-            FakeFileSystem::default(),
-            FakeEnv::default().with_var("NVM_DIR", "/n/"),
-        );
-        let context = Context { fs: &fs, env: &env };
-        assert_eq!(context.nvm_dir(), PathBuf::from("/n"));
+        let env = FakeEnv::default().with_var("NVM_DIR", "/n/");
+        assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from("/n"));
     }
 
     #[test]
     fn nvm_dir_keeps_the_root_directory() {
-        let (fs, env) = (
-            FakeFileSystem::default(),
-            FakeEnv::default().with_var("NVM_DIR", "/"),
-        );
-        let context = Context { fs: &fs, env: &env };
-        assert_eq!(context.nvm_dir(), PathBuf::from("/"));
+        let env = FakeEnv::default().with_var("NVM_DIR", "/");
+        assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from("/"));
     }
 
     #[test]
     fn nvm_dir_defaults_to_home_dot_nvm() {
-        let (fs, env) = (
-            FakeFileSystem::default(),
-            FakeEnv::default().with_var("HOME", "/home/me"),
-        );
-        let context = Context { fs: &fs, env: &env };
-        assert_eq!(context.nvm_dir(), PathBuf::from("/home/me/.nvm"));
+        let env = FakeEnv::default().with_var("HOME", "/home/me");
+        assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from("/home/me/.nvm"));
     }
 
     #[test]
-    fn installed_versions_reads_node_and_iojs_and_skips_junk() {
+    fn an_empty_nvm_dir_falls_back_to_home() {
+        let env = FakeEnv::default()
+            .with_var("NVM_DIR", "")
+            .with_var("HOME", "/home/me");
+        assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from("/home/me/.nvm"));
+    }
+
+    #[test]
+    fn nvm_dir_is_an_error_without_nvm_dir_and_home() {
+        let result = nvm_dir_for(&FakeEnv::default());
+        assert!(matches!(result, Err(CliError::NvmDirUnresolved)));
+    }
+
+    #[cfg(unix)]
+    #[test]
+    fn a_non_utf8_nvm_dir_is_honoured_not_ignored() {
+        use std::ffi::OsString;
+        use std::os::unix::ffi::OsStringExt;
+        let raw = OsString::from_vec(b"/n\xff".to_vec());
+        let env = FakeEnv::default()
+            .with_var_os("NVM_DIR", raw.clone())
+            .with_var("HOME", "/home/me");
+        assert_eq!(nvm_dir_for(&env).unwrap(), PathBuf::from(raw));
+    }
+
+    #[test]
+    fn installed_versions_reads_node_and_iojs_directories() {
         let fs = FakeFileSystem::default()
             .with_file("/n/versions/node/v20.1.0/bin/node", "")
+            .with_file("/n/versions/io.js/v3.0.0/bin/iojs", "");
+        assert_eq!(installed(&fs), ["iojs-v3.0.0", "v20.1.0"]);
+    }
+
+    #[test]
+    fn installed_versions_ignores_non_canonical_names_and_files() {
+        let fs = FakeFileSystem::default()
+            .with_file("/n/versions/node/v20.1.0/bin/node", "")
+            .with_file("/n/versions/node/20.2.0/bin/node", "")
+            .with_file("/n/versions/node/v020.3.0/bin/node", "")
             .with_file("/n/versions/node/not-a-version/x", "")
-            .with_file("/n/versions/io.js/v3.0.0/bin/iojs", "");
-        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
-        let context = Context { fs: &fs, env: &env };
-        let found: Vec<String> = context
-            .installed_versions()
-            .iter()
-            .map(ToString::to_string)
-            .collect();
-        assert_eq!(found, ["v20.1.0", "iojs-v3.0.0"]);
+            .with_file("/n/versions/node/v22.0.0", "a plain file");
+        assert_eq!(installed(&fs), ["v20.1.0"]);
+    }
+
+    #[test]
+    fn an_empty_version_directory_still_counts_as_installed() {
+        let fs = FakeFileSystem::default().with_dir("/n/versions/node/v18.0.0");
+        assert_eq!(installed(&fs), ["v18.0.0"]);
     }
 }
```

Apply to the test module of `src/error.rs`:

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -28,5 +28,6 @@
             CliError::NotInstalled.exit_code(),
             NvmExitCode::InvalidVersion
         );
+        assert_eq!(CliError::NvmDirUnresolved.exit_code(), NvmExitCode::Failure);
     }
 }
```

Apply to the test module of `src/fakes.rs`:

```diff
--- a/src/fakes.rs
+++ b/src/fakes.rs
@@ -2,29 +2,66 @@
 mod tests {
     use super::*;
 
+    fn entry(name: &str, is_dir: bool) -> DirEntry {
+        DirEntry {
+            name: name.to_owned(),
+            is_dir,
+        }
+    }
+
     #[test]
-    fn fake_file_system_lists_direct_children_only() {
+    fn fake_file_system_lists_direct_children_with_their_kind() {
         let fs = FakeFileSystem::default()
             .with_file("/d/a/x", "1")
             .with_file("/d/a/y", "2")
             .with_file("/d/b", "3");
-        assert_eq!(fs.read_dir_names(Path::new("/d")).unwrap(), ["a", "b"]);
-        assert_eq!(fs.read_dir_names(Path::new("/d/a")).unwrap(), ["x", "y"]);
-        assert!(fs.read_dir_names(Path::new("/missing")).is_err());
+        let root = fs.read_dir(Path::new("/d")).unwrap();
+        assert_eq!(root, [entry("a", true), entry("b", false)]);
+        let nested = fs.read_dir(Path::new("/d/a")).unwrap();
+        assert_eq!(nested, [entry("x", false), entry("y", false)]);
+    }
+
+    #[test]
+    fn fake_file_system_models_empty_directories() {
+        let fs = FakeFileSystem::default().with_dir("/d/empty");
+        assert_eq!(fs.read_dir(Path::new("/d/empty")).unwrap(), []);
+        assert_eq!(
+            fs.read_dir(Path::new("/d")).unwrap(),
+            [entry("empty", true)]
+        );
+    }
+
+    #[test]
+    fn fake_file_system_fails_on_missing_paths_and_on_files() {
+        let fs = FakeFileSystem::default().with_file("/d/f", "x");
+        assert!(fs.read_dir(Path::new("/missing")).is_err());
+        assert!(fs.read_dir(Path::new("/d/f")).is_err());
+    }
+
+    #[test]
+    fn fake_file_system_reads_files() {
+        let fs = FakeFileSystem::default().with_file("/f", "hi");
+        assert_eq!(fs.read_to_string(Path::new("/f")).unwrap(), "hi");
+        assert!(fs.read_to_string(Path::new("/g")).is_err());
+        assert!(fs.is_file(Path::new("/f")));
+        assert!(!fs.is_file(Path::new("/g")));
     }
 
     #[test]
     fn fake_env_returns_the_variables_it_was_given() {
         let env = FakeEnv::default().with_var("HOME", "/home/me");
         assert_eq!(env.var("HOME"), Some("/home/me".to_owned()));
+        assert_eq!(env.var_os("HOME"), Some(OsString::from("/home/me")));
         assert_eq!(env.var("MISSING"), None);
     }
 
+    #[cfg(unix)]
     #[test]
-    fn fake_file_system_reads_files() {
-        let fs = FakeFileSystem::default().with_file("/f", "hi");
-        assert_eq!(fs.read_to_string(Path::new("/f")).unwrap(), "hi");
-        assert!(fs.is_file(Path::new("/f")));
-        assert!(!fs.is_file(Path::new("/g")));
+    fn fake_env_keeps_non_utf8_values_for_var_os_only() {
+        use std::os::unix::ffi::OsStringExt;
+        let raw = OsString::from_vec(b"/n\xff".to_vec());
+        let env = FakeEnv::default().with_var_os("NVM_DIR", raw.clone());
+        assert_eq!(env.var("NVM_DIR"), None);
+        assert_eq!(env.var_os("NVM_DIR"), Some(raw));
     }
 }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "no method named `read_dir`
found", "no method named `with_dir` found", "no method named `var_os` found"
and a mismatched `nvm_dir()` return type.

- [ ] **Step 3: Write the implementation**

Apply to `src/adapters/std_env.rs` (above the test module):

```diff
--- a/src/adapters/std_env.rs
+++ b/src/adapters/std_env.rs
@@ -1,3 +1,5 @@
+use std::ffi::OsString;
+
 use crate::ports::Env;
 
 pub struct StdEnv;
@@ -6,4 +8,8 @@
     fn var(&self, key: &str) -> Option<String> {
         std::env::var(key).ok()
     }
+
+    fn var_os(&self, key: &str) -> Option<OsString> {
+        std::env::var_os(key)
+    }
 }
```

Apply to `src/adapters/std_fs.rs` (above the test module):

```diff
--- a/src/adapters/std_fs.rs
+++ b/src/adapters/std_fs.rs
@@ -2,7 +2,7 @@
 use std::io;
 use std::path::Path;
 
-use crate::ports::FileSystem;
+use crate::ports::{DirEntry, FileSystem};
 
 pub struct StdFileSystem;
 
@@ -11,9 +11,15 @@
         fs::read_to_string(path)
     }
 
-    fn read_dir_names(&self, path: &Path) -> io::Result<Vec<String>> {
+    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
         fs::read_dir(path)?
-            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
+            .map(|entry| {
+                let entry = entry?;
+                Ok(DirEntry {
+                    name: entry.file_name().to_string_lossy().into_owned(),
+                    is_dir: entry.path().is_dir(),
+                })
+            })
             .collect()
     }
 
```

Apply to `src/commands/version.rs` (above the test module):

```diff
--- a/src/commands/version.rs
+++ b/src/commands/version.rs
@@ -11,10 +11,10 @@
 /// - [`CliError::NotInstalled`] when no installed version matches, or the
 ///   resolved name is not a version pattern (as in `nvm.sh`, which prints `N/A`).
 pub fn run(context: &Context<'_>, name: &str) -> Result<String, CliError> {
-    let store = FsAliasStore::new(context.fs, &context.nvm_dir());
+    let store = FsAliasStore::new(context.fs, &context.nvm_dir()?);
     let resolved = alias::resolve(&store, name)?;
     let pattern: VersionPattern = resolved.parse().map_err(|_| CliError::NotInstalled)?;
-    let installed = context.installed_versions();
+    let installed = context.installed_versions()?;
     pattern
         .highest_match(&installed)
         .map(ToString::to_string)
```

Apply to `src/context.rs` (above the test module):

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -1,8 +1,9 @@
 //! What every command needs: the ports, plus values derived from them.
 
-use std::path::PathBuf;
+use std::path::{Path, PathBuf};
 
 use crate::domain::version::Version;
+use crate::error::CliError;
 use crate::ports::{Env, FileSystem};
 
 pub struct Context<'a> {
@@ -11,38 +12,51 @@
 }
 
 impl Context<'_> {
-    /// `$NVM_DIR` without a trailing slash, defaulting to `$HOME/.nvm`.
-    #[must_use]
-    pub fn nvm_dir(&self) -> PathBuf {
-        let configured = self.env.var("NVM_DIR").filter(|dir| !dir.is_empty());
-        let dir = configured
-            .unwrap_or_else(|| format!("{}/.nvm", self.env.var("HOME").unwrap_or_default()));
-        match dir.trim_end_matches('/') {
-            "" => PathBuf::from("/"),
-            trimmed => PathBuf::from(trimmed),
-        }
+    /// `$NVM_DIR` normalised (no trailing slash), defaulting to `$HOME/.nvm`.
+    /// Read as an OS string, so a non-UTF-8 path is honoured, never ignored.
+    ///
+    /// # Errors
+    /// Returns [`CliError::NvmDirUnresolved`] when neither variable is set.
+    pub fn nvm_dir(&self) -> Result<PathBuf, CliError> {
+        let configured = self.env.var_os("NVM_DIR").filter(|dir| !dir.is_empty());
+        let dir = match configured {
+            Some(dir) => PathBuf::from(dir),
+            None => {
+                let home = self.env.var_os("HOME").filter(|home| !home.is_empty());
+                PathBuf::from(home.ok_or(CliError::NvmDirUnresolved)?).join(".nvm")
+            }
+        };
+        Ok(dir.components().collect())
     }
 
-    /// Every version found under `versions/node` and `versions/io.js`.
-    /// Directories whose names are not versions are ignored.
-    #[must_use]
-    pub fn installed_versions(&self) -> Vec<Version> {
-        let versions_dir = self.nvm_dir().join("versions");
+    /// Every version installed under `versions/node` and `versions/io.js`.
+    /// Only directories with a canonical version name count: `v20.1.0`, never
+    /// `20.1.0`, `v020.1.0` or a plain file.
+    ///
+    /// # Errors
+    /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
+    pub fn installed_versions(&self) -> Result<Vec<Version>, CliError> {
+        let versions_dir = self.nvm_dir()?.join("versions");
         let node = self.list_versions(&versions_dir.join("node"), "");
         let iojs = self.list_versions(&versions_dir.join("io.js"), "iojs-");
-        node.chain(iojs).collect()
+        Ok(node.chain(iojs).collect())
     }
 
     fn list_versions<'s>(
         &'s self,
-        directory: &std::path::Path,
+        directory: &Path,
         prefix: &'s str,
     ) -> impl Iterator<Item = Version> + 's {
         self.fs
-            .read_dir_names(directory)
+            .read_dir(directory)
             .unwrap_or_default()
             .into_iter()
-            .filter_map(move |name| format!("{prefix}{name}").parse().ok())
+            .filter(|entry| entry.is_dir)
+            .filter_map(move |entry| {
+                let full_name = format!("{prefix}{}", entry.name);
+                let version: Version = full_name.parse().ok()?;
+                (version.to_string() == full_name).then_some(version)
+            })
     }
 }
 
```

Apply to `src/error.rs` (above the test module):

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -49,6 +49,8 @@
     Alias(#[from] AliasError),
     #[error("N/A")]
     NotInstalled,
+    #[error("Neither NVM_DIR nor HOME is set; cannot locate the nvm directory.")]
+    NvmDirUnresolved,
 }
 
 impl CliError {
@@ -56,6 +58,7 @@
     pub fn exit_code(&self) -> NvmExitCode {
         match self {
             Self::Version(_) | Self::NotInstalled => NvmExitCode::InvalidVersion,
+            Self::NvmDirUnresolved => NvmExitCode::Failure,
             Self::Floor(_) => NvmExitCode::BelowVersionFloor,
             Self::Alias(_) => NvmExitCode::AliasLoop,
         }
```

Apply to `src/fakes.rs` (above the test module):

```diff
--- a/src/fakes.rs
+++ b/src/fakes.rs
@@ -1,20 +1,29 @@
 //! In-memory implementations of the ports, for unit tests only.
 
 use std::collections::{BTreeMap, BTreeSet};
+use std::ffi::OsString;
 use std::io;
 use std::path::{Path, PathBuf};
 
-use crate::ports::{Env, FileSystem};
+use crate::ports::{DirEntry, Env, FileSystem};
 
 #[derive(Default)]
 pub struct FakeFileSystem {
     files: BTreeMap<PathBuf, String>,
+    dirs: BTreeSet<PathBuf>,
 }
 
 impl FakeFileSystem {
     #[must_use]
     pub fn with_file(mut self, path: &str, contents: &str) -> Self {
         self.files.insert(PathBuf::from(path), contents.to_owned());
+        self
+    }
+
+    /// An explicit, possibly empty, directory. Parents of files exist already.
+    #[must_use]
+    pub fn with_dir(mut self, path: &str) -> Self {
+        self.dirs.insert(PathBuf::from(path));
         self
     }
 }
@@ -27,18 +36,30 @@
             .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
     }
 
-    fn read_dir_names(&self, path: &Path) -> io::Result<Vec<String>> {
-        let names: BTreeSet<String> = self
-            .files
-            .keys()
-            .filter_map(|file| file.strip_prefix(path).ok())
-            .filter_map(|rest| rest.components().next())
-            .map(|part| part.as_os_str().to_string_lossy().into_owned())
-            .collect();
-        if names.is_empty() {
+    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
+        let mut children: BTreeMap<String, bool> = BTreeMap::new();
+        let mut exists = self.dirs.contains(path);
+        let files = self.files.keys().map(|file| (file, false));
+        let dirs = self.dirs.iter().map(|dir| (dir, true));
+        for (candidate, is_dir_path) in files.chain(dirs) {
+            let Ok(rest) = candidate.strip_prefix(path) else {
+                continue;
+            };
+            let mut parts = rest.components();
+            let Some(first) = parts.next() else {
+                continue;
+            };
+            exists = true;
+            let name = first.as_os_str().to_string_lossy().into_owned();
+            *children.entry(name).or_insert(false) |= is_dir_path || parts.next().is_some();
+        }
+        if !exists {
             return Err(io::Error::from(io::ErrorKind::NotFound));
         }
-        Ok(names.into_iter().collect())
+        Ok(children
+            .into_iter()
+            .map(|(name, is_dir)| DirEntry { name, is_dir })
+            .collect())
     }
 
     fn is_file(&self, path: &Path) -> bool {
@@ -48,19 +69,30 @@
 
 #[derive(Default)]
 pub struct FakeEnv {
-    vars: BTreeMap<String, String>,
+    vars: BTreeMap<String, OsString>,
 }
 
 impl FakeEnv {
     #[must_use]
-    pub fn with_var(mut self, key: &str, value: &str) -> Self {
-        self.vars.insert(key.to_owned(), value.to_owned());
+    pub fn with_var(self, key: &str, value: &str) -> Self {
+        self.with_var_os(key, OsString::from(value))
+    }
+
+    #[must_use]
+    pub fn with_var_os(mut self, key: &str, value: OsString) -> Self {
+        self.vars.insert(key.to_owned(), value);
         self
     }
 }
 
 impl Env for FakeEnv {
     fn var(&self, key: &str) -> Option<String> {
+        self.vars
+            .get(key)
+            .and_then(|value| value.to_str().map(str::to_owned))
+    }
+
+    fn var_os(&self, key: &str) -> Option<OsString> {
         self.vars.get(key).cloned()
     }
 }
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -1,22 +1,35 @@
 //! Traits through which the domain and commands reach the outside world.
 
+use std::ffi::OsString;
 use std::io;
 use std::path::Path;
+
+/// A direct child of a directory.
+#[derive(Debug, Clone, PartialEq, Eq)]
+pub struct DirEntry {
+    pub name: String,
+    pub is_dir: bool,
+}
 
 pub trait FileSystem {
     /// # Errors
     /// Propagates the underlying I/O error.
     fn read_to_string(&self, path: &Path) -> io::Result<String>;
 
-    /// Names (not paths) of the direct children of `path`.
+    /// The direct children of `path`, in unspecified order.
     ///
     /// # Errors
-    /// Propagates the underlying I/O error.
-    fn read_dir_names(&self, path: &Path) -> io::Result<Vec<String>>;
+    /// Propagates the underlying I/O error (for example when `path` is missing
+    /// or is not a directory).
+    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>>;
 
     fn is_file(&self, path: &Path) -> bool;
 }
 
 pub trait Env {
+    /// The variable as text; `None` when unset or not valid UTF-8.
     fn var(&self, key: &str) -> Option<String>;
+
+    /// The variable as an OS string, so non-UTF-8 paths survive.
+    fn var_os(&self, key: &str) -> Option<OsString>;
 }
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 61 unit tests plus the Plan 1 end-to-end test.

- [ ] **Step 5: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean and zero clippy warnings.

```bash
git add src tests
git commit -S -m "refactor(ports): list directory entries with their kind and read env vars as OS strings"
```

---

### Task 2: Shared resolution, `Output`, and the alias store on `Context`

**Files:**

- Create: `src/commands/resolve.rs`
- Modify: `src/commands/mod.rs`, `src/commands/version.rs`, `src/context.rs`,
  `src/cli.rs`

**Interfaces:**

- Consumes: Task 1's `Context::nvm_dir`/`installed_versions`,
  `domain::alias::resolve`, `adapters::fs_alias_store::FsAliasStore`.
- Produces: `commands::Output { stdout, stderr }` with `Output::stdout(text)`
  and `.with_stderr(text)`; `commands::resolve::{Resolved, resolve_installed}`
  where `Resolved` is `Installed(Version)` or `Missing { resolved: String }`;
  `Context::alias_store() -> Result<impl AliasStore, CliError>`. Every command
  now returns `Result<Output, CliError>`.
- Also fixes the Plan 1 minor finding: the stderr-failure test now triggers a
  real stderr write (an alias loop) and pins exit code 8.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -1 +1,2 @@
+pub mod resolve;
 pub mod version;
```

- [ ] **Step 2: Write the failing tests**

Apply to the test module of `src/cli.rs`:

```diff
--- a/src/cli.rs
+++ b/src/cli.rs
@@ -79,18 +79,20 @@
     }
 
     #[test]
-    fn a_failed_stderr_write_does_not_panic() {
-        let fs = FakeFileSystem::default();
+    fn a_failed_stderr_write_keeps_the_exit_code() {
+        let fs = FakeFileSystem::default()
+            .with_file("/n/alias/a", "b")
+            .with_file("/n/alias/b", "a");
         let env = FakeEnv::default().with_var("NVM_DIR", "/n");
         let mut out = Vec::new();
-        let args = ["nvm", "version", "a=b"];
+        let args = ["nvm", "version", "a"];
         let code = run(
             args,
             &Context { fs: &fs, env: &env },
             &mut out,
             &mut BrokenPipe,
         );
-        assert_ne!(code, 0);
+        assert_eq!(code, 8);
     }
 
     #[test]
```

Apply to the test module of `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -0,0 +1,20 @@
+#[cfg(test)]
+mod tests {
+    use super::*;
+
+    #[test]
+    fn stdout_output_has_an_empty_stderr() {
+        let output = Output::stdout("v20.1.0");
+        assert_eq!(output.stdout, "v20.1.0");
+        assert!(output.stderr.is_empty());
+    }
+
+    #[test]
+    fn with_stderr_keeps_stdout() {
+        let output = Output::stdout("out").with_stderr("warning");
+        assert_eq!(
+            (output.stdout.as_str(), output.stderr.as_str()),
+            ("out", "warning")
+        );
+    }
+}
```

Create `src/commands/resolve.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AliasError;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn resolve_with(fs: &FakeFileSystem, name: &str) -> Result<Resolved, CliError> {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        resolve_installed(&Context { fs, env: &env }, name)
    }

    fn installed() -> FakeFileSystem {
        FakeFileSystem::default()
            .with_file("/n/versions/node/v20.1.0/bin/node", "")
            .with_file("/n/versions/node/v20.10.0/bin/node", "")
            .with_file("/n/versions/node/v18.9.0/bin/node", "")
    }

    fn version(text: &str) -> Version {
        text.parse().expect("valid version")
    }

    #[test]
    fn a_pattern_resolves_to_the_highest_installed_match() {
        let resolved = resolve_with(&installed(), "20").unwrap();
        assert_eq!(resolved, Resolved::Installed(version("v20.10.0")));
    }

    #[test]
    fn an_alias_resolves_through_its_chain() {
        let fs = installed().with_file("/n/alias/default", "v18");
        let resolved = resolve_with(&fs, "default").unwrap();
        assert_eq!(resolved, Resolved::Installed(version("v18.9.0")));
    }

    #[test]
    fn a_missing_version_reports_where_the_chain_ended() {
        let fs = installed().with_file("/n/alias/old", "v16");
        let resolved = resolve_with(&fs, "old").unwrap();
        assert_eq!(
            resolved,
            Resolved::Missing {
                resolved: "v16".into()
            }
        );
    }

    #[test]
    fn a_name_that_is_not_a_version_is_missing_not_an_error() {
        let resolved = resolve_with(&installed(), "foo").unwrap();
        assert_eq!(
            resolved,
            Resolved::Missing {
                resolved: "foo".into()
            }
        );
    }

    #[test]
    fn an_alias_loop_is_an_error() {
        let fs = installed()
            .with_file("/n/alias/a", "b")
            .with_file("/n/alias/b", "a");
        let error = resolve_with(&fs, "a").unwrap_err();
        assert!(matches!(error, CliError::Alias(AliasError::Loop(_))));
    }
}
```

Apply to the test module of `src/commands/version.rs`:

```diff
--- a/src/commands/version.rs
+++ b/src/commands/version.rs
@@ -4,7 +4,7 @@
     use crate::error::NvmExitCode;
     use crate::fakes::{FakeEnv, FakeFileSystem};
 
-    fn run_with(fs: &FakeFileSystem, name: &str) -> Result<String, CliError> {
+    fn run_with(fs: &FakeFileSystem, name: &str) -> Result<Output, CliError> {
         let env = FakeEnv::default().with_var("NVM_DIR", "/n");
         run(&Context { fs, env: &env }, name)
     }
@@ -13,18 +13,12 @@
         FakeFileSystem::default()
             .with_file("/n/versions/node/v20.1.0/bin/node", "")
             .with_file("/n/versions/node/v20.10.0/bin/node", "")
-            .with_file("/n/versions/node/v18.9.0/bin/node", "")
     }
 
     #[test]
-    fn resolves_a_major_to_the_highest_installed_version() {
-        assert_eq!(run_with(&installed(), "20").unwrap(), "v20.10.0");
-    }
-
-    #[test]
-    fn resolves_through_an_alias() {
-        let fs = installed().with_file("/n/alias/default", "v18");
-        assert_eq!(run_with(&fs, "default").unwrap(), "v18.9.0");
+    fn prints_the_highest_installed_version() {
+        let output = run_with(&installed(), "20").unwrap();
+        assert_eq!(output, Output::stdout("v20.10.0"));
     }
 
     #[test]
@@ -35,10 +29,9 @@
     }
 
     #[test]
-    fn a_name_that_is_not_a_version_is_not_installed() {
+    fn a_name_that_is_not_a_version_is_not_installed_too() {
         let error = run_with(&installed(), "foo").unwrap_err();
         assert!(matches!(error, CliError::NotInstalled));
-        assert_eq!(error.exit_code(), NvmExitCode::InvalidVersion);
     }
 
     #[test]
```

Apply to the test module of `src/context.rs`:

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -66,6 +66,15 @@
     }
 
     #[test]
+    fn alias_store_reads_the_alias_directory() {
+        let fs = FakeFileSystem::default().with_file("/n/alias/default", "v20");
+        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
+        let context = Context { fs: &fs, env: &env };
+        let store = context.alias_store().unwrap();
+        assert_eq!(store.target("default"), Some("v20".to_owned()));
+    }
+
+    #[test]
     fn installed_versions_reads_node_and_iojs_directories() {
         let fs = FakeFileSystem::default()
             .with_file("/n/versions/node/v20.1.0/bin/node", "")
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find type `Output`",
"cannot find function `resolve_installed`" and "no method named `alias_store`
found".

- [ ] **Step 4: Write the implementation**

Apply to `src/cli.rs` (above the test module):

```diff
--- a/src/cli.rs
+++ b/src/cli.rs
@@ -7,7 +7,7 @@
 
 use crate::adapters::std_env::StdEnv;
 use crate::adapters::std_fs::StdFileSystem;
-use crate::commands;
+use crate::commands::{self, Output};
 use crate::context::Context;
 use crate::error::{CliError, NvmExitCode};
 
@@ -24,7 +24,7 @@
     Version { pattern: String },
 }
 
-fn dispatch(command: &Command, context: &Context<'_>) -> Result<String, CliError> {
+fn dispatch(command: &Command, context: &Context<'_>) -> Result<Output, CliError> {
     match command {
         Command::Version { pattern } => commands::version::run(context, pattern),
     }
@@ -62,16 +62,32 @@
         }
     };
     match dispatch(&cli.command, context) {
-        Ok(text) => exit_code_for_write(emit(out, &format!("{text}\n")), NvmExitCode::Success),
+        Ok(output) => finish(&output, out, err, NvmExitCode::Success),
         // nvm.sh prints N/A on stdout, not stderr.
-        Err(error @ CliError::NotInstalled) => {
-            exit_code_for_write(emit(out, &format!("{error}\n")), error.exit_code())
-        }
+        Err(error @ CliError::NotInstalled) => finish(
+            &Output::stdout(error.to_string()),
+            out,
+            err,
+            error.exit_code(),
+        ),
         Err(error) => {
-            let _ = emit(err, &format!("{error}\n"));
-            error.exit_code().code()
+            let output = Output::default().with_stderr(error.to_string());
+            finish(&output, out, err, error.exit_code())
         }
     }
+}
+
+/// Prints diagnostics first, then the result, and maps a failed stdout write
+/// to a failure exit code.
+fn finish(output: &Output, out: &mut dyn Write, err: &mut dyn Write, success: NvmExitCode) -> u8 {
+    if !output.stderr.is_empty() {
+        // Nowhere left to report a failed stderr write.
+        let _ = emit(err, &format!("{}\n", output.stderr));
+    }
+    if output.stdout.is_empty() {
+        return success.code();
+    }
+    exit_code_for_write(emit(out, &format!("{}\n", output.stdout)), success)
 }
 
 /// Entry point shared by the `nvmrc` and `nvm` binaries.
```

Apply to `src/commands/mod.rs` (above the test module):

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -1,2 +1,27 @@
 pub mod resolve;
 pub mod version;
+
+/// What a command wants printed. Text carries no trailing newline: the CLI
+/// adds one to each non-empty stream.
+#[derive(Debug, Default, PartialEq, Eq)]
+pub struct Output {
+    pub stdout: String,
+    pub stderr: String,
+}
+
+impl Output {
+    #[must_use]
+    pub fn stdout(text: impl Into<String>) -> Self {
+        Self {
+            stdout: text.into(),
+            stderr: String::new(),
+        }
+    }
+
+    #[must_use]
+    pub fn with_stderr(mut self, text: impl Into<String>) -> Self {
+        self.stderr = text.into();
+        self
+    }
+}
+
```

Insert above the `#[cfg(test)]` line of `src/commands/resolve.rs`:

```rust
//! Shared by the commands that take a version or alias: resolve the name
//! through the alias files, then match it against the installed versions.

use crate::context::Context;
use crate::domain::alias;
use crate::domain::version::{Version, VersionPattern};
use crate::error::CliError;

#[derive(Debug, PartialEq, Eq)]
pub enum Resolved {
    Installed(Version),
    /// No installed version matches. `resolved` is where the alias chain
    /// ended, which is the name itself when it is not an alias.
    Missing {
        resolved: String,
    },
}

/// # Errors
/// - [`CliError::Alias`] when the alias chain loops.
/// - [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn resolve_installed(context: &Context<'_>, name: &str) -> Result<Resolved, CliError> {
    let resolved = alias::resolve(&context.alias_store()?, name)?;
    let installed = context.installed_versions()?;
    let found = resolved
        .parse::<VersionPattern>()
        .ok()
        .and_then(|pattern| pattern.highest_match(&installed).copied());
    Ok(found.map_or(Resolved::Missing { resolved }, Resolved::Installed))
}

```

Apply to `src/commands/version.rs` (above the test module):

```diff
--- a/src/commands/version.rs
+++ b/src/commands/version.rs
@@ -1,23 +1,18 @@
 //! `nvm version <pattern>`: the highest installed version matching a pattern.
 
-use crate::adapters::fs_alias_store::FsAliasStore;
+use crate::commands::Output;
+use crate::commands::resolve::{Resolved, resolve_installed};
 use crate::context::Context;
-use crate::domain::alias;
-use crate::domain::version::VersionPattern;
 use crate::error::CliError;
 
 /// # Errors
 /// - [`CliError::Alias`] when the alias chain loops.
 /// - [`CliError::NotInstalled`] when no installed version matches, or the
 ///   resolved name is not a version pattern (as in `nvm.sh`, which prints `N/A`).
-pub fn run(context: &Context<'_>, name: &str) -> Result<String, CliError> {
-    let store = FsAliasStore::new(context.fs, &context.nvm_dir()?);
-    let resolved = alias::resolve(&store, name)?;
-    let pattern: VersionPattern = resolved.parse().map_err(|_| CliError::NotInstalled)?;
-    let installed = context.installed_versions()?;
-    pattern
-        .highest_match(&installed)
-        .map(ToString::to_string)
-        .ok_or(CliError::NotInstalled)
+pub fn run(context: &Context<'_>, name: &str) -> Result<Output, CliError> {
+    match resolve_installed(context, name)? {
+        Resolved::Installed(version) => Ok(Output::stdout(version.to_string())),
+        Resolved::Missing { .. } => Err(CliError::NotInstalled),
+    }
 }
 
```

Apply to `src/context.rs` (above the test module):

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -2,6 +2,8 @@
 
 use std::path::{Path, PathBuf};
 
+use crate::adapters::fs_alias_store::FsAliasStore;
+use crate::domain::alias::AliasStore;
 use crate::domain::version::Version;
 use crate::error::CliError;
 use crate::ports::{Env, FileSystem};
@@ -27,6 +29,14 @@
             }
         };
         Ok(dir.components().collect())
+    }
+
+    /// The alias files under `$NVM_DIR/alias`.
+    ///
+    /// # Errors
+    /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
+    pub fn alias_store(&self) -> Result<impl AliasStore, CliError> {
+        Ok(FsAliasStore::new(self.fs, &self.nvm_dir()?))
     }
 
     /// Every version installed under `versions/node` and `versions/io.js`.
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 68 unit tests plus the end-to-end test.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean and zero clippy warnings.

```bash
git add src tests
git commit -S -m "refactor(commands): share version resolution and expose the alias store through Context"
```

---

### Task 3: `nvm current` and `nvm version` without an argument

**Files:**

- Create: `src/domain/path_search.rs`, `src/domain/current.rs`,
  `src/commands/current.rs`
- Modify: `src/domain/mod.rs`, `src/commands/mod.rs`,
  `src/commands/version.rs`, `src/cli.rs`

**Interfaces:**

- Consumes: `Context`, `ports::{FileSystem, Env}`, `Version`.
- Produces: `domain::path_search::find_in_path(&dyn FileSystem, &OsStr, &str)
  -> Option<PathBuf>`; `domain::current::{Current, classify}` where `Current`
  is `None`, `System` or `Version(Version)` and displays as `none`, `system`
  or the version; `commands::current::run(&Context) -> Result<Output,
  CliError>`.
- Behaviour: `current` looks up the first `node` on `PATH` and classifies its
  path. Unlike `nvm.sh` it does not run `node --version`, so a path under
  `$NVM_DIR` that names no version (such as the `current` symlink) reports
  `none`. `nvm version current` and `nvm version` (no argument) mean the same.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -1,3 +1,4 @@
+pub mod current;
 pub mod resolve;
 pub mod version;
 
```

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -1,4 +1,6 @@
 pub mod alias;
+pub mod current;
 pub mod floor;
 pub mod nvmrc;
+pub mod path_search;
 pub mod version;
```

- [ ] **Step 2: Write the failing tests**

Apply to the test module of `src/cli.rs`:

```diff
--- a/src/cli.rs
+++ b/src/cli.rs
@@ -20,6 +20,30 @@
         assert_eq!(
             run_cli(&["nvm", "version", "20"]),
             (0, "v20.1.0\n".into(), String::new())
+        );
+    }
+
+    #[test]
+    fn current_prints_the_active_version() {
+        let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
+        let env = FakeEnv::default()
+            .with_var("NVM_DIR", "/n")
+            .with_var("PATH", "/n/versions/node/v20.1.0/bin");
+        let (mut out, mut err) = (Vec::new(), Vec::new());
+        let code = run(
+            ["nvm", "current"],
+            &Context { fs: &fs, env: &env },
+            &mut out,
+            &mut err,
+        );
+        assert_eq!((code, out, err), (0, b"v20.1.0\n".to_vec(), Vec::new()));
+    }
+
+    #[test]
+    fn version_without_an_argument_means_current() {
+        assert_eq!(
+            run_cli(&["nvm", "version"]),
+            (0, "none\n".into(), String::new())
         );
     }
 
```

Create `src/commands/current.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn current_with(fs: &FakeFileSystem, path: &str) -> String {
        let env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", path);
        run(&Context { fs, env: &env }).unwrap().stdout
    }

    #[test]
    fn reports_the_nvm_version_first_on_path() {
        let fs = FakeFileSystem::default()
            .with_file("/n/versions/node/v20.1.0/bin/node", "")
            .with_file("/usr/bin/node", "");
        let path = "/n/versions/node/v20.1.0/bin:/usr/bin";
        assert_eq!(current_with(&fs, path), "v20.1.0");
    }

    #[test]
    fn reports_system_when_the_first_node_is_outside_nvm_dir() {
        let fs = FakeFileSystem::default().with_file("/usr/bin/node", "");
        assert_eq!(current_with(&fs, "/usr/bin"), "system");
    }

    #[test]
    fn reports_none_without_any_node() {
        assert_eq!(current_with(&FakeFileSystem::default(), "/usr/bin"), "none");
    }

    #[test]
    fn reports_none_when_path_is_unset() {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let fs = FakeFileSystem::default();
        let output = run(&Context { fs: &fs, env: &env }).unwrap();
        assert_eq!(output.stdout, "none");
    }
}
```

Apply to the test module of `src/commands/version.rs`:

```diff
--- a/src/commands/version.rs
+++ b/src/commands/version.rs
@@ -22,6 +22,16 @@
     }
 
     #[test]
+    fn current_is_the_version_of_the_active_node() {
+        let fs = installed().with_file("/usr/bin/node", "");
+        let env = FakeEnv::default()
+            .with_var("NVM_DIR", "/n")
+            .with_var("PATH", "/n/versions/node/v20.1.0/bin:/usr/bin");
+        let output = run(&Context { fs: &fs, env: &env }, "current").unwrap();
+        assert_eq!(output, Output::stdout("v20.1.0"));
+    }
+
+    #[test]
     fn a_missing_version_is_not_installed_with_exit_code_3() {
         let error = run_with(&installed(), "16").unwrap_err();
         assert_eq!(error.to_string(), "N/A");
```

Create `src/domain/current.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn classify_path(path: &str) -> String {
        classify(Path::new(path), Path::new("/n")).to_string()
    }

    #[test]
    fn a_node_outside_nvm_dir_is_system() {
        assert_eq!(classify_path("/usr/bin/node"), "system");
    }

    #[test]
    fn a_directory_that_only_shares_the_prefix_is_still_system() {
        assert_eq!(
            classify_path("/nvm-other/versions/node/v1.0.0/bin/node"),
            "system"
        );
    }

    #[test]
    fn a_node_version_directory_names_the_version() {
        assert_eq!(
            classify_path("/n/versions/node/v20.1.0/bin/node"),
            "v20.1.0"
        );
    }

    #[test]
    fn an_iojs_version_directory_gets_the_iojs_prefix() {
        assert_eq!(
            classify_path("/n/versions/io.js/v3.0.0/bin/node"),
            "iojs-v3.0.0"
        );
    }

    #[test]
    fn the_legacy_layout_names_the_version() {
        assert_eq!(classify_path("/n/v0.10.48/bin/node"), "v0.10.48");
    }

    #[test]
    fn a_path_under_nvm_dir_without_a_version_is_none() {
        assert_eq!(classify_path("/n/current/bin/node"), "none");
        assert_eq!(
            classify_path("/n/versions/node/not-a-version/bin/node"),
            "none"
        );
    }

    #[test]
    fn display_of_none_and_system() {
        assert_eq!(Current::None.to_string(), "none");
        assert_eq!(Current::System.to_string(), "system");
    }
}
```

Create `src/domain/path_search.rs` containing only the test module:

```rust
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
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function
`find_in_path`", "cannot find function `classify`" and "unresolved import
`crate::commands::current`".

- [ ] **Step 4: Write the implementation**

Apply to `src/cli.rs` (above the test module):

```diff
--- a/src/cli.rs
+++ b/src/cli.rs
@@ -20,13 +20,19 @@
 
 #[derive(Subcommand)]
 enum Command {
-    /// Print the highest installed version matching a version or alias.
-    Version { pattern: String },
+    /// Print the highest installed version matching a version or alias
+    /// (the current version when no argument is given).
+    Version { pattern: Option<String> },
+    /// Print the version of the node that is active in this shell.
+    Current,
 }
 
 fn dispatch(command: &Command, context: &Context<'_>) -> Result<Output, CliError> {
     match command {
-        Command::Version { pattern } => commands::version::run(context, pattern),
+        Command::Version { pattern } => {
+            commands::version::run(context, pattern.as_deref().unwrap_or("current"))
+        }
+        Command::Current => commands::current::run(context),
     }
 }
 
```

Insert above the `#[cfg(test)]` line of `src/commands/current.rs`:

```rust
//! `nvm current`: the version of the `node` that is first on `PATH`.

use crate::commands::Output;
use crate::context::Context;
use crate::domain::current::{Current, classify};
use crate::domain::path_search::find_in_path;
use crate::error::CliError;

/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(context: &Context<'_>) -> Result<Output, CliError> {
    let nvm_dir = context.nvm_dir()?;
    let path_variable = context.env.var_os("PATH").unwrap_or_default();
    let current = find_in_path(context.fs, &path_variable, "node")
        .map_or(Current::None, |node| classify(&node, &nvm_dir));
    Ok(Output::stdout(current.to_string()))
}

```

Apply to `src/commands/version.rs` (above the test module):

```diff
--- a/src/commands/version.rs
+++ b/src/commands/version.rs
@@ -1,7 +1,7 @@
 //! `nvm version <pattern>`: the highest installed version matching a pattern.
 
-use crate::commands::Output;
 use crate::commands::resolve::{Resolved, resolve_installed};
+use crate::commands::{Output, current};
 use crate::context::Context;
 use crate::error::CliError;
 
@@ -10,6 +10,9 @@
 /// - [`CliError::NotInstalled`] when no installed version matches, or the
 ///   resolved name is not a version pattern (as in `nvm.sh`, which prints `N/A`).
 pub fn run(context: &Context<'_>, name: &str) -> Result<Output, CliError> {
+    if name == "current" {
+        return current::run(context);
+    }
     match resolve_installed(context, name)? {
         Resolved::Installed(version) => Ok(Output::stdout(version.to_string())),
         Resolved::Missing { .. } => Err(CliError::NotInstalled),
```

Insert above the `#[cfg(test)]` line of `src/domain/current.rs`:

```rust
//! What `nvm current` reports: which node the shell would run.

use std::fmt;
use std::path::Path;

use crate::domain::version::Version;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Current {
    /// No `node` on `PATH`.
    None,
    /// A `node` outside `$NVM_DIR`.
    System,
    Version(Version),
}

impl fmt::Display for Current {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str("none"),
            Self::System => formatter.write_str("system"),
            Self::Version(version) => version.fmt(formatter),
        }
    }
}

/// Classifies the `node` found first on `PATH`. Unlike `nvm.sh` this reads the
/// version from the path instead of running `node --version`, so a path under
/// `$NVM_DIR` that names no version (such as the `current` symlink) is `None`.
#[must_use]
pub fn classify(node_path: &Path, nvm_dir: &Path) -> Current {
    let Ok(relative) = node_path.strip_prefix(nvm_dir) else {
        return Current::System;
    };
    let parts: Vec<&str> = relative
        .components()
        .filter_map(|part| part.as_os_str().to_str())
        .collect();
    let version: Option<Version> = match parts.as_slice() {
        ["versions", "node", name, ..] => name.parse().ok(),
        ["versions", "io.js", name, ..] => format!("iojs-{name}").parse().ok(),
        // The legacy layout: $NVM_DIR/vX.Y.Z/bin/node.
        [name, ..] => name.parse().ok(),
        [] => None,
    };
    version.map_or(Current::None, Current::Version)
}

```

Insert above the `#[cfg(test)]` line of `src/domain/path_search.rs`:

```rust
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

```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 86 unit tests plus the end-to-end test.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean and zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(commands): add current and make version without an argument mean current"
```

---

### Task 4: `nvm which`

**Files:**

- Create: `src/commands/which.rs`
- Modify: `src/error.rs`, `src/domain/version.rs`, `src/domain/path_search.rs`,
  `src/commands/current.rs`, `src/commands/mod.rs`, `src/cli.rs`

**Interfaces:**

- Consumes: `commands::resolve::resolve_installed`, `commands::current`.
- Produces: `NvmExitCode::NotFound` (127); `CliError::{Usage(String),
  VersionNotInstalled(String), SystemNodeNotFound}`;
  `Flavor::versions_directory() -> &'static str` and
  `Version::directory_name() -> String` (`v3.0.0`, no `iojs-` prefix);
  `domain::path_search::find_in_dirs`; `commands::current::detect`;
  `commands::which::run(&Context, Option<&str>) -> Result<Output, CliError>`.
- Behaviour (from `nvm.sh`): prints `<nvm_dir>/versions/<node|io.js>/<vX.Y.Z>/bin/node`;
  `system` prints the first `node` on `PATH` outside `$NVM_DIR`; a missing
  version prints the `N/A: version "..." is not yet installed.` message on
  stderr and exits 1; no argument prints the usage and exits 127 (the
  `.nvmrc` fallback arrives with `.nvmrc` semantics in Plan 6).
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -1,6 +1,7 @@
 pub mod current;
 pub mod resolve;
 pub mod version;
+pub mod which;
 
 /// What a command wants printed. Text carries no trailing newline: the CLI
 /// adds one to each non-empty stream.
```

- [ ] **Step 2: Write the failing tests**

Apply to the test module of `src/cli.rs`:

```diff
--- a/src/cli.rs
+++ b/src/cli.rs
@@ -37,6 +37,33 @@
             &mut err,
         );
         assert_eq!((code, out, err), (0, b"v20.1.0\n".to_vec(), Vec::new()));
+    }
+
+    #[test]
+    fn which_prints_the_binary_path() {
+        assert_eq!(
+            run_cli(&["nvm", "which", "20"]),
+            (
+                0,
+                "/n/versions/node/v20.1.0/bin/node\n".into(),
+                String::new()
+            )
+        );
+    }
+
+    #[test]
+    fn which_of_a_missing_version_fails_on_stderr_with_exit_1() {
+        let (code, out, err) = run_cli(&["nvm", "which", "16"]);
+        assert_eq!(code, 1);
+        assert!(out.is_empty());
+        assert!(err.starts_with("N/A: version \"v16\" is not yet installed."));
+    }
+
+    #[test]
+    fn which_without_an_argument_is_a_usage_error_with_exit_127() {
+        let (code, out, err) = run_cli(&["nvm", "which"]);
+        assert_eq!(code, 127);
+        assert!(out.is_empty() && err.starts_with("Usage: nvm which"));
     }
 
     #[test]
```

Create `src/commands/which.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn installed() -> FakeFileSystem {
        FakeFileSystem::default()
            .with_file("/n/versions/node/v20.1.0/bin/node", "")
            .with_file("/n/versions/node/v18.9.0/bin/node", "")
            .with_file("/n/versions/io.js/v3.0.0/bin/node", "")
    }

    fn which_with(fs: &FakeFileSystem, path: &str, name: Option<&str>) -> Result<Output, CliError> {
        let env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", path);
        run(&Context { fs, env: &env }, name)
    }

    fn which(name: &str) -> Result<Output, CliError> {
        which_with(&installed(), "/usr/bin", Some(name))
    }

    #[test]
    fn prints_the_node_binary_of_the_highest_match() {
        let output = which("20").unwrap();
        assert_eq!(output, Output::stdout("/n/versions/node/v20.1.0/bin/node"));
    }

    #[test]
    fn follows_aliases() {
        let fs = installed().with_file("/n/alias/default", "v18");
        let output = which_with(&fs, "/usr/bin", Some("default")).unwrap();
        assert_eq!(output, Output::stdout("/n/versions/node/v18.9.0/bin/node"));
    }

    #[test]
    fn iojs_binaries_live_under_the_io_js_directory() {
        let output = which("iojs-v3.0.0").unwrap();
        assert_eq!(output, Output::stdout("/n/versions/io.js/v3.0.0/bin/node"));
    }

    #[test]
    fn current_means_the_active_version() {
        let path = "/n/versions/node/v18.9.0/bin";
        let output = which_with(&installed(), path, Some("current")).unwrap();
        assert_eq!(output, Output::stdout("/n/versions/node/v18.9.0/bin/node"));
    }

    #[test]
    fn a_missing_version_gets_the_install_hint_and_exit_1() {
        let error = which("16").unwrap_err();
        let expected = "N/A: version \"v16\" is not yet installed.\n\n\
                        You need to run `nvm install 16` to install and use it.";
        assert_eq!(error.to_string(), expected);
        assert_eq!(error.exit_code(), NvmExitCode::Failure);
    }

    #[test]
    fn a_missing_alias_target_shows_the_chain() {
        let fs = installed().with_file("/n/alias/old", "16");
        let error = which_with(&fs, "/usr/bin", Some("old")).unwrap_err();
        let expected = "N/A: version \"old -> v16\" is not yet installed.\n\n\
                        You need to run `nvm install old` to install and use it.";
        assert_eq!(error.to_string(), expected);
    }

    #[test]
    fn an_alias_loop_has_exit_code_8_and_names_the_alias() {
        let fs = installed().with_file("/n/alias/loop", "loop");
        let error = which_with(&fs, "/usr/bin", Some("loop")).unwrap_err();
        assert_eq!(
            error.to_string(),
            "The alias \"loop\" leads to an infinite loop. Aborting."
        );
        assert_eq!(error.exit_code(), NvmExitCode::AliasLoop);
    }

    #[test]
    fn system_is_the_first_node_outside_nvm_dir() {
        let fs = installed().with_file("/usr/bin/node", "");
        let path = "/n/versions/node/v20.1.0/bin:/usr/bin";
        let output = which_with(&fs, path, Some("system")).unwrap();
        assert_eq!(output, Output::stdout("/usr/bin/node"));
    }

    #[test]
    fn system_without_a_system_node_is_exit_127() {
        let path = "/n/versions/node/v20.1.0/bin";
        let error = which_with(&installed(), path, Some("system")).unwrap_err();
        assert_eq!(error.to_string(), "System version of node not found.");
        assert_eq!(error.exit_code(), NvmExitCode::NotFound);
    }

    #[test]
    fn without_a_version_it_is_a_usage_error_with_exit_127() {
        let error = which_with(&installed(), "/usr/bin", None).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("Usage: nvm which [current | <version>]")
        );
        assert_eq!(error.exit_code(), NvmExitCode::NotFound);
    }
}
```

Apply to the test module of `src/domain/version.rs`:

```diff
--- a/src/domain/version.rs
+++ b/src/domain/version.rs
@@ -16,6 +16,18 @@
         let parsed = version("iojs-v3.0.0");
         assert_eq!(parsed.flavor, Flavor::IoJs);
         assert_eq!(parsed.to_string(), "iojs-v3.0.0");
+    }
+
+    #[test]
+    fn directory_name_drops_the_iojs_prefix() {
+        assert_eq!(version("iojs-v3.0.0").directory_name(), "v3.0.0");
+        assert_eq!(version("20.1.2").directory_name(), "v20.1.2");
+    }
+
+    #[test]
+    fn flavors_name_their_versions_directory() {
+        assert_eq!(Flavor::Node.versions_directory(), "node");
+        assert_eq!(Flavor::IoJs.versions_directory(), "io.js");
     }
 
     #[test]
```

Apply to the test module of `src/error.rs`:

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -8,6 +8,7 @@
         assert_eq!(NvmExitCode::InvalidVersion.code(), 3);
         assert_eq!(NvmExitCode::BelowVersionFloor.code(), 7);
         assert_eq!(NvmExitCode::AliasLoop.code(), 8);
+        assert_eq!(NvmExitCode::NotFound.code(), 127);
     }
 
     #[test]
@@ -29,5 +30,11 @@
             NvmExitCode::InvalidVersion
         );
         assert_eq!(CliError::NvmDirUnresolved.exit_code(), NvmExitCode::Failure);
+        let not_installed = CliError::VersionNotInstalled("x".into());
+        assert_eq!(not_installed.exit_code(), NvmExitCode::Failure);
+        let usage = CliError::Usage("x".into());
+        assert_eq!(usage.exit_code(), NvmExitCode::NotFound);
+        let no_system_node = CliError::SystemNodeNotFound;
+        assert_eq!(no_system_node.exit_code(), NvmExitCode::NotFound);
     }
 }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find module `which`",
"no variant named `NotFound`", "no method named `directory_name` found" and
"cannot find function `find_in_dirs`".

- [ ] **Step 4: Write the implementation**

Apply to `src/cli.rs` (above the test module):

```diff
--- a/src/cli.rs
+++ b/src/cli.rs
@@ -25,6 +25,8 @@
     Version { pattern: Option<String> },
     /// Print the version of the node that is active in this shell.
     Current,
+    /// Print the path to the node binary of a version or alias.
+    Which { version: Option<String> },
 }
 
 fn dispatch(command: &Command, context: &Context<'_>) -> Result<Output, CliError> {
@@ -33,6 +35,7 @@
             commands::version::run(context, pattern.as_deref().unwrap_or("current"))
         }
         Command::Current => commands::current::run(context),
+        Command::Which { version } => commands::which::run(context, version.as_deref()),
     }
 }
 
```

Apply to `src/commands/current.rs` (above the test module):

```diff
--- a/src/commands/current.rs
+++ b/src/commands/current.rs
@@ -6,13 +6,20 @@
 use crate::domain::path_search::find_in_path;
 use crate::error::CliError;
 
+/// The classification of the `node` that is first on `PATH`.
+///
+/// # Errors
+/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
+pub fn detect(context: &Context<'_>) -> Result<Current, CliError> {
+    let nvm_dir = context.nvm_dir()?;
+    let path_variable = context.env.var_os("PATH").unwrap_or_default();
+    Ok(find_in_path(context.fs, &path_variable, "node")
+        .map_or(Current::None, |node| classify(&node, &nvm_dir)))
+}
+
 /// # Errors
 /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
 pub fn run(context: &Context<'_>) -> Result<Output, CliError> {
-    let nvm_dir = context.nvm_dir()?;
-    let path_variable = context.env.var_os("PATH").unwrap_or_default();
-    let current = find_in_path(context.fs, &path_variable, "node")
-        .map_or(Current::None, |node| classify(&node, &nvm_dir));
-    Ok(Output::stdout(current.to_string()))
+    Ok(Output::stdout(detect(context)?.to_string()))
 }
 
```

Insert above the `#[cfg(test)]` line of `src/commands/which.rs`:

```rust
//! `nvm which <version>`: the path to the `node` binary of a version.

use std::path::PathBuf;

use crate::commands::Output;
use crate::commands::current;
use crate::commands::resolve::{Resolved, resolve_installed};
use crate::context::Context;
use crate::domain::path_search::find_in_dirs;
use crate::domain::version::Version;
use crate::error::CliError;

const USAGE: &str = "Usage: nvm which [current | <version>]\n  \
Provide a <version>, or run from a directory containing an .nvmrc file.\n  \
Run `nvm --help` for full help.";

/// # Errors
/// - [`CliError::Usage`] when no version is given.
/// - [`CliError::Alias`] when the alias chain loops.
/// - [`CliError::VersionNotInstalled`] when the version is not installed.
/// - [`CliError::SystemNodeNotFound`] for `system` without a system node.
pub fn run(context: &Context<'_>, name: Option<&str>) -> Result<Output, CliError> {
    let name = name.ok_or_else(|| CliError::Usage(USAGE.to_owned()))?;
    let name = if name == "current" {
        current::detect(context)?.to_string()
    } else {
        name.to_owned()
    };
    if name == "system" {
        return system_node(context);
    }
    match resolve_installed(context, &name)? {
        Resolved::Installed(version) => {
            let binary = node_binary(context, &version)?;
            Ok(Output::stdout(binary.to_string_lossy()))
        }
        Resolved::Missing { resolved } => Err(CliError::VersionNotInstalled(
            not_installed_message(&name, &resolved),
        )),
    }
}

fn node_binary(context: &Context<'_>, version: &Version) -> Result<PathBuf, CliError> {
    Ok(context
        .nvm_dir()?
        .join("versions")
        .join(version.flavor.versions_directory())
        .join(version.directory_name())
        .join("bin")
        .join("node"))
}

/// The first `node` on `PATH` that does not live under `$NVM_DIR`.
fn system_node(context: &Context<'_>) -> Result<Output, CliError> {
    let nvm_dir = context.nvm_dir()?;
    let path_variable = context.env.var_os("PATH").unwrap_or_default();
    let outside_nvm = std::env::split_paths(&path_variable)
        .filter(|directory| !directory.starts_with(&nvm_dir))
        .collect::<Vec<_>>();
    find_in_dirs(context.fs, outside_nvm, "node")
        .map(|node| Output::stdout(node.to_string_lossy()))
        .ok_or(CliError::SystemNodeNotFound)
}

/// `20` is shown as `v20`, like `nvm_ensure_version_prefix`.
fn with_v_prefix(name: &str) -> String {
    if name.starts_with(|first: char| first.is_ascii_digit()) {
        format!("v{name}")
    } else {
        name.to_owned()
    }
}

fn not_installed_message(name: &str, resolved: &str) -> String {
    let shown = if resolved == name {
        with_v_prefix(name)
    } else {
        format!("{name} -> {}", with_v_prefix(resolved))
    };
    format!(
        "N/A: version \"{shown}\" is not yet installed.\n\n\
         You need to run `nvm install {name}` to install and use it."
    )
}

```

Apply to `src/domain/path_search.rs` (above the test module):

```diff
--- a/src/domain/path_search.rs
+++ b/src/domain/path_search.rs
@@ -5,13 +5,24 @@
 
 use crate::ports::FileSystem;
 
-/// The first `PATH` entry that contains a file called `name`. Empty entries
-/// are skipped rather than read as the current directory.
+/// The first of `directories` that contains a file called `name`. Empty
+/// entries are skipped rather than read as the current directory.
 #[must_use]
-pub fn find_in_path(fs: &dyn FileSystem, path_variable: &OsStr, name: &str) -> Option<PathBuf> {
-    std::env::split_paths(path_variable)
+pub fn find_in_dirs(
+    fs: &dyn FileSystem,
+    directories: impl IntoIterator<Item = PathBuf>,
+    name: &str,
+) -> Option<PathBuf> {
+    directories
+        .into_iter()
         .filter(|directory| !directory.as_os_str().is_empty())
         .map(|directory| directory.join(name))
         .find(|candidate| fs.is_file(candidate))
 }
 
+/// The first `PATH` entry that contains a file called `name`.
+#[must_use]
+pub fn find_in_path(fs: &dyn FileSystem, path_variable: &OsStr, name: &str) -> Option<PathBuf> {
+    find_in_dirs(fs, std::env::split_paths(path_variable), name)
+}
+
```

Apply to `src/domain/version.rs` (above the test module):

```diff
--- a/src/domain/version.rs
+++ b/src/domain/version.rs
@@ -87,15 +87,28 @@
             Flavor::Node => "",
             Flavor::IoJs => "iojs-",
         };
-        write!(
-            formatter,
-            "{prefix}v{}.{}.{}",
-            self.major, self.minor, self.patch
-        )
+        write!(formatter, "{prefix}{}", self.directory_name())
+    }
+}
+
+impl Flavor {
+    /// The directory under `$NVM_DIR/versions` holding this flavor.
+    #[must_use]
+    pub fn versions_directory(self) -> &'static str {
+        match self {
+            Self::Node => "node",
+            Self::IoJs => "io.js",
+        }
     }
 }
 
 impl Version {
+    /// The on-disk directory name, without the `iojs-` prefix: `v3.0.0`.
+    #[must_use]
+    pub fn directory_name(&self) -> String {
+        format!("v{}.{}.{}", self.major, self.minor, self.patch)
+    }
+
     /// The numeric part, ignoring flavor (used for floor comparison).
     #[must_use]
     pub fn triple(&self) -> (u64, u64, u64) {
```

Apply to `src/error.rs` (above the test module):

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -10,6 +10,8 @@
     InvalidVersion = 3,
     BelowVersionFloor = 7,
     AliasLoop = 8,
+    /// A usage error, or a requested system version that does not exist.
+    NotFound = 127,
 }
 
 impl NvmExitCode {
@@ -51,6 +53,14 @@
     NotInstalled,
     #[error("Neither NVM_DIR nor HOME is set; cannot locate the nvm directory.")]
     NvmDirUnresolved,
+    /// A multi-line usage message, printed as is.
+    #[error("{0}")]
+    Usage(String),
+    /// The full "not yet installed" message, printed as is.
+    #[error("{0}")]
+    VersionNotInstalled(String),
+    #[error("System version of node not found.")]
+    SystemNodeNotFound,
 }
 
 impl CliError {
@@ -58,7 +68,8 @@
     pub fn exit_code(&self) -> NvmExitCode {
         match self {
             Self::Version(_) | Self::NotInstalled => NvmExitCode::InvalidVersion,
-            Self::NvmDirUnresolved => NvmExitCode::Failure,
+            Self::NvmDirUnresolved | Self::VersionNotInstalled(_) => NvmExitCode::Failure,
+            Self::Usage(_) | Self::SystemNodeNotFound => NvmExitCode::NotFound,
             Self::Floor(_) => NvmExitCode::BelowVersionFloor,
             Self::Alias(_) => NvmExitCode::AliasLoop,
         }
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 101 unit tests plus the end-to-end test.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean and zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(commands): add which"
```

---

### Task 5: `nvm unalias`

**Files:**

- Create: `src/commands/unalias.rs`
- Modify: `src/ports/mod.rs`, `src/fakes.rs`, `src/adapters/std_fs.rs`,
  `src/error.rs`, `src/context.rs`, `src/domain/alias.rs`,
  `src/commands/mod.rs`, `src/cli.rs`

**Interfaces:**

- Consumes: `Context::alias_store`, `ports::FileSystem`.
- Produces: `FileSystem::remove_file(&self, &Path) -> io::Result<()>` (the fake
  now keeps its files behind a `RefCell`); `CliError::{InvalidArgument(String),
  Io(io::Error)}` (exit 1); `Context::alias_dir()`;
  `domain::alias::BUILTIN_ALIASES`; `commands::unalias::run(&Context,
  &[String]) -> Result<Output, CliError>`.
- Behaviour (from `nvm.sh`): exactly one name or a usage error (exit 127); a
  `/` in the name is rejected (exit 1); an existing alias is deleted and
  `Deleted alias X - restore it with `nvm alias "X" "<target>"`` is printed; a
  missing alias is only `Alias X doesn't exist!` on stderr with **exit 0**
  (an `nvm.sh` quirk kept on purpose); `stable`, `unstable`, `iojs`, `node`
  and `system` are built-in and cannot be deleted (exit 1).
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -1,5 +1,6 @@
 pub mod current;
 pub mod resolve;
+pub mod unalias;
 pub mod version;
 pub mod which;
 
```

- [ ] **Step 2: Write the failing tests**

Apply to the test module of `src/cli.rs`:

```diff
--- a/src/cli.rs
+++ b/src/cli.rs
@@ -2,6 +2,7 @@
 mod tests {
     use super::*;
     use crate::fakes::{FakeEnv, FakeFileSystem};
+    use crate::ports::FileSystem;
 
     fn run_cli(args: &[&str]) -> (u8, String, String) {
         let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
@@ -64,6 +65,26 @@
         let (code, out, err) = run_cli(&["nvm", "which"]);
         assert_eq!(code, 127);
         assert!(out.is_empty() && err.starts_with("Usage: nvm which"));
+    }
+
+    #[test]
+    fn unalias_removes_the_alias_and_says_how_to_restore_it() {
+        let fs = FakeFileSystem::default().with_file("/n/alias/work", "v18");
+        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
+        let (mut out, mut err) = (Vec::new(), Vec::new());
+        let args = ["nvm", "unalias", "work"];
+        let code = run(args, &Context { fs: &fs, env: &env }, &mut out, &mut err);
+        assert_eq!(code, 0);
+        let printed = String::from_utf8(out).unwrap();
+        assert!(printed.starts_with("Deleted alias work - restore it"));
+        assert!(!fs.is_file(std::path::Path::new("/n/alias/work")));
+    }
+
+    #[test]
+    fn unalias_without_a_name_is_a_usage_error_with_exit_127() {
+        let (code, out, err) = run_cli(&["nvm", "unalias"]);
+        assert_eq!(code, 127);
+        assert!(out.is_empty() && err.starts_with("Usage: nvm unalias <name>"));
     }
 
     #[test]
```

Create `src/commands/unalias.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use crate::ports::FileSystem;

    fn unalias(fs: &FakeFileSystem, names: &[&str]) -> Result<Output, CliError> {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let names: Vec<String> = names.iter().map(ToString::to_string).collect();
        run(&Context { fs, env: &env }, &names)
    }

    #[test]
    fn removes_the_alias_file_and_says_how_to_restore_it() {
        let fs = FakeFileSystem::default().with_file("/n/alias/work", "v18.9.0\n");
        let output = unalias(&fs, &["work"]).unwrap();
        assert_eq!(
            output,
            Output::stdout("Deleted alias work - restore it with `nvm alias \"work\" \"v18.9.0\"`")
        );
        assert!(!fs.is_file(Path::new("/n/alias/work")));
    }

    #[test]
    fn a_missing_alias_is_a_warning_on_stderr_and_still_succeeds() {
        let output = unalias(&FakeFileSystem::default(), &["nope"]).unwrap();
        assert_eq!(
            output,
            Output::default().with_stderr("Alias nope doesn't exist!")
        );
    }

    #[test]
    fn built_in_aliases_cannot_be_deleted() {
        for name in ["stable", "unstable", "iojs", "node", "system"] {
            let error = unalias(&FakeFileSystem::default(), &[name]).unwrap_err();
            let expected = format!("{name} is a default (built-in) alias and cannot be deleted.");
            assert_eq!(error.to_string(), expected);
            assert_eq!(error.exit_code(), NvmExitCode::Failure);
        }
    }

    #[test]
    fn aliases_in_subdirectories_are_rejected() {
        let fs = FakeFileSystem::default().with_file("/n/alias/lts/iron", "v20");
        let error = unalias(&fs, &["lts/iron"]).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Aliases in subdirectories are not supported."
        );
        assert!(fs.is_file(Path::new("/n/alias/lts/iron")));
    }

    #[test]
    fn exactly_one_name_is_required() {
        for names in [&[][..], &["a", "b"][..]] {
            let error = unalias(&FakeFileSystem::default(), names).unwrap_err();
            assert!(error.to_string().starts_with("Usage: nvm unalias <name>"));
            assert_eq!(error.exit_code(), NvmExitCode::NotFound);
        }
    }
}
```

Apply to the test module of `src/context.rs`:

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -66,6 +66,14 @@
     }
 
     #[test]
+    fn alias_dir_is_under_nvm_dir() {
+        let fs = FakeFileSystem::default();
+        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
+        let context = Context { fs: &fs, env: &env };
+        assert_eq!(context.alias_dir().unwrap(), PathBuf::from("/n/alias"));
+    }
+
+    #[test]
     fn alias_store_reads_the_alias_directory() {
         let fs = FakeFileSystem::default().with_file("/n/alias/default", "v20");
         let env = FakeEnv::default().with_var("NVM_DIR", "/n");
```

Apply to the test module of `src/error.rs`:

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -36,5 +36,9 @@
         assert_eq!(usage.exit_code(), NvmExitCode::NotFound);
         let no_system_node = CliError::SystemNodeNotFound;
         assert_eq!(no_system_node.exit_code(), NvmExitCode::NotFound);
+        let invalid = CliError::InvalidArgument("x".into());
+        assert_eq!(invalid.exit_code(), NvmExitCode::Failure);
+        let io_error = CliError::from(std::io::Error::from(std::io::ErrorKind::NotFound));
+        assert_eq!(io_error.exit_code(), NvmExitCode::Failure);
     }
 }
```

Apply to the test module of `src/fakes.rs`:

```diff
--- a/src/fakes.rs
+++ b/src/fakes.rs
@@ -48,6 +48,14 @@
     }
 
     #[test]
+    fn fake_file_system_removes_files() {
+        let fs = FakeFileSystem::default().with_file("/f", "hi");
+        fs.remove_file(Path::new("/f")).unwrap();
+        assert!(!fs.is_file(Path::new("/f")));
+        assert!(fs.remove_file(Path::new("/f")).is_err());
+    }
+
+    #[test]
     fn fake_env_returns_the_variables_it_was_given() {
         let env = FakeEnv::default().with_var("HOME", "/home/me");
         assert_eq!(env.var("HOME"), Some("/home/me".to_owned()));
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "no method named `remove_file`
found" and "cannot find module `unalias`".

- [ ] **Step 4: Write the implementation**

Apply to `src/adapters/std_fs.rs` (above the test module):

```diff
--- a/src/adapters/std_fs.rs
+++ b/src/adapters/std_fs.rs
@@ -26,4 +26,8 @@
     fn is_file(&self, path: &Path) -> bool {
         path.is_file()
     }
+
+    fn remove_file(&self, path: &Path) -> io::Result<()> {
+        fs::remove_file(path)
+    }
 }
```

Apply to `src/cli.rs` (above the test module):

```diff
--- a/src/cli.rs
+++ b/src/cli.rs
@@ -27,6 +27,8 @@
     Current,
     /// Print the path to the node binary of a version or alias.
     Which { version: Option<String> },
+    /// Delete an alias.
+    Unalias { names: Vec<String> },
 }
 
 fn dispatch(command: &Command, context: &Context<'_>) -> Result<Output, CliError> {
@@ -36,6 +38,7 @@
         }
         Command::Current => commands::current::run(context),
         Command::Which { version } => commands::which::run(context, version.as_deref()),
+        Command::Unalias { names } => commands::unalias::run(context, names),
     }
 }
 
```

Insert above the `#[cfg(test)]` line of `src/commands/unalias.rs`:

```rust
//! `nvm unalias <name>`: delete an alias file.

use crate::commands::Output;
use crate::context::Context;
use crate::domain::alias::{AliasStore, BUILTIN_ALIASES};
use crate::error::CliError;

const USAGE: &str = "Usage: nvm unalias <name>\n  Run `nvm --help` for full help.";

/// # Errors
/// - [`CliError::Usage`] unless exactly one name is given.
/// - [`CliError::InvalidArgument`] for a name with a `/`, or a built-in alias.
/// - [`CliError::Io`] when the alias file cannot be removed.
///
/// Like `nvm.sh`, an alias that does not exist is only a warning on stderr.
pub fn run(context: &Context<'_>, names: &[String]) -> Result<Output, CliError> {
    let [name] = names else {
        return Err(CliError::Usage(USAGE.to_owned()));
    };
    if name.contains('/') {
        let message = "Aliases in subdirectories are not supported.";
        return Err(CliError::InvalidArgument(message.to_owned()));
    }
    let path = context.alias_dir()?.join(name);
    if !context.fs.is_file(&path) {
        return missing(name);
    }
    let original = context.alias_store()?.target(name).unwrap_or_default();
    context.fs.remove_file(&path)?;
    Ok(Output::stdout(format!(
        "Deleted alias {name} - restore it with `nvm alias \"{name}\" \"{original}\"`"
    )))
}

fn missing(name: &str) -> Result<Output, CliError> {
    if BUILTIN_ALIASES.contains(&name) {
        let message = format!("{name} is a default (built-in) alias and cannot be deleted.");
        return Err(CliError::InvalidArgument(message));
    }
    Ok(Output::default().with_stderr(format!("Alias {name} doesn't exist!")))
}

```

Apply to `src/context.rs` (above the test module):

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -29,6 +29,14 @@
             }
         };
         Ok(dir.components().collect())
+    }
+
+    /// `$NVM_DIR/alias`, where alias files live.
+    ///
+    /// # Errors
+    /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
+    pub fn alias_dir(&self) -> Result<PathBuf, CliError> {
+        Ok(self.nvm_dir()?.join("alias"))
     }
 
     /// The alias files under `$NVM_DIR/alias`.
```

Apply to `src/domain/alias.rs` (above the test module):

```diff
--- a/src/domain/alias.rs
+++ b/src/domain/alias.rs
@@ -3,6 +3,9 @@
 use std::collections::HashSet;
 
 use crate::error::AliasError;
+
+/// Names that always exist and have no alias file.
+pub const BUILTIN_ALIASES: [&str; 5] = ["stable", "unstable", "iojs", "node", "system"];
 
 pub trait AliasStore {
     /// The target an alias points to, or `None` when `name` is not an alias.
```

Apply to `src/error.rs` (above the test module):

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -61,6 +61,11 @@
     VersionNotInstalled(String),
     #[error("System version of node not found.")]
     SystemNodeNotFound,
+    /// A rejected argument, with the message to print.
+    #[error("{0}")]
+    InvalidArgument(String),
+    #[error(transparent)]
+    Io(#[from] std::io::Error),
 }
 
 impl CliError {
@@ -68,7 +73,10 @@
     pub fn exit_code(&self) -> NvmExitCode {
         match self {
             Self::Version(_) | Self::NotInstalled => NvmExitCode::InvalidVersion,
-            Self::NvmDirUnresolved | Self::VersionNotInstalled(_) => NvmExitCode::Failure,
+            Self::NvmDirUnresolved
+            | Self::VersionNotInstalled(_)
+            | Self::InvalidArgument(_)
+            | Self::Io(_) => NvmExitCode::Failure,
             Self::Usage(_) | Self::SystemNodeNotFound => NvmExitCode::NotFound,
             Self::Floor(_) => NvmExitCode::BelowVersionFloor,
             Self::Alias(_) => NvmExitCode::AliasLoop,
```

Apply to `src/fakes.rs` (above the test module):

```diff
--- a/src/fakes.rs
+++ b/src/fakes.rs
@@ -1,5 +1,6 @@
 //! In-memory implementations of the ports, for unit tests only.
 
+use std::cell::RefCell;
 use std::collections::{BTreeMap, BTreeSet};
 use std::ffi::OsString;
 use std::io;
@@ -9,14 +10,16 @@
 
 #[derive(Default)]
 pub struct FakeFileSystem {
-    files: BTreeMap<PathBuf, String>,
+    files: RefCell<BTreeMap<PathBuf, String>>,
     dirs: BTreeSet<PathBuf>,
 }
 
 impl FakeFileSystem {
     #[must_use]
     pub fn with_file(mut self, path: &str, contents: &str) -> Self {
-        self.files.insert(PathBuf::from(path), contents.to_owned());
+        self.files
+            .get_mut()
+            .insert(PathBuf::from(path), contents.to_owned());
         self
     }
 
@@ -31,6 +34,7 @@
 impl FileSystem for FakeFileSystem {
     fn read_to_string(&self, path: &Path) -> io::Result<String> {
         self.files
+            .borrow()
             .get(path)
             .cloned()
             .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
@@ -39,9 +43,10 @@
     fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
         let mut children: BTreeMap<String, bool> = BTreeMap::new();
         let mut exists = self.dirs.contains(path);
-        let files = self.files.keys().map(|file| (file, false));
-        let dirs = self.dirs.iter().map(|dir| (dir, true));
-        for (candidate, is_dir_path) in files.chain(dirs) {
+        let files = self.files.borrow();
+        let file_paths = files.keys().map(|file| (file, false));
+        let dir_paths = self.dirs.iter().map(|dir| (dir, true));
+        for (candidate, is_dir_path) in file_paths.chain(dir_paths) {
             let Ok(rest) = candidate.strip_prefix(path) else {
                 continue;
             };
@@ -63,7 +68,15 @@
     }
 
     fn is_file(&self, path: &Path) -> bool {
-        self.files.contains_key(path)
+        self.files.borrow().contains_key(path)
+    }
+
+    fn remove_file(&self, path: &Path) -> io::Result<()> {
+        self.files
+            .borrow_mut()
+            .remove(path)
+            .map(|_| ())
+            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
     }
 }
 
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -24,6 +24,10 @@
     fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>>;
 
     fn is_file(&self, path: &Path) -> bool;
+
+    /// # Errors
+    /// Propagates the underlying I/O error (for example when `path` is missing).
+    fn remove_file(&self, path: &Path) -> io::Result<()>;
 }
 
 pub trait Env {
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 110 unit tests plus the end-to-end test.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean and zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(commands): add unalias"
```

---

### Task 6: Built-in `node` and `iojs` aliases in resolution

**Files:**

- Modify: `src/commands/resolve.rs`, `src/commands/which.rs`

**Interfaces:**

- Consumes: `commands::resolve::resolve_installed`.
- Produces: `node` resolves to the highest installed Node version and `iojs` to
  the highest installed io.js version, in `version`, `which` and (next task)
  `alias`; they also work as the target of another alias. `stable`,
  `unstable` and `lts/*` stay unresolved until Plan 3.
- [ ] **Step 1: Write the failing tests**

Apply to the test module of `src/commands/resolve.rs`:

```diff
--- a/src/commands/resolve.rs
+++ b/src/commands/resolve.rs
@@ -24,6 +24,41 @@
     fn a_pattern_resolves_to_the_highest_installed_match() {
         let resolved = resolve_with(&installed(), "20").unwrap();
         assert_eq!(resolved, Resolved::Installed(version("v20.10.0")));
+    }
+
+    #[test]
+    fn node_is_the_latest_installed_node_ignoring_iojs() {
+        let fs = installed().with_file("/n/versions/io.js/v3.0.0/bin/node", "");
+        let resolved = resolve_with(&fs, "node").unwrap();
+        assert_eq!(resolved, Resolved::Installed(version("v20.10.0")));
+    }
+
+    #[test]
+    fn iojs_is_the_latest_installed_iojs() {
+        let fs = installed()
+            .with_file("/n/versions/io.js/v3.0.0/bin/node", "")
+            .with_file("/n/versions/io.js/v2.5.0/bin/node", "");
+        let resolved = resolve_with(&fs, "iojs").unwrap();
+        assert_eq!(resolved, Resolved::Installed(version("iojs-v3.0.0")));
+    }
+
+    #[test]
+    fn a_built_in_alias_can_be_the_target_of_another_alias() {
+        let fs = installed().with_file("/n/alias/default", "node");
+        let resolved = resolve_with(&fs, "default").unwrap();
+        assert_eq!(resolved, Resolved::Installed(version("v20.10.0")));
+    }
+
+    #[test]
+    fn a_built_in_alias_without_a_matching_install_is_missing() {
+        let fs = FakeFileSystem::default().with_dir("/n/versions/node");
+        let resolved = resolve_with(&fs, "node").unwrap();
+        assert_eq!(
+            resolved,
+            Resolved::Missing {
+                resolved: "node".into()
+            }
+        );
     }
 
     #[test]
```

Apply to the test module of `src/commands/which.rs`:

```diff
--- a/src/commands/which.rs
+++ b/src/commands/which.rs
@@ -25,6 +25,12 @@
     #[test]
     fn prints_the_node_binary_of_the_highest_match() {
         let output = which("20").unwrap();
+        assert_eq!(output, Output::stdout("/n/versions/node/v20.1.0/bin/node"));
+    }
+
+    #[test]
+    fn node_is_the_latest_installed_node() {
+        let output = which("node").unwrap();
         assert_eq!(output, Output::stdout("/n/versions/node/v20.1.0/bin/node"));
     }
 
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: The code compiles, but 4 tests FAIL with assertion errors: the
`node`, `iojs` and built-in-target tests in `commands::resolve` and the `node`
test in `commands::which`.

- [ ] **Step 3: Write the implementation**

Apply to `src/commands/resolve.rs` (above the test module):

```diff
--- a/src/commands/resolve.rs
+++ b/src/commands/resolve.rs
@@ -3,7 +3,7 @@
 
 use crate::context::Context;
 use crate::domain::alias;
-use crate::domain::version::{Version, VersionPattern};
+use crate::domain::version::{Flavor, Version, VersionPattern};
 use crate::error::CliError;
 
 #[derive(Debug, PartialEq, Eq)]
@@ -22,10 +22,28 @@
 pub fn resolve_installed(context: &Context<'_>, name: &str) -> Result<Resolved, CliError> {
     let resolved = alias::resolve(&context.alias_store()?, name)?;
     let installed = context.installed_versions()?;
-    let found = resolved
-        .parse::<VersionPattern>()
-        .ok()
-        .and_then(|pattern| pattern.highest_match(&installed).copied());
+    let found = find_installed(&resolved, &installed);
     Ok(found.map_or(Resolved::Missing { resolved }, Resolved::Installed))
 }
 
+/// `node` and `iojs` are built-in aliases for the latest installed version of
+/// that flavor; anything else is matched as a version pattern.
+fn find_installed(resolved: &str, installed: &[Version]) -> Option<Version> {
+    match resolved {
+        "node" => latest_of(installed, Flavor::Node),
+        "iojs" => latest_of(installed, Flavor::IoJs),
+        other => other
+            .parse::<VersionPattern>()
+            .ok()
+            .and_then(|pattern| pattern.highest_match(installed).copied()),
+    }
+}
+
+fn latest_of(installed: &[Version], flavor: Flavor) -> Option<Version> {
+    installed
+        .iter()
+        .filter(|version| version.flavor == flavor)
+        .max()
+        .copied()
+}
+
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 115 unit tests plus the end-to-end test.

- [ ] **Step 5: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean and zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(commands): resolve the built-in node and iojs aliases"
```

---

### Task 7: `nvm alias <name> <target>`

**Files:**

- Create: `src/cli/tests.rs`, `src/commands/alias.rs`
- Modify: `src/cli.rs` (becomes `src/cli/mod.rs`), `src/ports/mod.rs`,
  `src/fakes.rs`, `src/adapters/std_fs.rs`, `src/commands/mod.rs`

**Interfaces:**

- Consumes: `commands::resolve::resolve_installed`, `commands::unalias::run`,
  `Context::alias_dir`.
- Produces: `FileSystem::write_file(&self, &Path, &str)` and
  `FileSystem::create_dir_all(&self, &Path)`; `commands::alias::run(&Context,
  &str, &str) -> Result<Output, CliError>`.
- Behaviour (from `nvm.sh`, plain output as when stdout is not a terminal):
  writes `<target>\n` to `$NVM_DIR/alias/<name>` and prints
  `name -> target *` when target is the resolved version, else
  `name -> target (-> vX.Y.Z *)`; a target that is not installed prints
  `name -> target (-> N/A)` plus `! WARNING: Version 'target' does not exist.`
  on stderr but still creates the alias; `system` is accepted as is; a target
  whose chain loops prints `(-> ∞)`; an empty target deletes the alias.
  Names with `#` or `/`, and `.`, `..` and the empty name, are rejected with
  exit 1 and nothing is written. Listing (`nvm alias` without arguments) comes
  with `nvm ls` in Plan 3, so both arguments are required for now.
- Housekeeping: `src/cli.rs` would pass 300 lines, so its tests move to
  `src/cli/tests.rs` in a mechanical first step.
- [ ] **Step 0: Split the CLI tests into their own file (no behaviour change)**

Run:

```bash
mkdir src/cli && git mv src/cli.rs src/cli/mod.rs
```

Cut everything inside the `mod tests { ... }` block of `src/cli/mod.rs`,
remove one level of indentation from it, and paste it into the new file
`src/cli/tests.rs` (it starts with `use super::*;`). Replace the block in
`src/cli/mod.rs` with:

```rust
#[cfg(test)]
mod tests;
```

Run: `cargo fmt && cargo test`
Expected: PASS, 115 unit tests, same as the end of Task 6.

- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -1,3 +1,4 @@
+pub mod alias;
 pub mod current;
 pub mod resolve;
 pub mod unalias;
```

- [ ] **Step 2: Write the failing tests**

Apply to `src/cli/tests.rs`:

```diff
--- a/src/cli/tests.rs
+++ b/src/cli/tests.rs
@@ -76,6 +76,21 @@
     let printed = String::from_utf8(out).unwrap();
     assert!(printed.starts_with("Deleted alias work - restore it"));
     assert!(!fs.is_file(std::path::Path::new("/n/alias/work")));
+}
+
+#[test]
+fn alias_creates_the_file_and_prints_the_formatted_line() {
+    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
+    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
+    let (mut out, mut err) = (Vec::new(), Vec::new());
+    let args = ["nvm", "alias", "work", "20"];
+    let code = run(args, &Context { fs: &fs, env: &env }, &mut out, &mut err);
+    assert_eq!(
+        (code, out, err),
+        (0, b"work -> 20 (-> v20.1.0 *)\n".to_vec(), Vec::new())
+    );
+    let stored = fs.read_to_string(std::path::Path::new("/n/alias/work"));
+    assert_eq!(stored.unwrap(), "20\n");
 }
 
 #[test]
```

Create `src/commands/alias.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use crate::ports::FileSystem;

    fn installed() -> FakeFileSystem {
        FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "")
    }

    fn alias(fs: &FakeFileSystem, name: &str, target: &str) -> Result<Output, CliError> {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        run(&Context { fs, env: &env }, name, target)
    }

    fn stored(fs: &FakeFileSystem, name: &str) -> Option<String> {
        fs.read_to_string(&Path::new("/n/alias").join(name)).ok()
    }

    #[test]
    fn writes_the_target_as_the_alias_file() {
        let fs = installed();
        alias(&fs, "test", "v20.1.0").unwrap();
        assert_eq!(stored(&fs, "test"), Some("v20.1.0\n".to_owned()));
    }

    #[test]
    fn an_exact_installed_target_prints_one_arrow() {
        let output = alias(&installed(), "test", "v20.1.0").unwrap();
        assert_eq!(output, Output::stdout("test -> v20.1.0 *"));
    }

    #[test]
    fn a_pattern_target_shows_what_it_resolves_to() {
        let output = alias(&installed(), "work", "20").unwrap();
        assert_eq!(output, Output::stdout("work -> 20 (-> v20.1.0 *)"));
    }

    #[test]
    fn a_target_that_is_not_installed_warns_but_still_creates_the_alias() {
        let fs = FakeFileSystem::default();
        let output = alias(&fs, "test", "v0.1.2").unwrap();
        assert_eq!(output.stdout, "test -> v0.1.2 (-> N/A)");
        assert_eq!(output.stderr, "! WARNING: Version 'v0.1.2' does not exist.");
        assert_eq!(stored(&fs, "test"), Some("v0.1.2\n".to_owned()));
    }

    #[test]
    fn system_is_a_valid_target_without_a_warning() {
        let output = alias(&FakeFileSystem::default(), "default", "system").unwrap();
        assert_eq!(output, Output::stdout("default -> system *"));
    }

    #[test]
    fn a_target_whose_chain_loops_shows_infinity_without_a_warning() {
        let fs = installed()
            .with_file("/n/alias/x", "y")
            .with_file("/n/alias/y", "x");
        let output = alias(&fs, "z", "x").unwrap();
        assert_eq!(output, Output::stdout("z -> x (-> ∞)"));
    }

    #[test]
    fn an_existing_alias_is_overwritten() {
        let fs = installed().with_file("/n/alias/work", "v18\n");
        alias(&fs, "work", "20").unwrap();
        assert_eq!(stored(&fs, "work"), Some("20\n".to_owned()));
    }

    #[test]
    fn an_empty_target_deletes_the_alias() {
        let fs = installed().with_file("/n/alias/work", "v18\n");
        let output = alias(&fs, "work", "").unwrap();
        assert!(output.stdout.starts_with("Deleted alias work"));
        assert_eq!(stored(&fs, "work"), None);
    }

    #[test]
    fn unsupported_names_are_rejected_and_nothing_is_written() {
        let cases = [
            (
                "a#b",
                "Aliases with a comment delimiter (#) are not supported.",
            ),
            ("lts/iron", "Aliases in subdirectories are not supported."),
            ("..", "invalid alias name: .."),
            (".", "invalid alias name: ."),
            ("", "invalid alias name: "),
        ];
        for (name, message) in cases {
            let fs = installed();
            let error = alias(&fs, name, "v20.1.0").unwrap_err();
            assert_eq!(error.to_string(), message);
            assert_eq!(fs.read_dir(Path::new("/n/alias")).ok(), None);
        }
    }
}
```

Apply to the test module of `src/fakes.rs`:

```diff
--- a/src/fakes.rs
+++ b/src/fakes.rs
@@ -56,6 +56,22 @@
     }
 
     #[test]
+    fn fake_file_system_writes_and_overwrites_files() {
+        let fs = FakeFileSystem::default();
+        fs.write_file(Path::new("/f"), "one").unwrap();
+        fs.write_file(Path::new("/f"), "two").unwrap();
+        assert_eq!(fs.read_to_string(Path::new("/f")).unwrap(), "two");
+    }
+
+    #[test]
+    fn fake_file_system_creates_directories() {
+        let fs = FakeFileSystem::default();
+        fs.create_dir_all(Path::new("/d/e")).unwrap();
+        fs.create_dir_all(Path::new("/d/e")).unwrap();
+        assert_eq!(fs.read_dir(Path::new("/d/e")).unwrap(), []);
+    }
+
+    #[test]
     fn fake_env_returns_the_variables_it_was_given() {
         let env = FakeEnv::default().with_var("HOME", "/home/me");
         assert_eq!(env.var("HOME"), Some("/home/me".to_owned()));
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find module `alias`",
"no method named `write_file` found" and "no method named `create_dir_all`
found".

- [ ] **Step 4: Write the implementation**

Apply to `src/adapters/std_fs.rs` (above the test module):

```diff
--- a/src/adapters/std_fs.rs
+++ b/src/adapters/std_fs.rs
@@ -30,4 +30,12 @@
     fn remove_file(&self, path: &Path) -> io::Result<()> {
         fs::remove_file(path)
     }
+
+    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()> {
+        fs::write(path, contents)
+    }
+
+    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
+        fs::create_dir_all(path)
+    }
 }
```

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -27,6 +27,8 @@
     Current,
     /// Print the path to the node binary of a version or alias.
     Which { version: Option<String> },
+    /// Create an alias for a version (an empty target deletes the alias).
+    Alias { name: String, target: String },
     /// Delete an alias.
     Unalias { names: Vec<String> },
 }
@@ -38,6 +40,7 @@
         }
         Command::Current => commands::current::run(context),
         Command::Which { version } => commands::which::run(context, version.as_deref()),
+        Command::Alias { name, target } => commands::alias::run(context, name, target),
         Command::Unalias { names } => commands::unalias::run(context, names),
     }
 }
```

Insert above the `#[cfg(test)]` line of `src/commands/alias.rs`:

```rust
//! `nvm alias <name> <target>`: create an alias file.
//!
//! Listing aliases (`nvm alias` without arguments) arrives with `nvm ls`, which
//! shares its formatting. Output is always plain, as `nvm.sh` prints when
//! stdout is not a terminal.

use crate::commands::resolve::{Resolved, resolve_installed};
use crate::commands::{Output, unalias};
use crate::context::Context;
use crate::error::CliError;

/// # Errors
/// - [`CliError::InvalidArgument`] for a name that is empty, `.`, `..`, or
///   contains `#` or `/`.
/// - [`CliError::Io`] when the alias file cannot be written.
///
/// A target that is not installed is only a warning on stderr.
pub fn run(context: &Context<'_>, name: &str, target: &str) -> Result<Output, CliError> {
    if target.is_empty() {
        return unalias::run(context, &[name.to_owned()]);
    }
    validate_name(name)?;
    let version = version_text(context, target)?;
    let alias_dir = context.alias_dir()?;
    context.fs.create_dir_all(&alias_dir)?;
    context
        .fs
        .write_file(&alias_dir.join(name), &format!("{target}\n"))?;
    let output = Output::stdout(format_line(name, target, &version));
    if version == "N/A" {
        return Ok(output.with_stderr(format!("! WARNING: Version '{target}' does not exist.")));
    }
    Ok(output)
}

fn validate_name(name: &str) -> Result<(), CliError> {
    let message = if name.contains('#') {
        "Aliases with a comment delimiter (#) are not supported.".to_owned()
    } else if name.contains('/') {
        "Aliases in subdirectories are not supported.".to_owned()
    } else if matches!(name, "" | "." | "..") {
        format!("invalid alias name: {name}")
    } else {
        return Ok(());
    };
    Err(CliError::InvalidArgument(message))
}

/// What `target` resolves to now: a version, `system`, `N/A` when nothing
/// installed matches, or `∞` when its alias chain loops.
fn version_text(context: &Context<'_>, target: &str) -> Result<String, CliError> {
    if target == "system" {
        return Ok("system".to_owned());
    }
    match resolve_installed(context, target) {
        Ok(Resolved::Installed(version)) => Ok(version.to_string()),
        Ok(Resolved::Missing { .. }) => Ok("N/A".to_owned()),
        Err(CliError::Alias(_)) => Ok("∞".to_owned()),
        Err(error) => Err(error),
    }
}

fn format_line(alias: &str, target: &str, version: &str) -> String {
    let marker = if matches!(version, "N/A" | "∞") {
        ""
    } else {
        " *"
    };
    if target == version {
        format!("{alias} -> {version}{marker}")
    } else {
        format!("{alias} -> {target} (-> {version}{marker})")
    }
}

```

Apply to `src/fakes.rs` (above the test module):

```diff
--- a/src/fakes.rs
+++ b/src/fakes.rs
@@ -11,7 +11,7 @@
 #[derive(Default)]
 pub struct FakeFileSystem {
     files: RefCell<BTreeMap<PathBuf, String>>,
-    dirs: BTreeSet<PathBuf>,
+    dirs: RefCell<BTreeSet<PathBuf>>,
 }
 
 impl FakeFileSystem {
@@ -26,7 +26,7 @@
     /// An explicit, possibly empty, directory. Parents of files exist already.
     #[must_use]
     pub fn with_dir(mut self, path: &str) -> Self {
-        self.dirs.insert(PathBuf::from(path));
+        self.dirs.get_mut().insert(PathBuf::from(path));
         self
     }
 }
@@ -42,10 +42,11 @@
 
     fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
         let mut children: BTreeMap<String, bool> = BTreeMap::new();
-        let mut exists = self.dirs.contains(path);
+        let dirs = self.dirs.borrow();
+        let mut exists = dirs.contains(path);
         let files = self.files.borrow();
         let file_paths = files.keys().map(|file| (file, false));
-        let dir_paths = self.dirs.iter().map(|dir| (dir, true));
+        let dir_paths = dirs.iter().map(|dir| (dir, true));
         for (candidate, is_dir_path) in file_paths.chain(dir_paths) {
             let Ok(rest) = candidate.strip_prefix(path) else {
                 continue;
@@ -77,6 +78,18 @@
             .remove(path)
             .map(|_| ())
             .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
+    }
+
+    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()> {
+        self.files
+            .borrow_mut()
+            .insert(path.to_path_buf(), contents.to_owned());
+        Ok(())
+    }
+
+    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
+        self.dirs.borrow_mut().insert(path.to_path_buf());
+        Ok(())
     }
 }
 
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -28,6 +28,18 @@
     /// # Errors
     /// Propagates the underlying I/O error (for example when `path` is missing).
     fn remove_file(&self, path: &Path) -> io::Result<()>;
+
+    /// Creates or replaces the file at `path`.
+    ///
+    /// # Errors
+    /// Propagates the underlying I/O error.
+    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()>;
+
+    /// Creates `path` and any missing parents; fine if it already exists.
+    ///
+    /// # Errors
+    /// Propagates the underlying I/O error.
+    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
 }
 
 pub trait Env {
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 127 unit tests plus the end-to-end test.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean and zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(commands): add alias to create aliases"
```

---

### Task 8: End-to-end tests and the quality gate

**Files:**

- Create: `tests/commands_cli.rs`

**Interfaces:**

- Consumes: the `nvm` and `nvmrc` binaries (`CARGO_BIN_EXE_nvm`,
  `CARGO_BIN_EXE_nvmrc`).
- Produces: acceptance tests for `alias`, `which`, `unalias`, `current`,
  exit codes 1, 8 and 127, both binaries, and (Linux only) a non-UTF-8
  `NVM_DIR`.
- [ ] **Step 1: Write the end-to-end tests**

Create `tests/commands_cli.rs`:

```rust
//! End-to-end: the real binaries against a real temporary `$NVM_DIR`.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn run(binary: &str, nvm_dir: &Path, path: &OsStr, args: &[&str]) -> Output {
    Command::new(binary)
        .args(args)
        .env("NVM_DIR", nvm_dir)
        .env("PATH", path)
        .output()
        .expect("run the binary")
}

fn nvm(nvm_dir: &Path, args: &[&str]) -> Output {
    run(
        env!("CARGO_BIN_EXE_nvm"),
        nvm_dir,
        OsStr::new("/nonexistent"),
        args,
    )
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn install(nvm_dir: &Path, version_directory: &str) {
    fs::create_dir_all(nvm_dir.join(version_directory).join("bin")).unwrap();
    fs::write(nvm_dir.join(version_directory).join("bin/node"), "").unwrap();
}

#[test]
fn alias_which_and_unalias_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), "versions/node/v20.1.0");

    let created = nvm(dir.path(), &["alias", "work", "20"]);
    assert!(created.status.success());
    assert_eq!(stdout(&created), "work -> 20 (-> v20.1.0 *)\n");
    assert_eq!(
        fs::read_to_string(dir.path().join("alias/work")).unwrap(),
        "20\n"
    );

    let which = nvm(dir.path(), &["which", "work"]);
    let expected = dir.path().join("versions/node/v20.1.0/bin/node");
    assert_eq!(stdout(&which), format!("{}\n", expected.display()));

    let removed = nvm(dir.path(), &["unalias", "work"]);
    assert!(stdout(&removed).starts_with("Deleted alias work - restore it with"));
    assert!(!dir.path().join("alias/work").exists());

    let again = nvm(dir.path(), &["unalias", "work"]);
    assert!(again.status.success());
    assert_eq!(stderr(&again), "Alias work doesn't exist!\n");
}

#[test]
fn alias_to_a_missing_version_warns_but_creates_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let created = nvm(dir.path(), &["alias", "test", "v0.1.2"]);
    assert!(created.status.success());
    assert_eq!(stdout(&created), "test -> v0.1.2 (-> N/A)\n");
    assert_eq!(
        stderr(&created),
        "! WARNING: Version 'v0.1.2' does not exist.\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("alias/test")).unwrap(),
        "v0.1.2\n"
    );
}

#[test]
fn which_of_a_missing_version_exits_1_and_an_alias_loop_exits_8() {
    let dir = tempfile::tempdir().unwrap();
    let missing = nvm(dir.path(), &["which", "16"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(stderr(&missing).starts_with("N/A: version \"v16\" is not yet installed."));

    fs::create_dir_all(dir.path().join("alias")).unwrap();
    fs::write(dir.path().join("alias/loop"), "loop\n").unwrap();
    let looped = nvm(dir.path(), &["which", "loop"]);
    assert_eq!(looped.status.code(), Some(8));
    assert_eq!(
        stderr(&looped),
        "The alias \"loop\" leads to an infinite loop. Aborting.\n"
    );
}

#[test]
fn usage_errors_exit_127() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(nvm(dir.path(), &["which"]).status.code(), Some(127));
    assert_eq!(nvm(dir.path(), &["unalias"]).status.code(), Some(127));
}

#[test]
fn current_reports_none_system_or_an_nvm_version() {
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), "versions/node/v20.1.0");
    let system = tempfile::tempdir().unwrap();
    fs::write(system.path().join("node"), "").unwrap();
    let bin = env!("CARGO_BIN_EXE_nvm");
    let nvm_bin = dir.path().join("versions/node/v20.1.0/bin");

    let none = run(bin, dir.path(), OsStr::new("/nonexistent"), &["current"]);
    assert_eq!(stdout(&none), "none\n");
    let system_path = std::env::join_paths([system.path()]).unwrap();
    let system_run = run(bin, dir.path(), &system_path, &["current"]);
    assert_eq!(stdout(&system_run), "system\n");
    let managed_path = std::env::join_paths([nvm_bin.as_path(), system.path()]).unwrap();
    let managed = run(bin, dir.path(), &managed_path, &["current"]);
    assert_eq!(stdout(&managed), "v20.1.0\n");
}

#[test]
fn both_binaries_behave_the_same() {
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), "versions/node/v20.1.0");
    let path = OsStr::new("/nonexistent");
    let from_nvm = run(
        env!("CARGO_BIN_EXE_nvm"),
        dir.path(),
        path,
        &["version", "20"],
    );
    let from_nvmrc = run(
        env!("CARGO_BIN_EXE_nvmrc"),
        dir.path(),
        path,
        &["version", "20"],
    );
    assert_eq!(stdout(&from_nvm), "v20.1.0\n");
    assert_eq!(from_nvm.stdout, from_nvmrc.stdout);
}

/// Linux only: macOS file systems reject names that are not valid UTF-8.
#[cfg(target_os = "linux")]
#[test]
fn a_non_utf8_nvm_dir_is_honoured() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let parent = tempfile::tempdir().unwrap();
    let nvm_dir = parent.path().join(OsString::from_vec(b"nvm-\xff".to_vec()));
    install(&nvm_dir, "versions/node/v20.1.0");
    let output = nvm(&nvm_dir, &["version", "20"]);
    assert_eq!(stdout(&output), "v20.1.0\n");
}
```

- [ ] **Step 2: Run them**

Run: `cargo test --test commands_cli`
Expected: PASS, 6 tests. They pass at once because the behaviour already
exists; they are the acceptance net for Tasks 3 to 7. If one fails, fix the
code, not the test, unless the test contradicts `nvm.sh`.

- [ ] **Step 3: Run the full quality gate**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
rustup run 1.88 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings, 127 unit tests plus 6 + 1
end-to-end tests passing, no advisories, and the crate still builds on MSRV
1.88.

Also check that no file is over 300 lines:

```bash
find src tests -name '*.rs' | xargs wc -l | sort -n | tail -3
```

- [ ] **Step 4: Commit**

```bash
git add tests
git commit -S -m "test(cli): add end-to-end tests for current, which, alias and unalias"
```

---

## Self-review against the spec

- **Spec coverage:** section 3 layering is kept (`commands/`, `domain/`,
  `ports/`, `adapters/`, `cli`); section 4.1 layout (versions, io.js, alias
  files); section 4.3 resolution is now one shared module; section 6 exit codes
  gain 127; section 11 step 3 delivers `current`, `which`, `alias`, `unalias`
  and `version`. `ls` and `alias` listing move to Plan 3 because they share the
  colored formatting and the remaining built-in aliases.
- **Deliberate deviations from `nvm.sh`, each to be pinned by the Plan 8
  compatibility contract:**
  - `current` reads the version from the `node` path instead of running
    `node --version`, so the `$NVM_DIR/current` symlink reports `none`.
  - `which` without an argument does not read `.nvmrc` yet (Plan 6), and
    `which --silent` is not accepted.
  - Output is always plain: `nvm.sh` colors it only when stdout is a terminal.
  - `which` and `alias` print paths through `to_string_lossy`, so a non-UTF-8
    `NVM_DIR` is honoured for lookups but shown with replacement characters.
  - `unalias` does not create `$NVM_DIR/alias` as a side effect.
  - The legacy layout (`$NVM_DIR/vX.Y.Z`, versions below 0.12) is not read.
  - `alias` with a bad name exits 1; `nvm.sh` prints the message and still
    exits 0 for `..`.
- **Type consistency:** names match across tasks (`Output`, `Resolved`,
  `resolve_installed`, `Context::{nvm_dir, alias_dir, alias_store,
  installed_versions}`, `CliError` variants, `FileSystem` methods).
