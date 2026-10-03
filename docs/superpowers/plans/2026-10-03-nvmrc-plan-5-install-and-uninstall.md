# nvmrc Plan 5: Install, Uninstall, npm and Source Builds Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `nvm install` and `nvm uninstall` with all their options: the
version is resolved on the mirror, its `.tar.gz` is downloaded into
`$NVM_DIR/.cache/bin`, checked against `SHASUMS256.txt`, unpacked and moved
into `$NVM_DIR/versions` in one rename, under a per-version lock; when the
binary cannot be had it is built from source; the `default`, `--default` and
`--alias` aliases are made; `npm` is run where `nvm.sh` runs it (`--latest-npm`,
the `default-packages` file, `--reinstall-packages-from`, and the commands
`install-latest-npm` and `reinstall-packages`) whenever the version has an
`npm`; and `--offline`, `--save`, `-s`, `-b`, `-j` and the environment
variables that `nvm.sh` reads for all of it are honoured.

**Architecture:** Same layering as Plans 1 to 4. `ports/` gains `Digest`,
`Archive` and `Cpu`, runs programs to the end (`Process::execute`), and grows
`FileSystem` (`file_info`, `rename`, `create_dir`, `write_bytes`) and `Http`
(`get_bytes`); `adapters/` gains `Sha256Digest`, `TarGzArchive` and `StdCpu`
(the only code that hashes, unpacks or counts processors); `domain/` gains the
platform and download-name rules, the checksum rules, the `npm` rules (which
`npm` a `node` can use, `default-packages`, `npm list -g`) and the source-build
rules (jobs, compiler, `make`) as pure functions;
`commands/install/` is split by concern (`options`, `resolve`, `fetch`, `place`,
`lock`, `acquire` with its `source` build and `hook`, `npm_steps`, `offline`,
`defaults`) behind one `run`; `commands/npm` runs the `npm` of a version;
`commands/uninstall`, `install_latest_npm` and `reinstall_packages` reuse the
resolution of Plan 3. `Context` carries the new ports and the host `Platform`.

**Tech Stack:** Rust 2024 edition (MSRV 1.88), `clap`, `thiserror`, `ureq` 3,
and three new crates chosen here as the spec asks (section 10): `sha2` (SHA-256),
`flate2` (gzip, pure Rust backend) and `tar` (without its default features, so
no extended-attribute code). `tempfile` (dev only).

**Spec:** `docs/superpowers/specs/2026-10-02-nvmrc-design.md` (sections 3, 4.1,
4.4, 6, 10 and 11). This plan starts from the final state of
`2026-10-02-nvmrc-plan-4-network.md` (the `main` branch after Plan 4 and its
follow-ups, including the system certificate store for https).

**Ground truth:** every expected output in this plan was captured by running
the real `nvm.sh` (the reference repository) against a fixture mirror served
on local ports, which holds real archives and a real `SHASUMS256.txt`. The
success, "already installed", `--lts`, `--default`, `--alias`, usage, not-found
and uninstall cases of the unit and end-to-end tests are byte-identical to
what `nvm.sh` printed apart from the deviations listed at the end. Where
`nvm.sh` falls back to a source build, the tests of Tasks 8 to 12 expect what
it prints with `-b` (binary only); Task 20 adds the fallback, and from then on
those tests pass `-b`.

**Revised roadmap:** Plan 6 shell (`use`, `deactivate`, `exec`, `run`,
`init`, `nvm-exec`, `.nvmrc` semantics, the "Now using" line, `NVM_BIN`,
`NVM_INC`, `NVM_SYMLINK_CURRENT`); Plan 7 colors and terminal detection
(`NVM_COLORS`, `NVM_NO_COLORS`); Plan 8 `doctor` and `migrate`; Plan 9
compatibility contract and CI. Later still: `.tar.xz` archives.

**Not in this plan, on purpose:**

- `.tar.xz` archives (`nvm.sh` prefers them where it can; this port uses the
  `.tar.gz` that every release also has).
- Activating the version (`nvm use`, the `Now using node ...` line): the shell
  does that (Plan 6). Where `nvm.sh` runs `npm` after activating, this port runs
  the `npm` of the version itself, with its `bin` directory first on `PATH`.
- Downloading and running an `npm` installer for a version that has no `npm`
  (`nvm.sh` fetches `https://npmjs.org/install.sh` and pipes it to `sh`): the
  `npm` steps are skipped with a warning instead.
- A `.nvmrc` lookup for `nvm install` with no version (Plan 6).

## Global Constraints

- Rust edition 2024, `rust-version = "1.88.0"`; no APIs newer than 1.88. The
  development toolchain is pinned separately by `rust-toolchain.toml`.
- Run cargo through the rustup proxies (Homebrew's `rust` formula ships a
  `cargo` that ignores `rust-toolchain.toml`).
- `.cargo/config.toml` sets `-D warnings` for every workspace build, test and
  check, so a dead-code or unused-import warning in any intermediate task is a
  build error. `Cargo.toml` must not declare any `[profile.*]` table; the only
  changes to it in this plan are the dependencies of Tasks 3 and 4.
- No async I/O, no workspace, no plugin system, and no dependency besides
  `ureq`, `sha2`, `flate2` and `tar`.
- Files under 300 lines (tests included), functions under 30 lines, cyclomatic
  complexity under 10, meaningful names without abbreviations.
- Errors use `thiserror` in the library; only the binaries touch
  `std::process::ExitCode`. Exit codes: 0 success, 1 generic failure, 2 the
  archive could not be had (download, checksum or unpack failed) or no such
  `lts/*` alias, 3 invalid or unknown version (and a listing that finds
  nothing), 4 `--reinstall-packages-from` the version being installed, 5
  `--reinstall-packages-from` a version that is not installed, 6 options that
  cannot be combined, 7 below the version floor, 8 alias loop, 33 an install
  hook that installed nothing, 55 unsupported option, 127 usage error or
  missing system node. An install hook that fails passes its own status on.
- Commands return an `Output { stdout, stderr, status }` or a `CliError`; the
  CLI prints stderr first, then stdout, adds a newline to each non-empty
  stream, and finishes with `Output::status`.
- `Context::new(fs, env)` builds a context that can neither run programs, nor
  reach the network, nor hash, nor unpack, nor wait, and that is Linux on x64;
  `.with_process`, `.with_http`, `.with_digest`, `.with_archive`,
  `.with_sleeper`, `.with_cpu` and `.with_platform` change that. Nothing else
  constructs a `Context` by struct literal.
- Nothing is ever written outside `$NVM_DIR`. An archive entry that would land
  outside the directory it is unpacked into (`..`, an absolute path, a path
  through a symlink that leaves it) fails the whole unpack.
- A version directory is replaced with one rename of a fully unpacked tree, so
  a partial install is never seen as installed.
- Programs `nvm` starts (`npm`, `./configure`, `make`, an install hook) run
  without a shell, to the end, with stdin closed and both output streams
  captured; what they print is shown when the command ends. `npm` runs with
  the `bin` directory of its own version first on `PATH`.
- Layout compatible with nvm: `$NVM_DIR/versions/node/vX.Y.Z`,
  `$NVM_DIR/versions/io.js/vX.Y.Z`, `$NVM_DIR/alias/<name>`,
  `$NVM_DIR/alias/lts/<name>`, `$NVM_DIR/.cache/bin/<slug>/<slug>.tar.gz`,
  `$NVM_DIR/.cache/bin/<slug>/files` and `$NVM_DIR/.cache/locks/<version>`.
- Output is plain text (no colors) and aligned by bytes, like the `awk` of
  `nvm.sh`.
- Commits: Conventional Commits, GPG-signed with `git commit -S`, and no AI
  attribution or `Co-Authored-By` trailer in the message.
- Do not keep test scripts or test-output directories in the repository.

Each task below lists its steps in TDD order. Where a file already exists, the
code is shown as a unified diff against the previous task's final state; where
a file is new, the full code is shown. Apply diffs by hand (hunk headers are
informative, line numbers may drift), then run `cargo fmt`.

---

### Task 1: The platform and the download name

**Files:**

- Create: `src/domain/platform/mod.rs`, `src/domain/platform/tests.rs`
- Modify: `src/domain/mod.rs`

**Interfaces:**

- Consumes: `domain::version::{Version, Flavor}`.
- Produces: `domain::platform::{Os, Platform, binary_available}`;
  `Platform { os, arch }` with `Platform::from_host(os, arch, musl) ->
  Option<Platform>` (Rust's `std::env::consts` names in, nodejs.org names out)
  and `Platform::download_slug(&Version) -> String`;
  `binary_available(&Version) -> bool`.
- Behaviour (from `nvm_get_os`, `nvm_get_arch` and `nvm_get_download_slug`,
  checked against the real script): `linux`, `macos` (`darwin`) and `aix` have
  binaries, any other OS has none (`None`); `x86_64` is `x64`, `aarch64` is
  `arm64`, `arm` is `armv7l`, `loongarch64` is `loong64`, `powerpc64` is
  `ppc64`; on Alpine (`musl`) `x64` and `arm64` get a `-musl` suffix. The slug
  is `<node|iojs>-<vX.Y.Z>-<os>-<arch>`; before v4 a 32-bit ARM is `arm-pi`, and
  before v16 Apple Silicon asks for the `x64` build (io.js too). Binaries start
  at v0.8.6.
- [ ] **Step 1: Declare the new modules**

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -11,6 +11,7 @@
 pub mod mirror;
 pub mod nvmrc;
 pub mod path_search;
+pub mod platform;
 pub mod remote;
 pub mod remote_format;
 pub mod version;
```

- [ ] **Step 2: Write the failing tests**

Create `src/domain/platform/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/domain/platform/tests.rs`:

```rust
use super::*;

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

fn platform(os: &str, arch: &str) -> Platform {
    Platform::from_host(os, arch, false).unwrap()
}

#[test]
fn rust_names_become_the_names_nodejs_org_uses() {
    assert_eq!(platform("linux", "x86_64").arch, "x64");
    assert_eq!(platform("linux", "aarch64").arch, "arm64");
    assert_eq!(
        platform("macos", "aarch64"),
        Platform {
            os: Os::Darwin,
            arch: "arm64".into()
        }
    );
    assert_eq!(platform("linux", "arm").arch, "armv7l");
    assert_eq!(platform("linux", "loongarch64").arch, "loong64");
    assert_eq!(platform("linux", "s390x").arch, "s390x");
}

#[test]
fn an_unsupported_operating_system_has_no_platform() {
    assert_eq!(Platform::from_host("freebsd", "x86_64", false), None);
    assert_eq!(Platform::from_host("windows", "x86_64", false), None);
}

#[test]
fn alpine_uses_the_musl_builds_of_x64_and_arm64_only() {
    assert_eq!(
        Platform::from_host("linux", "x86_64", true).unwrap().arch,
        "x64-musl"
    );
    assert_eq!(
        Platform::from_host("linux", "aarch64", true).unwrap().arch,
        "arm64-musl"
    );
    assert_eq!(
        Platform::from_host("linux", "arm", true).unwrap().arch,
        "armv7l"
    );
    assert_eq!(
        Platform::from_host("macos", "x86_64", true).unwrap().arch,
        "x64"
    );
}

#[test]
fn the_slug_names_flavor_version_os_and_arch() {
    let linux = platform("linux", "x86_64");
    assert_eq!(
        linux.download_slug(&version("v20.10.0")),
        "node-v20.10.0-linux-x64"
    );
    assert_eq!(
        linux.download_slug(&version("iojs-v3.3.1")),
        "iojs-v3.3.1-linux-x64"
    );
}

#[test]
fn old_arm_builds_are_arm_pi_and_merged_ones_are_not() {
    let pi = platform("linux", "arm");
    assert_eq!(
        pi.download_slug(&version("v0.12.18")),
        "node-v0.12.18-linux-arm-pi"
    );
    assert_eq!(
        pi.download_slug(&version("iojs-v3.3.1")),
        "iojs-v3.3.1-linux-arm-pi"
    );
    assert_eq!(
        pi.download_slug(&version("v4.0.0")),
        "node-v4.0.0-linux-armv7l"
    );
}

#[test]
fn apple_silicon_runs_the_x64_build_before_node_16() {
    let mac = platform("macos", "aarch64");
    assert_eq!(
        mac.download_slug(&version("v14.21.3")),
        "node-v14.21.3-darwin-x64"
    );
    assert_eq!(
        mac.download_slug(&version("v16.0.0")),
        "node-v16.0.0-darwin-arm64"
    );
    let linux = platform("linux", "aarch64");
    assert_eq!(
        linux.download_slug(&version("v14.21.3")),
        "node-v14.21.3-linux-arm64"
    );
}

#[test]
fn binaries_start_at_node_0_8_6() {
    assert!(!binary_available(&version("v0.8.5")));
    assert!(binary_available(&version("v0.8.6")));
    assert!(binary_available(&version("iojs-v1.0.0")));
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "unresolved import
`crate::domain::platform`" and "cannot find struct `Platform`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/domain/platform/mod.rs`:

```rust
//! Which prebuilt binary fits this machine: `nvm_get_os`, `nvm_get_arch` and
//! `nvm_get_download_slug`, as pure functions of the host description.

use crate::domain::version::{Flavor, Version};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Darwin,
    Aix,
}

impl Os {
    /// The word `nvm.sh` uses in a download name.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Darwin => "darwin",
            Self::Aix => "aix",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    /// The architecture as nodejs.org names it: `x64`, `arm64`, `armv7l`, ...
    pub arch: String,
}

impl Platform {
    /// The platform from Rust's own names for it (`std::env::consts`), or
    /// `None` for an operating system that has no official binaries here.
    /// `musl` is true on Alpine, whose binaries live under a `-musl` name.
    #[must_use]
    pub fn from_host(os: &str, arch: &str, musl: bool) -> Option<Self> {
        let os = match os {
            "linux" => Os::Linux,
            "macos" => Os::Darwin,
            "aix" => Os::Aix,
            _ => return None,
        };
        let mut arch = match arch {
            "x86_64" => "x64",
            "x86" => "x86",
            "aarch64" => "arm64",
            "arm" => "armv7l",
            "loongarch64" => "loong64",
            "powerpc64" => "ppc64",
            other => other,
        }
        .to_owned();
        if musl && os == Os::Linux && matches!(arch.as_str(), "x64" | "arm64") {
            arch.push_str("-musl");
        }
        Some(Self { os, arch })
    }

    /// `node-v20.1.0-linux-x64`: the name of the archive without its extension
    /// and of the directory the cache keeps it in.
    #[must_use]
    pub fn download_slug(&self, version: &Version) -> String {
        let flavor = match version.flavor {
            Flavor::Node => "node",
            Flavor::IoJs => "iojs",
        };
        format!(
            "{flavor}-{}-{}-{}",
            version.directory_name(),
            self.os.slug(),
            self.arch_for(version)
        )
    }

    /// Old 32-bit ARM builds are named `arm-pi`, and Apple Silicon runs the
    /// x64 build of anything before Node 16.
    fn arch_for(&self, version: &Version) -> &str {
        let merged = version.major >= 4;
        if !merged && matches!(self.arch.as_str(), "armv6l" | "armv7l") {
            return "arm-pi";
        }
        if version.major < 16 && self.os == Os::Darwin && self.arch == "arm64" {
            return "x64";
        }
        &self.arch
    }
}

/// Binaries exist from Node 0.8.6 on.
#[must_use]
pub fn binary_available(version: &Version) -> bool {
    version.triple() >= (0, 8, 6)
}

```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 351 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(domain): name the prebuilt binary of a platform like nvm.sh"
```

---

### Task 2: The `FileSystem` port grows

**Files:**

- Modify: `src/ports/mod.rs`, `src/adapters/std_fs.rs`,
  `src/fakes/file_system/mod.rs` and `src/fakes/file_system/tests.rs`
  (`src/fakes/file_system.rs` becomes a directory module first, Step 0)

**Interfaces:**

- Produces: `ports::FileInfo { is_dir, len, executable, modified }`;
  `FileSystem::{write_bytes, file_info, rename, create_dir}`:
  `write_bytes(&Path, &[u8])`, `file_info(&Path) -> io::Result<FileInfo>`
  (follows symlinks), `rename(&Path, &Path)` (a file or a whole directory
  tree), `create_dir(&Path)` (fails with `AlreadyExists`: the atomic step of a
  lock). The `FakeFileSystem` now stores bytes, and has `with_executable(path,
  contents)`, `with_modified(path, SystemTime)` and `set_executable(&Path)`.

- [ ] **Step 0: Split the tests out of `file_system.rs` (no behaviour change)**

`src/fakes/file_system.rs` would pass 300 lines in this task. Run:

```bash
mkdir src/fakes/file_system && git mv src/fakes/file_system.rs src/fakes/file_system/mod.rs
```

Cut everything inside the `mod tests { ... }` block of
`src/fakes/file_system/mod.rs`, remove one level of indentation from it, and
paste it into the new file `src/fakes/file_system/tests.rs` (it starts with
`use super::*;`). Replace the block in `src/fakes/file_system/mod.rs` with:

```rust
#[cfg(test)]
mod tests;
```

Run: `cargo fmt && cargo test`
Expected: PASS, 351 unit tests plus the end-to-end tests, as at the end of
Task 1.

- [ ] **Step 1: Write the failing tests**

Apply to the test module of `src/adapters/std_fs.rs`:

```diff
--- a/src/adapters/std_fs.rs
+++ b/src/adapters/std_fs.rs
@@ -27,4 +27,53 @@
         StdFileSystem.remove_dir_all(&link).unwrap();
         assert!(!link.exists() && target.exists());
     }
+
+    #[test]
+    fn file_info_tells_size_kind_and_executability() {
+        let root = tempfile::tempdir().unwrap();
+        let file = root.path().join("tool");
+        fs::write(&file, "abc").unwrap();
+        let info = StdFileSystem.file_info(&file).unwrap();
+        assert_eq!((info.is_dir, info.len), (false, 3));
+        assert!(info.modified.is_some());
+        assert!(StdFileSystem.file_info(root.path()).unwrap().is_dir);
+        assert!(
+            StdFileSystem
+                .file_info(&root.path().join("missing"))
+                .is_err()
+        );
+    }
+
+    #[cfg(unix)]
+    #[test]
+    fn file_info_sees_the_execute_bit() {
+        use std::os::unix::fs::PermissionsExt;
+        let root = tempfile::tempdir().unwrap();
+        let file = root.path().join("tool");
+        fs::write(&file, "x").unwrap();
+        assert!(!StdFileSystem.file_info(&file).unwrap().executable);
+        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
+        assert!(StdFileSystem.file_info(&file).unwrap().executable);
+    }
+
+    #[test]
+    fn create_dir_fails_when_the_directory_exists_and_rename_moves_a_tree() {
+        let root = tempfile::tempdir().unwrap();
+        let first = root.path().join("lock");
+        StdFileSystem.create_dir(&first).unwrap();
+        let again = StdFileSystem.create_dir(&first).unwrap_err();
+        assert_eq!(again.kind(), io::ErrorKind::AlreadyExists);
+        fs::write(first.join("f"), "x").unwrap();
+        let moved = root.path().join("moved");
+        StdFileSystem.rename(&first, &moved).unwrap();
+        assert!(moved.join("f").is_file() && !first.exists());
+    }
+
+    #[test]
+    fn write_bytes_stores_what_is_not_text() {
+        let root = tempfile::tempdir().unwrap();
+        let file = root.path().join("blob");
+        StdFileSystem.write_bytes(&file, &[0, 255, 1]).unwrap();
+        assert_eq!(fs::read(&file).unwrap(), [0, 255, 1]);
+    }
 }
```

Apply to `src/fakes/file_system/tests.rs`:

```diff
--- a/src/fakes/file_system/tests.rs
+++ b/src/fakes/file_system/tests.rs
@@ -28,6 +28,55 @@
     fs.remove_dir_all(Path::new("/d/a")).unwrap();
     assert_eq!(fs.read_dir(Path::new("/d")).unwrap(), [entry("ab", false)]);
     fs.remove_dir_all(Path::new("/d/missing")).unwrap();
+}
+
+#[test]
+fn fake_file_system_reports_what_a_path_is() {
+    let fs = FakeFileSystem::default()
+        .with_executable("/d/bin/node", "abc")
+        .with_file("/d/plain", "")
+        .with_dir("/d/empty");
+    let node = fs.file_info(Path::new("/d/bin/node")).unwrap();
+    assert_eq!((node.is_dir, node.len, node.executable), (false, 3, true));
+    assert!(!fs.file_info(Path::new("/d/plain")).unwrap().executable);
+    assert!(fs.file_info(Path::new("/d/bin")).unwrap().is_dir);
+    assert!(fs.file_info(Path::new("/d/empty")).unwrap().is_dir);
+    assert!(fs.file_info(Path::new("/nope")).is_err());
+}
+
+#[test]
+fn fake_file_system_reports_when_a_path_was_changed() {
+    let time = SystemTime::UNIX_EPOCH;
+    let fs = FakeFileSystem::default()
+        .with_file("/d/f", "")
+        .with_modified("/d/f", time);
+    assert_eq!(
+        fs.file_info(Path::new("/d/f")).unwrap().modified,
+        Some(time)
+    );
+}
+
+#[test]
+fn fake_file_system_renames_a_tree_and_creates_a_directory_once() {
+    let fs = FakeFileSystem::default()
+        .with_file("/a/x/y", "1")
+        .with_dir("/a/empty");
+    fs.rename(Path::new("/a"), Path::new("/b")).unwrap();
+    assert_eq!(fs.read_to_string(Path::new("/b/x/y")).unwrap(), "1");
+    assert!(fs.file_info(Path::new("/b/empty")).unwrap().is_dir);
+    assert!(fs.file_info(Path::new("/a")).is_err());
+    assert!(fs.rename(Path::new("/a"), Path::new("/c")).is_err());
+    fs.create_dir(Path::new("/lock")).unwrap();
+    let again = fs.create_dir(Path::new("/lock")).unwrap_err();
+    assert_eq!(again.kind(), io::ErrorKind::AlreadyExists);
+}
+
+#[test]
+fn fake_file_system_keeps_bytes_and_rejects_non_text_as_a_string() {
+    let fs = FakeFileSystem::default();
+    fs.write_bytes(Path::new("/blob"), &[0, 255]).unwrap();
+    let error = fs.read_to_string(Path::new("/blob")).unwrap_err();
+    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
 }
 
 #[test]
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find struct `FileInfo`"
and "no method named `file_info` found for struct `FakeFileSystem`".

- [ ] **Step 3: Write the implementation**

Apply to `src/adapters/std_fs.rs` (above the test module):

```diff
--- a/src/adapters/std_fs.rs
+++ b/src/adapters/std_fs.rs
@@ -2,7 +2,7 @@
 use std::io;
 use std::path::Path;
 
-use crate::ports::{DirEntry, FileSystem};
+use crate::ports::{DirEntry, FileInfo, FileSystem};
 
 pub struct StdFileSystem;
 
@@ -44,8 +44,41 @@
         fs::write(path, contents)
     }
 
+    fn write_bytes(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
+        fs::write(path, contents)
+    }
+
+    fn file_info(&self, path: &Path) -> io::Result<FileInfo> {
+        let metadata = fs::metadata(path)?;
+        Ok(FileInfo {
+            is_dir: metadata.is_dir(),
+            len: metadata.len(),
+            executable: is_executable(&metadata),
+            modified: metadata.modified().ok(),
+        })
+    }
+
+    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
+        fs::rename(from, to)
+    }
+
+    fn create_dir(&self, path: &Path) -> io::Result<()> {
+        fs::create_dir(path)
+    }
+
     fn create_dir_all(&self, path: &Path) -> io::Result<()> {
         fs::create_dir_all(path)
     }
 }
 
+#[cfg(unix)]
+fn is_executable(metadata: &fs::Metadata) -> bool {
+    use std::os::unix::fs::PermissionsExt;
+    metadata.permissions().mode() & 0o111 != 0
+}
+
+#[cfg(not(unix))]
+fn is_executable(_metadata: &fs::Metadata) -> bool {
+    true
+}
+
```

Apply to `src/fakes/file_system/mod.rs` (above the test module):

```diff
--- a/src/fakes/file_system/mod.rs
+++ b/src/fakes/file_system/mod.rs
@@ -2,13 +2,16 @@
 use std::collections::{BTreeMap, BTreeSet};
 use std::io;
 use std::path::{Path, PathBuf};
+use std::time::SystemTime;
 
-use crate::ports::{DirEntry, FileSystem};
+use crate::ports::{DirEntry, FileInfo, FileSystem};
 
 #[derive(Default)]
 pub struct FakeFileSystem {
-    files: RefCell<BTreeMap<PathBuf, String>>,
+    files: RefCell<BTreeMap<PathBuf, Vec<u8>>>,
     dirs: RefCell<BTreeSet<PathBuf>>,
+    executables: RefCell<BTreeSet<PathBuf>>,
+    modified: RefCell<BTreeMap<PathBuf, SystemTime>>,
 }
 
 impl FakeFileSystem {
@@ -16,7 +19,21 @@
     pub fn with_file(mut self, path: &str, contents: &str) -> Self {
         self.files
             .get_mut()
-            .insert(PathBuf::from(path), contents.to_owned());
+            .insert(PathBuf::from(path), contents.as_bytes().to_vec());
+        self
+    }
+
+    /// A file with the execute permission.
+    #[must_use]
+    pub fn with_executable(mut self, path: &str, contents: &str) -> Self {
+        self.executables.get_mut().insert(PathBuf::from(path));
+        self.with_file(path, contents)
+    }
+
+    /// When the file or directory was last changed.
+    #[must_use]
+    pub fn with_modified(mut self, path: &str, time: SystemTime) -> Self {
+        self.modified.get_mut().insert(PathBuf::from(path), time);
         self
     }
 
@@ -30,11 +47,9 @@
 
 impl FileSystem for FakeFileSystem {
     fn read_to_string(&self, path: &Path) -> io::Result<String> {
-        self.files
-            .borrow()
-            .get(path)
-            .cloned()
-            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
+        let bytes = self.files.borrow().get(path).cloned();
+        let bytes = bytes.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
+        String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
     }
 
     fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
@@ -86,9 +101,71 @@
     }
 
     fn write_file(&self, path: &Path, contents: &str) -> io::Result<()> {
+        self.write_bytes(path, contents.as_bytes())
+    }
+
+    fn write_bytes(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
         self.files
             .borrow_mut()
-            .insert(path.to_path_buf(), contents.to_owned());
+            .insert(path.to_path_buf(), contents.to_vec());
+        Ok(())
+    }
+
+    fn file_info(&self, path: &Path) -> io::Result<FileInfo> {
+        let modified = self.modified.borrow().get(path).copied();
+        if let Some(contents) = self.files.borrow().get(path) {
+            return Ok(FileInfo {
+                is_dir: false,
+                len: contents.len() as u64,
+                executable: self.executables.borrow().contains(path),
+                modified,
+            });
+        }
+        if self.read_dir(path).is_ok() {
+            return Ok(FileInfo {
+                is_dir: true,
+                len: 0,
+                executable: true,
+                modified,
+            });
+        }
+        Err(io::Error::from(io::ErrorKind::NotFound))
+    }
+
+    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
+        if self.file_info(from).is_err() {
+            return Err(io::Error::from(io::ErrorKind::NotFound));
+        }
+        let moved = |old: &Path| to.join(old.strip_prefix(from).unwrap_or(old));
+        let mut files = self.files.borrow_mut();
+        let names: Vec<PathBuf> = files
+            .keys()
+            .filter(|f| f.starts_with(from))
+            .cloned()
+            .collect();
+        for name in names {
+            if let Some(contents) = files.remove(&name) {
+                files.insert(moved(&name), contents);
+            }
+        }
+        let mut dirs = self.dirs.borrow_mut();
+        let names: Vec<PathBuf> = dirs
+            .iter()
+            .filter(|d| d.starts_with(from))
+            .cloned()
+            .collect();
+        for name in names {
+            dirs.remove(&name);
+            dirs.insert(moved(&name));
+        }
+        Ok(())
+    }
+
+    fn create_dir(&self, path: &Path) -> io::Result<()> {
+        if self.file_info(path).is_ok() {
+            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
+        }
+        self.dirs.borrow_mut().insert(path.to_path_buf());
         Ok(())
     }
 
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -3,7 +3,7 @@
 use std::ffi::OsString;
 use std::io;
 use std::path::Path;
-use std::time::Duration;
+use std::time::{Duration, SystemTime};
 
 use thiserror::Error;
 
@@ -12,6 +12,16 @@
 pub struct DirEntry {
     pub name: String,
     pub is_dir: bool,
+}
+
+/// What a path is, following symlinks.
+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+pub struct FileInfo {
+    pub is_dir: bool,
+    pub len: u64,
+    /// Any execute permission bit is set (always true off Unix).
+    pub executable: bool,
+    pub modified: Option<SystemTime>,
 }
 
 pub trait FileSystem {
@@ -44,6 +54,30 @@
     /// # Errors
     /// Propagates the underlying I/O error.
     fn write_file(&self, path: &Path, contents: &str) -> io::Result<()>;
+
+    /// Like [`Self::write_file`], for contents that are not text.
+    ///
+    /// # Errors
+    /// Propagates the underlying I/O error.
+    fn write_bytes(&self, path: &Path, contents: &[u8]) -> io::Result<()>;
+
+    /// # Errors
+    /// Propagates the underlying I/O error (for example when `path` is missing).
+    fn file_info(&self, path: &Path) -> io::Result<FileInfo>;
+
+    /// Moves a file or a directory (with everything in it) to `to`, which must
+    /// not exist.
+    ///
+    /// # Errors
+    /// Propagates the underlying I/O error.
+    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
+
+    /// Creates one directory, failing with `AlreadyExists` when it is there:
+    /// the atomic step a lock is made of.
+    ///
+    /// # Errors
+    /// Propagates the underlying I/O error.
+    fn create_dir(&self, path: &Path) -> io::Result<()>;
 
     /// Creates `path` and any missing parents; fine if it already exists.
     ///
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 359 unit tests plus the end-to-end tests.

- [ ] **Step 5: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(ports): let FileSystem rename, create a directory once, write bytes and stat"
```

---

### Task 3: Checksums and the `Digest` port

**Files:**

- Create: `src/domain/checksum/mod.rs`, `src/domain/checksum/tests.rs`,
  `src/adapters/sha256_digest.rs`, `src/adapters/no_digest.rs`,
  `src/fakes/digest.rs`
- Modify: `Cargo.toml`, `Cargo.lock`, `src/domain/mod.rs`, `src/ports/mod.rs`,
  `src/adapters/mod.rs`, `src/fakes/mod.rs`, `src/context.rs`

**Interfaces:**

- Produces: `domain::checksum::{expected_digest, compare, ChecksumError}`:
  `expected_digest(&str, file_name) -> Option<String>` (the hash `SHASUMS256.txt`
  lists for an exact file name), `compare(computed, expected) -> Result<(),
  ChecksumError>`; `ports::Digest::sha256_file(&Path) -> io::Result<String>`
  (lowercase hex); `adapters::sha256_digest::Sha256Digest`,
  `adapters::no_digest::NoDigest`, test-only `fakes::FakeDigest` with
  `with_digest(path, hex)`; `Context::with_digest(&dyn Digest)` and
  `Context::digest()`.
- Behaviour (from `nvm_compare_checksum`): a checksum that is empty is
  `Provided checksum to compare to is empty.`; a difference is `Checksums do not
  match: '<computed>' found, '<expected>' expected.`; a computed value with a
  leading backslash (a `sha256sum` marker) still matches.
- [ ] **Step 1: Add the dependency (sha2)**

Run: `cargo add sha2`

Expected: `Cargo.toml` gains a line like the one below under
`[dependencies]` and `Cargo.lock` is updated (keep the version cargo picks).
Commit both with this task.

```toml
sha2 = "0.11.0"
```

- [ ] **Step 2: Declare the new modules**

Apply to `src/adapters/mod.rs`:

```diff
--- a/src/adapters/mod.rs
+++ b/src/adapters/mod.rs
@@ -1,7 +1,9 @@
 pub mod fs_alias_store;
+pub mod no_digest;
 pub mod no_http;
 pub mod no_process;
 pub mod retrying_http;
+pub mod sha256_digest;
 pub mod std_env;
 pub mod std_fs;
 pub mod std_process;
```

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -1,5 +1,6 @@
 pub mod alias;
 pub mod alias_format;
+pub mod checksum;
 pub mod current;
 #[cfg(test)]
 pub(crate) mod fixtures;
```

- [ ] **Step 3: Write the failing tests**

Create `src/adapters/no_digest.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_nothing() {
        let error = NoDigest.sha256_file(Path::new("/x")).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}
```

Create `src/adapters/sha256_digest.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_a_file_to_lowercase_hex() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("abc");
        std::fs::write(&file, "abc").unwrap();
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(Sha256Digest.sha256_file(&file).unwrap(), expected);
    }

    #[test]
    fn hashes_a_file_longer_than_one_chunk() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("big");
        std::fs::write(&file, vec![b'a'; 200_000]).unwrap();
        let expected = "2287d207f24a941ff3b56c04c8a25ad56b63e3023207b3bb5b4ac0c9869d74be";
        assert_eq!(Sha256Digest.sha256_file(&file).unwrap(), expected);
    }

    #[test]
    fn a_missing_file_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        let error = Sha256Digest
            .sha256_file(&root.path().join("nope"))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
```

Apply to the test module of `src/context.rs`:

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -1,7 +1,7 @@
 #[cfg(test)]
 mod tests {
     use super::*;
-    use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};
+    use crate::fakes::{FakeDigest, FakeEnv, FakeFileSystem, FakeHttp};
 
     fn nvm_dir_for(env: &FakeEnv) -> Result<PathBuf, CliError> {
         let fs = FakeFileSystem::default();
@@ -81,6 +81,24 @@
     }
 
     #[test]
+    fn a_context_hashes_nothing_until_it_is_given_a_digest() {
+        let fs = FakeFileSystem::default();
+        let env = FakeEnv::default();
+        assert!(
+            Context::new(&fs, &env)
+                .digest()
+                .sha256_file(Path::new("/f"))
+                .is_err()
+        );
+        let digest = FakeDigest::default().with_digest("/f", "abc");
+        let context = Context::new(&fs, &env).with_digest(&digest);
+        assert_eq!(
+            context.digest().sha256_file(Path::new("/f")).unwrap(),
+            "abc"
+        );
+    }
+
+    #[test]
     fn alias_dir_is_under_nvm_dir() {
         let fs = FakeFileSystem::default();
         let env = FakeEnv::default().with_var("NVM_DIR", "/n");
```

Create `src/domain/checksum/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/domain/checksum/tests.rs`:

```rust
use super::*;

const SHASUMS: &str = "\
aaa111  node-v20.10.0-darwin-arm64.tar.gz
bbb222  node-v20.10.0-linux-x64.tar.gz
ccc333  node-v20.10.0-linux-x64.tar.xz
";

#[test]
fn the_checksum_of_a_listed_file_is_found_by_its_exact_name() {
    let found = expected_digest(SHASUMS, "node-v20.10.0-linux-x64.tar.gz");
    assert_eq!(found.as_deref(), Some("bbb222"));
    let other = expected_digest(SHASUMS, "node-v20.10.0-linux-x64.tar.xz");
    assert_eq!(other.as_deref(), Some("ccc333"));
}

#[test]
fn a_file_that_is_not_listed_has_no_checksum() {
    assert_eq!(expected_digest(SHASUMS, "node-v20.10.0-linux-x64"), None);
    assert_eq!(expected_digest("", "x"), None);
    assert_eq!(expected_digest("lonely\n", "x"), None);
}

#[test]
fn equal_digests_match_with_or_without_the_escape_marker() {
    assert_eq!(compare("abc", "abc"), Ok(()));
    assert_eq!(compare("\\abc", "abc"), Ok(()));
}

#[test]
fn different_digests_are_a_mismatch_with_the_message_of_nvm_sh() {
    let error = compare("abc", "def").unwrap_err();
    assert_eq!(
        error.to_string(),
        "Checksums do not match: 'abc' found, 'def' expected."
    );
}

#[test]
fn a_missing_expected_digest_is_an_error() {
    let error = compare("abc", "").unwrap_err();
    assert_eq!(error, ChecksumError::MissingExpected);
    assert_eq!(
        error.to_string(),
        "Provided checksum to compare to is empty."
    );
}
```

Create `src/fakes/digest.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_by_path() {
        let digest = FakeDigest::default().with_digest("/a", "abc");
        assert_eq!(digest.sha256_file(Path::new("/a")).unwrap(), "abc");
        assert!(digest.sha256_file(Path::new("/b")).is_err());
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function
`expected_digest`", "unresolved import `crate::ports::Digest`" and "cannot
find struct `FakeDigest`".

- [ ] **Step 5: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/adapters/no_digest.rs`:

```rust
use std::io;
use std::path::Path;

use crate::ports::Digest;

/// The `Digest` of a `Context` that was not given one: it hashes nothing.
pub struct NoDigest;

impl Digest for NoDigest {
    fn sha256_file(&self, _path: &Path) -> io::Result<String> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

```

Insert above the `#[cfg(test)]` line of `src/adapters/sha256_digest.rs`:

```rust
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::ports::Digest;

/// The real [`Digest`]: SHA-256 of a file read in chunks.
pub struct Sha256Digest;

impl Digest for Sha256Digest {
    fn sha256_file(&self, path: &Path) -> io::Result<String> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut chunk = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            hasher.update(&chunk[..read]);
        }
        Ok(hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
}

```

Apply to `src/context.rs` (above the test module):

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -3,18 +3,20 @@
 use std::path::{Path, PathBuf};
 
 use crate::adapters::fs_alias_store::FsAliasStore;
+use crate::adapters::no_digest::NoDigest;
 use crate::adapters::no_http::NoHttp;
 use crate::adapters::no_process::NoProcess;
 use crate::domain::alias::AliasStore;
 use crate::domain::version::Version;
 use crate::error::CliError;
-use crate::ports::{Env, FileSystem, Http, Process};
+use crate::ports::{Digest, Env, FileSystem, Http, Process};
 
 pub struct Context<'a> {
     pub fs: &'a dyn FileSystem,
     pub env: &'a dyn Env,
     process: &'a dyn Process,
     http: &'a dyn Http,
+    digest: &'a dyn Digest,
 }
 
 impl<'a> Context<'a> {
@@ -27,6 +29,7 @@
             env,
             process: &NoProcess,
             http: &NoHttp,
+            digest: &NoDigest,
         }
     }
 
@@ -39,6 +42,17 @@
     #[must_use]
     pub fn process(&self) -> &dyn Process {
         self.process
+    }
+
+    #[must_use]
+    pub fn with_digest(mut self, digest: &'a dyn Digest) -> Self {
+        self.digest = digest;
+        self
+    }
+
+    #[must_use]
+    pub fn digest(&self) -> &dyn Digest {
+        self.digest
     }
 
     #[must_use]
```

Insert above the `#[cfg(test)]` line of `src/domain/checksum/mod.rs`:

```rust
//! `SHASUMS256.txt` and the comparison of a download with it, as
//! `nvm_get_checksum` and `nvm_compare_checksum` do.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ChecksumError {
    #[error("Provided checksum to compare to is empty.")]
    MissingExpected,
    #[error("Checksums do not match: '{computed}' found, '{expected}' expected.")]
    Mismatch { computed: String, expected: String },
}

/// The checksum listed for `file_name` in a `SHASUMS256.txt` (`<hash>  <name>`
/// per line), or `None` when it is not listed.
#[must_use]
pub fn expected_digest(shasums: &str, file_name: &str) -> Option<String> {
    shasums.lines().find_map(|line| {
        let mut columns = line.split_whitespace();
        let digest = columns.next()?;
        (columns.next()? == file_name).then(|| digest.to_owned())
    })
}

/// # Errors
/// - [`ChecksumError::MissingExpected`] when the mirror listed no checksum.
/// - [`ChecksumError::Mismatch`] when the download differs; `nvm.sh` also
///   accepts the expected value with a leading backslash (a `sha256sum` marker
///   for a file name with escapes).
pub fn compare(computed: &str, expected: &str) -> Result<(), ChecksumError> {
    if expected.is_empty() {
        return Err(ChecksumError::MissingExpected);
    }
    if computed == expected || computed.strip_prefix('\\') == Some(expected) {
        return Ok(());
    }
    Err(ChecksumError::Mismatch {
        computed: computed.to_owned(),
        expected: expected.to_owned(),
    })
}

```

Insert above the `#[cfg(test)]` line of `src/fakes/digest.rs`:

```rust
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::ports::Digest;

/// Files by path: each has a fixed digest; any other file is not found.
#[derive(Default)]
pub struct FakeDigest {
    digests: BTreeMap<PathBuf, String>,
}

impl FakeDigest {
    #[must_use]
    pub fn with_digest(mut self, path: &str, digest: &str) -> Self {
        self.digests.insert(PathBuf::from(path), digest.to_owned());
        self
    }
}

impl Digest for FakeDigest {
    fn sha256_file(&self, path: &Path) -> io::Result<String> {
        self.digests
            .get(path)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}

```

Apply to `src/fakes/mod.rs` (above the test module):

```diff
--- a/src/fakes/mod.rs
+++ b/src/fakes/mod.rs
@@ -1,11 +1,13 @@
 //! In-memory implementations of the ports, for unit tests only.
 
+mod digest;
 mod env;
 mod file_system;
 mod http;
 mod process;
 mod sleeper;
 
+pub use digest::FakeDigest;
 pub use env::FakeEnv;
 pub use file_system::FakeFileSystem;
 pub use http::FakeHttp;
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -123,6 +123,14 @@
     fn get_text(&self, url: &str) -> Result<String, HttpError>;
 }
 
+pub trait Digest {
+    /// The SHA-256 of the file at `path`, in lowercase hex.
+    ///
+    /// # Errors
+    /// Propagates the underlying I/O error.
+    fn sha256_file(&self, path: &Path) -> io::Result<String>;
+}
+
 pub trait Sleeper {
     /// Waits for `duration` (between retries).
     fn sleep(&self, duration: Duration);
```

Then run `cargo fmt`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 370 unit tests plus the end-to-end tests.

- [ ] **Step 7: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
rustup run 1.88 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings, no advisories and the crate
still builds on MSRV 1.88.

```bash
git add src tests Cargo.toml Cargo.lock
git commit -S -m "feat(domain): check a download against SHASUMS256.txt with a Digest port"
```

---

### Task 4: Unpacking archives

**Files:**

- Create: `src/adapters/targz_archive/mod.rs`,
  `src/adapters/targz_archive/tests.rs`, `src/adapters/no_archive.rs`,
  `src/fakes/archive.rs`
- Modify: `Cargo.toml`, `Cargo.lock`, `src/ports/mod.rs`,
  `src/adapters/mod.rs`, `src/fakes/mod.rs`, `src/fakes/file_system/mod.rs`,
  `src/context.rs`

**Interfaces:**

- Produces: `ports::Archive::extract(&Path, &Path) -> io::Result<()>` (a
  `.tar.gz` into an existing directory); `adapters::targz_archive::TarGzArchive`
  (the `tar` and `flate2` crates; an unsafe entry fails the whole unpack with
  `InvalidData`; owners are not restored; execute bits and relative symlinks
  are kept); `adapters::no_archive::NoArchive`; test-only
  `fakes::FakeArchive::new(&FakeFileSystem).with_archive(path, &[(entry,
  contents, executable)])` and `FakeFileSystem::set_executable`;
  `Context::with_archive(&dyn Archive)` and `Context::archive()`.

- [ ] **Step 1: Add the dependency (flate2 and tar)**

Run: `cargo add flate2` and `cargo add tar --no-default-features`

Expected: `Cargo.toml` gains lines like the ones below under
`[dependencies]` (the second one has no default features, so no
extended-attribute code is built) and `Cargo.lock` is updated. Commit both
with this task.

```toml
flate2 = "1.1.10"
tar = { version = "0.4.46", default-features = false }
```

- [ ] **Step 2: Declare the new modules**

Apply to `src/adapters/mod.rs`:

```diff
--- a/src/adapters/mod.rs
+++ b/src/adapters/mod.rs
@@ -1,4 +1,5 @@
 pub mod fs_alias_store;
+pub mod no_archive;
 pub mod no_digest;
 pub mod no_http;
 pub mod no_process;
@@ -8,4 +9,5 @@
 pub mod std_fs;
 pub mod std_process;
 pub mod std_sleeper;
+pub mod targz_archive;
 pub mod ureq_http;
```

- [ ] **Step 3: Write the failing tests**

Create `src/adapters/no_archive.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacks_nothing() {
        let error = NoArchive
            .extract(Path::new("/a"), Path::new("/b"))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}
```

Create `src/adapters/targz_archive/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/adapters/targz_archive/tests.rs`:

```rust
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;

use super::*;

/// A `.tar.gz` of `(path, contents, mode)` files and `(path, target)`
/// symlinks (which come first), written to `directory/archive.tar.gz`.
fn build(directory: &Path, files: &[(&str, &str, u32)], links: &[(&str, &str)]) -> PathBuf {
    let path = directory.join("archive.tar.gz");
    let mut builder = tar::Builder::new(GzEncoder::new(
        File::create(&path).unwrap(),
        Compression::default(),
    ));
    for (name, target) in links {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        builder.append_link(&mut header, name, target).unwrap();
    }
    for (name, contents, mode) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(*mode);
        header.set_cksum();
        builder
            .append_data(&mut header, name, contents.as_bytes())
            .unwrap();
    }
    builder
        .into_inner()
        .unwrap()
        .finish()
        .unwrap()
        .flush()
        .unwrap();
    path
}

#[test]
fn it_unpacks_files_into_the_destination() {
    let root = tempfile::tempdir().unwrap();
    let archive = build(
        root.path(),
        &[
            ("node-v1/bin/node", "binary", 0o755),
            ("node-v1/README", "hi", 0o644),
        ],
        &[],
    );
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    TarGzArchive.extract(&archive, &destination).unwrap();
    let node = destination.join("node-v1/bin/node");
    assert_eq!(fs::read_to_string(&node).unwrap(), "binary");
    assert_eq!(
        fs::read_to_string(destination.join("node-v1/README")).unwrap(),
        "hi"
    );
}

#[cfg(unix)]
#[test]
fn it_keeps_the_execute_bit_and_relative_symlinks() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let archive = build(
        root.path(),
        &[
            ("node-v1/bin/node", "x", 0o755),
            ("node-v1/lib/npm-cli.js", "y", 0o644),
        ],
        &[("node-v1/bin/npm", "../lib/npm-cli.js")],
    );
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    TarGzArchive.extract(&archive, &destination).unwrap();
    let mode = fs::metadata(destination.join("node-v1/bin/node"))
        .unwrap()
        .permissions()
        .mode();
    assert_ne!(mode & 0o111, 0);
    let npm = destination.join("node-v1/bin/npm");
    assert_eq!(fs::read_to_string(npm).unwrap(), "y");
}

#[test]
fn a_missing_archive_is_an_error() {
    let root = tempfile::tempdir().unwrap();
    let error = TarGzArchive
        .extract(&root.path().join("nope.tar.gz"), root.path())
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}

#[test]
fn something_that_is_not_gzip_is_an_error() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("bad.tar.gz");
    fs::write(&archive, "not an archive").unwrap();
    assert!(TarGzArchive.extract(&archive, root.path()).is_err());
}

#[test]
fn an_entry_that_climbs_out_of_the_destination_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let archive = root.path().join("evil.tar.gz");
    // `tar::Builder` refuses `..`, so the entry is written by hand.
    let mut header = tar::Header::new_gnu();
    header.set_size(1);
    header.set_mode(0o644);
    let name = b"../escaped";
    header.as_old_mut().name[..name.len()].copy_from_slice(name);
    header.set_cksum();
    let mut encoder = GzEncoder::new(File::create(&archive).unwrap(), Compression::default());
    encoder.write_all(header.as_bytes()).unwrap();
    let mut block = [0_u8; 512];
    block[0] = b'x';
    encoder.write_all(&block).unwrap();
    encoder.write_all(&[0_u8; 1024]).unwrap();
    encoder.finish().unwrap();
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    let error = TarGzArchive.extract(&archive, &destination).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("unsafe path in archive"));
    assert!(!root.path().join("escaped").exists());
}

#[cfg(unix)]
#[test]
fn a_file_written_through_a_symlink_that_leaves_the_destination_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let outside = root.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let archive = build(
        root.path(),
        &[("top/link/stolen", "x", 0o644)],
        &[("top/link", outside.to_str().unwrap())],
    );
    let destination = root.path().join("out");
    fs::create_dir(&destination).unwrap();
    let result = TarGzArchive.extract(&archive, &destination);
    assert!(result.is_err());
    assert!(!outside.join("stolen").exists());
}
```

Apply to the test module of `src/context.rs`:

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -1,7 +1,7 @@
 #[cfg(test)]
 mod tests {
     use super::*;
-    use crate::fakes::{FakeDigest, FakeEnv, FakeFileSystem, FakeHttp};
+    use crate::fakes::{FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp};
 
     fn nvm_dir_for(env: &FakeEnv) -> Result<PathBuf, CliError> {
         let fs = FakeFileSystem::default();
@@ -99,6 +99,18 @@
     }
 
     #[test]
+    fn a_context_unpacks_nothing_until_it_is_given_an_archive() {
+        let fs = FakeFileSystem::default();
+        let env = FakeEnv::default();
+        let (a, b) = (Path::new("/a.tgz"), Path::new("/out"));
+        assert!(Context::new(&fs, &env).archive().extract(a, b).is_err());
+        let archive = FakeArchive::new(&fs).with_archive("/a.tgz", &[("t/f", "x", false)]);
+        let context = Context::new(&fs, &env).with_archive(&archive);
+        context.archive().extract(a, b).unwrap();
+        assert!(fs.is_file(Path::new("/out/t/f")));
+    }
+
+    #[test]
     fn alias_dir_is_under_nvm_dir() {
         let fs = FakeFileSystem::default();
         let env = FakeEnv::default().with_var("NVM_DIR", "/n");
```

Create `src/fakes/archive.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpacks_its_files_under_the_destination() {
        let fs = FakeFileSystem::default();
        let archive = FakeArchive::new(&fs).with_archive("/a.tgz", &[("top/bin/node", "x", true)]);
        archive
            .extract(Path::new("/a.tgz"), Path::new("/out"))
            .unwrap();
        let info = fs.file_info(Path::new("/out/top/bin/node")).unwrap();
        assert!(info.executable);
        assert!(
            archive
                .extract(Path::new("/b.tgz"), Path::new("/out"))
                .is_err()
        );
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "unresolved import
`crate::ports::Archive`", "cannot find struct `TarGzArchive`" and "cannot find
struct `FakeArchive`".

- [ ] **Step 5: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/adapters/no_archive.rs`:

```rust
use std::io;
use std::path::Path;

use crate::ports::Archive;

/// The `Archive` of a `Context` that was not given one: it unpacks nothing.
pub struct NoArchive;

impl Archive for NoArchive {
    fn extract(&self, _archive: &Path, _destination: &Path) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

```

Insert above the `#[cfg(test)]` line of `src/adapters/targz_archive/mod.rs`:

```rust
use std::fs::File;
use std::io;
use std::path::Path;

use flate2::read::GzDecoder;

use crate::ports::Archive;

/// The real [`Archive`]: a gzip-compressed tar, unpacked by the `tar` crate.
/// An entry that would land outside `destination` (`..`, an absolute path, or
/// a path that goes through a symlink out of it) fails the whole extraction,
/// and the owner of a file is not restored.
pub struct TarGzArchive;

impl Archive for TarGzArchive {
    fn extract(&self, archive: &Path, destination: &Path) -> io::Result<()> {
        let mut reader = tar::Archive::new(GzDecoder::new(File::open(archive)?));
        reader.set_preserve_ownerships(false);
        for entry in reader.entries()? {
            let mut entry = entry?;
            if !entry.unpack_in(destination)? {
                let message = format!("unsafe path in archive: {}", entry.path()?.display());
                return Err(io::Error::new(io::ErrorKind::InvalidData, message));
            }
        }
        Ok(())
    }
}

```

Apply to `src/context.rs` (above the test module):

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -3,13 +3,14 @@
 use std::path::{Path, PathBuf};
 
 use crate::adapters::fs_alias_store::FsAliasStore;
+use crate::adapters::no_archive::NoArchive;
 use crate::adapters::no_digest::NoDigest;
 use crate::adapters::no_http::NoHttp;
 use crate::adapters::no_process::NoProcess;
 use crate::domain::alias::AliasStore;
 use crate::domain::version::Version;
 use crate::error::CliError;
-use crate::ports::{Digest, Env, FileSystem, Http, Process};
+use crate::ports::{Archive, Digest, Env, FileSystem, Http, Process};
 
 pub struct Context<'a> {
     pub fs: &'a dyn FileSystem,
@@ -17,6 +18,7 @@
     process: &'a dyn Process,
     http: &'a dyn Http,
     digest: &'a dyn Digest,
+    archive: &'a dyn Archive,
 }
 
 impl<'a> Context<'a> {
@@ -30,6 +32,7 @@
             process: &NoProcess,
             http: &NoHttp,
             digest: &NoDigest,
+            archive: &NoArchive,
         }
     }
 
@@ -42,6 +45,17 @@
     #[must_use]
     pub fn process(&self) -> &dyn Process {
         self.process
+    }
+
+    #[must_use]
+    pub fn with_archive(mut self, archive: &'a dyn Archive) -> Self {
+        self.archive = archive;
+        self
+    }
+
+    #[must_use]
+    pub fn archive(&self) -> &dyn Archive {
+        self.archive
     }
 
     #[must_use]
```

Insert above the `#[cfg(test)]` line of `src/fakes/archive.rs`:

```rust
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::fakes::FakeFileSystem;
use crate::ports::{Archive, FileSystem};

/// One file of a fake archive.
struct Entry {
    path: &'static str,
    contents: &'static str,
    executable: bool,
}

/// Archives by path: unpacking one writes its files into a [`FakeFileSystem`]
/// under the destination; any other path is not found.
pub struct FakeArchive<'a> {
    fs: &'a FakeFileSystem,
    archives: BTreeMap<PathBuf, Vec<Entry>>,
}

impl<'a> FakeArchive<'a> {
    #[must_use]
    pub fn new(fs: &'a FakeFileSystem) -> Self {
        Self {
            fs,
            archives: BTreeMap::new(),
        }
    }

    /// An archive holding `(path, contents, executable)` files.
    #[must_use]
    pub fn with_archive(
        mut self,
        path: &str,
        files: &[(&'static str, &'static str, bool)],
    ) -> Self {
        let entries = files
            .iter()
            .map(|&(path, contents, executable)| Entry {
                path,
                contents,
                executable,
            })
            .collect();
        self.archives.insert(PathBuf::from(path), entries);
        self
    }
}

impl Archive for FakeArchive<'_> {
    fn extract(&self, archive: &Path, destination: &Path) -> io::Result<()> {
        let entries = self
            .archives
            .get(archive)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        for entry in entries {
            let target = destination.join(entry.path);
            self.fs.write_file(&target, entry.contents)?;
            if entry.executable {
                self.fs.set_executable(&target);
            }
        }
        Ok(())
    }
}

```

Apply to `src/fakes/file_system/mod.rs` (above the test module):

```diff
--- a/src/fakes/file_system/mod.rs
+++ b/src/fakes/file_system/mod.rs
@@ -28,6 +28,11 @@
     pub fn with_executable(mut self, path: &str, contents: &str) -> Self {
         self.executables.get_mut().insert(PathBuf::from(path));
         self.with_file(path, contents)
+    }
+
+    /// Gives an existing file the execute permission.
+    pub fn set_executable(&self, path: &Path) {
+        self.executables.borrow_mut().insert(path.to_path_buf());
     }
 
     /// When the file or directory was last changed.
```

Apply to `src/fakes/mod.rs` (above the test module):

```diff
--- a/src/fakes/mod.rs
+++ b/src/fakes/mod.rs
@@ -1,5 +1,6 @@
 //! In-memory implementations of the ports, for unit tests only.
 
+mod archive;
 mod digest;
 mod env;
 mod file_system;
@@ -7,6 +8,7 @@
 mod process;
 mod sleeper;
 
+pub use archive::FakeArchive;
 pub use digest::FakeDigest;
 pub use env::FakeEnv;
 pub use file_system::FakeFileSystem;
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -131,6 +131,16 @@
     fn sha256_file(&self, path: &Path) -> io::Result<String>;
 }
 
+pub trait Archive {
+    /// Unpacks the `.tar.gz` at `archive` into the existing directory
+    /// `destination`, keeping every path inside it.
+    ///
+    /// # Errors
+    /// Propagates the underlying I/O error, and fails on a file that is not a
+    /// gzip-compressed tar.
+    fn extract(&self, archive: &Path, destination: &Path) -> io::Result<()>;
+}
+
 pub trait Sleeper {
     /// Waits for `duration` (between retries).
     fn sleep(&self, duration: Duration);
```

Then run `cargo fmt`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 379 unit tests plus the end-to-end tests.

- [ ] **Step 7: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
rustup run 1.88 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings, no advisories and the crate
still builds on MSRV 1.88.

```bash
git add src tests Cargo.toml Cargo.lock
git commit -S -m "feat(adapters): unpack a tar.gz without letting an entry leave the destination"
```

---

### Task 5: Downloading bytes

**Files:**

- Modify: `src/ports/mod.rs`, `src/adapters/ureq_http.rs`,
  `src/adapters/retrying_http.rs`, `src/adapters/no_http.rs`,
  `src/fakes/http.rs`

**Interfaces:**

- Produces: `Http::get_bytes(&self, url) -> Result<Vec<u8>, HttpError>` next to
  `get_text`; `UreqHttp` reads up to 1 GiB (the index is still capped at 16 MiB),
  `RetryingHttp` retries it exactly as it retries text, `NoHttp` refuses it,
  and `FakeHttp::with_bytes(url, &[u8])` serves it (`get_text` of non-text bytes
  is a `Body` error).

- [ ] **Step 1: Write the failing tests**

Apply to the test module of `src/adapters/retrying_http.rs`:

```diff
--- a/src/adapters/retrying_http.rs
+++ b/src/adapters/retrying_http.rs
@@ -24,6 +24,10 @@
                 })
             })
         }
+
+        fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError> {
+            self.get_text(url).map(String::into_bytes)
+        }
     }
 
     fn status(code: u16) -> Result<String, HttpError> {
@@ -38,6 +42,15 @@
         let sleeper = FakeSleeper::default();
         let result = RetryingHttp::new(&inner, &sleeper).get_text("u");
         (result, sleeper.slept())
+    }
+
+    #[test]
+    fn bytes_are_retried_like_text() {
+        let inner = Scripted::new(vec![status(502), Ok("tar".to_owned())]);
+        let sleeper = FakeSleeper::default();
+        let body = RetryingHttp::new(&inner, &sleeper).get_bytes("u");
+        assert_eq!(body.unwrap(), b"tar");
+        assert_eq!(sleeper.slept(), [Duration::from_millis(250)]);
     }
 
     #[test]
```

Apply to the test module of `src/adapters/ureq_http.rs`:

```diff
--- a/src/adapters/ureq_http.rs
+++ b/src/adapters/ureq_http.rs
@@ -123,6 +123,13 @@
     }
 
     #[test]
+    fn bytes_that_are_not_text_are_fetched_whole() {
+        let base = serve_bytes(reply_with(&[0xff, 0x00, 0xfd], 3));
+        let body = UreqHttp::unproxied(None).get_bytes(&format!("{base}/a.tgz"));
+        assert_eq!(body.unwrap(), [0xff, 0x00, 0xfd]);
+    }
+
+    #[test]
     fn a_body_over_the_limit_is_a_body_error() {
         let size = usize::try_from(MAX_BODY_BYTES).unwrap() + 1024;
         let base = serve_bytes(reply_with(&vec![b'a'; size], size));
```

Apply to the test module of `src/fakes/http.rs`:

```diff
--- a/src/fakes/http.rs
+++ b/src/fakes/http.rs
@@ -1,6 +1,14 @@
 #[cfg(test)]
 mod tests {
     use super::*;
+
+    #[test]
+    fn fake_http_serves_bytes_and_refuses_to_read_them_as_text_when_they_are_not() {
+        let http = FakeHttp::default().with_bytes("http://m/a.tgz", &[0, 255]);
+        assert_eq!(http.get_bytes("http://m/a.tgz").unwrap(), [0, 255]);
+        let error = http.get_text("http://m/a.tgz").unwrap_err();
+        assert!(matches!(error, HttpError::Body { .. }));
+    }
 
     #[test]
     fn fake_http_answers_by_url_and_records_requests() {
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "no method named `get_bytes`
found" and "not all trait items implemented, missing: `get_bytes`".

- [ ] **Step 3: Write the implementation**

Apply to `src/adapters/no_http.rs` (above the test module):

```diff
--- a/src/adapters/no_http.rs
+++ b/src/adapters/no_http.rs
@@ -3,12 +3,20 @@
 /// The `Http` of a `Context` that was not given one: it fetches nothing.
 pub struct NoHttp;
 
-impl Http for NoHttp {
-    fn get_text(&self, url: &str) -> Result<String, HttpError> {
-        Err(HttpError::Transport {
-            url: url.to_owned(),
-            message: "network access is not available".to_owned(),
-        })
+fn unavailable(url: &str) -> HttpError {
+    HttpError::Transport {
+        url: url.to_owned(),
+        message: "network access is not available".to_owned(),
     }
 }
 
+impl Http for NoHttp {
+    fn get_text(&self, url: &str) -> Result<String, HttpError> {
+        Err(unavailable(url))
+    }
+
+    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError> {
+        Err(unavailable(url))
+    }
+}
+
```

Apply to `src/adapters/retrying_http.rs` (above the test module):

```diff
--- a/src/adapters/retrying_http.rs
+++ b/src/adapters/retrying_http.rs
@@ -41,12 +41,12 @@
     }
 }
 
-impl Http for RetryingHttp<'_> {
-    fn get_text(&self, url: &str) -> Result<String, HttpError> {
+impl RetryingHttp<'_> {
+    fn retrying<T>(&self, request: impl Fn() -> Result<T, HttpError>) -> Result<T, HttpError> {
         let mut delay = self.base_delay;
         let mut attempt = 1;
         loop {
-            match self.inner.get_text(url) {
+            match request() {
                 Err(error) if is_transient(&error) && attempt < self.attempts => {
                     self.sleeper.sleep(delay);
                     delay = delay.saturating_mul(2);
@@ -58,3 +58,13 @@
     }
 }
 
+impl Http for RetryingHttp<'_> {
+    fn get_text(&self, url: &str) -> Result<String, HttpError> {
+        self.retrying(|| self.inner.get_text(url))
+    }
+
+    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError> {
+        self.retrying(|| self.inner.get_bytes(url))
+    }
+}
+
```

Apply to `src/adapters/ureq_http.rs` (above the test module):

```diff
--- a/src/adapters/ureq_http.rs
+++ b/src/adapters/ureq_http.rs
@@ -9,6 +9,8 @@
 
 const TIMEOUT: Duration = Duration::from_secs(30);
 const MAX_BODY_BYTES: u64 = 16 * 1024 * 1024;
+/// An archive is far bigger than an index, but never this big.
+const MAX_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024;
 
 pub struct UreqHttp {
     agent: ureq::Agent,
@@ -77,18 +79,20 @@
     }
 }
 
-impl Http for UreqHttp {
-    fn get_text(&self, url: &str) -> Result<String, HttpError> {
+impl UreqHttp {
+    /// Sends the request and reads the body with `read`, capped at `limit`.
+    fn fetch<T>(
+        &self,
+        url: &str,
+        limit: u64,
+        read: impl FnOnce(ureq::BodyWithConfig<'_>) -> Result<T, ureq::Error>,
+    ) -> Result<T, HttpError> {
         let mut request = self.agent.get(url);
         if let Some(value) = &self.auth_header {
             request = request.header("Authorization", value);
         }
         match request.call() {
-            Ok(mut response) => response
-                .body_mut()
-                .with_config()
-                .limit(MAX_BODY_BYTES)
-                .read_to_string()
+            Ok(mut response) => read(response.body_mut().with_config().limit(limit))
                 .map_err(|error| read_failure(url, error)),
             Err(ureq::Error::StatusCode(code)) => Err(HttpError::Status {
                 url: url.to_owned(),
@@ -99,3 +103,13 @@
     }
 }
 
+impl Http for UreqHttp {
+    fn get_text(&self, url: &str) -> Result<String, HttpError> {
+        self.fetch(url, MAX_BODY_BYTES, |body| body.read_to_string())
+    }
+
+    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError> {
+        self.fetch(url, MAX_DOWNLOAD_BYTES, |body| body.read_to_vec())
+    }
+}
+
```

Apply to `src/fakes/http.rs` (above the test module):

```diff
--- a/src/fakes/http.rs
+++ b/src/fakes/http.rs
@@ -7,14 +7,19 @@
 /// every request is recorded.
 #[derive(Default)]
 pub struct FakeHttp {
-    responses: BTreeMap<String, Result<String, HttpError>>,
+    responses: BTreeMap<String, Result<Vec<u8>, HttpError>>,
     requests: RefCell<Vec<String>>,
 }
 
 impl FakeHttp {
     #[must_use]
-    pub fn with_body(mut self, url: &str, body: &str) -> Self {
-        self.responses.insert(url.to_owned(), Ok(body.to_owned()));
+    pub fn with_body(self, url: &str, body: &str) -> Self {
+        self.with_bytes(url, body.as_bytes())
+    }
+
+    #[must_use]
+    pub fn with_bytes(mut self, url: &str, body: &[u8]) -> Self {
+        self.responses.insert(url.to_owned(), Ok(body.to_vec()));
         self
     }
 
@@ -37,6 +42,14 @@
 
 impl Http for FakeHttp {
     fn get_text(&self, url: &str) -> Result<String, HttpError> {
+        let bytes = self.get_bytes(url)?;
+        String::from_utf8(bytes).map_err(|error| HttpError::Body {
+            url: url.to_owned(),
+            message: error.to_string(),
+        })
+    }
+
+    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError> {
         self.requests.borrow_mut().push(url.to_owned());
         self.responses.get(url).cloned().unwrap_or_else(|| {
             Err(HttpError::Transport {
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -115,6 +115,13 @@
 }
 
 pub trait Http {
+    /// Fetches `url` and returns the body as it is (an archive, say).
+    ///
+    /// # Errors
+    /// Fails on a non-success status, on a network error, or when the body is
+    /// over the size limit.
+    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError>;
+
     /// Fetches `url` and returns the body as text.
     ///
     /// # Errors
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 382 unit tests plus the end-to-end tests.

- [ ] **Step 5: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(http): download bytes as well as text"
```

---

### Task 6: The install lock and the sleeper

**Files:**

- Create: `src/commands/install/mod.rs`, `src/commands/install/lock/mod.rs`,
  `src/commands/install/lock/tests.rs`, `src/adapters/no_sleeper.rs`
- Modify: `src/commands/mod.rs`, `src/adapters/mod.rs`, `src/context/mod.rs`
  and `src/context/tests.rs` (`src/context.rs` becomes a directory module
  first, Step 0)

**Interfaces:**

- Consumes: `FileSystem::{create_dir, create_dir_all, file_info,
  remove_dir_all}`, `ports::Sleeper`.
- Produces: `commands::install::lock::{LockRequest, InstallLock, acquire}`;
  `acquire(&dyn FileSystem, &dyn Sleeper, &LockRequest, &mut Vec<String>) ->
  Result<Option<InstallLock>, CliError>`; `InstallLock` removes its directory
  when dropped; `adapters::no_sleeper::NoSleeper`; `Context::with_sleeper(&dyn
  Sleeper)` and `Context::sleeper()`.
- Behaviour (from `nvm_acquire_install_lock`): the lock is the directory
  `$NVM_DIR/.cache/locks/<version>` (characters other than letters, digits and
  `._+-` become `_`); a held lock is waited for a second at a time, announced
  once with `Waiting for another install of <v> to finish...`; one older than
  `NVM_INSTALL_LOCK_STALE` minutes (0, the default, never) is stolen with
  `Removing stale install lock for <v> (older than N minute(s))`; after
  `NVM_INSTALL_LOCK_TIMEOUT` seconds (600) the error is `Timed out after <N>s
  waiting for another install of <v> to finish.` and `If no other install is
  running, remove <lock> and try again.` (exit 1). A lock directory that cannot
  be created at all does not stop the install.
- [ ] **Step 0: Split the tests out of `context.rs` (no behaviour change)**

`src/context.rs` would pass 300 lines in this task. Run:

```bash
mkdir src/context && git mv src/context.rs src/context/mod.rs
```

Cut everything inside the `mod tests { ... }` block of `src/context/mod.rs`,
remove one level of indentation from it, and paste it into the new file
`src/context/tests.rs` (it starts with `use super::*;`). Replace the block in
`src/context/mod.rs` with:

```rust
#[cfg(test)]
mod tests;
```

Run: `cargo fmt && cargo test`
Expected: PASS, 382 unit tests plus the end-to-end tests, as at the end of
Task 5.

- [ ] **Step 1: Declare the new modules**

Apply to `src/adapters/mod.rs`:

```diff
--- a/src/adapters/mod.rs
+++ b/src/adapters/mod.rs
@@ -3,6 +3,7 @@
 pub mod no_digest;
 pub mod no_http;
 pub mod no_process;
+pub mod no_sleeper;
 pub mod retrying_http;
 pub mod sha256_digest;
 pub mod std_env;
```

Create `src/commands/install/mod.rs`:

```rust
//! `nvm install`.

pub mod lock;
```

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -2,6 +2,7 @@
 pub mod aliases;
 pub mod cache;
 pub mod current;
+pub mod install;
 pub mod ls;
 pub mod ls_remote;
 pub mod remote_index;
```

- [ ] **Step 2: Write the failing tests**

Create `src/adapters/no_sleeper.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_at_once() {
        NoSleeper.sleep(Duration::from_secs(3600));
    }
}
```

Create `src/commands/install/lock/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/install/lock/tests.rs`:

```rust
use std::cell::Cell;

use super::*;
use crate::fakes::{FakeFileSystem, FakeSleeper};

const ROOT: &str = "/n/.cache/locks";

fn request(stale_minutes: u64, timeout_seconds: u64) -> LockRequest<'static> {
    LockRequest {
        root: Path::new(ROOT),
        version: "v20.10.0",
        timeout_seconds,
        stale_minutes,
        now: SystemTime::UNIX_EPOCH + Duration::from_secs(10_000),
    }
}

fn exists(fs: &FakeFileSystem, name: &str) -> bool {
    fs.file_info(&Path::new(ROOT).join(name)).is_ok()
}

/// Lets go of the lock once it has been asked to sleep `after` times.
struct ReleasesAfter<'a> {
    fs: &'a FakeFileSystem,
    after: u32,
    slept: Cell<u32>,
}

impl Sleeper for ReleasesAfter<'_> {
    fn sleep(&self, _duration: Duration) {
        self.slept.set(self.slept.get() + 1);
        if self.slept.get() == self.after {
            self.fs
                .remove_dir_all(&Path::new(ROOT).join("v20.10.0"))
                .unwrap();
        }
    }
}

#[test]
fn a_free_lock_is_taken_at_once_and_released_when_dropped() {
    let fs = FakeFileSystem::default();
    let sleeper = FakeSleeper::default();
    let mut notes = Vec::new();
    let lock = acquire(&fs, &sleeper, &request(0, 600), &mut notes).unwrap();
    assert!(exists(&fs, "v20.10.0"));
    assert!(notes.is_empty() && sleeper.slept().is_empty());
    drop(lock);
    assert!(!exists(&fs, "v20.10.0"));
}

#[test]
fn the_name_of_the_lock_keeps_only_safe_characters() {
    assert_eq!(lock_name("iojs-v3.3.1"), "iojs-v3.3.1");
    assert_eq!(lock_name("v1/../2 x"), "v1_.._2_x");
}

#[test]
fn a_held_lock_is_waited_for_a_second_at_a_time_and_announced_once() {
    let fs = FakeFileSystem::default().with_dir("/n/.cache/locks/v20.10.0");
    let sleeper = ReleasesAfter {
        fs: &fs,
        after: 3,
        slept: Cell::new(0),
    };
    let mut notes = Vec::new();
    let lock = acquire(&fs, &sleeper, &request(0, 600), &mut notes).unwrap();
    assert!(lock.is_some());
    assert_eq!(sleeper.slept.get(), 3);
    assert_eq!(
        notes,
        ["Waiting for another install of v20.10.0 to finish..."]
    );
}

#[test]
fn a_lock_that_stays_held_times_out_with_both_messages() {
    let fs = FakeFileSystem::default().with_dir("/n/.cache/locks/v20.10.0");
    let sleeper = FakeSleeper::default();
    let error = acquire(&fs, &sleeper, &request(0, 2), &mut Vec::new())
        .err()
        .unwrap();
    assert_eq!(
        error.to_string(),
        "Timed out after 2s waiting for another install of v20.10.0 to finish.\n\
         If no other install is running, remove /n/.cache/locks/v20.10.0 and try again."
    );
    assert_eq!(sleeper.slept().len(), 2);
}

#[test]
fn an_old_lock_is_stolen_when_a_stale_age_is_set() {
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000 - 11 * 60);
    let fs = FakeFileSystem::default()
        .with_dir("/n/.cache/locks/v20.10.0")
        .with_modified("/n/.cache/locks/v20.10.0", old);
    let mut notes = Vec::new();
    let lock = acquire(&fs, &FakeSleeper::default(), &request(10, 600), &mut notes).unwrap();
    assert!(lock.is_some());
    assert_eq!(
        notes,
        ["Removing stale install lock for v20.10.0 (older than 10 minute(s))"]
    );
}

#[test]
fn a_recent_lock_is_not_stolen_and_stale_zero_never_steals() {
    let recent = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000 - 60);
    let held = || {
        FakeFileSystem::default()
            .with_dir("/n/.cache/locks/v20.10.0")
            .with_modified("/n/.cache/locks/v20.10.0", recent)
    };
    for stale in [10, 0] {
        let fs = held();
        let result = acquire(
            &fs,
            &FakeSleeper::default(),
            &request(stale, 1),
            &mut Vec::new(),
        );
        assert!(result.is_err(), "stale = {stale}");
    }
}
```

Apply to `src/context/tests.rs`:

```diff
--- a/src/context/tests.rs
+++ b/src/context/tests.rs
@@ -1,5 +1,5 @@
 use super::*;
-use crate::fakes::{FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp};
+use crate::fakes::{FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeSleeper};
 
 fn nvm_dir_for(env: &FakeEnv) -> Result<PathBuf, CliError> {
     let fs = FakeFileSystem::default();
@@ -109,6 +109,18 @@
 }
 
 #[test]
+fn a_context_uses_the_sleeper_it_is_given() {
+    let (fs, env, sleeper) = (
+        FakeFileSystem::default(),
+        FakeEnv::default(),
+        FakeSleeper::default(),
+    );
+    let context = Context::new(&fs, &env).with_sleeper(&sleeper);
+    context.sleeper().sleep(std::time::Duration::from_secs(1));
+    assert_eq!(sleeper.slept().len(), 1);
+}
+
+#[test]
 fn alias_dir_is_under_nvm_dir() {
     let fs = FakeFileSystem::default();
     let env = FakeEnv::default().with_var("NVM_DIR", "/n");
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function
`acquire`", "cannot find struct `LockRequest`" and "no method named
`with_sleeper` found".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/adapters/no_sleeper.rs`:

```rust
use std::time::Duration;

use crate::ports::Sleeper;

/// The `Sleeper` of a `Context` that was not given one: it never waits.
pub struct NoSleeper;

impl Sleeper for NoSleeper {
    fn sleep(&self, _duration: Duration) {}
}

```

Insert above the `#[cfg(test)]` line of `src/commands/install/lock/mod.rs`:

```rust
//! The per-version install lock of `nvm_acquire_install_lock`: a directory
//! under `$NVM_DIR/.cache/locks`, created atomically, so two installs of one
//! version never work on its directory at once.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::error::CliError;
use crate::ports::{FileSystem, Sleeper};

const SECONDS_PER_MINUTE: u64 = 60;

pub struct LockRequest<'a> {
    /// `$NVM_DIR/.cache/locks`.
    pub root: &'a Path,
    /// The version being installed: its characters other than letters,
    /// digits and `._+-` become `_` in the lock's name.
    pub version: &'a str,
    pub timeout_seconds: u64,
    /// A lock older than this many minutes is stolen; 0 never steals.
    pub stale_minutes: u64,
    pub now: SystemTime,
}

/// A held lock, released when dropped.
pub struct InstallLock<'a> {
    fs: &'a dyn FileSystem,
    path: PathBuf,
}

impl Drop for InstallLock<'_> {
    fn drop(&mut self) {
        // Nothing can be done about a lock that cannot be removed.
        let _ = self.fs.remove_dir_all(&self.path);
    }
}

fn lock_name(version: &str) -> String {
    version
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._+-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn is_stale(fs: &dyn FileSystem, path: &Path, request: &LockRequest<'_>, waited: u64) -> bool {
    if request.stale_minutes == 0 {
        return false;
    }
    let age = fs
        .file_info(path)
        .ok()
        .and_then(|info| info.modified)
        .and_then(|modified| {
            let now = request.now + Duration::from_secs(waited);
            now.duration_since(modified).ok()
        });
    age.is_some_and(|age| age > Duration::from_secs(request.stale_minutes * SECONDS_PER_MINUTE))
}

/// Takes the lock, waiting a second at a time for another install to finish.
/// `None` when the lock directory cannot be created at all: `nvm.sh` does not
/// let that stop an install. `notes` collects what `nvm.sh` prints on stderr.
///
/// # Errors
/// [`CliError::InvalidArgument`] (exit 1) when the lock is still held after
/// `timeout_seconds`.
pub fn acquire<'a>(
    fs: &'a dyn FileSystem,
    sleeper: &dyn Sleeper,
    request: &LockRequest<'_>,
    notes: &mut Vec<String>,
) -> Result<Option<InstallLock<'a>>, CliError> {
    if fs.create_dir_all(request.root).is_err() {
        return Ok(None);
    }
    let path = request.root.join(lock_name(request.version));
    let mut waited = 0;
    loop {
        match fs.create_dir(&path) {
            Ok(()) => return Ok(Some(InstallLock { fs, path })),
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Ok(None),
            Err(_) => {}
        }
        if is_stale(fs, &path, request, waited) {
            notes.push(format!(
                "Removing stale install lock for {} (older than {} minute(s))",
                request.version, request.stale_minutes
            ));
            let _ = fs.remove_dir_all(&path);
            continue;
        }
        if waited >= request.timeout_seconds {
            return Err(timed_out(request, &path));
        }
        if waited == 0 {
            notes.push(format!(
                "Waiting for another install of {} to finish...",
                request.version
            ));
        }
        sleeper.sleep(Duration::from_secs(1));
        waited += 1;
    }
}

fn timed_out(request: &LockRequest<'_>, path: &Path) -> CliError {
    CliError::InvalidArgument(format!(
        "Timed out after {}s waiting for another install of {} to finish.\n\
         If no other install is running, remove {} and try again.",
        request.timeout_seconds,
        request.version,
        path.display()
    ))
}

```

Apply to `src/context/mod.rs` (above the test module):

```diff
--- a/src/context/mod.rs
+++ b/src/context/mod.rs
@@ -7,10 +7,11 @@
 use crate::adapters::no_digest::NoDigest;
 use crate::adapters::no_http::NoHttp;
 use crate::adapters::no_process::NoProcess;
+use crate::adapters::no_sleeper::NoSleeper;
 use crate::domain::alias::AliasStore;
 use crate::domain::version::Version;
 use crate::error::CliError;
-use crate::ports::{Archive, Digest, Env, FileSystem, Http, Process};
+use crate::ports::{Archive, Digest, Env, FileSystem, Http, Process, Sleeper};
 
 pub struct Context<'a> {
     pub fs: &'a dyn FileSystem,
@@ -19,6 +20,7 @@
     http: &'a dyn Http,
     digest: &'a dyn Digest,
     archive: &'a dyn Archive,
+    sleeper: &'a dyn Sleeper,
 }
 
 impl<'a> Context<'a> {
@@ -33,6 +35,7 @@
             http: &NoHttp,
             digest: &NoDigest,
             archive: &NoArchive,
+            sleeper: &NoSleeper,
         }
     }
 
@@ -45,6 +48,17 @@
     #[must_use]
     pub fn process(&self) -> &dyn Process {
         self.process
+    }
+
+    #[must_use]
+    pub fn with_sleeper(mut self, sleeper: &'a dyn Sleeper) -> Self {
+        self.sleeper = sleeper;
+        self
+    }
+
+    #[must_use]
+    pub fn sleeper(&self) -> &dyn Sleeper {
+        self.sleeper
     }
 
     #[must_use]
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 390 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(install): add the per-version install lock and a sleeper for the context"
```

---

### Task 7: The foundations of `install`

**Files:**

- Create: `src/commands/transcript.rs`
- Modify: `src/error.rs`, `src/domain/mirror.rs`, `src/context/mod.rs`,
  `src/context/tests.rs`, `src/commands/mod.rs`,
  `src/commands/version_remote/mod.rs`, `src/commands/version_remote/tests.rs`,
  `src/commands/aliases/mod.rs`, `src/commands/aliases/tests.rs`

**Interfaces:**

- Produces: `NvmExitCode::MissingTarget` (2, the old `NoSuchAlias`) and
  `NvmExitCode::InvalidOptions` (6) with `CliError::InvalidOptions(String)`;
  `MirrorUrl::join(&str)` (`<mirror>/<path>`); `Context::with_platform(Option<
  Platform>)` and `Context::platform()`; `commands::version_remote::{Lookup,
  lookup}`, which `version-remote` and `install` share
  (`lookup(&Context, Query) -> Result<Lookup { version, warnings }, CliError>`);
  `commands::transcript::Transcript` (`out`, `err`, `absorb(Output)`,
  `finish(status) -> Output`), what a command prints while it works, so a
  failure can still show everything before it.

- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -7,6 +7,7 @@
 pub mod ls_remote;
 pub mod remote_index;
 pub mod resolve;
+pub mod transcript;
 pub mod unalias;
 pub mod version;
 pub mod version_remote;
```

- [ ] **Step 2: Write the failing tests**

Apply to `src/commands/aliases/tests.rs`:

```diff
--- a/src/commands/aliases/tests.rs
+++ b/src/commands/aliases/tests.rs
@@ -96,7 +96,7 @@
     let fs = fixture();
     let expected = Output::default()
         .with_stderr("Alias does not exist.")
-        .with_status(NvmExitCode::NoSuchAlias);
+        .with_status(NvmExitCode::MissingTarget);
     assert_eq!(list_with(&fs, Some("lts/nope")), expected);
     assert_eq!(list_with(&fs, Some("lts/")), expected);
 }
```

Create `src/commands/transcript.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_keeps_the_lines_of_each_stream_in_order_and_ends_with_a_status() {
        let mut transcript = Transcript::default();
        transcript.out("one");
        transcript.err("warning");
        transcript.out("two");
        let output = transcript.finish(NvmExitCode::MissingTarget);
        assert_eq!(output.stdout, "one\ntwo");
        assert_eq!(output.stderr, "warning");
        assert_eq!(output.status, NvmExitCode::MissingTarget);
    }

    #[test]
    fn absorbing_an_output_adds_its_non_empty_streams() {
        let mut transcript = Transcript::default();
        transcript.absorb(Output::stdout("a -> b").with_stderr("careful"));
        transcript.absorb(Output::default());
        let output = transcript.finish(NvmExitCode::Success);
        assert_eq!(
            (output.stdout.as_str(), output.stderr.as_str()),
            ("a -> b", "careful")
        );
    }
}
```

Apply to `src/commands/version_remote/tests.rs`:

```diff
--- a/src/commands/version_remote/tests.rs
+++ b/src/commands/version_remote/tests.rs
@@ -110,3 +110,26 @@
     assert_eq!(output.stdout, "N/A");
     assert_eq!(output.status, NvmExitCode::InvalidVersion);
 }
+
+#[test]
+fn lookup_resolves_a_query_and_carries_the_warnings() {
+    let fs = FakeFileSystem::default();
+    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
+    let http = mirror();
+    let context = Context::new(&fs, &env).with_http(&http);
+    let query = |pattern: Option<&str>, lts: Option<&str>| Query {
+        pattern: pattern.map(str::to_owned),
+        lts: lts.map(str::to_owned),
+    };
+    let found = lookup(&context, query(Some("18"), None)).unwrap();
+    assert_eq!(found.version.unwrap().to_string(), "v18.19.0");
+    let lts = lookup(&context, query(None, Some("*"))).unwrap();
+    assert_eq!(lts.version.unwrap().to_string(), "v20.10.0");
+    let bad = lookup(&context, query(None, Some("Iron"))).unwrap();
+    assert_eq!(bad.version, None);
+    assert_eq!(bad.warnings, ["LTS names must be lowercase"]);
+    assert_eq!(
+        lookup(&context, query(Some("99"), None)).unwrap().version,
+        None
+    );
+}
```

Apply to `src/context/tests.rs`:

```diff
--- a/src/context/tests.rs
+++ b/src/context/tests.rs
@@ -1,4 +1,5 @@
 use super::*;
+use crate::domain::platform::Platform;
 use crate::fakes::{FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeSleeper};
 
 fn nvm_dir_for(env: &FakeEnv) -> Result<PathBuf, CliError> {
@@ -121,6 +122,17 @@
 }
 
 #[test]
+fn a_context_is_linux_x64_until_told_otherwise() {
+    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
+    let context = Context::new(&fs, &env);
+    assert_eq!(context.platform().map(|p| p.arch.as_str()), Some("x64"));
+    let mac = Platform::from_host("macos", "aarch64", false);
+    let context = context.with_platform(mac.clone());
+    assert_eq!(context.platform(), mac.as_ref());
+    assert_eq!(context.with_platform(None).platform(), None);
+}
+
+#[test]
 fn alias_dir_is_under_nvm_dir() {
     let fs = FakeFileSystem::default();
     let env = FakeEnv::default().with_var("NVM_DIR", "/n");
```

Apply to the test module of `src/domain/mirror.rs`:

```diff
--- a/src/domain/mirror.rs
+++ b/src/domain/mirror.rs
@@ -56,6 +56,15 @@
     }
 
     #[test]
+    fn paths_are_joined_with_one_slash_after_the_mirror() {
+        let mirror = MirrorUrl::parse("http://x/dist").unwrap();
+        assert_eq!(
+            mirror.join("v1.0.0/f.tar.gz"),
+            "http://x/dist/v1.0.0/f.tar.gz"
+        );
+    }
+
+    #[test]
     fn the_defaults_are_nodejs_org_and_iojs_org() {
         let env = FakeEnv::default();
         let node = from_env(&env, Flavor::Node).unwrap();
```

Apply to the test module of `src/error.rs`:

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -8,7 +8,8 @@
         assert_eq!(NvmExitCode::InvalidVersion.code(), 3);
         assert_eq!(NvmExitCode::BelowVersionFloor.code(), 7);
         assert_eq!(NvmExitCode::AliasLoop.code(), 8);
-        assert_eq!(NvmExitCode::NoSuchAlias.code(), 2);
+        assert_eq!(NvmExitCode::MissingTarget.code(), 2);
+        assert_eq!(NvmExitCode::InvalidOptions.code(), 6);
         assert_eq!(NvmExitCode::UnsupportedOption.code(), 55);
         assert_eq!(NvmExitCode::NotFound.code(), 127);
     }
@@ -40,6 +41,8 @@
         assert_eq!(no_system_node.exit_code(), NvmExitCode::NotFound);
         let unsupported = CliError::Unsupported("x".into());
         assert_eq!(unsupported.exit_code(), NvmExitCode::UnsupportedOption);
+        let options = CliError::InvalidOptions("x".into());
+        assert_eq!(options.exit_code(), NvmExitCode::InvalidOptions);
         let invalid = CliError::InvalidArgument("x".into());
         assert_eq!(invalid.exit_code(), NvmExitCode::Failure);
         let source = std::io::Error::from(std::io::ErrorKind::NotFound);
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "no variant named
`InvalidOptions` found", "cannot find function `lookup`", "no method named
`join` found for struct `MirrorUrl`" and "cannot find struct `Transcript`".

- [ ] **Step 4: Write the implementation**

Apply to `src/commands/aliases/mod.rs` (above the test module):

```diff
--- a/src/commands/aliases/mod.rs
+++ b/src/commands/aliases/mod.rs
@@ -85,7 +85,7 @@
         Some(target) => Ok(Output::stdout(target)),
         None => Ok(Output::default()
             .with_stderr("Alias does not exist.")
-            .with_status(NvmExitCode::NoSuchAlias)),
+            .with_status(NvmExitCode::MissingTarget)),
     }
 }
 
```

Insert above the `#[cfg(test)]` line of `src/commands/transcript.rs`:

```rust
//! What a command prints while it works, kept apart so a failure can still
//! show everything that came before it.

use crate::commands::Output;
use crate::error::NvmExitCode;

#[derive(Debug, Default)]
pub struct Transcript {
    stdout: Vec<String>,
    stderr: Vec<String>,
}

impl Transcript {
    pub fn out(&mut self, line: impl Into<String>) {
        self.stdout.push(line.into());
    }

    pub fn err(&mut self, line: impl Into<String>) {
        self.stderr.push(line.into());
    }

    /// Adds the stdout and stderr of an [`Output`] a command returned.
    pub fn absorb(&mut self, output: Output) {
        self.stdout
            .extend(Some(output.stdout).filter(|text| !text.is_empty()));
        self.stderr
            .extend(Some(output.stderr).filter(|text| !text.is_empty()));
    }

    #[must_use]
    pub fn finish(self, status: NvmExitCode) -> Output {
        Output::stdout(self.stdout.join("\n"))
            .with_stderr(self.stderr.join("\n"))
            .with_status(status)
    }
}

```

Apply to `src/commands/version_remote/mod.rs` (above the test module):

```diff
--- a/src/commands/version_remote/mod.rs
+++ b/src/commands/version_remote/mod.rs
@@ -11,6 +11,7 @@
 use crate::domain::remote::resolve::resolve;
 use crate::domain::remote::{Query, scope};
 use crate::domain::version::Flavor;
+use crate::domain::version::Version;
 use crate::error::{CliError, NvmExitCode};
 
 /// The command line of `nvm version-remote`, as `nvm.sh` reads it: the first
@@ -70,15 +71,22 @@
         .with_status(NvmExitCode::InvalidVersion)
 }
 
+/// What the mirror answered to a description of a version.
+pub struct Lookup {
+    /// `None` is `N/A`: nothing matched, or an index or `--lts` name was
+    /// unusable.
+    pub version: Option<Version>,
+    /// What `nvm.sh` prints on stderr meanwhile.
+    pub warnings: Vec<String>,
+}
+
+/// Downloads what `query` needs from the mirrors and resolves it, as
+/// `nvm_remote_version` does. `query.lts` is the raw `--lts` value.
+///
 /// # Errors
-/// As [`parse_options`], and [`CliError::NvmDirUnresolved`] when `$NVM_DIR`
-/// cannot be found.
-pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
-    let options = parse_options(args)?;
-    let mut query = Query {
-        pattern: options.pattern,
-        lts: options.lts.filter(|text| !text.is_empty()),
-    };
+/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
+pub fn lookup(context: &Context<'_>, mut query: Query) -> Result<Lookup, CliError> {
+    query.lts = query.lts.filter(|text| !text.is_empty());
     let (node_runs, iojs_runs) = needs(&query);
     let mut warnings = Vec::new();
     let node = fetch_if(context, node_runs, Flavor::Node, &mut warnings)?;
@@ -87,12 +95,29 @@
             Ok(name) => query.lts = Some(name),
             Err(message) => {
                 warnings.push(message);
-                return Ok(not_available(&warnings));
+                return Ok(Lookup {
+                    version: None,
+                    warnings,
+                });
             }
         }
     }
     let iojs = fetch_if(context, iojs_runs, Flavor::IoJs, &mut warnings)?;
-    match resolve(node.as_deref(), iojs.as_deref(), &query) {
+    let version = resolve(node.as_deref(), iojs.as_deref(), &query);
+    Ok(Lookup { version, warnings })
+}
+
+/// # Errors
+/// As [`parse_options`], and [`CliError::NvmDirUnresolved`] when `$NVM_DIR`
+/// cannot be found.
+pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
+    let options = parse_options(args)?;
+    let query = Query {
+        pattern: options.pattern,
+        lts: options.lts,
+    };
+    let Lookup { version, warnings } = lookup(context, query)?;
+    match version {
         Some(version) => Ok(Output::stdout(version.to_string()).with_stderr(warnings.join("\n"))),
         None => Ok(not_available(&warnings)),
     }
```

Apply to `src/context/mod.rs` (above the test module):

```diff
--- a/src/context/mod.rs
+++ b/src/context/mod.rs
@@ -9,6 +9,7 @@
 use crate::adapters::no_process::NoProcess;
 use crate::adapters::no_sleeper::NoSleeper;
 use crate::domain::alias::AliasStore;
+use crate::domain::platform::{Os, Platform};
 use crate::domain::version::Version;
 use crate::error::CliError;
 use crate::ports::{Archive, Digest, Env, FileSystem, Http, Process, Sleeper};
@@ -21,6 +22,7 @@
     digest: &'a dyn Digest,
     archive: &'a dyn Archive,
     sleeper: &'a dyn Sleeper,
+    platform: Option<Platform>,
 }
 
 impl<'a> Context<'a> {
@@ -36,6 +38,10 @@
             digest: &NoDigest,
             archive: &NoArchive,
             sleeper: &NoSleeper,
+            platform: Some(Platform {
+                os: Os::Linux,
+                arch: "x64".to_owned(),
+            }),
         }
     }
 
@@ -48,6 +54,19 @@
     #[must_use]
     pub fn process(&self) -> &dyn Process {
         self.process
+    }
+
+    /// The machine the binaries are for; `None` when it has no official ones.
+    /// A context starts out as Linux on x64.
+    #[must_use]
+    pub fn with_platform(mut self, platform: Option<Platform>) -> Self {
+        self.platform = platform;
+        self
+    }
+
+    #[must_use]
+    pub fn platform(&self) -> Option<&Platform> {
+        self.platform.as_ref()
     }
 
     #[must_use]
```

Apply to `src/domain/mirror.rs` (above the test module):

```diff
--- a/src/domain/mirror.rs
+++ b/src/domain/mirror.rs
@@ -38,11 +38,17 @@
         Ok(Self(raw.to_owned()))
     }
 
-    /// The release index: `<mirror>/index.tab`, appended as is (a trailing
-    /// slash in the mirror stays, as in `nvm.sh`).
+    /// `<mirror>/<path>`, appended as is (a trailing slash in the mirror
+    /// stays, as in `nvm.sh`).
+    #[must_use]
+    pub fn join(&self, path: &str) -> String {
+        format!("{}/{path}", self.0)
+    }
+
+    /// The release index: `<mirror>/index.tab`.
     #[must_use]
     pub fn index_url(&self) -> String {
-        format!("{}/index.tab", self.0)
+        self.join("index.tab")
     }
 }
 
```

Apply to `src/error.rs` (above the test module):

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -13,8 +13,11 @@
     InvalidVersion = 3,
     BelowVersionFloor = 7,
     AliasLoop = 8,
-    /// `nvm alias lts/<name>` for an alias that does not exist.
-    NoSuchAlias = 2,
+    /// Something that was asked for does not exist: an `lts/<name>` alias, or
+    /// the archive of a version whose download failed.
+    MissingTarget = 2,
+    /// Options that cannot be combined, or given twice.
+    InvalidOptions = 6,
     /// An option `nvm.sh` does not support, or one used in a combination it
     /// does not support.
     UnsupportedOption = 55,
@@ -75,6 +78,9 @@
     /// An unsupported option, with the message to print.
     #[error("{0}")]
     Unsupported(String),
+    /// Options that cannot be combined, with the message to print.
+    #[error("{0}")]
+    InvalidOptions(String),
     /// A failed file-system change, naming the path it was made on.
     #[error("{}: {source}", path.display())]
     Io {
@@ -104,6 +110,7 @@
             | Self::Io { .. } => NvmExitCode::Failure,
             Self::Usage(_) | Self::SystemNodeNotFound => NvmExitCode::NotFound,
             Self::Unsupported(_) => NvmExitCode::UnsupportedOption,
+            Self::InvalidOptions(_) => NvmExitCode::InvalidOptions,
             Self::Floor(_) => NvmExitCode::BelowVersionFloor,
             Self::Alias(_) => NvmExitCode::AliasLoop,
         }
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 395 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "refactor(commands): share the remote lookup and add the install foundations"
```

---

### Task 8: The options of `install`

**Files:**

- Create: `src/commands/install/options/mod.rs`,
  `src/commands/install/options/tests.rs`
- Modify: `src/commands/install/mod.rs`

**Interfaces:**

- Produces: `commands::install::options::{Options, parse}` with `Options {
  version, lts, alias, version_given, announce_lts }`.
- Behaviour (from `nvm.sh`): options come first, then the version; what follows
  the version is not read. `-b` and `--no-progress` are accepted and do nothing;
  `--lts` is `*`, `--lts=<name>` a codename; `lts/*` or `lts/<name>` as the
  version replace any `--lts`; `--default` and `--alias=<name>` name the alias
  to make, and either given twice, or both, is `--default and --alias are
  mutually exclusive, and may not be provided more than once` (exit 6); `---x`
  is `arguments with `---` are not supported - this is likely a typo` (55); an
  unknown `--x` is taken as the version, as `nvm.sh` does, and so is not found.
  Until Tasks 16 to 20 enable them, the options below are `The option "<x>"
  is not supported yet.` (55): `-s`, `-j`, `--offline`, `--latest-npm`,
  `--reinstall-packages-from`, `--copy-packages-from`,
  `--skip-default-packages`, `--save` and `-w`. Tasks 16 to 20 remove them
  from this list one by one.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/install/mod.rs`:

```diff
--- a/src/commands/install/mod.rs
+++ b/src/commands/install/mod.rs
@@ -1,3 +1,4 @@
 //! `nvm install`.
 
 pub mod lock;
+pub mod options;
```

- [ ] **Step 2: Write the failing tests**

Create `src/commands/install/options/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/install/options/tests.rs`:

```rust
use super::*;
use crate::error::NvmExitCode;

fn parsed(line: &str) -> Result<Options, CliError> {
    let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
    parse(&words)
}

#[test]
fn a_version_is_the_first_word_that_is_not_an_option() {
    let options = parsed("20").unwrap();
    assert_eq!(
        (options.version.as_str(), options.version_given),
        ("20", true)
    );
    assert_eq!(options.lts, None);
    assert_eq!(options.alias, None);
}

#[test]
fn nothing_at_all_is_no_version() {
    let options = parsed("").unwrap();
    assert_eq!(
        (options.version.as_str(), options.version_given),
        ("", false)
    );
}

#[test]
fn lts_options_and_lts_versions_give_the_lts_filter() {
    assert_eq!(parsed("--lts").unwrap().lts.as_deref(), Some("*"));
    assert_eq!(parsed("--lts=iron").unwrap().lts.as_deref(), Some("iron"));
    assert_eq!(parsed("lts/*").unwrap().lts.as_deref(), Some("*"));
    let named = parsed("--lts=gallium lts/iron").unwrap();
    assert_eq!(named.lts.as_deref(), Some("iron"));
    assert_eq!(named.version, "");
}

#[test]
fn default_and_alias_name_the_alias_to_make() {
    assert_eq!(
        parsed("--default 20").unwrap().alias.as_deref(),
        Some("default")
    );
    assert_eq!(
        parsed("--alias=work 20").unwrap().alias.as_deref(),
        Some("work")
    );
}

#[test]
fn default_and_alias_together_or_twice_are_status_6() {
    for line in [
        "--default --alias=x 20",
        "--alias=a --alias=b 20",
        "--default --default 20",
    ] {
        let error = parsed(line).unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::InvalidOptions, "{line}");
        assert_eq!(
            error.to_string(),
            "--default and --alias are mutually exclusive, and may not be provided more than once"
        );
    }
}

#[test]
fn harmless_options_are_accepted() {
    let options = parsed("-b --no-progress 20").unwrap();
    assert_eq!(options.version, "20");
}

#[test]
fn three_dashes_are_a_typo_with_status_55() {
    let error = parsed("---x").unwrap_err();
    assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption);
    assert_eq!(
        error.to_string(),
        "arguments with `---` are not supported - this is likely a typo"
    );
}

#[test]
fn options_that_need_a_source_build_or_npm_are_not_supported_yet() {
    for line in [
        "-s 20",
        "-j 4 20",
        "--offline 20",
        "--latest-npm 20",
        "--reinstall-packages-from=18 20",
        "20 --reinstall-packages-from=18",
        "20 --copy-packages-from=18",
        "20 --skip-default-packages",
        "20 --save",
    ] {
        let error = parsed(line).unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption, "{line}");
        assert!(
            error.to_string().ends_with("is not supported yet."),
            "{line}"
        );
    }
}

#[test]
fn an_unknown_option_is_taken_as_the_version_like_nvm_sh_does() {
    let options = parsed("--bogus").unwrap();
    assert_eq!(options.version, "--bogus");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function `parse` in
module `options`" and "cannot find struct `Options`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/commands/install/options/mod.rs`:

```rust
//! The command line of `nvm install`, as `nvm.sh` reads it: options first,
//! then the version (or `lts/*`, `lts/<name>`), and what follows it is not
//! looked at, except for the options that this port does not have.

use crate::error::CliError;

/// Options that need a source build, `npm` or a `.nvmrc`: not in this port yet.
const NOT_YET: [&str; 4] = ["-s", "-j", "--offline", "--latest-npm"];
const NOT_YET_WITH_VALUE: [&str; 2] = ["--reinstall-packages-from", "--copy-packages-from"];
const NOT_YET_AFTER_VERSION: [&str; 3] = ["--skip-default-packages", "--save", "-w"];

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// The version, partial version or alias: what follows the options.
    pub version: String,
    /// `*` or a codename (as given): from `--lts[=name]`, `lts/*` or `lts/<name>`.
    pub lts: Option<String>,
    /// From `--default` or `--alias=<name>`.
    pub alias: Option<String>,
    /// There was a version argument (even an empty one).
    pub version_given: bool,
}

fn unsupported(option: &str) -> CliError {
    CliError::Unsupported(format!("The option \"{option}\" is not supported yet."))
}

fn already_given() -> CliError {
    let message =
        "--default and --alias are mutually exclusive, and may not be provided more than once";
    CliError::InvalidOptions(message.to_owned())
}

fn is_not_yet(option: &str, names: &[&str]) -> bool {
    names
        .iter()
        .any(|name| option == *name || option.starts_with(&format!("{name}=")))
}

/// # Errors
/// - [`CliError::Unsupported`] for `---x`, and for the options listed above.
/// - [`CliError::InvalidOptions`] for `--default` with `--alias`, or either twice.
pub fn parse(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    let mut rest = args.iter().peekable();
    while let Some(option) = rest.next_if(|arg| is_option(arg)) {
        match option.as_str() {
            "-b" | "--no-progress" => {}
            "--lts" => options.lts = Some("*".to_owned()),
            "--default" => set_alias(&mut options, "default")?,
            other if other.starts_with("--lts=") => {
                options.lts = Some(other["--lts=".len()..].to_owned());
            }
            other if other.starts_with("--alias=") => {
                set_alias(&mut options, &other["--alias=".len()..])?;
            }
            other if other.starts_with("---") => {
                let message = "arguments with `---` are not supported - this is likely a typo";
                return Err(CliError::Unsupported(message.to_owned()));
            }
            other => return Err(unsupported(other)),
        }
    }
    if let Some(version) = rest.next() {
        options.version.clone_from(version);
        options.version_given = true;
    }
    for option in rest {
        if is_not_yet(option, &NOT_YET_WITH_VALUE)
            || NOT_YET_AFTER_VERSION.contains(&option.as_str())
        {
            return Err(unsupported(option));
        }
    }
    take_lts_version(&mut options);
    Ok(options)
}

/// A word that `nvm.sh` reads as an option rather than as the version.
fn is_option(arg: &str) -> bool {
    matches!(arg, "-b" | "--no-progress" | "--lts" | "--default")
        || arg.starts_with("--lts=")
        || arg.starts_with("--alias=")
        || arg.starts_with("---")
        || NOT_YET.contains(&arg)
        || is_not_yet(arg, &NOT_YET_WITH_VALUE)
        || NOT_YET_AFTER_VERSION.contains(&arg)
}

fn set_alias(options: &mut Options, name: &str) -> Result<(), CliError> {
    if options.alias.is_some() {
        return Err(already_given());
    }
    options.alias = Some(name.to_owned());
    Ok(())
}

/// `lts/*` and `lts/<name>` given as the version are the LTS filter, whatever
/// `--lts` said.
fn take_lts_version(options: &mut Options) {
    let Some(name) = options.version.strip_prefix("lts/") else {
        return;
    };
    options.lts = Some(name.to_owned());
    options.version.clear();
}

```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 404 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(install): read the options of nvm install"
```

---

### Task 9: Download, verify and put in place

**Files:**

- Create: `src/commands/install/fetch/mod.rs`,
  `src/commands/install/fetch/tests.rs`, `src/commands/install/place/mod.rs`,
  `src/commands/install/place/tests.rs`
- Modify: `src/commands/install/mod.rs`, `src/fakes/file_system/mod.rs`,
  `src/fakes/file_system/tests.rs`

**Interfaces:**

- Consumes: Tasks 1 to 7.
- Produces: `commands::install::fetch::{Artifact, Failed, fetch}`:
  `Artifact::of(&Context, &Version) -> Option<Artifact { file_name, directory,
  tarball }>` with `files()`, and `fetch(&Context, &Version, &mut Transcript) ->
  Result<PathBuf, Failed>` (the messages are in the transcript);
  `commands::install::place::{version_path, is_valid_install, place}`:
  `place(&Context, tarball, files_dir, version_path) -> Result<(), String>`.
  `FakeFileSystem::rename` now moves execute bits too.
- Behaviour (from `nvm_download_artifact` and `nvm_install_binary_extract`,
  checked against the real script): the archive is
  `<mirror>/<vX.Y.Z>/<slug>.tar.gz` and its checksum is the line for that name
  in `<mirror>/<vX.Y.Z>/SHASUMS256.txt` (a failed or missing list is an empty
  checksum). A cached archive prints `Local cache found: <path>` (with `$NVM_DIR`
  and `$HOME` shown as `${NVM_DIR}` and `${HOME}`), then `Checksums match! Using
  existing downloaded archive <path>`, or, when it does not match, the
  mismatch, `Checksum check failed!` and `Removing the broken local cache...`
  and a new download. A download prints `Downloading <url>...`, then `Checksums
  matched!`, or the mismatch (the archive is kept, the unpack directory is
  removed), or `download from <url> failed` (the whole cache directory of that
  version is removed). Unpacking goes to `.cache/bin/<slug>/files`, whose only
  entry (the archive's top-level folder) is moved to the version path with one
  rename, after the old version path is removed. An install is valid when
  `bin/node` is a non-empty executable file.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/install/mod.rs`:

```diff
--- a/src/commands/install/mod.rs
+++ b/src/commands/install/mod.rs
@@ -1,4 +1,6 @@
 //! `nvm install`.
 
+pub mod fetch;
 pub mod lock;
 pub mod options;
+pub mod place;
```

- [ ] **Step 2: Write the failing tests**

Create `src/commands/install/fetch/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/install/fetch/tests.rs`:

```rust
use super::*;
use crate::fakes::{FakeDigest, FakeEnv, FakeFileSystem, FakeHttp};
use crate::ports::FileSystem;

const SUMS: &str = "http://127.0.0.1:1/v20.10.0/SHASUMS256.txt";
const TARBALL_URL: &str = "http://127.0.0.1:1/v20.10.0/node-v20.10.0-linux-x64.tar.gz";
const TARBALL: &str =
    "/home/me/.nvm/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz";
const GOOD: &str = "aa11";

fn version() -> Version {
    "v20.10.0".parse().unwrap()
}

fn env() -> FakeEnv {
    FakeEnv::default()
        .with_var("NVM_DIR", "/home/me/.nvm")
        .with_var("HOME", "/home/me")
        .with_var("NVM_NODEJS_ORG_MIRROR", "http://127.0.0.1:1")
}

fn sums(digest: &str) -> String {
    format!("{digest}  node-v20.10.0-linux-x64.tar.gz\nff00  node-v20.10.0-linux-x64.tar.xz\n")
}

fn run(
    fs: &FakeFileSystem,
    http: &FakeHttp,
    digest: &FakeDigest,
) -> (Result<PathBuf, Failed>, Transcript) {
    let env = env();
    let context = Context::new(fs, &env).with_http(http).with_digest(digest);
    let mut transcript = Transcript::default();
    let result = fetch(&context, &version(), &mut transcript);
    (result, transcript)
}

fn stderr(transcript: Transcript) -> Vec<String> {
    transcript
        .finish(crate::error::NvmExitCode::Success)
        .stderr
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn it_downloads_checks_and_keeps_the_archive_in_the_cache() {
    let fs = FakeFileSystem::default();
    let http = FakeHttp::default()
        .with_body(SUMS, &sums(GOOD))
        .with_bytes(TARBALL_URL, b"tarball");
    let digest = FakeDigest::default().with_digest(TARBALL, GOOD);
    let (result, transcript) = run(&fs, &http, &digest);
    assert_eq!(result.unwrap(), PathBuf::from(TARBALL));
    assert_eq!(fs.file_info(Path::new(TARBALL)).unwrap().len, 7);
    assert_eq!(
        stderr(transcript),
        [
            format!("Downloading {TARBALL_URL}..."),
            "Checksums matched!".to_owned()
        ]
    );
}

#[test]
fn a_matching_cached_archive_is_used_without_downloading() {
    let fs = FakeFileSystem::default().with_file(TARBALL, "cached");
    let http = FakeHttp::default().with_body(SUMS, &sums(GOOD));
    let digest = FakeDigest::default().with_digest(TARBALL, GOOD);
    let (result, transcript) = run(&fs, &http, &digest);
    assert!(result.is_ok());
    assert_eq!(http.requests(), [SUMS]);
    assert_eq!(
        stderr(transcript),
        [
            "Local cache found: ${NVM_DIR}/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz",
            "Checksums match! Using existing downloaded archive ${NVM_DIR}/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz",
        ]
    );
}

#[test]
fn a_broken_cached_archive_is_removed_and_downloaded_again() {
    let fs = FakeFileSystem::default().with_file(TARBALL, "corrupt");
    let http = FakeHttp::default()
        .with_body(SUMS, &sums(GOOD))
        .with_bytes(TARBALL_URL, b"fresh");
    let digest = FakeDigest::default().with_digest(TARBALL, "bad0");
    let (result, transcript) = run(&fs, &http, &digest);
    // The fake digest answers the same for the new file, so it fails again.
    assert_eq!(result, Err(Failed));
    let lines = stderr(transcript);
    assert_eq!(
        lines[1],
        "Checksums do not match: 'bad0' found, 'aa11' expected."
    );
    assert_eq!(lines[2], "Checksum check failed!");
    assert_eq!(lines[3], "Removing the broken local cache...");
    assert_eq!(lines[4], format!("Downloading {TARBALL_URL}..."));
}

#[test]
fn a_wrong_checksum_fails_and_leaves_the_archive_but_not_the_unpack_directory() {
    let fs = FakeFileSystem::default();
    let http = FakeHttp::default()
        .with_body(SUMS, &sums(GOOD))
        .with_bytes(TARBALL_URL, b"tarball");
    let digest = FakeDigest::default().with_digest(TARBALL, "bad0");
    let (result, transcript) = run(&fs, &http, &digest);
    assert_eq!(result, Err(Failed));
    assert_eq!(
        stderr(transcript)[1],
        "Checksums do not match: 'bad0' found, 'aa11' expected."
    );
    assert!(fs.file_info(Path::new(TARBALL)).is_ok());
    let files = Path::new("/home/me/.nvm/.cache/bin/node-v20.10.0-linux-x64/files");
    assert!(fs.file_info(files).is_err());
}

#[test]
fn no_listed_checksum_is_a_failure() {
    let fs = FakeFileSystem::default();
    let http = FakeHttp::default().with_bytes(TARBALL_URL, b"tarball");
    let digest = FakeDigest::default().with_digest(TARBALL, GOOD);
    let (result, transcript) = run(&fs, &http, &digest);
    assert_eq!(result, Err(Failed));
    assert_eq!(
        stderr(transcript)[1],
        "Provided checksum to compare to is empty."
    );
}

#[test]
fn a_failed_download_removes_the_cache_directory_and_says_so() {
    let fs = FakeFileSystem::default();
    let http = FakeHttp::default()
        .with_body(SUMS, &sums(GOOD))
        .with_status(TARBALL_URL, 404);
    let (result, transcript) = run(&fs, &http, &FakeDigest::default());
    assert_eq!(result, Err(Failed));
    assert_eq!(
        stderr(transcript),
        [
            format!("Downloading {TARBALL_URL}..."),
            format!("download from {TARBALL_URL} failed")
        ]
    );
    assert!(
        fs.file_info(Path::new(
            "/home/me/.nvm/.cache/bin/node-v20.10.0-linux-x64"
        ))
        .is_err()
    );
}

#[test]
fn a_mirror_that_is_not_a_url_is_reported_and_nothing_is_requested() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/home/me/.nvm")
        .with_var("NVM_NODEJS_ORG_MIRROR", "not a url");
    let http = FakeHttp::default();
    let digest = FakeDigest::default();
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_digest(&digest);
    let mut transcript = Transcript::default();
    assert_eq!(fetch(&context, &version(), &mut transcript), Err(Failed));
    assert!(stderr(transcript)[0].contains("may only contain a URL"));
    assert!(http.requests().is_empty());
}

#[test]
fn the_artifact_is_named_after_the_platform() {
    let fs = FakeFileSystem::default();
    let env = env();
    let mac = crate::domain::platform::Platform::from_host("macos", "aarch64", false);
    let context = Context::new(&fs, &env).with_platform(mac);
    let artifact = Artifact::of(&context, &"v20.10.0".parse().unwrap()).unwrap();
    assert_eq!(artifact.file_name, "node-v20.10.0-darwin-arm64.tar.gz");
    assert_eq!(
        artifact.files(),
        PathBuf::from("/home/me/.nvm/.cache/bin/node-v20.10.0-darwin-arm64/files")
    );
    assert!(Artifact::of(&context.with_platform(None), &"v20.10.0".parse().unwrap()).is_none());
}
```

Create `src/commands/install/place/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/install/place/tests.rs`:

```rust
use super::*;
use crate::fakes::{FakeArchive, FakeEnv, FakeFileSystem};
use crate::ports::FileSystem;

const TARBALL: &str = "/n/.cache/bin/s/s.tar.gz";
const FILES: &str = "/n/.cache/bin/s/files";
const TARGET: &str = "/n/versions/node/v20.10.0";

fn version() -> Version {
    "v20.10.0".parse().unwrap()
}

fn env() -> FakeEnv {
    FakeEnv::default().with_var("NVM_DIR", "/n")
}

fn read(fs: &FakeFileSystem, path: &str) -> Option<String> {
    fs.read_to_string(Path::new(path)).ok()
}

#[test]
fn the_version_path_is_per_flavor() {
    let (fs, env) = (FakeFileSystem::default(), env());
    let context = Context::new(&fs, &env);
    assert_eq!(
        version_path(&context, &version()).unwrap(),
        PathBuf::from(TARGET)
    );
    let iojs = "iojs-v3.3.1".parse().unwrap();
    let expected = PathBuf::from("/n/versions/io.js/v3.3.1");
    assert_eq!(version_path(&context, &iojs).unwrap(), expected);
}

#[test]
fn an_install_needs_a_runnable_node_that_is_not_empty() {
    let env = env();
    let check = |fs: &FakeFileSystem| is_valid_install(&Context::new(fs, &env), Path::new(TARGET));
    let good = FakeFileSystem::default().with_executable("/n/versions/node/v20.10.0/bin/node", "x");
    assert!(check(&good));
    let empty = FakeFileSystem::default().with_executable("/n/versions/node/v20.10.0/bin/node", "");
    assert!(!check(&empty));
    let not_runnable =
        FakeFileSystem::default().with_file("/n/versions/node/v20.10.0/bin/node", "x");
    assert!(!check(&not_runnable));
    assert!(!check(&FakeFileSystem::default()));
}

#[test]
fn placing_moves_the_top_level_directory_to_the_version_path() {
    let fs = FakeFileSystem::default();
    let archive = FakeArchive::new(&fs).with_archive(
        TARBALL,
        &[
            ("node-v20.10.0-linux-x64/bin/node", "binary", true),
            ("node-v20.10.0-linux-x64/README", "hi", false),
        ],
    );
    let env = env();
    let context = Context::new(&fs, &env).with_archive(&archive);
    place(
        &context,
        Path::new(TARBALL),
        Path::new(FILES),
        Path::new(TARGET),
    )
    .unwrap();
    assert_eq!(
        read(&fs, "/n/versions/node/v20.10.0/README").as_deref(),
        Some("hi")
    );
    assert!(is_valid_install(&context, Path::new(TARGET)));
    assert!(fs.file_info(Path::new(FILES)).is_err());
}

#[test]
fn a_broken_install_that_was_there_is_replaced_whole() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.10.0/stale", "old");
    let archive = FakeArchive::new(&fs).with_archive(TARBALL, &[("top/bin/node", "binary", true)]);
    let env = env();
    let context = Context::new(&fs, &env).with_archive(&archive);
    place(
        &context,
        Path::new(TARBALL),
        Path::new(FILES),
        Path::new(TARGET),
    )
    .unwrap();
    assert!(
        fs.file_info(Path::new("/n/versions/node/v20.10.0/stale"))
            .is_err()
    );
    assert!(
        fs.file_info(Path::new("/n/versions/node/v20.10.0/bin/node"))
            .is_ok()
    );
}

#[test]
fn leftovers_in_the_unpack_directory_do_not_confuse_the_layout() {
    let fs = FakeFileSystem::default().with_file("/n/.cache/bin/s/files/old-run/f", "x");
    let archive = FakeArchive::new(&fs).with_archive(TARBALL, &[("top/bin/node", "b", true)]);
    let env = env();
    let context = Context::new(&fs, &env).with_archive(&archive);
    place(
        &context,
        Path::new(TARBALL),
        Path::new(FILES),
        Path::new(TARGET),
    )
    .unwrap();
    assert!(is_valid_install(&context, Path::new(TARGET)));
}

#[test]
fn an_archive_with_two_top_level_entries_is_refused_and_leaves_the_install_alone() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.10.0/keep", "x");
    let archive =
        FakeArchive::new(&fs).with_archive(TARBALL, &[("a/f", "1", false), ("b/f", "2", false)]);
    let env = env();
    let context = Context::new(&fs, &env).with_archive(&archive);
    let error = place(
        &context,
        Path::new(TARBALL),
        Path::new(FILES),
        Path::new(TARGET),
    )
    .unwrap_err();
    assert_eq!(
        error,
        "the archive does not hold exactly one top-level directory"
    );
    assert!(
        fs.file_info(Path::new("/n/versions/node/v20.10.0/keep"))
            .is_ok()
    );
}

#[test]
fn an_archive_that_cannot_be_unpacked_is_an_error() {
    let (fs, env) = (FakeFileSystem::default(), env());
    let context = Context::new(&fs, &env);
    assert!(
        place(
            &context,
            Path::new(TARBALL),
            Path::new(FILES),
            Path::new(TARGET)
        )
        .is_err()
    );
}
```

Apply to `src/fakes/file_system/tests.rs`:

```diff
--- a/src/fakes/file_system/tests.rs
+++ b/src/fakes/file_system/tests.rs
@@ -60,10 +60,13 @@
 fn fake_file_system_renames_a_tree_and_creates_a_directory_once() {
     let fs = FakeFileSystem::default()
         .with_file("/a/x/y", "1")
+        .with_executable("/a/bin/tool", "t")
         .with_dir("/a/empty");
     fs.rename(Path::new("/a"), Path::new("/b")).unwrap();
     assert_eq!(fs.read_to_string(Path::new("/b/x/y")).unwrap(), "1");
     assert!(fs.file_info(Path::new("/b/empty")).unwrap().is_dir);
+    assert!(!fs.file_info(Path::new("/b/x/y")).unwrap().executable);
+    assert!(fs.file_info(Path::new("/b/bin/tool")).unwrap().executable);
     assert!(fs.file_info(Path::new("/a")).is_err());
     assert!(fs.rename(Path::new("/a"), Path::new("/c")).is_err());
     fs.create_dir(Path::new("/lock")).unwrap();
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function `fetch`",
"cannot find struct `Artifact`" and "cannot find function `place`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/commands/install/fetch/mod.rs`:

```rust
//! Getting the archive of a version into `$NVM_DIR/.cache/bin/<slug>/`, and
//! checking it against the mirror's `SHASUMS256.txt`, as `nvm_download_artifact`
//! does (but the `.tar.gz`, never the `.tar.xz`).

use std::path::{Path, PathBuf};

use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::checksum::{compare, expected_digest};
use crate::domain::mirror::{self, MirrorUrl};
use crate::domain::version::Version;

/// A failure whose messages are in the transcript already.
#[derive(Debug, PartialEq, Eq)]
pub struct Failed;

/// Where an archive lives in the cache.
pub struct Artifact {
    /// `node-v20.10.0-linux-x64.tar.gz`.
    pub file_name: String,
    /// `$NVM_DIR/.cache/bin/<slug>`.
    pub directory: PathBuf,
    pub tarball: PathBuf,
}

impl Artifact {
    /// `None` when there are no binaries for this machine, or `$NVM_DIR` is
    /// unknown.
    #[must_use]
    pub fn of(context: &Context<'_>, version: &Version) -> Option<Self> {
        let slug = context.platform()?.download_slug(version);
        let directory = context.cache_dir().ok()?.join("bin").join(&slug);
        let file_name = format!("{slug}.tar.gz");
        let tarball = directory.join(&file_name);
        Some(Self {
            file_name,
            directory,
            tarball,
        })
    }

    /// Where the archive is unpacked.
    #[must_use]
    pub fn files(&self) -> PathBuf {
        self.directory.join("files")
    }
}

/// `$NVM_DIR` and `$HOME` in a path shown to the user become the variables.
fn sanitize(context: &Context<'_>, path: &Path) -> String {
    let mut text = path.display().to_string();
    for (variable, name) in [("NVM_DIR", "${NVM_DIR}"), ("HOME", "${HOME}")] {
        let value = match variable {
            "NVM_DIR" => context.nvm_dir().ok().map(|dir| dir.display().to_string()),
            _ => context.env.var("HOME"),
        };
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            text = text.replace(&value, name);
        }
    }
    text
}

/// The archive of `version`, from the cache when its checksum matches and from
/// the mirror otherwise.
///
/// # Errors
/// [`Failed`], after the transcript says why.
pub fn fetch(
    context: &Context<'_>,
    version: &Version,
    transcript: &mut Transcript,
) -> Result<PathBuf, Failed> {
    let artifact = Artifact::of(context, version).ok_or(Failed)?;
    let mirror = mirror::from_env(context.env, version.flavor).map_err(|error| {
        transcript.err(error.to_string());
        Failed
    })?;
    let expected = expected_checksum(context, &mirror, version, &artifact);
    let files = artifact.files();
    context.fs.create_dir_all(&files).map_err(|_| {
        transcript.err(format!("creating directory {} failed", files.display()));
        Failed
    })?;
    if reuse_cache(context, &artifact, &expected, transcript) {
        return Ok(artifact.tarball);
    }
    download(context, &mirror, version, &artifact, transcript)?;
    verify(context, &artifact, &expected, transcript)?;
    Ok(artifact.tarball)
}

/// What `SHASUMS256.txt` lists for the archive; empty when it cannot be had.
fn expected_checksum(
    context: &Context<'_>,
    mirror: &MirrorUrl,
    version: &Version,
    artifact: &Artifact,
) -> String {
    let url = mirror.join(&format!("{}/SHASUMS256.txt", version.directory_name()));
    context
        .http()
        .get_text(&url)
        .ok()
        .and_then(|shasums| expected_digest(&shasums, &artifact.file_name))
        .unwrap_or_default()
}

fn digest_of(context: &Context<'_>, tarball: &Path) -> String {
    context.digest().sha256_file(tarball).unwrap_or_default()
}

/// True when a cached archive exists and is the right one; a broken one is
/// removed.
fn reuse_cache(
    context: &Context<'_>,
    artifact: &Artifact,
    expected: &str,
    transcript: &mut Transcript,
) -> bool {
    if context.fs.file_info(&artifact.tarball).is_err() {
        return false;
    }
    let shown = sanitize(context, &artifact.tarball);
    transcript.err(format!("Local cache found: {shown}"));
    match compare(&digest_of(context, &artifact.tarball), expected) {
        Ok(()) => {
            transcript.err(format!(
                "Checksums match! Using existing downloaded archive {shown}"
            ));
            true
        }
        Err(error) => {
            transcript.err(error.to_string());
            transcript.err("Checksum check failed!");
            transcript.err("Removing the broken local cache...");
            let _ = context.fs.remove_dir_all(&artifact.tarball);
            false
        }
    }
}

fn download(
    context: &Context<'_>,
    mirror: &MirrorUrl,
    version: &Version,
    artifact: &Artifact,
    transcript: &mut Transcript,
) -> Result<(), Failed> {
    let url = mirror.join(&format!(
        "{}/{}",
        version.directory_name(),
        artifact.file_name
    ));
    transcript.err(format!("Downloading {url}..."));
    let stored = context
        .http()
        .get_bytes(&url)
        .map_err(|_| ())
        .and_then(|bytes| {
            context
                .fs
                .write_bytes(&artifact.tarball, &bytes)
                .map_err(|_| ())
        });
    if stored.is_err() {
        let _ = context.fs.remove_dir_all(&artifact.directory);
        transcript.err(format!("download from {url} failed"));
        return Err(Failed);
    }
    Ok(())
}

fn verify(
    context: &Context<'_>,
    artifact: &Artifact,
    expected: &str,
    transcript: &mut Transcript,
) -> Result<(), Failed> {
    match compare(&digest_of(context, &artifact.tarball), expected) {
        Ok(()) => {
            transcript.err("Checksums matched!");
            Ok(())
        }
        Err(error) => {
            transcript.err(error.to_string());
            let _ = context.fs.remove_dir_all(&artifact.files());
            Err(Failed)
        }
    }
}

```

Insert above the `#[cfg(test)]` line of `src/commands/install/place/mod.rs`:

```rust
//! Unpacking an archive and putting the result where nvm keeps a version:
//! `nvm_install_binary_extract` and `nvm_validate_install`.

use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::domain::version::Version;
use crate::error::CliError;

/// `$NVM_DIR/versions/node/v20.10.0`, or `.../io.js/v3.3.1`.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn version_path(context: &Context<'_>, version: &Version) -> Result<PathBuf, CliError> {
    Ok(context
        .nvm_dir()?
        .join("versions")
        .join(version.flavor.versions_directory())
        .join(version.directory_name()))
}

/// The version's directory holds a `bin/node` that is not empty and can run.
#[must_use]
pub fn is_valid_install(context: &Context<'_>, version_path: &Path) -> bool {
    context
        .fs
        .file_info(&version_path.join("bin/node"))
        .is_ok_and(|node| !node.is_dir && node.len > 0 && node.executable)
}

/// Unpacks `tarball` into `files`, then replaces `version_path` with the one
/// directory the archive holds, in a single rename. A half-installed
/// directory is never visible, and a broken one that was there is gone.
///
/// # Errors
/// What went wrong, as a message for stderr.
pub fn place(
    context: &Context<'_>,
    tarball: &Path,
    files: &Path,
    version_path: &Path,
) -> Result<(), String> {
    let fs = context.fs;
    fs.remove_dir_all(files)
        .map_err(|error| error.to_string())?;
    fs.create_dir_all(files)
        .map_err(|error| error.to_string())?;
    context
        .archive()
        .extract(tarball, files)
        .map_err(|error| error.to_string())?;
    let top = single_directory(context, files)?;
    fs.remove_dir_all(version_path)
        .map_err(|error| error.to_string())?;
    if let Some(parent) = version_path.parent() {
        fs.create_dir_all(parent)
            .map_err(|error| error.to_string())?;
    }
    fs.rename(&top, version_path)
        .map_err(|error| error.to_string())?;
    let _ = fs.remove_dir_all(files);
    Ok(())
}

/// The only entry of `directory`, which must be a directory: the archive's
/// top-level `node-vX.Y.Z-<os>-<arch>` folder.
fn single_directory(context: &Context<'_>, directory: &Path) -> Result<PathBuf, String> {
    let entries = context
        .fs
        .read_dir(directory)
        .map_err(|error| error.to_string())?;
    match entries.as_slice() {
        [only] if only.is_dir => Ok(directory.join(&only.name)),
        _ => Err("the archive does not hold exactly one top-level directory".to_owned()),
    }
}

```

Apply to `src/fakes/file_system/mod.rs` (above the test module):

```diff
--- a/src/fakes/file_system/mod.rs
+++ b/src/fakes/file_system/mod.rs
@@ -153,6 +153,16 @@
                 files.insert(moved(&name), contents);
             }
         }
+        let mut executables = self.executables.borrow_mut();
+        let names: Vec<PathBuf> = executables
+            .iter()
+            .filter(|e| e.starts_with(from))
+            .cloned()
+            .collect();
+        for name in names {
+            executables.remove(&name);
+            executables.insert(moved(&name));
+        }
         let mut dirs = self.dirs.borrow_mut();
         let names: Vec<PathBuf> = dirs
             .iter()
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 419 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(install): download, verify and put an archive in place"
```

---

### Task 10: `nvm install`

**Files:**

- Create: `src/commands/install/flow.rs`, `src/commands/install/defaults.rs`,
  `src/commands/install/tests/mod.rs`, `src/commands/install/tests/failures.rs`
- Modify: `src/commands/install/mod.rs`, `src/commands/install/options/mod.rs`,
  `src/commands/install/options/tests.rs`

**Interfaces:**

- Produces: `commands::install::run(&Context, &[String]) -> Result<Output,
  CliError>`; `Options::announce_lts`.
- Behaviour (verified against `nvm.sh` with captured runs): without a version
  or `--lts` the error is the usage (`No version provided and no .nvmrc file
  found`, `Usage: nvm install [<version>]` and two more lines, exit 127);
  `--lts` alone prints `Installing latest LTS version.` and `--lts=<name>`
  `Installing with latest version of LTS line: <name>`. The version is resolved
  on the mirror as `version-remote` does; nothing found is `Version '<v>' not
  found - try `nvm ls-remote` to browse available versions.` (or `... (with LTS
  filter) ...`, `Version with LTS filter '<name>' not found - try `nvm
  ls-remote --lts=<name>` ...`), exit 3; the floor (`NVM_MIN_VERSION` or
  `$NVM_DIR/min-version`) comes next, with its message and `Lower or unset
  NVM_MIN_VERSION (or edit $NVM_DIR/min-version) to install it.`, exit 7. A
  valid install prints `<version> is already installed.` on stderr and nothing
  is downloaded; otherwise the lock is taken, `Downloading and installing node
  v20.10.0...` (`io.js` for io.js) is printed, the archive is fetched, placed
  and checked, and a failure at any of those prints
  `Binary download failed. Download from source aborted.` and exits 2. `--default`
  and `--alias=<name>` run `nvm alias <name> <version as given>` (a fresh install
  does it before the `default` alias is ensured, an already installed one
  after); the `default` alias is made when there is none, printing `Creating
  default alias: <the alias line>`, pointing at the version as given or at
  `lts/<name>`. A machine without binaries, a version before v0.8.6 or before
  v0.12.0 (the legacy layout) is `Binary download is not available for <v>` (3).
- [ ] **Step 1: Write the failing tests**

Create `src/commands/install/defaults.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use crate::ports::FileSystem;

    fn lines(
        fs: &FakeFileSystem,
        run: impl Fn(&Context<'_>, &mut Transcript) -> Step<()>,
    ) -> (String, String) {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let context = Context::new(fs, &env);
        let mut transcript = Transcript::default();
        run(&context, &mut transcript).unwrap();
        let output = transcript.finish(NvmExitCode::Success);
        (output.stdout, output.stderr)
    }

    #[test]
    fn a_missing_default_is_created_and_announced() {
        let fs =
            FakeFileSystem::default().with_executable("/n/versions/node/v20.10.0/bin/node", "x");
        let (stdout, _) = lines(&fs, |c, t| ensure_default(c, "20", t));
        assert_eq!(
            stdout,
            "Creating default alias: default -> 20 (-> v20.10.0 *)"
        );
        assert_eq!(
            fs.read_to_string(Path::new("/n/alias/default")).unwrap(),
            "20\n"
        );
    }

    #[test]
    fn an_existing_default_is_left_alone() {
        let fs = FakeFileSystem::default().with_file("/n/alias/default", "18\n");
        let (stdout, stderr) = lines(&fs, |c, t| ensure_default(c, "20", t));
        assert_eq!((stdout.as_str(), stderr.as_str()), ("", ""));
        assert_eq!(
            fs.read_to_string(Path::new("/n/alias/default")).unwrap(),
            "18\n"
        );
    }

    #[test]
    fn make_alias_shows_the_line_nvm_alias_prints() {
        let fs =
            FakeFileSystem::default().with_executable("/n/versions/node/v18.19.0/bin/node", "x");
        let (stdout, _) = lines(&fs, |c, t| make_alias(c, "work", "18", t));
        assert_eq!(stdout, "work -> 18 (-> v18.19.0 *)");
    }
}
```

Apply to the test module of `src/commands/install/mod.rs`:

```diff
--- a/src/commands/install/mod.rs
+++ b/src/commands/install/mod.rs
@@ -0,0 +1,2 @@
+#[cfg(test)]
+mod tests;
```

Apply to `src/commands/install/options/tests.rs`:

```diff
--- a/src/commands/install/options/tests.rs
+++ b/src/commands/install/options/tests.rs
@@ -15,6 +15,15 @@
     );
     assert_eq!(options.lts, None);
     assert_eq!(options.alias, None);
+}
+
+#[test]
+fn only_an_lts_option_without_a_version_is_announced() {
+    assert!(parsed("--lts").unwrap().announce_lts);
+    assert!(parsed("--lts=iron").unwrap().announce_lts);
+    assert!(!parsed("lts/iron").unwrap().announce_lts);
+    assert!(!parsed("--lts 20").unwrap().announce_lts);
+    assert!(!parsed("20").unwrap().announce_lts);
 }
 
 #[test]
```

Create `src/commands/install/tests/failures.rs`:

```rust
use super::*;

#[test]
fn a_version_that_does_not_exist_is_status_3_with_the_hint() {
    let output = World::new().run("99").unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stderr,
        "Version '99' not found - try `nvm ls-remote` to browse available versions."
    );
}

#[test]
fn no_version_at_all_is_the_usage_with_status_127() {
    let error = World::new().run("").unwrap_err();
    assert_eq!(error.exit_code(), NvmExitCode::NotFound);
    assert!(error.to_string().starts_with(
        "No version provided and no .nvmrc file found\nUsage: nvm install [<version>]"
    ));
}

#[test]
fn a_version_below_the_floor_is_status_7_with_both_lines() {
    let mut world = World::new();
    world.env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("NVM_MIN_VERSION", "v22");
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert_eq!(
        lines(&output.stderr),
        [
            "Version v20.10.0 is below the minimum allowed version v22.0.0.",
            "Lower or unset NVM_MIN_VERSION (or edit $NVM_DIR/min-version) to install it.",
        ]
    );
    assert!(!world.installed());
}

#[test]
fn the_floor_can_come_from_the_min_version_file() {
    let world = World::new();
    world
        .fs
        .write_file(Path::new("/n/min-version"), "v22\n")
        .unwrap();
    assert_eq!(
        world.run("20").unwrap().status,
        NvmExitCode::BelowVersionFloor
    );
}

#[test]
fn a_failed_download_is_status_2_and_installs_nothing() {
    let mut world = World::new();
    world.http = FakeHttp::default()
        .with_body(NODE_INDEX, &index_text(&[("v20.10.0", "Iron")]))
        .with_body(IOJS_INDEX, &index_text(&[]))
        .with_status(TARBALL_URL, 404);
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    assert!(
        output
            .stderr
            .ends_with("Binary download failed. Download from source aborted.")
    );
    assert!(!world.installed());
}

#[test]
fn a_wrong_checksum_is_status_2_and_installs_nothing() {
    let mut world = World::new();
    world.digest = FakeDigest::default().with_digest(TARBALL, "bad0");
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    assert!(
        output
            .stderr
            .contains("Checksums do not match: 'bad0' found, 'aa11' expected.")
    );
    assert!(!world.installed());
}

#[test]
fn a_machine_without_binaries_is_status_3() {
    let world = World::new();
    let archive = FakeArchive::new(&world.fs);
    let context = Context::new(&world.fs, &world.env)
        .with_http(&world.http)
        .with_archive(&archive)
        .with_platform(None);
    let output = super::run(&context, &["20".to_owned()]).unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stderr,
        "Binary download is not available for v20.10.0"
    );
}

#[test]
fn a_held_lock_that_never_clears_stops_the_install_with_status_1() {
    let mut world = World::new();
    world.fs = FakeFileSystem::default().with_dir("/n/.cache/locks/v20.10.0");
    world.env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("NVM_INSTALL_LOCK_TIMEOUT", "1");
    let error = world.run("20").unwrap_err();
    assert_eq!(error.exit_code(), NvmExitCode::Failure);
    assert!(
        error
            .to_string()
            .starts_with("Timed out after 1s waiting for another install of v20.10.0")
    );
}
```

Create `src/commands/install/tests/mod.rs`:

```rust
use std::path::Path;

use super::*;
use crate::domain::fixtures::index_text;
use crate::fakes::{FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeSleeper};
use crate::ports::FileSystem;

const NODE_INDEX: &str = "https://nodejs.org/dist/index.tab";
const IOJS_INDEX: &str = "https://iojs.org/dist/index.tab";
const SUMS: &str = "https://nodejs.org/dist/v20.10.0/SHASUMS256.txt";
const TARBALL_URL: &str = "https://nodejs.org/dist/v20.10.0/node-v20.10.0-linux-x64.tar.gz";
const TARBALL: &str = "/n/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz";
const NODE: &str = "/n/versions/node/v20.10.0/bin/node";
const GOOD: &str = "aa11";

/// Everything a successful install of v20.10.0 touches, as one small world.
struct World {
    fs: FakeFileSystem,
    http: FakeHttp,
    digest: FakeDigest,
    sleeper: FakeSleeper,
    env: FakeEnv,
}

impl World {
    fn new() -> Self {
        let node = index_text(&[("v20.10.0", "Iron"), ("v18.19.0", "Hydrogen")]);
        let iojs = index_text(&[]);
        Self {
            fs: FakeFileSystem::default(),
            http: FakeHttp::default()
                .with_body(NODE_INDEX, &node)
                .with_body(IOJS_INDEX, &iojs)
                .with_body(SUMS, &format!("{GOOD}  node-v20.10.0-linux-x64.tar.gz\n"))
                .with_bytes(TARBALL_URL, b"tarball"),
            digest: FakeDigest::default().with_digest(TARBALL, GOOD),
            sleeper: FakeSleeper::default(),
            env: FakeEnv::default().with_var("NVM_DIR", "/n"),
        }
    }

    fn run(&self, line: &str) -> Result<Output, CliError> {
        let archive = FakeArchive::new(&self.fs).with_archive(
            TARBALL,
            &[("node-v20.10.0-linux-x64/bin/node", "binary", true)],
        );
        let context = Context::new(&self.fs, &self.env)
            .with_http(&self.http)
            .with_digest(&self.digest)
            .with_archive(&archive)
            .with_sleeper(&self.sleeper);
        let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        super::run(&context, &words)
    }

    fn installed(&self) -> bool {
        self.fs.file_info(Path::new(NODE)).is_ok()
    }

    fn text(&self, path: &str) -> Option<String> {
        self.fs.read_to_string(Path::new(path)).ok()
    }
}

fn lines(text: &str) -> Vec<&str> {
    text.lines().collect()
}

mod failures;

#[test]
fn a_fresh_install_downloads_unpacks_and_makes_the_default_alias() {
    let world = World::new();
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        lines(&output.stdout),
        [
            "Downloading and installing node v20.10.0...",
            "Creating default alias: default -> 20 (-> v20.10.0 *)",
        ]
    );
    assert_eq!(
        lines(&output.stderr),
        [
            format!("Downloading {TARBALL_URL}..."),
            "Checksums matched!".to_owned()
        ]
    );
    assert!(world.installed());
    assert_eq!(world.text("/n/alias/default").as_deref(), Some("20\n"));
}

#[test]
fn an_installed_version_is_not_downloaded_again() {
    let world = World::new();
    world.run("20").unwrap();
    let requests = world.http.requests().len();
    let output = world.run("v20.10.0").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(output.stderr, "v20.10.0 is already installed.");
    assert_eq!(output.stdout, "");
    assert_eq!(
        world.http.requests().len(),
        requests + 2,
        "only the indexes"
    );
}

#[test]
fn an_existing_default_is_not_replaced_by_a_second_install() {
    let world = World {
        fs: FakeFileSystem::default().with_file("/n/alias/default", "18\n"),
        ..World::new()
    };
    let output = world.run("20").unwrap();
    assert!(!output.stdout.contains("Creating default alias"));
    assert_eq!(world.text("/n/alias/default").as_deref(), Some("18\n"));
}

#[test]
fn lts_installs_the_newest_lts_and_makes_default_point_at_lts_star() {
    let world = World::new();
    let output = world.run("--lts").unwrap();
    assert_eq!(
        lines(&output.stdout),
        [
            "Installing latest LTS version.",
            "Downloading and installing node v20.10.0...",
            "Creating default alias: default -> lts/* (-> v20.10.0 *)",
        ]
    );
}

#[test]
fn a_named_lts_is_announced_and_lowercased_in_the_default_alias() {
    let world = World::new();
    let output = world.run("--lts=Iron").unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stdout,
        "Installing with latest version of LTS line: Iron"
    );
    assert_eq!(
        lines(&output.stderr),
        [
            "LTS names must be lowercase",
            "Version with LTS filter 'Iron' not found - try `nvm ls-remote --lts=Iron` to browse available versions.",
        ]
    );
}

#[test]
fn default_and_alias_make_the_alias_before_the_default_is_ensured() {
    let world = World::new();
    world
        .fs
        .write_file(Path::new("/n/alias/default"), "18\n")
        .unwrap();
    let output = world.run("--alias=work 20").unwrap();
    assert_eq!(
        lines(&output.stdout),
        [
            "Downloading and installing node v20.10.0...",
            "work -> 20 (-> v20.10.0 *)"
        ]
    );
    let again = world.run("--default 20").unwrap();
    assert_eq!(again.stdout, "default -> 20 (-> v20.10.0 *)");
    assert_eq!(world.text("/n/alias/default").as_deref(), Some("20\n"));
}

#[test]
fn the_lock_is_released_after_an_install() {
    let world = World::new();
    world.run("20").unwrap();
    assert!(
        world
            .fs
            .file_info(Path::new("/n/.cache/locks/v20.10.0"))
            .is_err()
    );
}

#[test]
fn a_broken_install_without_a_working_node_is_installed_again() {
    let world = World::new();
    world
        .fs
        .write_file(Path::new("/n/versions/node/v20.10.0/bin/node"), "")
        .unwrap();
    let output = world.run("20").unwrap();
    assert!(
        output
            .stdout
            .starts_with("Downloading and installing node v20.10.0...")
    );
    assert!(world.installed());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function `run` in
this scope", "cannot find module `defaults`" and "no field `announce_lts` on
type `Options`".

- [ ] **Step 3: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/commands/install/defaults.rs`:

```rust
//! The aliases an install leaves behind: `--default`, `--alias=<name>`, and
//! the `default` alias when there is none yet (`nvm_ensure_default_set`).

use crate::commands::alias;
use crate::commands::install::flow::Step;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::alias::AliasStore;

/// `nvm alias <name> <target>`, with what it prints.
pub fn make_alias(
    context: &Context<'_>,
    name: &str,
    target: &str,
    transcript: &mut Transcript,
) -> Step<()> {
    transcript.absorb(alias::run(context, name, target)?);
    Ok(())
}

/// Creates `default` pointing at `target`, unless there is a `default`
/// already.
pub fn ensure_default(
    context: &Context<'_>,
    target: &str,
    transcript: &mut Transcript,
) -> Step<()> {
    if context.alias_store()?.target("default").is_some() {
        return Ok(());
    }
    let output = alias::run(context, "default", target)?;
    transcript.out(format!("Creating default alias: {}", output.stdout));
    if !output.stderr.is_empty() {
        transcript.err(output.stderr);
    }
    Ok(())
}

```

Create `src/commands/install/flow.rs`:

```rust
//! How the steps of an install end early.

use crate::error::{CliError, NvmExitCode};

/// Why an install stops before its last step.
#[derive(Debug)]
pub enum Halt {
    /// Stop, printing what the transcript holds, with this status.
    Exit(NvmExitCode),
    /// Stop with an error that the CLI prints.
    Error(CliError),
}

impl From<CliError> for Halt {
    fn from(error: CliError) -> Self {
        Self::Error(error)
    }
}

pub type Step<T> = Result<T, Halt>;
```

Apply to `src/commands/install/mod.rs` (above the test module):

```diff
--- a/src/commands/install/mod.rs
+++ b/src/commands/install/mod.rs
@@ -1,6 +1,218 @@
-//! `nvm install`.
-
+//! `nvm install`: the version a description stands for, from the mirror, into
+//! `$NVM_DIR/versions`. Only prebuilt binaries (`.tar.gz`) are installed; the
+//! install is not activated, which is the shell's job (`nvm use`).
+
+mod defaults;
 pub mod fetch;
+mod flow;
 pub mod lock;
 pub mod options;
 pub mod place;
+
+use std::time::SystemTime;
+
+use crate::commands::Output;
+use crate::commands::transcript::Transcript;
+use crate::commands::version_remote::lookup;
+use crate::context::Context;
+use crate::domain::floor::VersionFloor;
+use crate::domain::platform::binary_available;
+use crate::domain::remote::Query;
+use crate::domain::version::{Flavor, Version};
+use crate::error::{CliError, NvmExitCode};
+use flow::{Halt, Step};
+use lock::{LockRequest, acquire};
+use options::Options;
+
+const USAGE: &str = "No version provided and no .nvmrc file found\n\
+Usage: nvm install [<version>]\n  \
+Provide a <version>, or run from a directory containing an .nvmrc file.\n  \
+Run `nvm --help` for full help.";
+const DEFAULT_LOCK_TIMEOUT_SECONDS: u64 = 600;
+
+/// # Errors
+/// As [`options::parse`], [`CliError::Usage`] without a version, and
+/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
+pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
+    let options = options::parse(args)?;
+    let mut transcript = Transcript::default();
+    match install(context, &options, &mut transcript) {
+        Ok(()) => Ok(transcript.finish(NvmExitCode::Success)),
+        Err(Halt::Exit(status)) => Ok(transcript.finish(status)),
+        Err(Halt::Error(error)) => Err(error),
+    }
+}
+
+fn install(context: &Context<'_>, options: &Options, transcript: &mut Transcript) -> Step<()> {
+    announce(options, transcript)?;
+    let version = resolve(context, options, transcript)?;
+    check_floor(context, &version, transcript)?;
+    let version_path = place::version_path(context, &version)?;
+    if place::is_valid_install(context, &version_path) {
+        transcript.err(format!("{version} is already installed."));
+        defaults::ensure_default(context, &alias_target(options), transcript)?;
+        return apply_alias(context, options, transcript);
+    }
+    install_binary(context, &version, &version_path, transcript)?;
+    apply_alias(context, options, transcript)?;
+    if !place::is_valid_install(context, &version_path) {
+        let message = format!(
+            "The install of {version} reported success but failed verification; not activating it."
+        );
+        transcript.err(message);
+        return Err(Halt::Exit(NvmExitCode::Failure));
+    }
+    defaults::ensure_default(context, &alias_target(options), transcript)
+}
+
+/// What an alias made for this install points at.
+fn alias_target(options: &Options) -> String {
+    match (&options.lts, options.version.is_empty()) {
+        (Some(lts), true) => format!("lts/{}", lts.to_lowercase()),
+        _ => options.version.clone(),
+    }
+}
+
+/// `--default` and `--alias=<name>`.
+fn apply_alias(context: &Context<'_>, options: &Options, transcript: &mut Transcript) -> Step<()> {
+    match &options.alias {
+        Some(name) => defaults::make_alias(context, name, &alias_target(options), transcript),
+        None => Ok(()),
+    }
+}
+
+fn announce(options: &Options, transcript: &mut Transcript) -> Step<()> {
+    if !options.version_given && options.lts.is_none() {
+        return Err(Halt::Error(CliError::Usage(USAGE.to_owned())));
+    }
+    match (options.announce_lts, options.lts.as_deref()) {
+        (true, Some("*")) => transcript.out("Installing latest LTS version."),
+        (true, Some(name)) => {
+            transcript.out(format!(
+                "Installing with latest version of LTS line: {name}"
+            ));
+        }
+        _ => {}
+    }
+    Ok(())
+}
+
+fn resolve(context: &Context<'_>, options: &Options, transcript: &mut Transcript) -> Step<Version> {
+    let query = Query {
+        pattern: Some(options.version.clone()).filter(|text| !text.is_empty()),
+        lts: options.lts.clone(),
+    };
+    let found = lookup(context, query)?;
+    for warning in found.warnings {
+        transcript.err(warning);
+    }
+    found.version.ok_or_else(|| {
+        transcript.err(not_found_message(options));
+        Halt::Exit(NvmExitCode::InvalidVersion)
+    })
+}
+
+fn not_found_message(options: &Options) -> String {
+    let version = &options.version;
+    match options.lts.as_deref() {
+        Some("*") => format!(
+            "Version '{version}' (with LTS filter) not found - try `nvm ls-remote --lts` to browse available versions."
+        ),
+        Some(lts) if version.is_empty() => format!(
+            "Version with LTS filter '{lts}' not found - try `nvm ls-remote --lts={lts}` to browse available versions."
+        ),
+        Some(lts) => format!(
+            "Version '{version}' (with LTS filter '{lts}') not found - try `nvm ls-remote --lts={lts}` to browse available versions."
+        ),
+        None => format!(
+            "Version '{version}' not found - try `nvm ls-remote` to browse available versions."
+        ),
+    }
+}
+
+fn check_floor(context: &Context<'_>, version: &Version, transcript: &mut Transcript) -> Step<()> {
+    let from_file = context
+        .fs
+        .read_to_string(&context.nvm_dir()?.join("min-version"))
+        .ok();
+    let from_env = context.env.var("NVM_MIN_VERSION");
+    let floor = VersionFloor::from_sources(from_env.as_deref(), from_file.as_deref())
+        .and_then(|floor| floor.map_or(Ok(()), |floor| floor.check(version)));
+    let Err(error) = floor else {
+        return Ok(());
+    };
+    transcript.err(error.to_string());
+    if matches!(error, crate::error::FloorError::Below { .. }) {
+        transcript
+            .err("Lower or unset NVM_MIN_VERSION (or edit $NVM_DIR/min-version) to install it.");
+    }
+    Err(Halt::Exit(NvmExitCode::BelowVersionFloor))
+}
+
+/// Everything from the lock to the unpacked directory.
+fn install_binary(
+    context: &Context<'_>,
+    version: &Version,
+    version_path: &std::path::Path,
+    transcript: &mut Transcript,
+) -> Step<()> {
+    let unavailable = !binary_available(version)
+        || context.platform().is_none()
+        || (version.flavor == Flavor::Node && version.triple() < (0, 12, 0));
+    if unavailable {
+        transcript.err(format!("Binary download is not available for {version}"));
+        return Err(Halt::Exit(NvmExitCode::InvalidVersion));
+    }
+    let _lock = take_lock(context, version, transcript)?;
+    let name = match version.flavor {
+        Flavor::Node => "node",
+        Flavor::IoJs => "io.js",
+    };
+    transcript.out(format!(
+        "Downloading and installing {name} {}...",
+        version.directory_name()
+    ));
+    let tarball =
+        fetch::fetch(context, version, transcript).map_err(|_| binary_failed(transcript))?;
+    let artifact =
+        fetch::Artifact::of(context, version).ok_or_else(|| binary_failed(transcript))?;
+    place::place(context, &tarball, &artifact.files(), version_path).map_err(|message| {
+        transcript.err(message);
+        binary_failed(transcript)
+    })
+}
+
+fn binary_failed(transcript: &mut Transcript) -> Halt {
+    transcript.err("Binary download failed. Download from source aborted.");
+    Halt::Exit(NvmExitCode::MissingTarget)
+}
+
+fn take_lock<'a>(
+    context: &'a Context<'_>,
+    version: &Version,
+    transcript: &mut Transcript,
+) -> Step<Option<lock::InstallLock<'a>>> {
+    let number = |name: &str, default: u64| {
+        context
+            .env
+            .var(name)
+            .and_then(|text| text.trim().parse().ok())
+            .unwrap_or(default)
+    };
+    let root = context.cache_dir()?.join("locks");
+    let text = version.to_string();
+    let request = LockRequest {
+        root: &root,
+        version: &text,
+        timeout_seconds: number("NVM_INSTALL_LOCK_TIMEOUT", DEFAULT_LOCK_TIMEOUT_SECONDS),
+        stale_minutes: number("NVM_INSTALL_LOCK_STALE", 0),
+        now: SystemTime::now(),
+    };
+    let mut notes = Vec::new();
+    let lock = acquire(context.fs, context.sleeper(), &request, &mut notes);
+    for note in notes {
+        transcript.err(note);
+    }
+    Ok(lock?)
+}
+
```

Apply to `src/commands/install/options/mod.rs` (above the test module):

```diff
--- a/src/commands/install/options/mod.rs
+++ b/src/commands/install/options/mod.rs
@@ -19,6 +19,9 @@
     pub alias: Option<String>,
     /// There was a version argument (even an empty one).
     pub version_given: bool,
+    /// The LTS filter came from `--lts` with no version, which `nvm.sh`
+    /// announces ("Installing latest LTS version.").
+    pub announce_lts: bool,
 }
 
 fn unsupported(option: &str) -> CliError {
@@ -72,6 +75,7 @@
             return Err(unsupported(option));
         }
     }
+    options.announce_lts = options.lts.is_some() && options.version.is_empty();
     take_lts_version(&mut options);
     Ok(options)
 }
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 439 unit tests plus the end-to-end tests.

- [ ] **Step 5: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(install): install a version from the mirror"
```

---

### Task 11: `nvm uninstall`

**Files:**

- Create: `src/commands/uninstall/mod.rs`, `src/commands/uninstall/tests.rs`
- Modify: `src/commands/mod.rs`

**Interfaces:**

- Consumes: `commands::resolve::resolve_installed`, `commands::current::detect`,
  `commands::install::{fetch::Artifact, place::version_path}`,
  `commands::unalias::run`.
- Produces: `commands::uninstall::run(&Context, &[String]) -> Result<Output,
  CliError>`.
- Behaviour (verified against `nvm.sh`): exactly one word, else the four-line
  usage with exit 127. `--lts`, `--lts=<name>`, `lts/*` and `lts/<name>` resolve
  through the `lts/*` aliases. The version in use (`nvm current`) is `nvm:
  Cannot uninstall currently-active node version, <v> (inferred from <word>).`
  (`io.js version` for io.js), exit 1. A version that is not installed is only
  `Version '<word>' is not installed.` (or `Version '<v>' (inferred from <word>)
  is not installed.`) on stderr, exit 0. Otherwise the version directory and the
  `files` directories of its cache entry are removed, `Uninstalled node <v>`
  (`io.js <v>`) is printed, and then every alias file that mentions the version
  is deleted, printing the `Deleted alias <name> - restore it with ...` line of
  `nvm unalias` for each, in name order. The cached archive stays.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -9,6 +9,7 @@
 pub mod resolve;
 pub mod transcript;
 pub mod unalias;
+pub mod uninstall;
 pub mod version;
 pub mod version_remote;
 pub mod which;
```

- [ ] **Step 2: Write the failing tests**

Create `src/commands/uninstall/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/uninstall/tests.rs`:

```rust
use std::path::Path;

use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem};
use crate::ports::FileSystem;

fn world() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_executable("/n/versions/node/v20.10.0/bin/node", "x")
        .with_executable("/n/versions/node/v18.19.0/bin/node", "x")
        .with_file("/n/.cache/bin/node-v20.10.0-linux-x64/files/top/f", "x")
        .with_file(
            "/n/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz",
            "t",
        )
}

fn run_in(fs: &FakeFileSystem, path: &str, line: &str) -> Result<Output, CliError> {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path);
    let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
    run(&Context::new(fs, &env), &words)
}

fn exists(fs: &FakeFileSystem, path: &str) -> bool {
    fs.file_info(Path::new(path)).is_ok()
}

#[test]
fn it_removes_the_version_and_its_unpack_leftovers_but_keeps_the_archive() {
    let fs = world();
    let output = run_in(&fs, "", "20").unwrap();
    assert_eq!(output.stdout, "Uninstalled node v20.10.0");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(!exists(&fs, "/n/versions/node/v20.10.0"));
    assert!(!exists(&fs, "/n/.cache/bin/node-v20.10.0-linux-x64/files"));
    assert!(exists(
        &fs,
        "/n/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz"
    ));
    assert!(exists(&fs, "/n/versions/node/v18.19.0/bin/node"));
}

#[test]
fn aliases_that_name_the_version_are_deleted_and_others_stay() {
    let fs = world()
        .with_file("/n/alias/bar", "v20.10.0\n")
        .with_file("/n/alias/foo", "20\n")
        .with_file("/n/alias/also", "# note\nv20.10.0\n")
        .with_file("/n/alias/lts/iron", "v20.10.0\n");
    let output = run_in(&fs, "", "v20.10.0").unwrap();
    assert_eq!(
        output.stdout,
        "Uninstalled node v20.10.0\n\
         Deleted alias also - restore it with `nvm alias \"also\" \"v20.10.0\"`\n\
         Deleted alias bar - restore it with `nvm alias \"bar\" \"v20.10.0\"`"
    );
    assert!(exists(&fs, "/n/alias/foo"));
    assert!(exists(&fs, "/n/alias/lts/iron"));
}

#[test]
fn lts_options_resolve_through_the_lts_aliases() {
    for line in ["--lts", "lts/*", "--lts=iron", "lts/iron"] {
        let fs = world()
            .with_file("/n/alias/lts/*", "lts/iron\n")
            .with_file("/n/alias/lts/iron", "v20.10.0\n");
        let output = run_in(&fs, "", line).unwrap();
        assert_eq!(
            output.stdout.lines().next(),
            Some("Uninstalled node v20.10.0"),
            "{line}"
        );
    }
}

#[test]
fn a_version_that_is_not_installed_is_a_message_with_status_0() {
    let fs = world();
    let output = run_in(&fs, "", "99").unwrap();
    assert_eq!(output.stderr, "Version '99' is not installed.");
    assert_eq!(
        (output.stdout.as_str(), output.status),
        ("", NvmExitCode::Success)
    );
    let lts = run_in(&FakeFileSystem::default(), "", "--lts").unwrap();
    assert_eq!(lts.stderr, "Version '--lts' is not installed.");
}

#[test]
fn a_directory_without_a_runnable_node_is_not_installed_and_is_left_alone() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.10.0/README", "x");
    let output = run_in(&fs, "", "20").unwrap();
    assert_eq!(
        output.stderr,
        "Version 'v20.10.0' (inferred from 20) is not installed."
    );
    assert!(exists(&fs, "/n/versions/node/v20.10.0/README"));
}

#[test]
fn the_version_in_use_cannot_be_uninstalled() {
    let fs = world();
    let path = "/n/versions/node/v20.10.0/bin";
    for line in ["20", "v20.10.0"] {
        let error = run_in(&fs, path, line).unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::Failure);
        assert_eq!(
            error.to_string(),
            format!(
                "nvm: Cannot uninstall currently-active node version, v20.10.0 (inferred from {line})."
            )
        );
    }
    assert!(exists(&fs, "/n/versions/node/v20.10.0/bin/node"));
}

#[test]
fn an_iojs_version_is_named_as_such() {
    let fs = FakeFileSystem::default().with_executable("/n/versions/io.js/v3.3.1/bin/node", "x");
    let active = run_in(&fs, "/n/versions/io.js/v3.3.1/bin", "iojs-v3.3.1").unwrap_err();
    assert!(
        active
            .to_string()
            .contains("currently-active io.js version, iojs-v3.3.1")
    );
    let output = run_in(&fs, "", "iojs-v3.3.1").unwrap();
    assert_eq!(output.stdout, "Uninstalled io.js v3.3.1");
}

#[test]
fn anything_but_one_word_is_the_usage_with_status_127() {
    let fs = world();
    for line in ["", "18 20"] {
        let error = run_in(&fs, "", line).unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::NotFound);
        assert!(
            error
                .to_string()
                .starts_with("Usage: nvm uninstall <version>\n       nvm uninstall --lts\n")
        );
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find module `uninstall`"
and "cannot find function `run` in this scope".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/commands/uninstall/mod.rs`:

```rust
//! `nvm uninstall <version>`: remove an installed version, its leftovers in the
//! cache, and the aliases that name it.

use crate::commands::current;
use crate::commands::install::fetch::Artifact;
use crate::commands::install::place::version_path;
use crate::commands::resolve::{Resolved, resolve_installed};
use crate::commands::transcript::Transcript;
use crate::commands::{Output, unalias};
use crate::context::Context;
use crate::domain::version::{Flavor, Version};
use crate::error::{CliError, NvmExitCode};

const USAGE: &str = "Usage: nvm uninstall <version>\n       nvm uninstall --lts\n       nvm uninstall --lts=<LTS name>\n  Run `nvm --help` for full help.";

/// The alias to resolve for what was asked: `--lts`, `--lts=<name>` and
/// `lts/*` mean the LTS aliases.
fn name_to_resolve(pattern: &str) -> String {
    match pattern {
        "--lts" => "lts/*".to_owned(),
        other => match other.strip_prefix("--lts=") {
            Some(name) => format!("lts/{name}"),
            None => other.to_owned(),
        },
    }
}

/// # Errors
/// - [`CliError::Usage`] unless exactly one word is given.
/// - [`CliError::InvalidArgument`] for the version in use.
/// - [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
///
/// A version that is not installed is only a message on stderr, with status 0,
/// as in `nvm.sh`.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let [pattern] = args else {
        return Err(CliError::Usage(USAGE.to_owned()));
    };
    let resolved = resolve_installed(context, &name_to_resolve(pattern))?;
    let mut transcript = Transcript::default();
    let Resolved::Installed(version) = resolved else {
        return Ok(not_installed(None, pattern, transcript));
    };
    refuse_the_active_version(context, &version, pattern)?;
    let path = version_path(context, &version)?;
    if !is_installed(context, &path) {
        return Ok(not_installed(Some(&version), pattern, transcript));
    }
    remove(context, &version, &path, &mut transcript)?;
    Ok(transcript.finish(NvmExitCode::Success))
}

fn not_installed(version: Option<&Version>, pattern: &str, mut transcript: Transcript) -> Output {
    match version
        .map(ToString::to_string)
        .filter(|text| text != pattern)
    {
        Some(text) => transcript.err(format!(
            "Version '{text}' (inferred from {pattern}) is not installed."
        )),
        None => transcript.err(format!("Version '{pattern}' is not installed.")),
    }
    transcript.finish(NvmExitCode::Success)
}

fn is_installed(context: &Context<'_>, version_path: &std::path::Path) -> bool {
    context
        .fs
        .file_info(&version_path.join("bin/node"))
        .is_ok_and(|node| !node.is_dir && node.executable)
}

fn refuse_the_active_version(
    context: &Context<'_>,
    version: &Version,
    pattern: &str,
) -> Result<(), CliError> {
    if current::detect(context)?.to_string() != version.to_string() {
        return Ok(());
    }
    let kind = match version.flavor {
        Flavor::Node => "node",
        Flavor::IoJs => "io.js",
    };
    Err(CliError::InvalidArgument(format!(
        "nvm: Cannot uninstall currently-active {kind} version, {version} (inferred from {pattern})."
    )))
}

fn remove(
    context: &Context<'_>,
    version: &Version,
    path: &std::path::Path,
    transcript: &mut Transcript,
) -> Result<(), CliError> {
    let cache = context.cache_dir()?;
    let source_slug = match version.flavor {
        Flavor::Node => format!("node-{}", version.directory_name()),
        Flavor::IoJs => format!("iojs-{}", version.directory_name()),
    };
    if let Some(artifact) = Artifact::of(context, version) {
        let _ = context.fs.remove_dir_all(&artifact.files());
    }
    let _ = context
        .fs
        .remove_dir_all(&cache.join("src").join(source_slug).join("files"));
    context
        .fs
        .remove_dir_all(path)
        .map_err(|source| CliError::io(path, source))?;
    let name = match version.flavor {
        Flavor::Node => "node",
        Flavor::IoJs => "io.js",
    };
    transcript.out(format!("Uninstalled {name} {}", version.directory_name()));
    remove_aliases_to(context, version, transcript)
}

/// Every alias file that mentions the version is deleted.
fn remove_aliases_to(
    context: &Context<'_>,
    version: &Version,
    transcript: &mut Transcript,
) -> Result<(), CliError> {
    let directory = context.alias_dir()?;
    let mut names: Vec<String> = context
        .fs
        .read_dir(&directory)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.name)
        .collect();
    names.sort();
    let text = version.to_string();
    for name in names {
        let mentions = context
            .fs
            .read_to_string(&directory.join(&name))
            .is_ok_and(|contents| contents.contains(&text));
        if mentions {
            transcript.absorb(unalias::run(context, &[name])?);
        }
    }
    Ok(())
}

```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 447 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(uninstall): remove a version, its unpack leftovers and the aliases that name it"
```

---

### Task 12: The CLI, end-to-end tests and the gate

**Files:**

- Create: `tests/install_cli.rs`
- Modify: `src/cli/mod.rs`, `src/cli/tests.rs`

**Interfaces:**

- Produces: the `install` (alias `i`) and `uninstall` subcommands, whose words
  are taken verbatim; the real stack in `run_from_env` (`Sha256Digest`,
  `TarGzArchive`, `StdSleeper`, and the `Platform` of the host, with Alpine
  detected from `/etc/alpine-release`); acceptance tests that run the real
  binary against a mirror served from a `TcpListener` that holds a real
  `.tar.gz` and its checksum: a fresh install (and `ls` seeing it, the node
  running), a second install, an uninstall followed by a reinstall from the
  cache, a wrong checksum (exit 2, nothing installed), an unknown version
  (exit 3), and `--lts`.

- [ ] **Step 1: Write the failing tests**

Apply to `src/cli/tests.rs`:

```diff
--- a/src/cli/tests.rs
+++ b/src/cli/tests.rs
@@ -266,3 +266,24 @@
         assert_eq!(code, 0);
     }
 }
+
+#[test]
+fn install_and_its_alias_i_without_a_version_are_a_usage_error_with_exit_127() {
+    for command in ["install", "i"] {
+        let (code, out, err) = run_cli(&["nvm", command]);
+        assert_eq!((code, out.as_str()), (127, ""));
+        assert!(
+            err.starts_with("No version provided and no .nvmrc file found\nUsage: nvm install")
+        );
+    }
+}
+
+#[test]
+fn uninstall_needs_one_word_and_a_missing_version_is_only_a_message() {
+    let (code, _, err) = run_cli(&["nvm", "uninstall"]);
+    assert_eq!(code, 127);
+    assert!(err.starts_with("Usage: nvm uninstall <version>"));
+    let (code, out, err) = run_cli(&["nvm", "uninstall", "99"]);
+    assert_eq!((code, out.as_str()), (0, ""));
+    assert_eq!(err, "Version '99' is not installed.\n");
+}
```

Create `tests/install_cli.rs`:

```rust
//! End-to-end: `install` and `uninstall` with the real binary, a real
//! temporary `$NVM_DIR` and a mirror served on a local port that holds a real
//! `.tar.gz`.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};
use std::thread;

use flate2::Compression;
use flate2::write::GzEncoder;
use nvmrc::domain::platform::Platform;
use sha2::{Digest, Sha256};

const NODE_INDEX: &str = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n\
v20.10.0\t2023-11-22\tx\t10.2.3\t11.3\t1.46\t1.3\t3.0\t115\tIron\t-\n\
v18.19.0\t2023-11-29\tx\t10.2.3\t10.2\t1.44\t1.3\t3.0\t108\tHydrogen\t-\n";
const IOJS_INDEX: &str =
    "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n";
const INHERITED_NETWORK_SETTINGS: [&str; 7] = [
    "ALL_PROXY",
    "all_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
    "NVM_AUTH_HEADER",
];

/// The archive name nvmrc will ask for on this machine, or `None` where it has
/// no binaries (the tests then have nothing to check).
fn slug(version: &str) -> Option<String> {
    let platform = Platform::from_host(std::env::consts::OS, std::env::consts::ARCH, false)?;
    Some(platform.download_slug(&version.parse().unwrap()))
}

/// `<slug>/bin/node` (executable) and `<slug>/lib/README`, gzip-compressed.
fn tarball(slug: &str, version: &str) -> Vec<u8> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let script = format!("#!/bin/sh\necho {version}\n");
    for (path, contents, mode) in [
        (format!("{slug}/bin/node"), script.as_str(), 0o755),
        (format!("{slug}/lib/README"), "readme", 0o644),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        builder
            .append_data(&mut header, path, contents.as_bytes())
            .unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Serves each path with its body and 404 for the rest; returns the URL.
fn serve(files: BTreeMap<String, Vec<u8>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).unwrap_or(0);
            let text = String::from_utf8_lossy(&request[..read]).into_owned();
            let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let (status, body) = match files.get(&path) {
                Some(body) => ("200 OK", body.clone()),
                None => ("404 Not Found", Vec::new()),
            };
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    format!("http://127.0.0.1:{port}")
}

/// A mirror with Node 20.10.0 whose checksum is `listed` (or the right one).
fn mirror(listed: Option<&str>) -> Option<String> {
    let slug = slug("v20.10.0")?;
    let archive = tarball(&slug, "v20.10.0");
    let name = format!("{slug}.tar.gz");
    let checksum = listed.map_or_else(|| digest(&archive), str::to_owned);
    let mut files = BTreeMap::new();
    files.insert("/index.tab".to_owned(), NODE_INDEX.as_bytes().to_vec());
    files.insert(
        "/v20.10.0/SHASUMS256.txt".to_owned(),
        format!("{checksum}  {name}\n").into_bytes(),
    );
    files.insert(format!("/v20.10.0/{name}"), archive);
    Some(serve(files))
}

fn nvm(nvm_dir: &Path, mirror: &str, args: &[&str]) -> Output {
    let iojs = serve(BTreeMap::from([(
        "/index.tab".to_owned(),
        IOJS_INDEX.as_bytes().to_vec(),
    )]));
    let mut command = Command::new(env!("CARGO_BIN_EXE_nvm"));
    command
        .args(args)
        .env("NVM_DIR", nvm_dir)
        .env("PATH", "/nonexistent")
        .env("NVM_NODEJS_ORG_MIRROR", mirror)
        .env("NVM_IOJS_ORG_MIRROR", iojs)
        .env("NO_PROXY", "127.0.0.1");
    for name in INHERITED_NETWORK_SETTINGS {
        command.env_remove(name);
    }
    command.output().expect("run the binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn install_puts_a_working_version_in_place_and_ls_sees_it() {
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), &mirror, &["install", "20"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "Downloading and installing node v20.10.0...\n\
         Creating default alias: default -> 20 (-> v20.10.0 *)\n"
    );
    assert!(stderr(&output).contains("Checksums matched!"));
    let node = home.path().join("versions/node/v20.10.0/bin/node");
    assert_eq!(
        fs::read_to_string(&node).unwrap(),
        "#!/bin/sh\necho v20.10.0\n"
    );
    let listed = nvm(home.path(), &mirror, &["ls", "--no-alias"]);
    assert_eq!(stdout(&listed), "       v20.10.0 *\n");
    assert!(
        !home
            .path()
            .join(".cache/bin")
            .join(slug("v20.10.0").unwrap())
            .join("files")
            .exists()
    );
}

#[cfg(unix)]
#[test]
fn the_installed_node_can_run() {
    use std::os::unix::fs::PermissionsExt;
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    nvm(home.path(), &mirror, &["install", "20"]);
    let node = home.path().join("versions/node/v20.10.0/bin/node");
    assert_ne!(fs::metadata(&node).unwrap().permissions().mode() & 0o111, 0);
    let ran = Command::new(&node).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&ran.stdout), "v20.10.0\n");
}

#[test]
fn a_second_install_says_so_and_a_cached_archive_is_reused_after_uninstall() {
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    nvm(home.path(), &mirror, &["install", "20"]);
    let again = nvm(home.path(), &mirror, &["install", "v20.10.0"]);
    assert_eq!(stderr(&again), "v20.10.0 is already installed.\n");
    assert_eq!(stdout(&again), "");
    let removed = nvm(home.path(), &mirror, &["uninstall", "20"]);
    assert_eq!(stdout(&removed), "Uninstalled node v20.10.0\n");
    assert!(!home.path().join("versions/node/v20.10.0").exists());
    let reinstalled = nvm(home.path(), &mirror, &["install", "20"]);
    assert!(stderr(&reinstalled).contains("Checksums match! Using existing downloaded archive"));
    assert!(
        home.path()
            .join("versions/node/v20.10.0/bin/node")
            .is_file()
    );
}

#[test]
fn a_wrong_checksum_installs_nothing_and_exits_2() {
    let Some(mirror) = mirror(Some("0000")) else {
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), &mirror, &["install", "20"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("Checksums do not match:"));
    assert!(stderr(&output).ends_with("Binary download failed. Download from source aborted.\n"));
    assert!(!home.path().join("versions").exists());
}

#[test]
fn a_version_that_is_not_on_the_mirror_exits_3() {
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), &mirror, &["install", "99"]);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(
        stderr(&output),
        "Version '99' not found - try `nvm ls-remote` to browse available versions.\n"
    );
}

#[test]
fn install_lts_makes_the_default_point_at_lts_star() {
    let Some(mirror) = mirror(None) else { return };
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), &mirror, &["install", "--lts"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).starts_with("Installing latest LTS version.\n"));
    assert_eq!(
        fs::read_to_string(home.path().join("alias/default")).unwrap(),
        "lts/*\n"
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL: the new CLI unit tests fail with `error: unrecognized
subcommand 'install'` (exit code 127 instead of the usage), and the end-to-end
tests fail the same way, because the binary has no `install` or `uninstall`
subcommand yet.

- [ ] **Step 3: Wire the commands into the CLI**

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -2,20 +2,24 @@
 
 use std::ffi::OsString;
 use std::io::Write;
+use std::path::Path;
 
 use clap::{Parser, Subcommand};
 
 use crate::adapters::retrying_http::RetryingHttp;
+use crate::adapters::sha256_digest::Sha256Digest;
 use crate::adapters::std_env::StdEnv;
 use crate::adapters::std_fs::StdFileSystem;
 use crate::adapters::std_process::StdProcess;
 use crate::adapters::std_sleeper::StdSleeper;
+use crate::adapters::targz_archive::TarGzArchive;
 use crate::adapters::ureq_http::UreqHttp;
 use crate::commands::{self, Output};
 use crate::context::Context;
 use crate::domain::http_header::sanitize_auth_header;
+use crate::domain::platform::Platform;
 use crate::error::{CliError, NvmExitCode};
-use crate::ports::Env;
+use crate::ports::{Env, FileSystem};
 
 #[derive(Parser)]
 #[command(name = "nvm", version, about = "Node Version Manager, in Rust")]
@@ -50,6 +54,19 @@
         #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
         args: Vec<String>,
     },
+    /// Download and install a version (a partial version, an alias, `--lts`,
+    /// `lts/<name>`); `--default` or `--alias=<name>` also make an alias.
+    #[command(visible_alias = "i")]
+    Install {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
+    /// Remove an installed version (`--lts` and `--lts=<name>` pick one by
+    /// its LTS alias) and the aliases that name it.
+    Uninstall {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
     /// Print the newest release a version, alias or `--lts[=name]` stands for
     /// on the mirror (`N/A` when there is none).
     #[command(name = "version-remote")]
@@ -79,6 +96,8 @@
         Command::LsRemote { args } => commands::ls_remote::run(context, args),
         Command::VersionRemote { args } => commands::version_remote::run(context, args),
         Command::Cache { args } => commands::cache::run(context, args),
+        Command::Install { args } => commands::install::run(context, args),
+        Command::Uninstall { args } => commands::uninstall::run(context, args),
         Command::Which { version } => commands::which::run(context, version.as_deref()),
         Command::Alias { args } => commands::aliases::run(context, args),
         Command::Unalias { names } => commands::unalias::run(context, names),
@@ -156,9 +175,15 @@
         .map(|value| sanitize_auth_header(&value));
     let network = UreqHttp::new(auth_header);
     let http = RetryingHttp::new(&network, &StdSleeper);
+    let musl = StdFileSystem.is_file(Path::new("/etc/alpine-release"));
+    let platform = Platform::from_host(std::env::consts::OS, std::env::consts::ARCH, musl);
     let context = Context::new(&StdFileSystem, &StdEnv)
         .with_process(&process)
-        .with_http(&http);
+        .with_http(&http)
+        .with_digest(&Sha256Digest)
+        .with_archive(&TarGzArchive)
+        .with_sleeper(&StdSleeper)
+        .with_platform(platform);
     run(
         std::env::args_os(),
         &context,
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 449 unit tests plus 6 + 10 + 6 + 5 + 1 end-to-end tests (7 +
10 + 6 + 5 + 1 on Linux). The end-to-end tests also pass with `HTTP_PROXY` and
`ALL_PROXY` set to an unreachable address, because the helper strips them.

- [ ] **Step 5: Run the full quality gate**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
rustup run 1.88 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings, every test passing, no
advisories, and the crate still builds on MSRV 1.88.

Also check that no file is over 300 lines:

```bash
find src tests -name '*.rs' | xargs wc -l | sort -n | tail -3
```

- [ ] **Step 6: Run the tests on Linux**

The repository's `Dockerfile` runs `cargo test`:

```bash
docker buildx build -f Dockerfile -t nvmrc:trixie .
docker run --rm nvmrc:trixie
```

Expected: the same test counts, all passing.

- [ ] **Step 7: Compare with the real `nvm.sh` (optional, manual)**

Serve a fixture mirror (an `index.tab`, and per version a `SHASUMS256.txt` and
the archives) on a local port, point `NVM_NODEJS_ORG_MIRROR` at it, and run
the same `install` and `uninstall` commands with the reference `nvm.sh` and
with the binary, in two empty `NVM_DIR`s. Apart from the deviations listed at
the end of this plan (the `.tar.xz` name, `Now using ...`, the progress bar,
`Computing checksum with ...`, and, until Task 20, the source fallback), the
output and the exit status are the same. Build the fixture archives with
`COPYFILE_DISABLE=1` on macOS, so `tar` does not add `._*` entries.

- [ ] **Step 8: Commit**

```bash
git add src tests
git commit -S -m "feat(cli): add install and uninstall and run them end to end"
```

---

### Task 13: Running programs to the end

**Files:**

- Modify: `src/ports/mod.rs`, `src/adapters/std_process.rs`,
  `src/adapters/no_process.rs`, `src/fakes/process.rs`

**Interfaces:**

- Produces: `ports::{Invocation, Completed}` and `Process::execute(&Invocation)
  -> io::Result<Completed>`. `Invocation::new(program)` with `.args(&[&str])`,
  `.dir(path)`, `.env(name, value)` and `.path_prefix(dir)`; `Completed {
  success, stdout, stderr }`. `StdProcess::execute` has no time limit and no
  output cap, reads both streams at once (a full pipe cannot stall the
  program), closes stdin, and puts `path_prefix` first on `PATH`;
  `NoProcess::execute` is `Unsupported`; the test-only `FakeProcess` gains
  `with_execution(program, args, Completed)`, `with_success(program, args,
  stdout)`, `with_effect(program, args, closure)` and `executed()` (every
  `Invocation` it was given). `Process::run` (bounded, for `node --version`)
  is unchanged.

- [ ] **Step 1: Write the failing tests**

Apply to the test module of `src/adapters/no_process.rs`:

```diff
--- a/src/adapters/no_process.rs
+++ b/src/adapters/no_process.rs
@@ -6,5 +6,9 @@
     fn runs_nothing() {
         let error = NoProcess.run(Path::new("/bin/true"), &[]).unwrap_err();
         assert_eq!(error.kind(), io::ErrorKind::Unsupported);
+        let error = NoProcess
+            .execute(&Invocation::new("/bin/true"))
+            .unwrap_err();
+        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
     }
 }
```

Apply to the test module of `src/fakes/process.rs`:

```diff
--- a/src/fakes/process.rs
+++ b/src/fakes/process.rs
@@ -1,6 +1,18 @@
 #[cfg(test)]
 mod tests {
     use super::*;
+
+    #[test]
+    fn fake_process_executes_by_program_and_arguments_and_records_what_it_ran() {
+        let process = FakeProcess::default().with_success("/n/bin/npm", "--version", "10.2.3\n");
+        let invocation = Invocation::new("/n/bin/npm")
+            .args(&["--version"])
+            .dir("/work");
+        assert_eq!(process.execute(&invocation).unwrap().stdout, "10.2.3\n");
+        let other = Invocation::new("/n/bin/npm").args(&["list"]);
+        assert!(process.execute(&other).is_err());
+        assert_eq!(process.executed(), [invocation, other]);
+    }
 
     #[test]
     fn fake_process_answers_by_program_path() {
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find struct
`Invocation`", "cannot find struct `Completed`" and "no method named `execute`
found".

- [ ] **Step 3: Write the implementation**

Apply to `src/adapters/no_process.rs` (above the test module):

```diff
--- a/src/adapters/no_process.rs
+++ b/src/adapters/no_process.rs
@@ -1,12 +1,16 @@
 use std::io;
 use std::path::Path;
 
-use crate::ports::{Process, ProcessOutput};
+use crate::ports::{Completed, Invocation, Process, ProcessOutput};
 
 /// The `Process` of a `Context` that was not given one: it runs nothing.
 pub struct NoProcess;
 
 impl Process for NoProcess {
+    fn execute(&self, _invocation: &Invocation) -> io::Result<Completed> {
+        Err(io::Error::from(io::ErrorKind::Unsupported))
+    }
+
     fn run(&self, _program: &Path, _args: &[&str]) -> io::Result<ProcessOutput> {
         Err(io::Error::from(io::ErrorKind::Unsupported))
     }
```

Apply to `src/adapters/std_process.rs` (above the test module):

```diff
--- a/src/adapters/std_process.rs
+++ b/src/adapters/std_process.rs
@@ -5,7 +5,7 @@
 use std::thread;
 use std::time::{Duration, Instant};
 
-use crate::ports::{Process, ProcessOutput};
+use crate::ports::{Completed, Invocation, Process, ProcessOutput};
 
 const POLL_INTERVAL: Duration = Duration::from_millis(10);
 
@@ -30,6 +30,33 @@
 }
 
 impl Process for StdProcess {
+    /// No time limit and no cap: a build may take an hour and print a lot.
+    /// Both streams are read at once, so a full pipe cannot stall the program.
+    fn execute(&self, invocation: &Invocation) -> io::Result<Completed> {
+        let mut command = Command::new(&invocation.program);
+        command
+            .args(&invocation.args)
+            .envs(invocation.env.iter().map(|(name, value)| (name, value)))
+            .stdin(Stdio::null())
+            .stdout(Stdio::piped())
+            .stderr(Stdio::piped());
+        if let Some(dir) = &invocation.dir {
+            command.current_dir(dir);
+        }
+        if let Some(prefix) = &invocation.path_prefix {
+            command.env("PATH", path_with_prefix(prefix)?);
+        }
+        let mut child = command.spawn()?;
+        let stdout = collect(child.stdout.take());
+        let stderr = collect(child.stderr.take());
+        let status = child.wait()?;
+        Ok(Completed {
+            success: status.success(),
+            stdout: stdout.join().unwrap_or_default(),
+            stderr: stderr.join().unwrap_or_default(),
+        })
+    }
+
     fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput> {
         let deadline = Instant::now() + self.timeout;
         let mut child = Command::new(program)
@@ -52,6 +79,24 @@
     }
 }
 
+/// `prefix` in front of the `PATH` this process has.
+fn path_with_prefix(prefix: &Path) -> io::Result<std::ffi::OsString> {
+    let current = std::env::var_os("PATH").unwrap_or_default();
+    let directories = std::iter::once(prefix.to_path_buf()).chain(std::env::split_paths(&current));
+    std::env::join_paths(directories).map_err(io::Error::other)
+}
+
+/// Reads a pipe to its end on another thread.
+fn collect<R: Read + Send + 'static>(pipe: Option<R>) -> thread::JoinHandle<String> {
+    thread::spawn(move || {
+        let mut bytes = Vec::new();
+        if let Some(mut pipe) = pipe {
+            let _ = pipe.read_to_end(&mut bytes);
+        }
+        String::from_utf8_lossy(&bytes).into_owned()
+    })
+}
+
 /// Reads at most `cap` bytes on another thread. The thread is never joined:
 /// a grandchild holding the pipe open must not hold `nvm` up with it.
 fn read_in_background(stdout: ChildStdout, cap: usize) -> Receiver<io::Result<Vec<u8>>> {
@@ -148,4 +193,62 @@
         let missing = Path::new("/nonexistent/node");
         assert!(StdProcess::default().run(missing, &[]).is_err());
     }
-}
+
+    fn execute(script: &str) -> Completed {
+        let invocation = Invocation::new("/bin/sh").args(&["-c", script]);
+        StdProcess::default().execute(&invocation).unwrap()
+    }
+
+    #[test]
+    fn execute_returns_both_streams_and_the_outcome() {
+        let done = execute("echo out; echo err >&2; exit 3");
+        assert_eq!(
+            (done.stdout.as_str(), done.stderr.as_str()),
+            ("out\n", "err\n")
+        );
+        assert!(!done.success);
+        assert!(execute("true").success);
+    }
+
+    #[test]
+    fn execute_has_no_output_cap_and_no_deadlock_on_a_full_pipe() {
+        let done = execute(
+            "head -c 300000 /dev/zero | tr '\\0' a; head -c 300000 /dev/zero | tr '\\0' b >&2",
+        );
+        assert_eq!((done.stdout.len(), done.stderr.len()), (300_000, 300_000));
+    }
+
+    #[test]
+    fn execute_runs_where_it_is_told_with_the_environment_it_is_given() {
+        let root = tempfile::tempdir().unwrap();
+        let invocation = Invocation::new("/bin/sh")
+            .args(&["-c", "pwd; echo $GREETING"])
+            .dir(root.path())
+            .env("GREETING", "hello");
+        let done = StdProcess::default().execute(&invocation).unwrap();
+        let lines: Vec<&str> = done.stdout.lines().collect();
+        let real = std::fs::canonicalize(root.path()).unwrap();
+        assert_eq!(std::path::PathBuf::from(lines[0]), real);
+        assert_eq!(lines[1], "hello");
+    }
+
+    #[test]
+    fn execute_puts_the_prefix_first_on_the_path() {
+        let root = tempfile::tempdir().unwrap();
+        let tool = root.path().join("mytool");
+        std::fs::write(&tool, "#!/bin/sh\necho from-prefix\n").unwrap();
+        std::fs::set_permissions(&tool, std::os::unix::fs::PermissionsExt::from_mode(0o755))
+            .unwrap();
+        let invocation = Invocation::new("/bin/sh")
+            .args(&["-c", "mytool"])
+            .path_prefix(root.path());
+        let done = StdProcess::default().execute(&invocation).unwrap();
+        assert_eq!(done.stdout, "from-prefix\n");
+    }
+
+    #[test]
+    fn execute_of_a_missing_program_is_an_error() {
+        let invocation = Invocation::new("/nonexistent/npm");
+        assert!(StdProcess::default().execute(&invocation).is_err());
+    }
+}
```

Apply to `src/fakes/process.rs` (above the test module):

```diff
--- a/src/fakes/process.rs
+++ b/src/fakes/process.rs
@@ -1,13 +1,16 @@
+use std::cell::RefCell;
 use std::collections::BTreeMap;
 use std::io;
 use std::path::{Path, PathBuf};
 
-use crate::ports::{Process, ProcessOutput};
+use crate::ports::{Completed, Invocation, Process, ProcessOutput};
 
 /// Programs by path: each prints a fixed output, or fails when it has none.
 #[derive(Default)]
 pub struct FakeProcess {
     outputs: BTreeMap<PathBuf, ProcessOutput>,
+    executions: BTreeMap<(PathBuf, String), Completed>,
+    executed: RefCell<Vec<Invocation>>,
 }
 
 impl FakeProcess {
@@ -32,7 +35,44 @@
     }
 }
 
+impl FakeProcess {
+    /// What `execute` answers to `program` run with exactly `args` (joined
+    /// with spaces); any other invocation is not found.
+    #[must_use]
+    pub fn with_execution(mut self, program: &str, args: &str, done: Completed) -> Self {
+        self.executions
+            .insert((PathBuf::from(program), args.to_owned()), done);
+        self
+    }
+
+    /// A successful execution that printed `stdout`.
+    #[must_use]
+    pub fn with_success(self, program: &str, args: &str, stdout: &str) -> Self {
+        let done = Completed {
+            success: true,
+            stdout: stdout.to_owned(),
+            stderr: String::new(),
+        };
+        self.with_execution(program, args, done)
+    }
+
+    /// Every invocation `execute` was given, in order.
+    #[must_use]
+    pub fn executed(&self) -> Vec<Invocation> {
+        self.executed.borrow().clone()
+    }
+}
+
 impl Process for FakeProcess {
+    fn execute(&self, invocation: &Invocation) -> io::Result<Completed> {
+        self.executed.borrow_mut().push(invocation.clone());
+        let key = (invocation.program.clone(), invocation.args.join(" "));
+        self.executions
+            .get(&key)
+            .cloned()
+            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
+    }
+
     fn run(&self, program: &Path, _args: &[&str]) -> io::Result<ProcessOutput> {
         self.outputs
             .get(program)
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -2,7 +2,7 @@
 
 use std::ffi::OsString;
 use std::io;
-use std::path::Path;
+use std::path::{Path, PathBuf};
 use std::time::{Duration, SystemTime};
 
 use thiserror::Error;
@@ -93,7 +93,70 @@
     pub stdout: String,
 }
 
+/// A program to run to completion, with no time limit: `npm install`,
+/// `./configure`, `make`.
+#[derive(Debug, Clone, Default, PartialEq, Eq)]
+pub struct Invocation {
+    pub program: PathBuf,
+    pub args: Vec<String>,
+    /// Where to run it; the current directory when `None`.
+    pub dir: Option<PathBuf>,
+    /// Variables added to the environment it inherits.
+    pub env: Vec<(String, String)>,
+    /// A directory put in front of `PATH`, so a `node` that `npm` starts is
+    /// the one next to it.
+    pub path_prefix: Option<PathBuf>,
+}
+
+impl Invocation {
+    #[must_use]
+    pub fn new(program: impl Into<PathBuf>) -> Self {
+        Self {
+            program: program.into(),
+            ..Self::default()
+        }
+    }
+
+    #[must_use]
+    pub fn args(mut self, args: &[&str]) -> Self {
+        self.args.extend(args.iter().map(|arg| (*arg).to_owned()));
+        self
+    }
+
+    #[must_use]
+    pub fn dir(mut self, dir: impl Into<PathBuf>) -> Self {
+        self.dir = Some(dir.into());
+        self
+    }
+
+    #[must_use]
+    pub fn env(mut self, name: &str, value: &str) -> Self {
+        self.env.push((name.to_owned(), value.to_owned()));
+        self
+    }
+
+    #[must_use]
+    pub fn path_prefix(mut self, directory: impl Into<PathBuf>) -> Self {
+        self.path_prefix = Some(directory.into());
+        self
+    }
+}
+
+/// What a program that ran to the end printed.
+#[derive(Debug, Clone, Default, PartialEq, Eq)]
+pub struct Completed {
+    pub success: bool,
+    pub stdout: String,
+    pub stderr: String,
+}
+
 pub trait Process {
+    /// Runs `invocation` until it ends and returns everything it printed.
+    ///
+    /// # Errors
+    /// Fails when the program cannot be started.
+    fn execute(&self, invocation: &Invocation) -> io::Result<Completed>;
+
     /// Runs `program` with `args` and waits for it.
     ///
     /// # Errors
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 455 unit tests plus the end-to-end tests.

- [ ] **Step 5: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(process): run a program to the end and keep its output"
```

---

### Task 14: What `nvm` knows about `npm` without running it

**Files:**

- Create: `src/domain/npm/mod.rs`, `src/domain/npm/upgrade/mod.rs`,
  `src/domain/npm/upgrade/tests.rs`, `src/domain/npm/default_packages/mod.rs`,
  `src/domain/npm/default_packages/tests.rs`,
  `src/domain/npm/global_packages/mod.rs`,
  `src/domain/npm/global_packages/tests.rs`
- Modify: `src/domain/mod.rs`

**Interfaces:**

- Produces: `domain::npm::upgrade::{steps, Step}`: `steps(node, npm) ->
  Vec<Step>` for `(major, minor, patch)` triples, where `Step { note:
  Option<&str>, install: Option<&str> }` (`None` installs plain `npm`, the
  latest); `domain::npm::default_packages::{parse, DefaultPackagesError}`;
  `domain::npm::global_packages::{parse, GlobalPackages { installs, links }}`.
- Behaviour (every expectation was printed by the real `nvm.sh`:
  `nvm_install_latest_npm` run with `NVM_DEBUG=1` for 41 `node` versions, 35
  more `(node, npm)` pairs and 39 distinct notes; `nvm_get_default_packages`
  for eight files; `nvm_npm_global_modules` for six listings): the newest
  `npm` that works on a `node` is a table (`node` below 1.1 gets `npm@4.5`,
  below 4 `npm@4`, then `5.3`, `5.4.1`, `5`, `6.9`, `6`, `7`, `8.6`, `9`, `10`
  and finally the latest; an `npm` 1.x or 2.x first hops to `1.4.28` or `2`,
  and an `npm` below 4.4.4 on a `node` below 10 hops to `npm@4` before `6`;
  `node` 0.6 can only reach `1.3` and 0.6 and 0.9 are told they cannot go
  further). `default-packages` is one package per line with `#` comments; the
  result is the packages joined by spaces with a space in front of every one
  that is not on the first line (a quirk kept on purpose), and a line with a
  blank in it is the error `Only one package per line is allowed in
  `<file>`. Please remove any lines with multiple space-separated values.`.
  `npm list -g --depth=0` is read as the `sed` of `nvm.sh` does: without its
  first line and the unmet peer dependencies, `name@version` of each package
  other than `npm` and `corepack`, and the target of each `pkg -> target`
  link.
- [ ] **Step 1: Declare the new modules**

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -10,6 +10,7 @@
 pub mod index;
 pub mod listing;
 pub mod mirror;
+pub mod npm;
 pub mod nvmrc;
 pub mod path_search;
 pub mod platform;
```

Create `src/domain/npm/mod.rs`:

```rust
//! What `nvm` knows about `npm` without running it: which `npm` a `node` can
//! upgrade to, the `default-packages` file, and the output of `npm list -g`.

pub mod default_packages;
pub mod global_packages;
pub mod upgrade;
```

- [ ] **Step 2: Write the failing tests**

Create `src/domain/npm/default_packages/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/domain/npm/default_packages/tests.rs`:

```rust
use super::*;

const FILE: &str = "/n/default-packages";

fn parsed(contents: &str) -> Result<String, DefaultPackagesError> {
    parse(contents, FILE)
}

/// Every expectation is what `nvm_get_default_packages` of the real `nvm.sh`
/// printed for the same file.
#[test]
fn one_package_per_line_are_joined_by_spaces() {
    assert_eq!(parsed("a\nb\n").unwrap(), "a b");
    assert_eq!(parsed("a\n#c\nb").unwrap(), "a b");
}

#[test]
fn a_package_that_is_not_on_the_first_line_gets_a_space_in_front_like_nvm_sh() {
    assert_eq!(parsed("# c\na\n\nb\n").unwrap(), " a b");
    assert_eq!(parsed("\n\na\n").unwrap(), " a");
}

#[test]
fn comments_and_blank_lines_alone_are_no_packages() {
    assert_eq!(parsed("#x\n#y\n").unwrap(), "");
    assert_eq!(parsed("").unwrap(), "");
}

#[test]
fn two_values_on_a_line_or_any_blank_in_it_is_an_error_naming_the_file() {
    for contents in ["a b\n", "  a\n", "a\tb\n"] {
        let error = parsed(contents).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Only one package per line is allowed in `/n/default-packages`. Please remove any lines with multiple space-separated values."
        );
    }
}
```

Create `src/domain/npm/global_packages/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/domain/npm/global_packages/tests.rs`:

```rust
use super::*;

fn parsed(output: &str) -> (Vec<String>, Vec<String>) {
    let packages = parse(output);
    (packages.installs, packages.links)
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_owned()).collect()
}

/// Every expectation is what `nvm_npm_global_modules` of the real `nvm.sh`
/// printed for the same `npm list -g --depth=0`.
#[test]
fn plain_and_scoped_packages_are_installed_but_npm_and_corepack_are_not() {
    let output = "/home/me/.nvm/versions/node/v20/lib\n├── corepack@0.20.0\n├── npm@10.2.3\n├── yarn@1.22.19\n└── @angular/cli@17.0.0\n";
    let (installs, links) = parsed(output);
    assert_eq!(installs, words(&["yarn@1.22.19", "@angular/cli@17.0.0"]));
    assert!(links.is_empty());
}

#[test]
fn links_unmet_peers_and_trailing_words_are_handled() {
    let output = "/x/lib\n├── corepack@0.20.0\n├── npm@10.2.3\n├── lodash@4.17.21 extraneous\n├── mylink@1.0.0 -> /home/me/src/mylink\n├── typescript@5.3.3\n└── UNMET PEER DEPENDENCY react@^18\n";
    let (installs, links) = parsed(output);
    assert_eq!(installs, words(&["lodash@4.17.21", "typescript@5.3.3"]));
    assert_eq!(links, words(&["/home/me/src/mylink"]));
}

#[test]
fn an_empty_list_has_nothing() {
    assert_eq!(parsed("/x/lib\n└── (empty)\n"), (vec![], vec![]));
    assert_eq!(parsed("/x/lib\n"), (vec![], vec![]));
    assert_eq!(parsed(""), (vec![], vec![]));
}

#[test]
fn old_npm_tree_drawings_work_too() {
    let (installs, _) = parsed("/x/lib\n+-- foo@1.0.0\n`-- bar@2.0.0\n");
    assert_eq!(installs, words(&["foo@1.0.0", "bar@2.0.0"]));
}

#[test]
fn a_line_that_does_not_fit_stays_as_it_is_and_the_last_at_wins() {
    let (installs, _) = parsed("/x/lib\n├── weird line without at\n├── a@b@c d\n");
    assert_eq!(
        installs,
        words(&["├──", "weird", "line", "without", "at", "a@b@c"])
    );
}
```

Create `src/domain/npm/upgrade/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/domain/npm/upgrade/tests.rs`:

```rust
use super::*;

fn triple(text: &str) -> Triple {
    let mut parts = text.split('.').map(|part| part.parse().unwrap());
    (
        parts.next().unwrap(),
        parts.next().unwrap(),
        parts.next().unwrap(),
    )
}

/// The `npm install -g` arguments of a plan, `npm` alone for the latest.
fn installs(node: &str, npm: &str) -> Vec<&'static str> {
    steps(triple(node), triple(npm))
        .iter()
        .filter_map(|step| {
            step.install.or(Some("npm")).filter(|_| {
                step.install.is_some() || step.note.is_some_and(|n| n.contains("Installing latest"))
            })
        })
        .collect()
}

/// Every expectation below is what `nvm_install_latest_npm` of the real
/// `nvm.sh` printed (with `NVM_DEBUG=1`) for the same node and npm.
const NOTES: [&str; 17] = [
    "* `node` v0.6.x can only upgrade to `npm` v1.3.x",
    "* node v0.6 and v0.9 are unable to upgrade further",
    "* `npm` v4.5.x is the last version that works on `node` versions < v1.1.0",
    "* `npm` v1.x needs to first jump to `npm` v1.4.28 to be able to upgrade further",
    "* `npm` v2.x needs to first jump to the latest v2 to be able to upgrade further",
    "* `npm` v5 and higher do not work on `node` versions below v4.0.0",
    "* `npm` `v5.3.x` is the last version that works on `node` 4.x versions below v4.4, or 5.x versions below v5.10, due to `Buffer.alloc`",
    "* `npm` `v5.4.1` is the last version that works on `node` `v4.5` and `v4.6`",
    "* `npm` `v5.x` is the last version that works on `node` below `v6.0.0`",
    "* `npm` `v6.9` is the last version that works on `node` `v6.0.x`, `v6.1.x`, `v9.0.x`, `v9.1.x`, or `v9.2.x`",
    "* `npm` `v6.x` is the last version that works on `node` below `v10.0.0`",
    "* `npm` `v4.4.4` or later is required to install npm v6.14.18",
    "* `npm` `v7.x` is the last version that works on `node` `v13`, `v15`, below `v12.13`, or `v14.0` - `v14.15`",
    "* `npm` `v8.6` is the last version that works on `node` `v12`, `v14.13` - `v14.16`, or `v16.0` - `v16.12`",
    "* `npm` `v9.x` is the last version that works on `node` `< v18.17`, `v19`, or `v20.0` - `v20.4`",
    "* `npm` `v10.x` is the last version that works on `node` `< v20.17`, `v21`, or `v22.0` - `v22.8`",
    "* Installing latest `npm`; if this does not work on your node version, please report a bug!",
];

/// `(node, npm, expected installs)` with the newest npm that is there.
const CURRENT_NPM: [(&str, &str); 41] = [
    ("0.6.21", "npm@1.3"),
    ("0.9.12", ""),
    ("0.10.48", "npm@4.5"),
    ("0.12.18", "npm@4.5"),
    ("1.0.0", "npm@4.5"),
    ("3.3.1", "npm@4"),
    ("4.0.0", "npm@5.3"),
    ("4.4.7", "npm@5.3"),
    ("4.6.2", "npm@5.4.1"),
    ("4.9.1", "npm@5"),
    ("5.9.1", "npm@5.3"),
    ("5.10.0", "npm@5"),
    ("6.1.0", "npm@6.9"),
    ("6.17.1", "npm@6"),
    ("8.17.0", "npm@6"),
    ("9.2.0", "npm@6.9"),
    ("9.4.0", "npm@6"),
    ("10.24.1", "npm@7"),
    ("12.0.0", "npm@7"),
    ("12.13.0", "npm@8.6"),
    ("13.14.0", "npm@7"),
    ("14.0.0", "npm@7"),
    ("14.15.5", "npm@8.6"),
    ("14.17.0", "npm@9"),
    ("15.14.0", "npm@7"),
    ("16.0.0", "npm@8.6"),
    ("16.12.0", "npm@8.6"),
    ("16.13.0", "npm@9"),
    ("17.9.1", "npm@8.6"),
    ("18.0.0", "npm@9"),
    ("18.16.0", "npm@9"),
    ("18.17.0", "npm@10"),
    ("19.9.0", "npm@9"),
    ("20.4.0", "npm@9"),
    ("20.5.0", "npm@10"),
    ("20.16.0", "npm@10"),
    ("20.17.0", "npm"),
    ("21.7.3", "npm@10"),
    ("22.8.0", "npm@10"),
    ("22.9.0", "npm"),
    ("24.0.0", "npm"),
];

#[test]
fn each_node_gets_the_npm_the_real_script_picks() {
    for (node, expected) in CURRENT_NPM {
        let got = installs(node, "10.2.3").join(",");
        assert_eq!(got, expected, "node {node}");
    }
}

/// `(node, npm, expected installs)` for an npm old enough to need a hop.
const OLD_NPM: [(&str, &str, &str); 35] = [
    ("0.10.48", "1.4.0", "npm@1.4.28,npm@4.5"),
    ("0.10.48", "2.15.0", "npm@2,npm@4.5"),
    ("0.10.48", "3.10.0", "npm@4.5"),
    ("0.10.48", "4.4.3", "npm@4.5"),
    ("0.10.48", "4.4.4", "npm@4.5"),
    ("1.0.0", "1.4.0", "npm@1.4.28,npm@4.5"),
    ("1.0.0", "2.15.0", "npm@2,npm@4.5"),
    ("1.0.0", "3.10.0", "npm@4.5"),
    ("1.0.0", "4.4.3", "npm@4.5"),
    ("1.0.0", "4.4.4", "npm@4.5"),
    ("3.3.1", "1.4.0", "npm@1.4.28,npm@4"),
    ("3.3.1", "2.15.0", "npm@2,npm@4"),
    ("3.3.1", "3.10.0", "npm@4"),
    ("3.3.1", "4.4.3", "npm@4"),
    ("3.3.1", "4.4.4", "npm@4"),
    ("4.9.1", "1.4.0", "npm@1.4.28,npm@5"),
    ("4.9.1", "2.15.0", "npm@2,npm@5"),
    ("4.9.1", "3.10.0", "npm@5"),
    ("4.9.1", "4.4.3", "npm@5"),
    ("4.9.1", "4.4.4", "npm@5"),
    ("8.17.0", "1.4.0", "npm@1.4.28,npm@4,npm@6"),
    ("8.17.0", "2.15.0", "npm@2,npm@4,npm@6"),
    ("8.17.0", "3.10.0", "npm@4,npm@6"),
    ("8.17.0", "4.4.3", "npm@4,npm@6"),
    ("8.17.0", "4.4.4", "npm@6"),
    ("10.24.1", "1.4.0", "npm@1.4.28,npm@7"),
    ("10.24.1", "2.15.0", "npm@2,npm@7"),
    ("10.24.1", "3.10.0", "npm@7"),
    ("10.24.1", "4.4.3", "npm@7"),
    ("10.24.1", "4.4.4", "npm@7"),
    ("20.10.0", "1.4.0", "npm@1.4.28,npm@10"),
    ("20.10.0", "2.15.0", "npm@2,npm@10"),
    ("20.10.0", "3.10.0", "npm@10"),
    ("20.10.0", "4.4.3", "npm@10"),
    ("20.10.0", "4.4.4", "npm@10"),
];

#[test]
fn an_old_npm_may_have_to_hop_through_its_last_release_first() {
    for (node, npm, expected) in OLD_NPM {
        let got = installs(node, npm).join(",");
        assert_eq!(got, expected, "node {node}, npm {npm}");
    }
}

/// `(node, npm, indexes into NOTES)`: what is said, in order.
const SAID: [(&str, &str, &[usize]); 39] = [
    ("0.6.21", "10.2.3", &[0, 1]),
    ("0.9.12", "10.2.3", &[1]),
    ("0.10.48", "10.2.3", &[2]),
    ("0.10.48", "1.4.0", &[3, 2]),
    ("0.10.48", "2.15.0", &[4, 2]),
    ("3.3.1", "10.2.3", &[5]),
    ("3.3.1", "1.4.0", &[3, 5]),
    ("3.3.1", "2.15.0", &[4, 5]),
    ("4.0.0", "10.2.3", &[6]),
    ("4.0.0", "1.4.0", &[3, 6]),
    ("4.0.0", "2.15.0", &[4, 6]),
    ("4.6.2", "10.2.3", &[7]),
    ("4.6.2", "1.4.0", &[3, 7]),
    ("4.6.2", "2.15.0", &[4, 7]),
    ("4.9.1", "10.2.3", &[8]),
    ("4.9.1", "1.4.0", &[3, 8]),
    ("4.9.1", "2.15.0", &[4, 8]),
    ("6.1.0", "10.2.3", &[9]),
    ("6.1.0", "1.4.0", &[3, 9]),
    ("6.1.0", "2.15.0", &[4, 9]),
    ("6.17.1", "10.2.3", &[10]),
    ("6.17.1", "3.10.0", &[11, 10]),
    ("6.17.1", "1.4.0", &[3, 11, 10]),
    ("6.17.1", "2.15.0", &[4, 11, 10]),
    ("10.24.1", "10.2.3", &[12]),
    ("10.24.1", "1.4.0", &[3, 12]),
    ("10.24.1", "2.15.0", &[4, 12]),
    ("12.13.0", "10.2.3", &[13]),
    ("12.13.0", "1.4.0", &[3, 13]),
    ("12.13.0", "2.15.0", &[4, 13]),
    ("14.17.0", "10.2.3", &[14]),
    ("14.17.0", "1.4.0", &[3, 14]),
    ("14.17.0", "2.15.0", &[4, 14]),
    ("18.17.0", "10.2.3", &[15]),
    ("18.17.0", "1.4.0", &[3, 15]),
    ("18.17.0", "2.15.0", &[4, 15]),
    ("20.17.0", "10.2.3", &[16]),
    ("20.17.0", "1.4.0", &[3, 16]),
    ("20.17.0", "2.15.0", &[4, 16]),
];

#[test]
fn every_step_says_what_the_real_script_says() {
    for (node, npm, indexes) in SAID {
        let said: Vec<&str> = steps(triple(node), triple(npm))
            .iter()
            .filter_map(|step| step.note)
            .collect();
        let expected: Vec<&str> = indexes.iter().map(|index| NOTES[*index]).collect();
        assert_eq!(said, expected, "node {node}, npm {npm}");
    }
}

#[test]
fn the_newest_node_installs_plain_npm() {
    assert_eq!(installs("24.0.0", "10.2.3"), ["npm"]);
    let plan = steps(triple("24.0.0"), triple("10.2.3"));
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].install, None);
}

#[test]
fn node_0_6_and_0_9_are_told_they_cannot_go_further_without_an_install() {
    let plan = steps(triple("0.9.12"), triple("1.4.0"));
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].install, None);
    assert!(plan[0].note.unwrap().contains("unable to upgrade further"));
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find module `npm` in
`domain`", "cannot find function `steps`" and "cannot find function `parse`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/domain/npm/default_packages/mod.rs`:

```rust
//! `$NVM_DIR/default-packages`: one package per line, `#` comments, installed
//! with `npm install -g` after every install (`nvm_get_default_packages`).

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error(
    "Only one package per line is allowed in `{file}`. Please remove any lines with multiple space-separated values."
)]
pub struct DefaultPackagesError {
    pub file: String,
}

/// What `nvm.sh`'s `awk` prints for the file: the packages joined by spaces,
/// with a space in front of every one that is not on the first line (so a
/// file that starts with a comment gives a string that starts with a space,
/// as in `nvm.sh`). Empty when there are no packages.
///
/// # Errors
/// [`DefaultPackagesError`] for a line with a space or a tab that is not a
/// comment: more than one value on it.
pub fn parse(contents: &str, file: &str) -> Result<String, DefaultPackagesError> {
    let mut joined = String::new();
    for (index, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start_matches([' ', '\t']);
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.contains([' ', '\t']) {
            return Err(DefaultPackagesError {
                file: file.to_owned(),
            });
        }
        if index > 0 {
            joined.push(' ');
        }
        joined.push_str(line);
    }
    Ok(joined)
}

```

Insert above the `#[cfg(test)]` line of `src/domain/npm/global_packages/mod.rs`:

```rust
//! The global packages of a node, read from `npm list -g --depth=0`, as
//! `nvm_npm_global_modules` does with `sed`.

/// What `reinstall-packages` installs and links.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalPackages {
    /// `name@version`, without `npm` and `corepack`, which come with node.
    pub installs: Vec<String>,
    /// Where the linked packages point (`pkg -> /path`).
    pub links: Vec<String>,
}

/// `npm list -g --depth=0` without its first line (the prefix) and without the
/// lines about unmet peer dependencies.
fn listed(output: &str) -> impl Iterator<Item = &str> {
    output
        .lines()
        .skip(1)
        .filter(|line| !line.contains("UNMET PEER DEPENDENCY"))
}

/// `s/^.* \(.*@[^ ]*\).*/\1/`: from the last space that still leaves an `@`
/// after it, up to the end of the word that holds the last `@`. A line that
/// does not fit stays as it is.
fn package_of(line: &str) -> &str {
    let Some(space) = line
        .char_indices()
        .rev()
        .find(|&(index, character)| character == ' ' && line[index + 1..].contains('@'))
        .map(|(index, _)| index)
    else {
        return line;
    };
    let rest = &line[space + 1..];
    let at = rest.rfind('@').unwrap_or(0);
    let word_end = rest[at..]
        .find(' ')
        .map_or(rest.len(), |offset| at + offset);
    &rest[..word_end]
}

/// # Panics
/// Never.
#[must_use]
pub fn parse(output: &str) -> GlobalPackages {
    let installs = listed(output)
        .filter(|line| !line.contains(" -> ") && !line.contains("(empty)"))
        .map(package_of)
        .filter(|package| !is_bundled(package))
        .flat_map(str::split_whitespace)
        .map(str::to_owned)
        .collect();
    let links = listed(output)
        .filter_map(|line| line.rfind(" -> ").map(|at| line[at + 4..].to_owned()))
        .collect();
    GlobalPackages { installs, links }
}

/// `^npm@` and `^corepack@`.
fn is_bundled(package: &str) -> bool {
    package.starts_with("npm@") || package.starts_with("corepack@")
}

```

Insert above the `#[cfg(test)]` line of `src/domain/npm/upgrade/mod.rs`:

```rust
//! The newest `npm` that works on a `node`: `nvm_install_latest_npm`, as data.

/// One step of an upgrade: what `nvm.sh` says about it, and the argument of
/// `npm install -g` (`None` installs plain `npm`, the latest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub note: Option<&'static str>,
    pub install: Option<&'static str>,
}

type Triple = (u64, u64, u64);

const fn step(note: &'static str, install: &'static str) -> Step {
    Step {
        note: Some(note),
        install: Some(install),
    }
}

/// The steps to run, in order, to bring `npm` at `npm` to the newest one that
/// works on `node`.
#[must_use]
pub fn steps(node: Triple, npm: Triple) -> Vec<Step> {
    let is_0_6 = ((0, 6, 0)..(0, 7, 0)).contains(&node);
    let is_0_9 = ((0, 9, 0)..(0, 10, 0)).contains(&node);
    let mut plan = Vec::new();
    if is_0_6 {
        plan.push(step(
            "* `node` v0.6.x can only upgrade to `npm` v1.3.x",
            "npm@1.3",
        ));
    } else if !is_0_9 {
        plan.extend(first_jump(npm));
    }
    if is_0_6 || is_0_9 {
        plan.push(Step {
            note: Some("* node v0.6 and v0.9 are unable to upgrade further"),
            install: None,
        });
    } else if node < (1, 1, 0) {
        plan.push(step(
            "* `npm` v4.5.x is the last version that works on `node` versions < v1.1.0",
            "npm@4.5",
        ));
    } else if node < (4, 0, 0) {
        plan.push(step(
            "* `npm` v5 and higher do not work on `node` versions below v4.0.0",
            "npm@4",
        ));
    } else {
        plan.extend(modern(node, npm));
    }
    plan
}

/// `npm` 1.x and 2.x have to hop through their last release first.
fn first_jump(npm: Triple) -> Option<Step> {
    if ((1, 0, 0)..(2, 0, 0)).contains(&npm) {
        Some(step(
            "* `npm` v1.x needs to first jump to `npm` v1.4.28 to be able to upgrade further",
            "npm@1.4.28",
        ))
    } else if ((2, 0, 0)..(3, 0, 0)).contains(&npm) {
        Some(step(
            "* `npm` v2.x needs to first jump to the latest v2 to be able to upgrade further",
            "npm@2",
        ))
    } else {
        None
    }
}

/// One row of the table for `node` 4 and later: the first row whose
/// condition holds on the `node` gives the `npm` to install.
struct Rule {
    applies: fn(Triple) -> bool,
    note: &'static str,
    install: &'static str,
}

const fn at_least(node: Triple, major: u64, minor: u64) -> bool {
    node.0 > major || (node.0 == major && node.1 >= minor)
}

/// The rules, in the order `nvm.sh` tries them. Each condition is the
/// combination of `node` release lines it names, which is how `nvm.sh` writes
/// them too (`IS_13_OR_ABOVE && !IS_14_LTS_OR_ABOVE`).
const RULES: [Rule; 9] = [
    Rule {
        applies: |n| !at_least(n, 4, 5) || (at_least(n, 5, 0) && !at_least(n, 5, 10)),
        note: "* `npm` `v5.3.x` is the last version that works on `node` 4.x versions below v4.4, or 5.x versions below v5.10, due to `Buffer.alloc`",
        install: "npm@5.3",
    },
    Rule {
        applies: |n| !at_least(n, 4, 7),
        note: "* `npm` `v5.4.1` is the last version that works on `node` `v4.5` and `v4.6`",
        install: "npm@5.4.1",
    },
    Rule {
        applies: |n| !at_least(n, 6, 0),
        note: "* `npm` `v5.x` is the last version that works on `node` below `v6.0.0`",
        install: "npm@5",
    },
    Rule {
        applies: |n| {
            (at_least(n, 6, 0) && !at_least(n, 6, 2)) || (at_least(n, 9, 0) && !at_least(n, 9, 3))
        },
        note: "* `npm` `v6.9` is the last version that works on `node` `v6.0.x`, `v6.1.x`, `v9.0.x`, `v9.1.x`, or `v9.2.x`",
        install: "npm@6.9",
    },
    Rule {
        applies: |n| !at_least(n, 10, 0),
        note: "* `npm` `v6.x` is the last version that works on `node` below `v10.0.0`",
        install: "npm@6",
    },
    Rule {
        applies: |n| {
            !at_least(n, 12, 13)
                || (at_least(n, 13, 0) && !at_least(n, 14, 15))
                || (at_least(n, 15, 0) && !at_least(n, 16, 0))
        },
        note: "* `npm` `v7.x` is the last version that works on `node` `v13`, `v15`, below `v12.13`, or `v14.0` - `v14.15`",
        install: "npm@7",
    },
    Rule {
        applies: |n| {
            (at_least(n, 12, 13) && !at_least(n, 13, 0))
                || (at_least(n, 14, 15) && !at_least(n, 14, 17))
                || (at_least(n, 16, 0) && !at_least(n, 16, 13))
                || (at_least(n, 17, 0) && !at_least(n, 18, 0))
        },
        note: "* `npm` `v8.6` is the last version that works on `node` `v12`, `v14.13` - `v14.16`, or `v16.0` - `v16.12`",
        install: "npm@8.6",
    },
    Rule {
        applies: |n| !at_least(n, 18, 17) || (at_least(n, 19, 0) && !at_least(n, 20, 5)),
        note: "* `npm` `v9.x` is the last version that works on `node` `< v18.17`, `v19`, or `v20.0` - `v20.4`",
        install: "npm@9",
    },
    Rule {
        applies: |n| !at_least(n, 20, 17) || (at_least(n, 21, 0) && !at_least(n, 22, 9)),
        note: "* `npm` `v10.x` is the last version that works on `node` `< v20.17`, `v21`, or `v22.0` - `v22.8`",
        install: "npm@10",
    },
];

const LATEST: Step = Step {
    note: Some(
        "* Installing latest `npm`; if this does not work on your node version, please report a bug!",
    ),
    install: None,
};

/// `node` 4 and later: the newest `npm` is bounded by the `node` release.
fn modern(node: Triple, npm: Triple) -> Vec<Step> {
    let Some(rule) = RULES.iter().find(|rule| (rule.applies)(node)) else {
        return vec![LATEST];
    };
    let mut plan = Vec::new();
    // Below `node` 10, `npm` 6 needs `npm` 4.4.4 or later to install it.
    if rule.install == "npm@6" && npm < (4, 4, 4) {
        plan.push(step(
            "* `npm` `v4.4.4` or later is required to install npm v6.14.18",
            "npm@4",
        ));
    }
    plan.push(step(rule.note, rule.install));
    plan
}

```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 469 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(npm): encode which npm each node can run, default-packages and npm list"
```

---

### Task 15: The `npm` of a version, and `nvm install-latest-npm`

**Files:**

- Create: `src/commands/npm/mod.rs`, `src/commands/npm/tests.rs`,
  `src/commands/npm/latest/mod.rs`, `src/commands/npm/latest/tests.rs`,
  `src/commands/install_latest_npm/mod.rs`,
  `src/commands/install_latest_npm/tests.rs`
- Modify: `src/commands/mod.rs`, `src/cli/mod.rs`,
  `src/domain/npm/upgrade/mod.rs`, `src/domain/npm/upgrade/tests.rs`

**Interfaces:**

- Consumes: Tasks 13 and 14. `Step` becomes `Step { note: &str, install:
  Install }` with `Install` being `Nothing`, `Latest` (plain `npm`) or
  `Spec("npm@6")`, so that a step that installs nothing is not a `None`.
- Produces: `commands::npm::Npm` with `in_version(&Context, version_path)`,
  `on_path(&Context)`, `version(&Context)`, `run(&Context, args,
  &mut Transcript) -> bool` (its output goes to the transcript) and
  `output(&Context, args)`; `commands::npm::latest::{install_latest, Node}`;
  `commands::install_latest_npm::run`; the `install-latest-npm` subcommand.
- Behaviour (verified against `nvm.sh` with a fake `npm` that logs its
  calls, in eight scenarios): `Attempting to upgrade to the latest working
  version of npm...`, each step's note and its `npm install -g <spec>`, then
  `* npm upgraded to: v<version>`; `NVM_DEBUG=1` prints the `npm` commands
  instead of running them, after `Detected node version <v>, npm version
  v<n>`; no `node` is `Unable to obtain node version.` (1), no `npm`
  `Unable to obtain npm version.` (2); the node is the version in use, or the
  system node asked with `--version`; a word after the command is the usage
  `Usage: nvm install-latest-npm` (127).
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -3,8 +3,10 @@
 pub mod cache;
 pub mod current;
 pub mod install;
+pub mod install_latest_npm;
 pub mod ls;
 pub mod ls_remote;
+pub mod npm;
 pub mod remote_index;
 pub mod resolve;
 pub mod transcript;
```

- [ ] **Step 2: Write the failing tests**

Create `src/commands/install_latest_npm/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/install_latest_npm/tests.rs`:

```rust
use super::*;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};

fn run_with(
    fs: &FakeFileSystem,
    process: &FakeProcess,
    path: &str,
    args: &[&str],
) -> Result<Output, CliError> {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("NVM_DEBUG", "1")
        .with_var("PATH", path);
    let words: Vec<String> = args.iter().map(|word| (*word).to_owned()).collect();
    run(&Context::new(fs, &env).with_process(process), &words)
}

#[test]
fn it_upgrades_the_npm_of_the_version_in_use() {
    let fs = FakeFileSystem::default()
        .with_executable("/n/versions/node/v20.10.0/bin/node", "")
        .with_executable("/n/versions/node/v20.10.0/bin/npm", "");
    let process = FakeProcess::default().with_success(
        "/n/versions/node/v20.10.0/bin/npm",
        "--version",
        "10.2.3\n",
    );
    let output = run_with(&fs, &process, "/n/versions/node/v20.10.0/bin:/usr/bin", &[]).unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(
        output
            .stdout
            .contains("Detected node version v20.10.0, npm version v10.2.3")
    );
    assert!(output.stdout.contains("npm install -g npm@10"));
}

#[test]
fn a_system_node_is_asked_for_its_version() {
    let fs = FakeFileSystem::default()
        .with_executable("/usr/bin/node", "")
        .with_executable("/usr/bin/npm", "");
    let process = FakeProcess::default()
        .with_output("/usr/bin/node", "v18.19.0\n")
        .with_success("/usr/bin/npm", "--version", "10.2.3\n");
    let output = run_with(&fs, &process, "/usr/bin", &[]).unwrap();
    assert!(
        output
            .stdout
            .contains("Detected node version v18.19.0, npm version v10.2.3")
    );
}

#[test]
fn no_node_is_status_1_and_no_npm_status_2() {
    let (fs, process) = (FakeFileSystem::default(), FakeProcess::default());
    assert_eq!(
        run_with(&fs, &process, "", &[]).unwrap().status,
        NvmExitCode::Failure
    );
    let fs = FakeFileSystem::default().with_executable("/n/versions/node/v20.10.0/bin/node", "");
    let output = run_with(&fs, &process, "/n/versions/node/v20.10.0/bin", &[]).unwrap();
    assert_eq!(output.status, NvmExitCode::MissingTarget);
}

#[test]
fn any_word_is_the_usage_with_status_127() {
    let (fs, process) = (FakeFileSystem::default(), FakeProcess::default());
    let error = run_with(&fs, &process, "", &["x"]).unwrap_err();
    assert_eq!(error.exit_code(), NvmExitCode::NotFound);
    assert_eq!(
        error.to_string(),
        "Usage: nvm install-latest-npm\n  Run `nvm --help` for full help."
    );
}
```

Create `src/commands/npm/latest/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/npm/latest/tests.rs`:

```rust
use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};

const NPM: &str = "/n/versions/node/v20.10.0/bin/npm";

struct Setup {
    fs: FakeFileSystem,
    env: FakeEnv,
    process: FakeProcess,
}

impl Setup {
    fn new(npm_prints: &str) -> Self {
        Self {
            fs: FakeFileSystem::default().with_executable(NPM, ""),
            env: FakeEnv::default(),
            process: FakeProcess::default()
                .with_success(NPM, "--version", &format!("{npm_prints}\n"))
                .with_success(NPM, "install -g npm@10", "added 1 package\n"),
        }
    }

    fn run(&self, node: &Node<'_>, with_npm: bool) -> (NvmExitCode, String, String) {
        let context = Context::new(&self.fs, &self.env).with_process(&self.process);
        let npm = Npm::in_version(&context, std::path::Path::new("/n/versions/node/v20.10.0"));
        let mut transcript = Transcript::default();
        let status = install_latest(
            &context,
            npm.as_ref().filter(|_| with_npm),
            node,
            &mut transcript,
        );
        let output = transcript.finish(status);
        (status, output.stdout, output.stderr)
    }
}

/// The expectations are what `nvm install-latest-npm` of the real `nvm.sh`
/// printed.
#[test]
fn it_says_why_installs_and_reports_the_new_version() {
    let setup = Setup::new("10.2.3");
    let (status, stdout, stderr) = setup.run(&Node::Version("v20.10.0"), true);
    assert_eq!(status, NvmExitCode::Success);
    assert_eq!(
        stdout,
        "Attempting to upgrade to the latest working version of npm...\n\
         * `npm` `v10.x` is the last version that works on `node` `< v20.17`, `v21`, or `v22.0` - `v22.8`\n\
         added 1 package\n\
         * npm upgraded to: v10.2.3"
    );
    assert_eq!(stderr, "");
    let ran: Vec<String> = setup
        .process
        .executed()
        .iter()
        .map(|i| i.args.join(" "))
        .collect();
    assert_eq!(ran, ["--version", "install -g npm@10", "--version"]);
}

#[test]
fn debug_prints_the_commands_instead_of_running_them() {
    let mut setup = Setup::new("3.10.0");
    setup.env = FakeEnv::default().with_var("NVM_DEBUG", "1");
    let (_, stdout, _) = setup.run(&Node::Version("v8.17.0"), true);
    assert_eq!(
        stdout,
        "Attempting to upgrade to the latest working version of npm...\n\
         Detected node version v8.17.0, npm version v3.10.0\n\
         * `npm` `v4.4.4` or later is required to install npm v6.14.18\n\
         npm install -g npm@4\n\
         * `npm` `v6.x` is the last version that works on `node` below `v10.0.0`\n\
         npm install -g npm@6\n\
         * npm upgraded to: v3.10.0"
    );
    assert_eq!(setup.process.executed().len(), 2);
}

#[test]
fn node_0_9_is_told_it_cannot_go_further() {
    let mut setup = Setup::new("1.4.0");
    setup.env = FakeEnv::default().with_var("NVM_DEBUG", "1");
    let (_, stdout, _) = setup.run(&Node::Version("v0.9.12"), true);
    assert!(stdout.contains("unable to upgrade further"), "{stdout}");
}

#[test]
fn an_iojs_version_is_read_without_its_prefix() {
    let mut setup = Setup::new("2.14.7");
    setup.env = FakeEnv::default().with_var("NVM_DEBUG", "1");
    let (_, stdout, _) = setup.run(&Node::Version("iojs-v3.3.1"), true);
    assert!(stdout.contains("Detected node version v3.3.1, npm version v2.14.7"));
}

#[test]
fn no_active_node_is_status_1() {
    let setup = Setup::new("10.2.3");
    let (status, stdout, stderr) = setup.run(&Node::None, true);
    assert_eq!(status, NvmExitCode::Failure);
    assert_eq!(
        stdout,
        "Attempting to upgrade to the latest working version of npm...\n\
         Detected node version none, npm version v10.2.3"
    );
    assert_eq!(stderr, "Unable to obtain node version.");
}

#[test]
fn no_npm_is_status_2() {
    let setup = Setup::new("10.2.3");
    let (status, stdout, stderr) = setup.run(&Node::Version("v20.10.0"), false);
    assert_eq!(status, NvmExitCode::MissingTarget);
    assert_eq!(
        stdout,
        "Attempting to upgrade to the latest working version of npm..."
    );
    assert_eq!(stderr, "Unable to obtain npm version.");
}
```

Create `src/commands/npm/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/npm/tests.rs`:

```rust
use std::path::Path;

use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};
use crate::ports::Completed;

const NPM: &str = "/n/versions/node/v20.10.0/bin/npm";

fn context<'a>(fs: &'a FakeFileSystem, env: &'a FakeEnv, process: &'a FakeProcess) -> Context<'a> {
    Context::new(fs, env).with_process(process)
}

#[test]
fn a_version_has_an_npm_when_bin_npm_is_a_file() {
    let fs = FakeFileSystem::default().with_executable(NPM, "#!/bin/sh");
    let (env, process) = (FakeEnv::default(), FakeProcess::default());
    let context = context(&fs, &env, &process);
    assert!(Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).is_some());
    assert!(Npm::in_version(&context, Path::new("/n/versions/node/v18.19.0")).is_none());
}

#[test]
fn the_npm_on_path_is_the_first_one() {
    let fs = FakeFileSystem::default()
        .with_executable("/a/bin/npm", "")
        .with_executable("/b/bin/npm", "");
    let env = FakeEnv::default().with_var("PATH", "/x:/a/bin:/b/bin");
    let process = FakeProcess::default();
    let npm = Npm::on_path(&context(&fs, &env, &process)).unwrap();
    assert_eq!(npm.program, Path::new("/a/bin/npm"));
    assert_eq!(npm.bin_dir, Path::new("/a/bin"));
}

#[test]
fn it_runs_with_its_own_bin_directory_first_on_the_path() {
    let fs = FakeFileSystem::default().with_executable(NPM, "");
    let env = FakeEnv::default();
    let process = FakeProcess::default().with_success(NPM, "--version", "10.2.3\n");
    let context = context(&fs, &env, &process);
    let npm = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
    assert_eq!(npm.version(&context).as_deref(), Some("10.2.3"));
    let ran = process.executed();
    assert_eq!(
        ran[0].path_prefix.as_deref(),
        Some(Path::new("/n/versions/node/v20.10.0/bin"))
    );
}

#[test]
fn a_version_that_is_empty_or_fails_is_none() {
    let fs = FakeFileSystem::default().with_executable(NPM, "");
    let env = FakeEnv::default();
    let silent = FakeProcess::default().with_success(NPM, "--version", "\n");
    let failing = FakeProcess::default().with_execution(NPM, "--version", Completed::default());
    for process in [&silent, &failing, &FakeProcess::default()] {
        let context = context(&fs, &env, process);
        let npm = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
        assert_eq!(npm.version(&context), None);
    }
}

#[test]
fn run_passes_the_output_on_and_says_whether_it_worked() {
    let fs = FakeFileSystem::default().with_executable(NPM, "");
    let env = FakeEnv::default();
    let done = Completed {
        success: false,
        stdout: "out1\nout2\n".to_owned(),
        stderr: "oops\n".to_owned(),
    };
    let process = FakeProcess::default().with_execution(NPM, "install -g left-pad", done);
    let context = context(&fs, &env, &process);
    let npm = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
    let mut transcript = Transcript::default();
    assert!(!npm.run(&context, &["install", "-g", "left-pad"], &mut transcript));
    let output = transcript.finish(crate::error::NvmExitCode::Success);
    assert_eq!(
        (output.stdout.as_str(), output.stderr.as_str()),
        ("out1\nout2", "oops")
    );
}

#[test]
fn a_program_that_cannot_start_is_reported_and_is_a_failure() {
    let fs = FakeFileSystem::default().with_executable(NPM, "");
    let (env, process) = (FakeEnv::default(), FakeProcess::default());
    let context = context(&fs, &env, &process);
    let npm = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
    let mut transcript = Transcript::default();
    assert!(!npm.run(&context, &["list"], &mut transcript));
    let stderr = transcript.finish(crate::error::NvmExitCode::Success).stderr;
    assert!(stderr.starts_with("/n/versions/node/v20.10.0/bin/npm: "));
    assert_eq!(npm.output(&context, &["list"]), None);
}

#[test]
fn output_returns_what_a_successful_run_printed() {
    let fs = FakeFileSystem::default().with_executable(NPM, "");
    let env = FakeEnv::default();
    let process = FakeProcess::default().with_success(NPM, "list -g --depth=0", "tree\n");
    let context = context(&fs, &env, &process);
    let npm = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
    assert_eq!(
        npm.output(&context, &["list", "-g", "--depth=0"])
            .as_deref(),
        Some("tree\n")
    );
}
```

Apply to `src/domain/npm/upgrade/tests.rs`:

```diff
--- a/src/domain/npm/upgrade/tests.rs
+++ b/src/domain/npm/upgrade/tests.rs
@@ -13,10 +13,10 @@
 fn installs(node: &str, npm: &str) -> Vec<&'static str> {
     steps(triple(node), triple(npm))
         .iter()
-        .filter_map(|step| {
-            step.install.or(Some("npm")).filter(|_| {
-                step.install.is_some() || step.note.is_some_and(|n| n.contains("Installing latest"))
-            })
+        .filter_map(|step| match step.install {
+            Install::Nothing => None,
+            Install::Latest => Some("npm"),
+            Install::Spec(spec) => Some(spec),
         })
         .collect()
 }
@@ -191,7 +191,7 @@
     for (node, npm, indexes) in SAID {
         let said: Vec<&str> = steps(triple(node), triple(npm))
             .iter()
-            .filter_map(|step| step.note)
+            .map(|step| step.note)
             .collect();
         let expected: Vec<&str> = indexes.iter().map(|index| NOTES[*index]).collect();
         assert_eq!(said, expected, "node {node}, npm {npm}");
@@ -203,13 +203,13 @@
     assert_eq!(installs("24.0.0", "10.2.3"), ["npm"]);
     let plan = steps(triple("24.0.0"), triple("10.2.3"));
     assert_eq!(plan.len(), 1);
-    assert_eq!(plan[0].install, None);
+    assert_eq!(plan[0].install, Install::Latest);
 }
 
 #[test]
 fn node_0_6_and_0_9_are_told_they_cannot_go_further_without_an_install() {
     let plan = steps(triple("0.9.12"), triple("1.4.0"));
     assert_eq!(plan.len(), 1);
-    assert_eq!(plan[0].install, None);
-    assert!(plan[0].note.unwrap().contains("unable to upgrade further"));
-}
+    assert_eq!(plan[0].install, Install::Nothing);
+    assert!(plan[0].note.contains("unable to upgrade further"));
+}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find struct `Npm`" and
"cannot find function `install_latest`".

- [ ] **Step 4: Write the implementation**

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -61,6 +61,12 @@
         #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
         args: Vec<String>,
     },
+    /// Upgrade the `npm` of the node in use to the newest one that works on it.
+    #[command(name = "install-latest-npm")]
+    InstallLatestNpm {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
     /// Remove an installed version (`--lts` and `--lts=<name>` pick one by
     /// its LTS alias) and the aliases that name it.
     Uninstall {
@@ -97,6 +103,7 @@
         Command::VersionRemote { args } => commands::version_remote::run(context, args),
         Command::Cache { args } => commands::cache::run(context, args),
         Command::Install { args } => commands::install::run(context, args),
+        Command::InstallLatestNpm { args } => commands::install_latest_npm::run(context, args),
         Command::Uninstall { args } => commands::uninstall::run(context, args),
         Command::Which { version } => commands::which::run(context, version.as_deref()),
         Command::Alias { args } => commands::aliases::run(context, args),
```

Insert above the `#[cfg(test)]` line of `src/commands/install_latest_npm/mod.rs`:

```rust
//! `nvm install-latest-npm`: upgrade the `npm` of the node in use.

use crate::commands::Output;
use crate::commands::current;
use crate::commands::npm::Npm;
use crate::commands::npm::latest::{Node, install_latest};
use crate::commands::resolve::system_version;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::current::Current;
use crate::error::CliError;

const USAGE: &str = "Usage: nvm install-latest-npm\n  Run `nvm --help` for full help.";

/// # Errors
/// - [`CliError::Usage`] when given any word.
/// - [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    if !args.is_empty() {
        return Err(CliError::Usage(USAGE.to_owned()));
    }
    let active = current::detect(context)?;
    let system = system_version(context)?;
    let node = match (&active, &system) {
        (Current::Version(version), _) => Some(version.to_string()),
        (Current::System, version) => version.clone(),
        (Current::None, _) => None,
    };
    let npm = Npm::on_path(context);
    let mut transcript = Transcript::default();
    let known = node.as_deref().map_or(Node::None, Node::Version);
    let status = install_latest(context, npm.as_ref(), &known, &mut transcript);
    Ok(transcript.finish(status))
}

```

Insert above the `#[cfg(test)]` line of `src/commands/npm/latest/mod.rs`:

```rust
//! `nvm install-latest-npm`: bring `npm` to the newest version that works on
//! the `node` it belongs to (`nvm_install_latest_npm`).

use crate::commands::npm::Npm;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::npm::upgrade::{Install, steps};
use crate::domain::version::Version;
use crate::error::NvmExitCode;

/// What is known of the `node` that the `npm` belongs to.
pub enum Node<'a> {
    /// `v20.10.0` (or `iojs-v3.3.1`).
    Version(&'a str),
    /// No `node` is active.
    None,
}

fn triple(text: &str) -> Option<(u64, u64, u64)> {
    let version: Version = text.parse().ok()?;
    Some(version.triple())
}

/// Runs the upgrade, printing as `nvm.sh` does. With `NVM_DEBUG=1` the `npm`
/// commands are printed instead of run.
pub fn install_latest(
    context: &Context<'_>,
    npm: Option<&Npm>,
    node: &Node<'_>,
    transcript: &mut Transcript,
) -> NvmExitCode {
    transcript.out("Attempting to upgrade to the latest working version of npm...");
    let npm_version = npm.and_then(|npm| npm.version(context));
    let node_text = match node {
        Node::Version(text) => Some(text.strip_prefix("iojs-").unwrap_or(text)),
        Node::None => {
            let shown = npm_version.as_deref().unwrap_or_default();
            transcript.out(format!("Detected node version none, npm version v{shown}"));
            None
        }
    };
    let Some(node_text) = node_text.filter(|text| triple(text).is_some()) else {
        transcript.err("Unable to obtain node version.");
        return NvmExitCode::Failure;
    };
    let (Some(npm), Some(npm_version)) = (npm, npm_version) else {
        transcript.err("Unable to obtain npm version.");
        return NvmExitCode::MissingTarget;
    };
    let debug = context.env.var("NVM_DEBUG").as_deref() == Some("1");
    if debug {
        transcript.out(format!(
            "Detected node version {node_text}, npm version v{npm_version}"
        ));
    }
    let plan = steps(
        triple(node_text).unwrap_or_default(),
        triple(&npm_version).unwrap_or_default(),
    );
    for step in plan {
        transcript.out(step.note);
        match step.install {
            Install::Nothing => {}
            Install::Latest => install(context, npm, "npm", debug, transcript),
            Install::Spec(spec) => install(context, npm, spec, debug, transcript),
        }
    }
    let upgraded = npm.version(context).unwrap_or_default();
    transcript.out(format!("* npm upgraded to: v{upgraded}"));
    NvmExitCode::Success
}

fn install(context: &Context<'_>, npm: &Npm, spec: &str, debug: bool, transcript: &mut Transcript) {
    if debug {
        transcript.out(format!("npm install -g {spec}"));
    } else {
        npm.run(context, &["install", "-g", spec], transcript);
    }
}

```

Insert above the `#[cfg(test)]` line of `src/commands/npm/mod.rs`:

```rust
//! Running the `npm` of a node version: the one thing `nvm` asks of a version
//! it has installed besides `node` itself. `nvm.sh` has `npm` on its `PATH`
//! because it activated the version; here the version's `bin` directory is put
//! in front of the `PATH` of each run instead.

pub mod latest;

use std::path::PathBuf;

use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::path_search::find_in_path;
use crate::ports::Invocation;

/// An `npm` that exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Npm {
    program: PathBuf,
    /// Its directory, which also holds the `node` it belongs to.
    bin_dir: PathBuf,
}

impl Npm {
    /// The `npm` in `<version_path>/bin`, when the version has one.
    #[must_use]
    pub fn in_version(context: &Context<'_>, version_path: &std::path::Path) -> Option<Self> {
        let bin_dir = version_path.join("bin");
        let program = bin_dir.join("npm");
        let info = context.fs.file_info(&program).ok()?;
        (!info.is_dir).then_some(Self { program, bin_dir })
    }

    /// The first `npm` on `PATH`.
    #[must_use]
    pub fn on_path(context: &Context<'_>) -> Option<Self> {
        let path_variable = context.env.var_os("PATH").unwrap_or_default();
        let program = find_in_path(context.fs, &path_variable, "npm")?;
        let bin_dir = program.parent()?.to_path_buf();
        Some(Self { program, bin_dir })
    }

    fn invocation(&self, args: &[&str]) -> Invocation {
        Invocation::new(&self.program)
            .args(args)
            .path_prefix(&self.bin_dir)
    }

    /// What `npm --version` prints, when it prints something.
    #[must_use]
    pub fn version(&self, context: &Context<'_>) -> Option<String> {
        let done = context
            .process()
            .execute(&self.invocation(&["--version"]))
            .ok()?;
        let text = done.stdout.trim().to_owned();
        (done.success && !text.is_empty()).then_some(text)
    }

    /// Runs `npm` with `args`, its output going on to the transcript like it
    /// goes to the terminal in `nvm.sh`. True when it succeeded.
    pub fn run(&self, context: &Context<'_>, args: &[&str], transcript: &mut Transcript) -> bool {
        match context.process().execute(&self.invocation(args)) {
            Ok(done) => {
                done.stdout.lines().for_each(|line| transcript.out(line));
                done.stderr.lines().for_each(|line| transcript.err(line));
                done.success
            }
            Err(error) => {
                transcript.err(format!("{}: {error}", self.program.display()));
                false
            }
        }
    }

    /// Runs `npm` for its output (`npm list -g`), which is returned instead.
    #[must_use]
    pub fn output(&self, context: &Context<'_>, args: &[&str]) -> Option<String> {
        let done = context.process().execute(&self.invocation(args)).ok()?;
        done.success.then_some(done.stdout)
    }
}

```

Apply to `src/domain/npm/upgrade/mod.rs` (above the test module):

```diff
--- a/src/domain/npm/upgrade/mod.rs
+++ b/src/domain/npm/upgrade/mod.rs
@@ -1,19 +1,29 @@
 //! The newest `npm` that works on a `node`: `nvm_install_latest_npm`, as data.
 
-/// One step of an upgrade: what `nvm.sh` says about it, and the argument of
-/// `npm install -g` (`None` installs plain `npm`, the latest).
+/// What a step installs.
+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+pub enum Install {
+    /// Nothing: the step only says why there is no more to do.
+    Nothing,
+    /// `npm install -g npm`: the latest.
+    Latest,
+    /// `npm install -g <argument>`, such as `npm@6`.
+    Spec(&'static str),
+}
+
+/// One step of an upgrade: what `nvm.sh` says about it, and what it installs.
 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 pub struct Step {
-    pub note: Option<&'static str>,
-    pub install: Option<&'static str>,
+    pub note: &'static str,
+    pub install: Install,
 }
 
 type Triple = (u64, u64, u64);
 
 const fn step(note: &'static str, install: &'static str) -> Step {
     Step {
-        note: Some(note),
-        install: Some(install),
+        note,
+        install: Install::Spec(install),
     }
 }
 
@@ -34,8 +44,8 @@
     }
     if is_0_6 || is_0_9 {
         plan.push(Step {
-            note: Some("* node v0.6 and v0.9 are unable to upgrade further"),
-            install: None,
+            note: "* node v0.6 and v0.9 are unable to upgrade further",
+            install: Install::Nothing,
         });
     } else if node < (1, 1, 0) {
         plan.push(step(
@@ -145,10 +155,8 @@
 ];
 
 const LATEST: Step = Step {
-    note: Some(
-        "* Installing latest `npm`; if this does not work on your node version, please report a bug!",
-    ),
-    install: None,
+    note: "* Installing latest `npm`; if this does not work on your node version, please report a bug!",
+    install: Install::Latest,
 };
 
 /// `node` 4 and later: the newest `npm` is bounded by the `node` release.
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 486 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(npm): add install-latest-npm"
```

---

### Task 16: `--latest-npm`, the default packages and `--skip-default-packages`

**Files:**

- Create: `src/commands/install/npm_steps/mod.rs`,
  `src/commands/install/npm_steps/tests.rs`,
  `src/commands/install/tests/npm.rs`
- Modify: `src/commands/install/mod.rs`, `src/commands/install/options/mod.rs`,
  `src/commands/install/options/tests.rs`, `src/commands/install/tests/mod.rs`

**Interfaces:**

- Produces: `Options::{latest_npm, skip_default_packages}` (the second is read
  after the version too); `commands::install::npm_steps::run`.
- Behaviour (verified against `nvm.sh` in seven scenarios, comparing output,
  exit status and the `npm` commands that ran): after the install, `--latest-npm`
  runs the upgrade of Task 15, then `$NVM_DIR/default-packages` is installed
  with one `npm install -g --quiet <packages>` (`Installing default global
  packages from <file>...` and the command line are printed), unless
  `--skip-default-packages`. A fresh install makes the `default` alias before
  these steps, an installed one after them. The first step that does not
  succeed gives the install its status (`Failed installing default
  packages...` is status 1). A version with no `npm` skips the steps with
  `npm was not found in <v>; skipping <the npm upgrade|the default
  packages>.` and succeeds.
- [ ] **Step 1: Write the failing tests**

Create `src/commands/install/npm_steps/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/install/npm_steps/tests.rs`:

```rust
use std::path::Path;

use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};
use crate::ports::Completed;

const NPM: &str = "/n/versions/node/v20.10.0/bin/npm";
const PATH: &str = "/n/versions/node/v20.10.0";

fn version() -> Version {
    "v20.10.0".parse().unwrap()
}

fn options(latest_npm: bool, skip: bool) -> Options {
    Options {
        latest_npm,
        skip_default_packages: skip,
        ..Options::default()
    }
}

fn run_steps(
    fs: &FakeFileSystem,
    process: &FakeProcess,
    options: &Options,
) -> (NvmExitCode, String, String) {
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let context = Context::new(fs, &env).with_process(process);
    let mut transcript = Transcript::default();
    let status = run(
        &context,
        options,
        &version(),
        Path::new(PATH),
        &mut transcript,
    )
    .unwrap();
    let output = transcript.finish(status);
    (status, output.stdout, output.stderr)
}

fn world(default_packages: Option<&str>) -> FakeFileSystem {
    let fs = FakeFileSystem::default().with_executable(NPM, "");
    match default_packages {
        Some(contents) => fs.with_file("/n/default-packages", contents),
        None => fs,
    }
}

fn calls(process: &FakeProcess) -> Vec<String> {
    process
        .executed()
        .iter()
        .map(|i| i.args.join(" "))
        .collect()
}

/// The expectations are what `nvm install` of the real `nvm.sh` printed, and
/// the `npm` commands it ran.
#[test]
fn the_default_packages_are_installed_with_one_npm_command() {
    let fs = world(Some("# my packages\nyarn\n\n@scope/tool\n"));
    let process =
        FakeProcess::default().with_success(NPM, "install -g --quiet yarn @scope/tool", "added\n");
    let (status, stdout, _) = run_steps(&fs, &process, &options(false, false));
    assert_eq!(status, NvmExitCode::Success);
    assert_eq!(
        stdout,
        "Installing default global packages from /n/default-packages...\n\
         npm install -g --quiet  yarn @scope/tool\n\
         added"
    );
}

#[test]
fn skip_default_packages_leaves_the_file_alone() {
    let fs = world(Some("yarn\n"));
    let process = FakeProcess::default();
    let (status, stdout, _) = run_steps(&fs, &process, &options(false, true));
    assert_eq!((status, stdout.as_str()), (NvmExitCode::Success, ""));
    assert!(calls(&process).is_empty());
}

#[test]
fn no_default_packages_file_or_an_empty_one_runs_nothing() {
    let process = FakeProcess::default();
    for fs in [world(None), world(Some("# nothing\n"))] {
        let (status, stdout, _) = run_steps(&fs, &process, &options(false, false));
        assert_eq!((status, stdout.as_str()), (NvmExitCode::Success, ""));
    }
    assert!(calls(&process).is_empty());
}

#[test]
fn a_line_with_two_values_is_status_1_and_names_the_file() {
    let process = FakeProcess::default();
    let (status, _, stderr) = run_steps(&world(Some("a b\n")), &process, &options(false, false));
    assert_eq!(status, NvmExitCode::Failure);
    assert_eq!(
        stderr,
        "Only one package per line is allowed in `/n/default-packages`. Please remove any lines with multiple space-separated values."
    );
}

#[test]
fn a_failed_package_install_is_status_1_with_the_hint() {
    let failed = Completed {
        success: false,
        stdout: String::new(),
        stderr: "E404\n".to_owned(),
    };
    let process = FakeProcess::default().with_execution(NPM, "install -g --quiet yarn", failed);
    let (status, _, stderr) = run_steps(&world(Some("yarn\n")), &process, &options(false, false));
    assert_eq!(status, NvmExitCode::Failure);
    assert_eq!(
        stderr,
        "E404\nFailed installing default packages. Please check if your default-packages file or a package in it has problems!"
    );
}

#[test]
fn latest_npm_runs_before_the_default_packages() {
    let fs = world(Some("yarn\n"));
    let process = FakeProcess::default()
        .with_success(NPM, "--version", "10.2.3\n")
        .with_success(NPM, "install -g npm@10", "")
        .with_success(NPM, "install -g --quiet yarn", "");
    let (status, stdout, _) = run_steps(&fs, &process, &options(true, false));
    assert_eq!(status, NvmExitCode::Success);
    assert!(stdout.starts_with("Attempting to upgrade to the latest working version of npm..."));
    assert!(stdout.contains("npm install -g --quiet yarn"));
    let ran = calls(&process);
    let upgrade = ran
        .iter()
        .position(|call| call == "install -g npm@10")
        .unwrap();
    let packages = ran
        .iter()
        .position(|call| call == "install -g --quiet yarn")
        .unwrap();
    assert!(upgrade < packages);
}

#[test]
fn a_failed_upgrade_stops_before_the_default_packages() {
    let fs = world(Some("yarn\n"));
    let process = FakeProcess::default();
    let (status, _, stderr) = run_steps(&fs, &process, &options(true, false));
    assert_eq!(status, NvmExitCode::MissingTarget);
    assert_eq!(stderr, "Unable to obtain npm version.");
    assert!(!calls(&process).iter().any(|call| call.contains("yarn")));
}

#[test]
fn a_version_without_npm_skips_the_steps_with_a_warning_and_never_fails() {
    let fs = FakeFileSystem::default().with_file("/n/default-packages", "yarn\n");
    let process = FakeProcess::default();
    let (status, _, stderr) = run_steps(&fs, &process, &options(true, false));
    assert_eq!(status, NvmExitCode::Success);
    assert_eq!(
        stderr,
        "npm was not found in v20.10.0; skipping the npm upgrade.\nnpm was not found in v20.10.0; skipping the default packages."
    );
    assert!(process.executed().is_empty());
}
```

Apply to `src/commands/install/options/tests.rs`:

```diff
--- a/src/commands/install/options/tests.rs
+++ b/src/commands/install/options/tests.rs
@@ -24,6 +24,16 @@
     assert!(!parsed("lts/iron").unwrap().announce_lts);
     assert!(!parsed("--lts 20").unwrap().announce_lts);
     assert!(!parsed("20").unwrap().announce_lts);
+}
+
+#[test]
+fn the_npm_options_are_read_before_and_after_the_version() {
+    let before = parsed("--latest-npm --skip-default-packages 20").unwrap();
+    assert!(before.latest_npm && before.skip_default_packages);
+    let after = parsed("20 --skip-default-packages").unwrap();
+    assert!(!after.latest_npm && after.skip_default_packages);
+    let neither = parsed("20").unwrap();
+    assert!(!neither.latest_npm && !neither.skip_default_packages);
 }
 
 #[test]
@@ -95,11 +105,9 @@
         "-s 20",
         "-j 4 20",
         "--offline 20",
-        "--latest-npm 20",
         "--reinstall-packages-from=18 20",
         "20 --reinstall-packages-from=18",
         "20 --copy-packages-from=18",
-        "20 --skip-default-packages",
         "20 --save",
     ] {
         let error = parsed(line).unwrap_err();
```

Apply to `src/commands/install/tests/mod.rs`:

```diff
--- a/src/commands/install/tests/mod.rs
+++ b/src/commands/install/tests/mod.rs
@@ -2,7 +2,9 @@
 
 use super::*;
 use crate::domain::fixtures::index_text;
-use crate::fakes::{FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeSleeper};
+use crate::fakes::{
+    FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeProcess, FakeSleeper,
+};
 use crate::ports::FileSystem;
 
 const NODE_INDEX: &str = "https://nodejs.org/dist/index.tab";
@@ -20,6 +22,7 @@
     digest: FakeDigest,
     sleeper: FakeSleeper,
     env: FakeEnv,
+    process: FakeProcess,
 }
 
 impl World {
@@ -36,19 +39,24 @@
             digest: FakeDigest::default().with_digest(TARBALL, GOOD),
             sleeper: FakeSleeper::default(),
             env: FakeEnv::default().with_var("NVM_DIR", "/n"),
+            process: FakeProcess::default(),
         }
     }
 
     fn run(&self, line: &str) -> Result<Output, CliError> {
         let archive = FakeArchive::new(&self.fs).with_archive(
             TARBALL,
-            &[("node-v20.10.0-linux-x64/bin/node", "binary", true)],
+            &[
+                ("node-v20.10.0-linux-x64/bin/node", "binary", true),
+                ("node-v20.10.0-linux-x64/bin/npm", "npm", true),
+            ],
         );
         let context = Context::new(&self.fs, &self.env)
             .with_http(&self.http)
             .with_digest(&self.digest)
             .with_archive(&archive)
-            .with_sleeper(&self.sleeper);
+            .with_sleeper(&self.sleeper)
+            .with_process(&self.process);
         let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
         super::run(&context, &words)
     }
@@ -67,6 +75,7 @@
 }
 
 mod failures;
+mod npm;
 
 #[test]
 fn a_fresh_install_downloads_unpacks_and_makes_the_default_alias() {
```

Create `src/commands/install/tests/npm.rs`:

```rust
use super::*;

const NPM: &str = "/n/versions/node/v20.10.0/bin/npm";

fn with_npm(world: World, script: FakeProcess) -> World {
    World {
        process: script,
        ..world
    }
}

fn npm_calls(world: &World) -> Vec<String> {
    world
        .process
        .executed()
        .iter()
        .map(|i| i.args.join(" "))
        .collect()
}

#[test]
fn a_fresh_install_makes_the_default_alias_before_it_upgrades_npm() {
    let process = FakeProcess::default()
        .with_success(NPM, "--version", "10.2.3\n")
        .with_success(NPM, "install -g npm@10", "added\n");
    let world = with_npm(World::new(), process);
    let output = world.run("--latest-npm 20").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    let out = lines(&output.stdout);
    assert_eq!(out[0], "Downloading and installing node v20.10.0...");
    assert_eq!(
        out[1],
        "Creating default alias: default -> 20 (-> v20.10.0 *)"
    );
    assert_eq!(
        out[2],
        "Attempting to upgrade to the latest working version of npm..."
    );
    assert_eq!(*out.last().unwrap(), "* npm upgraded to: v10.2.3");
}

#[test]
fn an_installed_version_still_gets_the_npm_steps_and_the_default_alias_after_them() {
    let process = FakeProcess::default()
        .with_success(NPM, "--version", "10.2.3\n")
        .with_success(NPM, "install -g npm@10", "added\n");
    let world = with_npm(World::new(), process);
    world.run("20").unwrap();
    let again = world.run("--latest-npm 20").unwrap();
    assert_eq!(again.stderr, "v20.10.0 is already installed.");
    assert!(
        again
            .stdout
            .starts_with("Attempting to upgrade to the latest working version of npm...")
    );
    assert!(npm_calls(&world).contains(&"install -g npm@10".to_owned()));
}

#[test]
fn a_failing_npm_step_is_the_status_of_the_install_but_the_version_stays() {
    let world = World::new();
    let output = world.run("--latest-npm 20").unwrap();
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    assert!(output.stderr.ends_with("Unable to obtain npm version."));
    assert!(world.installed());
    assert!(world.text("/n/alias/default").is_some());
}

#[test]
fn the_alias_of_an_installed_version_is_not_made_when_an_npm_step_failed() {
    let world = World::new();
    world.run("20").unwrap();
    let output = world.run("--latest-npm --alias=work 20").unwrap();
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    assert!(world.text("/n/alias/work").is_none());
}

#[test]
fn skip_default_packages_does_not_read_the_file() {
    let world = World::new();
    world
        .fs
        .write_file(Path::new("/n/default-packages"), "a b\n")
        .unwrap();
    assert_eq!(
        world.run("--skip-default-packages 20").unwrap().status,
        NvmExitCode::Success
    );
    assert_eq!(World::new().run("20").unwrap().status, NvmExitCode::Success);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "no field `latest_npm` on type
`Options`" and "cannot find module `npm_steps`".

- [ ] **Step 3: Write the implementation**

Apply to `src/commands/install/mod.rs` (above the test module):

```diff
--- a/src/commands/install/mod.rs
+++ b/src/commands/install/mod.rs
@@ -6,6 +6,7 @@
 pub mod fetch;
 mod flow;
 pub mod lock;
+mod npm_steps;
 pub mod options;
 pub mod place;
 
@@ -50,8 +51,12 @@
     let version_path = place::version_path(context, &version)?;
     if place::is_valid_install(context, &version_path) {
         transcript.err(format!("{version} is already installed."));
+        let status = npm_steps::run(context, options, &version, &version_path, transcript)?;
         defaults::ensure_default(context, &alias_target(options), transcript)?;
-        return apply_alias(context, options, transcript);
+        if status == NvmExitCode::Success {
+            apply_alias(context, options, transcript)?;
+        }
+        return end_with(status);
     }
     install_binary(context, &version, &version_path, transcript)?;
     apply_alias(context, options, transcript)?;
@@ -62,7 +67,22 @@
         transcript.err(message);
         return Err(Halt::Exit(NvmExitCode::Failure));
     }
-    defaults::ensure_default(context, &alias_target(options), transcript)
+    defaults::ensure_default(context, &alias_target(options), transcript)?;
+    end_with(npm_steps::run(
+        context,
+        options,
+        &version,
+        &version_path,
+        transcript,
+    )?)
+}
+
+/// The install ends with the status of its last step.
+fn end_with(status: NvmExitCode) -> Step<()> {
+    match status {
+        NvmExitCode::Success => Ok(()),
+        failed => Err(Halt::Exit(failed)),
+    }
 }
 
 /// What an alias made for this install points at.
```

Insert above the `#[cfg(test)]` line of `src/commands/install/npm_steps/mod.rs`:

```rust
//! What an install does with `npm` once the version is in place, in the order
//! of `nvm.sh`: `--latest-npm`, then the default packages. A version without
//! an `npm` skips these steps with a warning (`nvm.sh` would download an `npm`
//! installer from the internet and run it; this port never does).

use std::path::Path;

use crate::commands::install::flow::Step;
use crate::commands::install::options::Options;
use crate::commands::npm::Npm;
use crate::commands::npm::latest::{Node, install_latest};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::npm::default_packages;
use crate::domain::version::Version;
use crate::error::NvmExitCode;

/// The steps, stopping at the first one that does not succeed; its status is
/// the install's.
///
/// # Errors
/// [`crate::commands::install::flow::Halt::Error`] when `$NVM_DIR` is unknown.
pub fn run(
    context: &Context<'_>,
    options: &Options,
    version: &Version,
    version_path: &Path,
    transcript: &mut Transcript,
) -> Step<NvmExitCode> {
    let npm = Npm::in_version(context, version_path);
    if options.latest_npm {
        let status = upgrade(context, npm.as_ref(), version, transcript);
        if status != NvmExitCode::Success {
            return Ok(status);
        }
    }
    if !options.skip_default_packages {
        return default_packages(context, npm.as_ref(), version, transcript);
    }
    Ok(NvmExitCode::Success)
}

fn skip(version: &Version, what: &str, transcript: &mut Transcript) -> NvmExitCode {
    transcript.err(format!("npm was not found in {version}; skipping {what}."));
    NvmExitCode::Success
}

fn upgrade(
    context: &Context<'_>,
    npm: Option<&Npm>,
    version: &Version,
    transcript: &mut Transcript,
) -> NvmExitCode {
    if npm.is_none() {
        return skip(version, "the npm upgrade", transcript);
    }
    let text = version.to_string();
    install_latest(context, npm, &Node::Version(&text), transcript)
}

/// `nvm_install_default_packages`: one `npm install -g --quiet` for the lot.
fn default_packages(
    context: &Context<'_>,
    npm: Option<&Npm>,
    version: &Version,
    transcript: &mut Transcript,
) -> Step<NvmExitCode> {
    let file = context.nvm_dir()?.join("default-packages");
    let Ok(contents) = context.fs.read_to_string(&file) else {
        return Ok(NvmExitCode::Success);
    };
    let shown = file.display().to_string();
    let joined = match default_packages::parse(&contents, &shown) {
        Ok(joined) => joined,
        Err(error) => {
            transcript.err(error.to_string());
            return Ok(NvmExitCode::Failure);
        }
    };
    if joined.is_empty() {
        return Ok(NvmExitCode::Success);
    }
    let Some(npm) = npm else {
        return Ok(skip(version, "the default packages", transcript));
    };
    transcript.out(format!(
        "Installing default global packages from {shown}..."
    ));
    transcript.out(format!("npm install -g --quiet {joined}"));
    let mut args = vec!["install", "-g", "--quiet"];
    args.extend(joined.split_whitespace());
    if npm.run(context, &args, transcript) {
        return Ok(NvmExitCode::Success);
    }
    transcript.err("Failed installing default packages. Please check if your default-packages file or a package in it has problems!");
    Ok(NvmExitCode::Failure)
}

```

Apply to `src/commands/install/options/mod.rs` (above the test module):

```diff
--- a/src/commands/install/options/mod.rs
+++ b/src/commands/install/options/mod.rs
@@ -5,9 +5,9 @@
 use crate::error::CliError;
 
 /// Options that need a source build, `npm` or a `.nvmrc`: not in this port yet.
-const NOT_YET: [&str; 4] = ["-s", "-j", "--offline", "--latest-npm"];
+const NOT_YET: [&str; 3] = ["-s", "-j", "--offline"];
 const NOT_YET_WITH_VALUE: [&str; 2] = ["--reinstall-packages-from", "--copy-packages-from"];
-const NOT_YET_AFTER_VERSION: [&str; 3] = ["--skip-default-packages", "--save", "-w"];
+const NOT_YET_AFTER_VERSION: [&str; 2] = ["--save", "-w"];
 
 #[derive(Debug, Default, PartialEq, Eq)]
 pub struct Options {
@@ -22,6 +22,10 @@
     /// The LTS filter came from `--lts` with no version, which `nvm.sh`
     /// announces ("Installing latest LTS version.").
     pub announce_lts: bool,
+    /// `--latest-npm`: upgrade the `npm` of the installed version.
+    pub latest_npm: bool,
+    /// `--skip-default-packages`: leave `$NVM_DIR/default-packages` alone.
+    pub skip_default_packages: bool,
 }
 
 fn unsupported(option: &str) -> CliError {
@@ -49,6 +53,8 @@
     while let Some(option) = rest.next_if(|arg| is_option(arg)) {
         match option.as_str() {
             "-b" | "--no-progress" => {}
+            "--latest-npm" => options.latest_npm = true,
+            "--skip-default-packages" => options.skip_default_packages = true,
             "--lts" => options.lts = Some("*".to_owned()),
             "--default" => set_alias(&mut options, "default")?,
             other if other.starts_with("--lts=") => {
@@ -69,7 +75,9 @@
         options.version_given = true;
     }
     for option in rest {
-        if is_not_yet(option, &NOT_YET_WITH_VALUE)
+        if option == "--skip-default-packages" {
+            options.skip_default_packages = true;
+        } else if is_not_yet(option, &NOT_YET_WITH_VALUE)
             || NOT_YET_AFTER_VERSION.contains(&option.as_str())
         {
             return Err(unsupported(option));
@@ -82,8 +90,10 @@
 
 /// A word that `nvm.sh` reads as an option rather than as the version.
 fn is_option(arg: &str) -> bool {
-    matches!(arg, "-b" | "--no-progress" | "--lts" | "--default")
-        || arg.starts_with("--lts=")
+    matches!(
+        arg,
+        "-b" | "--no-progress" | "--lts" | "--default" | "--latest-npm" | "--skip-default-packages"
+    ) || arg.starts_with("--lts=")
         || arg.starts_with("--alias=")
         || arg.starts_with("---")
         || NOT_YET.contains(&arg)
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 500 unit tests plus the end-to-end tests.

- [ ] **Step 5: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(install): upgrade npm and install the default packages"
```

---

### Task 17: `nvm reinstall-packages` and `--reinstall-packages-from`

**Files:**

- Create: `src/commands/npm/packages/mod.rs`,
  `src/commands/npm/packages/tests.rs`,
  `src/commands/reinstall_packages/mod.rs`,
  `src/commands/reinstall_packages/tests.rs`
- Modify: `src/error.rs`, `src/commands/npm/mod.rs`,
  `src/commands/npm/tests.rs`, `src/commands/mod.rs`, `src/cli/mod.rs`,
  `src/commands/install/mod.rs`, `src/commands/install/npm_steps/mod.rs`,
  `src/commands/install/npm_steps/tests.rs`,
  `src/commands/install/options/mod.rs`,
  `src/commands/install/options/tests.rs`, `src/commands/install/tests/npm.rs`

**Interfaces:**

- Produces: `NvmExitCode::{SameVersion (4), SourceNotInstalled (5)}`;
  `commands::npm::packages::{Source, reinstall}`; `Npm::run_in(&Context, dir,
  args, &mut Transcript)`; `commands::reinstall_packages::run(&Context, command,
  args)`; the `reinstall-packages` and `copy-packages` subcommands (two
  subcommands, because the usage names the word that was typed);
  `Options::reinstall_from`. `Npm::output` returns the output of
  a run whatever its status (`npm list` exits with an error when a package has
  problems, and still lists the others).
- Behaviour (verified against `nvm.sh` in nine scenarios, with the `npm`
  commands and the directories they ran in): `Reinstalling global packages from
  <v>...`, one `npm install -g --quiet <packages>` into the node in use, then
  `Linking global packages from <v>...` and one `npm link` per linked package
  (an absolute path as it is, a relative one from `$(npm root -g)/../`); with
  nothing to do, `No installed global packages found...` and `No linked global
  packages found...`; the status is that of the last link. From the version in
  use it is `Can not reinstall packages from the current version of node.` (2);
  from an unknown version it prints `N/A: version "N/A" is not yet installed.`
  and finds nothing (0); `system` needs a system node (`No system version of
  node or io.js detected.`, 3). `nvm install --reinstall-packages-from=<v>` (or
  `--copy-packages-from=`, before or after the version) does the same into the
  new version, after the default packages, and its messages are
  `--reinstall-packages-from may not be provided more than once` (and `, or
  combined with `--copy-packages-from``), `If <option> is provided, it must
  point to an installed version of node.` and `... using `=`.` (6), `You can't
  reinstall global packages from the same version of node you're installing.`
  (4) and `If --reinstall-packages-from is provided, it must point to an
  installed version of node.` (5), the last two before anything is downloaded.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -7,6 +7,7 @@
 pub mod ls;
 pub mod ls_remote;
 pub mod npm;
+pub mod reinstall_packages;
 pub mod remote_index;
 pub mod resolve;
 pub mod transcript;
```

- [ ] **Step 2: Write the failing tests**

Apply to `src/commands/install/npm_steps/tests.rs`:

```diff
--- a/src/commands/install/npm_steps/tests.rs
+++ b/src/commands/install/npm_steps/tests.rs
@@ -32,6 +32,7 @@
         options,
         &version(),
         Path::new(PATH),
+        None,
         &mut transcript,
     )
     .unwrap();
@@ -163,3 +164,60 @@
     );
     assert!(process.executed().is_empty());
 }
+
+fn reinstall_from(
+    source: &Source,
+    fs: &FakeFileSystem,
+    process: &FakeProcess,
+) -> (NvmExitCode, String, String) {
+    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
+    let context = Context::new(fs, &env).with_process(process);
+    let mut transcript = Transcript::default();
+    let status = run(
+        &context,
+        &options(false, true),
+        &version(),
+        Path::new(PATH),
+        Some(source),
+        &mut transcript,
+    )
+    .unwrap();
+    let output = transcript.finish(status);
+    (status, output.stdout, output.stderr)
+}
+
+#[test]
+fn the_packages_of_the_source_are_installed_into_the_new_version() {
+    let old = "/n/versions/node/v18.19.0/bin/npm";
+    let fs = world(None).with_executable(old, "");
+    let process = FakeProcess::default()
+        .with_success(old, "list -g --depth=0", "/x\n├── yarn@1.22.19\n")
+        .with_success(NPM, "install -g --quiet yarn@1.22.19", "added\n");
+    let source = Source::Version("v18.19.0".parse().unwrap());
+    let (status, stdout, _) = reinstall_from(&source, &fs, &process);
+    assert_eq!(status, NvmExitCode::Success);
+    assert!(stdout.starts_with("Reinstalling global packages from v18.19.0..."));
+}
+
+#[test]
+fn the_version_cannot_be_its_own_source() {
+    let source = Source::Version(version());
+    let (status, _, stderr) = reinstall_from(&source, &world(None), &FakeProcess::default());
+    assert_eq!(status, NvmExitCode::MissingTarget);
+    assert_eq!(
+        stderr,
+        "Can not reinstall packages from the current version of node."
+    );
+}
+
+#[test]
+fn a_version_without_npm_skips_the_reinstall_with_a_warning() {
+    let fs = FakeFileSystem::default();
+    let source = Source::Version("v18.19.0".parse().unwrap());
+    let (status, _, stderr) = reinstall_from(&source, &fs, &FakeProcess::default());
+    assert_eq!(status, NvmExitCode::Success);
+    assert_eq!(
+        stderr,
+        "npm was not found in v20.10.0; skipping the reinstall of global packages."
+    );
+}
```

Apply to `src/commands/install/options/tests.rs`:

```diff
--- a/src/commands/install/options/tests.rs
+++ b/src/commands/install/options/tests.rs
@@ -24,6 +24,60 @@
     assert!(!parsed("lts/iron").unwrap().announce_lts);
     assert!(!parsed("--lts 20").unwrap().announce_lts);
     assert!(!parsed("20").unwrap().announce_lts);
+}
+
+#[test]
+fn reinstall_and_copy_packages_from_name_the_version_to_take_packages_from() {
+    for line in [
+        "--reinstall-packages-from=18 20",
+        "20 --reinstall-packages-from=18",
+        "--copy-packages-from=18 20",
+        "20 --copy-packages-from=18",
+    ] {
+        let options = parsed(line).unwrap();
+        assert_eq!(options.reinstall_from.as_deref(), Some("18"), "{line}");
+        assert_eq!(options.version, "20", "{line}");
+    }
+}
+
+/// Every message is what the real `nvm.sh` printed.
+#[test]
+fn a_reinstall_option_given_twice_or_without_a_version_is_status_6() {
+    let cases = [
+        (
+            "--reinstall-packages-from=18 --reinstall-packages-from=18 20",
+            "--reinstall-packages-from may not be provided more than once",
+        ),
+        (
+            "--copy-packages-from=18 --reinstall-packages-from=18 20",
+            "--reinstall-packages-from may not be provided more than once",
+        ),
+        (
+            "--reinstall-packages-from=18 --copy-packages-from=18 20",
+            "--reinstall-packages-from may not be provided more than once, or combined with `--copy-packages-from`",
+        ),
+        (
+            "--reinstall-packages-from= 20",
+            "If --reinstall-packages-from is provided, it must point to an installed version of node.",
+        ),
+        (
+            "--copy-packages-from= 20",
+            "If --copy-packages-from is provided, it must point to an installed version of node.",
+        ),
+        (
+            "--reinstall-packages-from 20",
+            "If --reinstall-packages-from is provided, it must point to an installed version of node using `=`.",
+        ),
+        (
+            "20 --copy-packages-from",
+            "If --copy-packages-from is provided, it must point to an installed version of node using `=`.",
+        ),
+    ];
+    for (line, message) in cases {
+        let error = parsed(line).unwrap_err();
+        assert_eq!(error.exit_code(), NvmExitCode::InvalidOptions, "{line}");
+        assert_eq!(error.to_string(), message, "{line}");
+    }
 }
 
 #[test]
@@ -101,15 +155,7 @@
 
 #[test]
 fn options_that_need_a_source_build_or_npm_are_not_supported_yet() {
-    for line in [
-        "-s 20",
-        "-j 4 20",
-        "--offline 20",
-        "--reinstall-packages-from=18 20",
-        "20 --reinstall-packages-from=18",
-        "20 --copy-packages-from=18",
-        "20 --save",
-    ] {
+    for line in ["-s 20", "-j 4 20", "--offline 20", "20 --save"] {
         let error = parsed(line).unwrap_err();
         assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption, "{line}");
         assert!(
```

Apply to `src/commands/install/tests/npm.rs`:

```diff
--- a/src/commands/install/tests/npm.rs
+++ b/src/commands/install/tests/npm.rs
@@ -88,3 +88,58 @@
     );
     assert_eq!(World::new().run("20").unwrap().status, NvmExitCode::Success);
 }
+
+#[test]
+fn reinstalling_from_the_version_being_installed_is_status_4() {
+    let world = World::new();
+    let output = world.run("--reinstall-packages-from=v20.10.0 20").unwrap();
+    assert_eq!(output.status, NvmExitCode::SameVersion);
+    assert_eq!(
+        output.stderr,
+        "You can't reinstall global packages from the same version of node you're installing."
+    );
+    assert!(!world.installed());
+}
+
+#[test]
+fn reinstalling_from_a_version_that_is_not_installed_is_status_5_before_anything_is_downloaded() {
+    let world = World::new();
+    let output = world.run("--reinstall-packages-from=99 20").unwrap();
+    assert_eq!(output.status, NvmExitCode::SourceNotInstalled);
+    assert_eq!(
+        output.stderr,
+        "If --reinstall-packages-from is provided, it must point to an installed version of node."
+    );
+    assert!(
+        world
+            .http
+            .requests()
+            .iter()
+            .all(|url| !url.contains("tar.gz"))
+    );
+}
+
+#[test]
+fn reinstalling_from_an_installed_version_runs_after_the_default_packages() {
+    let old = "/n/versions/node/v18.19.0/bin/npm";
+    let process = FakeProcess::default()
+        .with_success(old, "list -g --depth=0", "/x\n├── yarn@1.22.19\n")
+        .with_success(NPM, "install -g --quiet yarn@1.22.19", "added\n");
+    let world = with_npm(World::new(), process);
+    world
+        .fs
+        .write_file(Path::new("/n/versions/node/v18.19.0/bin/node"), "x")
+        .unwrap();
+    world
+        .fs
+        .set_executable(Path::new("/n/versions/node/v18.19.0/bin/node"));
+    world.fs.write_file(Path::new(old), "").unwrap();
+    world.fs.set_executable(Path::new(old));
+    let output = world.run("--reinstall-packages-from=18 20").unwrap();
+    assert_eq!(output.status, NvmExitCode::Success);
+    assert!(
+        output
+            .stdout
+            .contains("Reinstalling global packages from v18.19.0...")
+    );
+}
```

Create `src/commands/npm/packages/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/npm/packages/tests.rs`:

```rust
use std::path::Path;

use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};
use crate::ports::Completed;

const OLD_NPM: &str = "/n/versions/node/v18.19.0/bin/npm";
const NEW_NPM: &str = "/n/versions/node/v20.10.0/bin/npm";
const LISTING: &str = "/x/lib\n├── corepack@0.20.0\n├── npm@10.2.3\n├── yarn@1.22.19\n├── mylink@1.0.0 -> /abs/link/dir\n├── rel@1.0.0 -> ../../rellink\n└── @scope/tool@2.0.0\n";

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

fn fs() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_executable(OLD_NPM, "")
        .with_executable(NEW_NPM, "")
}

fn run_reinstall(source: &Source, process: &FakeProcess) -> (NvmExitCode, String, String) {
    let fs = fs();
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let context = Context::new(&fs, &env).with_process(process);
    let destination = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
    let mut transcript = Transcript::default();
    let status = reinstall(&context, source, &destination, &mut transcript);
    let output = transcript.finish(status);
    (status, output.stdout, output.stderr)
}

fn calls(process: &FakeProcess) -> Vec<String> {
    process
        .executed()
        .iter()
        .map(|i| {
            format!(
                "{} {} [{}]",
                i.program.display(),
                i.args.join(" "),
                i.dir
                    .as_ref()
                    .map_or(String::new(), |d| d.display().to_string())
            )
        })
        .collect()
}

/// The expectations are what `nvm install --reinstall-packages-from` of the
/// real `nvm.sh` printed and ran, with a fake `npm`.
#[test]
fn the_packages_are_installed_in_one_command_and_the_links_are_linked() {
    let process = FakeProcess::default()
        .with_success(OLD_NPM, "list -g --depth=0", LISTING)
        .with_success(
            NEW_NPM,
            "install -g --quiet yarn@1.22.19 @scope/tool@2.0.0",
            "added\n",
        )
        .with_success(
            NEW_NPM,
            "root -g",
            "/n/versions/node/v20.10.0/lib/node_modules\n",
        )
        .with_success(NEW_NPM, "link", "linked\n");
    let (status, stdout, stderr) = run_reinstall(&Source::Version(version("v18.19.0")), &process);
    assert_eq!(status, NvmExitCode::Success);
    assert_eq!(
        stdout,
        "Reinstalling global packages from v18.19.0...\nadded\nLinking global packages from v18.19.0...\nlinked\nlinked"
    );
    assert_eq!(stderr, "");
    assert_eq!(
        calls(&process),
        [
            format!("{OLD_NPM} list -g --depth=0 []"),
            format!("{NEW_NPM} install -g --quiet yarn@1.22.19 @scope/tool@2.0.0 []"),
            format!("{NEW_NPM} root -g []"),
            format!("{NEW_NPM} link [/abs/link/dir]"),
            format!("{NEW_NPM} link [/n/versions/node/v20.10.0/lib/node_modules/../../../rellink]"),
        ]
    );
}

#[test]
fn nothing_to_install_or_link_says_so() {
    let process =
        FakeProcess::default().with_success(OLD_NPM, "list -g --depth=0", "/x/lib\n└── (empty)\n");
    let (status, stdout, _) = run_reinstall(&Source::Version(version("v18.19.0")), &process);
    assert_eq!(status, NvmExitCode::Success);
    assert_eq!(
        stdout,
        "Reinstalling global packages from v18.19.0...\nNo installed global packages found...\nLinking global packages from v18.19.0...\nNo linked global packages found..."
    );
}

#[test]
fn a_missing_version_prints_the_use_message_and_finds_nothing() {
    let (status, stdout, stderr) = run_reinstall(&Source::Missing, &FakeProcess::default());
    assert_eq!(status, NvmExitCode::Success);
    assert!(stdout.starts_with(
        "Reinstalling global packages from N/A...\nNo installed global packages found..."
    ));
    assert_eq!(
        stderr,
        "N/A: version \"N/A\" is not yet installed.\n\nYou need to run `nvm install N/A` to install and use it."
    );
}

#[test]
fn the_status_is_that_of_the_last_link() {
    let failed = Completed {
        success: false,
        stdout: String::new(),
        stderr: String::new(),
    };
    let process = FakeProcess::default()
        .with_success(
            OLD_NPM,
            "list -g --depth=0",
            "/x\n├── a@1.0.0 -> /one\n├── b@1.0.0 -> /two\n",
        )
        .with_success(NEW_NPM, "install -g --quiet a@1.0.0 b@1.0.0", "")
        .with_success(NEW_NPM, "root -g", "/r\n")
        .with_execution(NEW_NPM, "link", failed);
    let (status, _, _) = run_reinstall(&Source::Version(version("v18.19.0")), &process);
    assert_eq!(status, NvmExitCode::Failure);
}
```

Apply to `src/commands/npm/tests.rs`:

```diff
--- a/src/commands/npm/tests.rs
+++ b/src/commands/npm/tests.rs
@@ -91,6 +91,51 @@
     let stderr = transcript.finish(crate::error::NvmExitCode::Success).stderr;
     assert!(stderr.starts_with("/n/versions/node/v20.10.0/bin/npm: "));
     assert_eq!(npm.output(&context, &["list"]), None);
+    assert!(!npm.run_in(
+        &context,
+        Path::new("/x"),
+        &["link"],
+        &mut Transcript::default()
+    ));
+}
+
+#[test]
+fn output_returns_what_a_failing_run_printed_too() {
+    let fs = FakeFileSystem::default().with_executable(NPM, "");
+    let env = FakeEnv::default();
+    let done = Completed {
+        success: false,
+        stdout: "tree\n".to_owned(),
+        stderr: String::new(),
+    };
+    let process = FakeProcess::default().with_execution(NPM, "list -g --depth=0", done);
+    let context = context(&fs, &env, &process);
+    let npm = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
+    assert_eq!(
+        npm.output(&context, &["list", "-g", "--depth=0"])
+            .as_deref(),
+        Some("tree\n")
+    );
+}
+
+#[test]
+fn run_in_runs_where_it_is_told() {
+    let fs = FakeFileSystem::default().with_executable(NPM, "");
+    let env = FakeEnv::default();
+    let process = FakeProcess::default().with_success(NPM, "link", "linked\n");
+    let context = context(&fs, &env, &process);
+    let npm = Npm::in_version(&context, Path::new("/n/versions/node/v20.10.0")).unwrap();
+    let mut transcript = Transcript::default();
+    assert!(npm.run_in(
+        &context,
+        Path::new("/src/mylink"),
+        &["link"],
+        &mut transcript
+    ));
+    assert_eq!(
+        process.executed()[0].dir.as_deref(),
+        Some(Path::new("/src/mylink"))
+    );
 }
 
 #[test]
```

Create `src/commands/reinstall_packages/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/reinstall_packages/tests.rs`:

```rust
use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};

const OLD_NPM: &str = "/n/versions/node/v18.19.0/bin/npm";
const NEW_NPM: &str = "/n/versions/node/v20.10.0/bin/npm";

fn fs() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_executable("/n/versions/node/v18.19.0/bin/node", "")
        .with_executable(OLD_NPM, "")
        .with_executable("/n/versions/node/v20.10.0/bin/node", "")
        .with_executable(NEW_NPM, "")
}

fn run_with(
    fs: &FakeFileSystem,
    process: &FakeProcess,
    path: &str,
    command: &str,
    line: &str,
) -> Result<Output, CliError> {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path);
    let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
    run(
        &Context::new(fs, &env).with_process(process),
        command,
        &words,
    )
}

fn active_20() -> &'static str {
    "/n/versions/node/v20.10.0/bin:/usr/bin"
}

/// The expectations are what the real `nvm reinstall-packages` printed.
#[test]
fn it_reinstalls_the_packages_of_another_version_into_the_one_in_use() {
    let process = FakeProcess::default()
        .with_success(OLD_NPM, "list -g --depth=0", "/x\n├── yarn@1.22.19\n")
        .with_success(NEW_NPM, "install -g --quiet yarn@1.22.19", "added\n");
    let output = run_with(&fs(), &process, active_20(), "reinstall-packages", "18").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        output.stdout,
        "Reinstalling global packages from v18.19.0...\nadded\nLinking global packages from v18.19.0...\nNo linked global packages found..."
    );
}

#[test]
fn the_version_in_use_cannot_be_its_own_source() {
    let process = FakeProcess::default();
    for word in ["20", "v20.10.0"] {
        let output = run_with(&fs(), &process, active_20(), "reinstall-packages", word).unwrap();
        assert_eq!(output.status, NvmExitCode::MissingTarget);
        assert_eq!(
            output.stderr,
            "Can not reinstall packages from the current version of node."
        );
    }
}

#[test]
fn an_unknown_version_prints_the_use_message_and_finds_nothing_with_status_0() {
    let output = run_with(
        &fs(),
        &FakeProcess::default(),
        active_20(),
        "reinstall-packages",
        "99",
    )
    .unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(
        output
            .stderr
            .starts_with("N/A: version \"N/A\" is not yet installed.")
    );
}

#[test]
fn without_one_word_the_usage_names_the_command_used() {
    for command in ["reinstall-packages", "copy-packages"] {
        for line in ["", "18 20"] {
            let error = run_with(&fs(), &FakeProcess::default(), "", command, line).unwrap_err();
            assert_eq!(error.exit_code(), NvmExitCode::NotFound);
            assert_eq!(
                error.to_string(),
                format!("Usage: nvm {command} <version>\n  Run `nvm --help` for full help.")
            );
        }
    }
}

#[test]
fn without_an_npm_in_use_there_is_nowhere_to_install() {
    let output = run_with(
        &fs(),
        &FakeProcess::default(),
        "/usr/bin",
        "reinstall-packages",
        "18",
    )
    .unwrap();
    assert_eq!(output.status, NvmExitCode::Failure);
    assert!(output.stderr.starts_with("npm was not found on PATH"));
}

#[test]
fn system_needs_a_system_node() {
    let output = run_with(
        &fs(),
        &FakeProcess::default(),
        active_20(),
        "reinstall-packages",
        "system",
    )
    .unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stderr,
        "No system version of node or io.js detected."
    );
    let with_system = fs()
        .with_executable("/usr/bin/node", "")
        .with_executable("/usr/bin/npm", "");
    let process = FakeProcess::default()
        .with_success(
            "/usr/bin/npm",
            "list -g --depth=0",
            "/x\n├── yarn@1.22.19\n",
        )
        .with_success(NEW_NPM, "install -g --quiet yarn@1.22.19", "");
    let output = run_with(
        &with_system,
        &process,
        active_20(),
        "reinstall-packages",
        "system",
    )
    .unwrap();
    assert!(
        output
            .stdout
            .starts_with("Reinstalling global packages from system...")
    );
}
```

Apply to the test module of `src/error.rs`:

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -9,6 +9,8 @@
         assert_eq!(NvmExitCode::BelowVersionFloor.code(), 7);
         assert_eq!(NvmExitCode::AliasLoop.code(), 8);
         assert_eq!(NvmExitCode::MissingTarget.code(), 2);
+        assert_eq!(NvmExitCode::SameVersion.code(), 4);
+        assert_eq!(NvmExitCode::SourceNotInstalled.code(), 5);
         assert_eq!(NvmExitCode::InvalidOptions.code(), 6);
         assert_eq!(NvmExitCode::UnsupportedOption.code(), 55);
         assert_eq!(NvmExitCode::NotFound.code(), 127);
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "no variant named `SameVersion`
found", "cannot find module `reinstall_packages`" and "no field
`reinstall_from` on type `Options`".

- [ ] **Step 4: Write the implementation**

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -67,6 +67,18 @@
         #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
         args: Vec<String>,
     },
+    /// Install, in the node in use, the global packages of another version.
+    #[command(name = "reinstall-packages")]
+    ReinstallPackages {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
+    /// The same as `reinstall-packages`.
+    #[command(name = "copy-packages")]
+    CopyPackages {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
     /// Remove an installed version (`--lts` and `--lts=<name>` pick one by
     /// its LTS alias) and the aliases that name it.
     Uninstall {
@@ -104,6 +116,12 @@
         Command::Cache { args } => commands::cache::run(context, args),
         Command::Install { args } => commands::install::run(context, args),
         Command::InstallLatestNpm { args } => commands::install_latest_npm::run(context, args),
+        Command::ReinstallPackages { args } => {
+            commands::reinstall_packages::run(context, "reinstall-packages", args)
+        }
+        Command::CopyPackages { args } => {
+            commands::reinstall_packages::run(context, "copy-packages", args)
+        }
         Command::Uninstall { args } => commands::uninstall::run(context, args),
         Command::Which { version } => commands::which::run(context, version.as_deref()),
         Command::Alias { args } => commands::aliases::run(context, args),
```

Apply to `src/commands/install/mod.rs` (above the test module):

```diff
--- a/src/commands/install/mod.rs
+++ b/src/commands/install/mod.rs
@@ -13,6 +13,8 @@
 use std::time::SystemTime;
 
 use crate::commands::Output;
+use crate::commands::npm::packages::Source;
+use crate::commands::resolve::{Resolved, resolve_installed};
 use crate::commands::transcript::Transcript;
 use crate::commands::version_remote::lookup;
 use crate::context::Context;
@@ -20,6 +22,7 @@
 use crate::domain::platform::binary_available;
 use crate::domain::remote::Query;
 use crate::domain::version::{Flavor, Version};
+use crate::domain::version_prefix::with_v_prefix;
 use crate::error::{CliError, NvmExitCode};
 use flow::{Halt, Step};
 use lock::{LockRequest, acquire};
@@ -49,9 +52,17 @@
     let version = resolve(context, options, transcript)?;
     check_floor(context, &version, transcript)?;
     let version_path = place::version_path(context, &version)?;
+    let source = reinstall_source(context, options, &version, transcript)?;
     if place::is_valid_install(context, &version_path) {
         transcript.err(format!("{version} is already installed."));
-        let status = npm_steps::run(context, options, &version, &version_path, transcript)?;
+        let status = npm_steps::run(
+            context,
+            options,
+            &version,
+            &version_path,
+            source.as_ref(),
+            transcript,
+        )?;
         defaults::ensure_default(context, &alias_target(options), transcript)?;
         if status == NvmExitCode::Success {
             apply_alias(context, options, transcript)?;
@@ -68,13 +79,44 @@
         return Err(Halt::Exit(NvmExitCode::Failure));
     }
     defaults::ensure_default(context, &alias_target(options), transcript)?;
-    end_with(npm_steps::run(
+    let status = npm_steps::run(
         context,
         options,
         &version,
         &version_path,
+        source.as_ref(),
         transcript,
-    )?)
+    )?;
+    end_with(status)
+}
+
+/// The version `--reinstall-packages-from` names, which must be installed and
+/// must not be the one being installed.
+fn reinstall_source(
+    context: &Context<'_>,
+    options: &Options,
+    version: &Version,
+    transcript: &mut Transcript,
+) -> Step<Option<Source>> {
+    let Some(provided) = &options.reinstall_from else {
+        return Ok(None);
+    };
+    if with_v_prefix(provided) == version.to_string() {
+        transcript.err(
+            "You can't reinstall global packages from the same version of node you're installing.",
+        );
+        return Err(Halt::Exit(NvmExitCode::SameVersion));
+    }
+    match resolve_installed(context, provided)? {
+        Resolved::Installed(from) => Ok(Some(Source::Version(from))),
+        Resolved::System => Ok(Some(Source::System)),
+        Resolved::Missing { .. } => {
+            transcript.err(
+                "If --reinstall-packages-from is provided, it must point to an installed version of node.",
+            );
+            Err(Halt::Exit(NvmExitCode::SourceNotInstalled))
+        }
+    }
 }
 
 /// The install ends with the status of its last step.
```

Apply to `src/commands/install/npm_steps/mod.rs` (above the test module):

```diff
--- a/src/commands/install/npm_steps/mod.rs
+++ b/src/commands/install/npm_steps/mod.rs
@@ -9,14 +9,16 @@
 use crate::commands::install::options::Options;
 use crate::commands::npm::Npm;
 use crate::commands::npm::latest::{Node, install_latest};
+use crate::commands::npm::packages::{Source, reinstall};
 use crate::commands::transcript::Transcript;
 use crate::context::Context;
 use crate::domain::npm::default_packages;
 use crate::domain::version::Version;
 use crate::error::NvmExitCode;
 
-/// The steps, stopping at the first one that does not succeed; its status is
-/// the install's.
+/// The steps (`--latest-npm`, the default packages, the packages of
+/// `--reinstall-packages-from`), stopping at the first one that does not
+/// succeed; its status is the install's.
 ///
 /// # Errors
 /// [`crate::commands::install::flow::Halt::Error`] when `$NVM_DIR` is unknown.
@@ -25,6 +27,7 @@
     options: &Options,
     version: &Version,
     version_path: &Path,
+    source: Option<&Source>,
     transcript: &mut Transcript,
 ) -> Step<NvmExitCode> {
     let npm = Npm::in_version(context, version_path);
@@ -35,9 +38,32 @@
         }
     }
     if !options.skip_default_packages {
-        return default_packages(context, npm.as_ref(), version, transcript);
+        let status = default_packages(context, npm.as_ref(), version, transcript)?;
+        if status != NvmExitCode::Success {
+            return Ok(status);
+        }
     }
-    Ok(NvmExitCode::Success)
+    Ok(source.map_or(NvmExitCode::Success, |source| {
+        copy_packages(context, npm.as_ref(), version, source, transcript)
+    }))
+}
+
+/// `nvm reinstall-packages <source>` into the version just installed.
+fn copy_packages(
+    context: &Context<'_>,
+    npm: Option<&Npm>,
+    version: &Version,
+    source: &Source,
+    transcript: &mut Transcript,
+) -> NvmExitCode {
+    if *source == Source::Version(*version) {
+        transcript.err("Can not reinstall packages from the current version of node.");
+        return NvmExitCode::MissingTarget;
+    }
+    match npm {
+        Some(npm) => reinstall(context, source, npm, transcript),
+        None => skip(version, "the reinstall of global packages", transcript),
+    }
 }
 
 fn skip(version: &Version, what: &str, transcript: &mut Transcript) -> NvmExitCode {
```

Apply to `src/commands/install/options/mod.rs` (above the test module):

```diff
--- a/src/commands/install/options/mod.rs
+++ b/src/commands/install/options/mod.rs
@@ -6,8 +6,9 @@
 
 /// Options that need a source build, `npm` or a `.nvmrc`: not in this port yet.
 const NOT_YET: [&str; 3] = ["-s", "-j", "--offline"];
-const NOT_YET_WITH_VALUE: [&str; 2] = ["--reinstall-packages-from", "--copy-packages-from"];
 const NOT_YET_AFTER_VERSION: [&str; 2] = ["--save", "-w"];
+/// Both options mean the same; `nvm.sh` words its messages after the one used.
+const REINSTALL_OPTIONS: [&str; 2] = ["--reinstall-packages-from", "--copy-packages-from"];
 
 #[derive(Debug, Default, PartialEq, Eq)]
 pub struct Options {
@@ -26,6 +27,9 @@
     pub latest_npm: bool,
     /// `--skip-default-packages`: leave `$NVM_DIR/default-packages` alone.
     pub skip_default_packages: bool,
+    /// `--reinstall-packages-from=<version>` (or `--copy-packages-from`): the
+    /// version, as given, whose global packages are installed again.
+    pub reinstall_from: Option<String>,
 }
 
 fn unsupported(option: &str) -> CliError {
@@ -38,10 +42,37 @@
     CliError::InvalidOptions(message.to_owned())
 }
 
-fn is_not_yet(option: &str, names: &[&str]) -> bool {
-    names
-        .iter()
-        .any(|name| option == *name || option.starts_with(&format!("{name}=")))
+/// `--reinstall-packages-from=18`, `--copy-packages-from=18` or either of
+/// them with no value.
+fn reinstall_option(option: &str) -> Option<(&'static str, Option<&str>)> {
+    REINSTALL_OPTIONS.iter().find_map(|name| {
+        if option == *name {
+            return Some((*name, None));
+        }
+        let value = option.strip_prefix(&format!("{name}="))?;
+        Some((*name, Some(value)))
+    })
+}
+
+fn read_reinstall(options: &mut Options, option: &str) -> Result<bool, CliError> {
+    let Some((name, value)) = reinstall_option(option) else {
+        return Ok(false);
+    };
+    let message = match value {
+        None => format!("If {name} is provided, it must point to an installed version of node using `=`."),
+        Some(_) if options.reinstall_from.is_some() && name == "--copy-packages-from" => {
+            "--reinstall-packages-from may not be provided more than once, or combined with `--copy-packages-from`".to_owned()
+        }
+        Some(_) if options.reinstall_from.is_some() => {
+            "--reinstall-packages-from may not be provided more than once".to_owned()
+        }
+        Some("") => format!("If {name} is provided, it must point to an installed version of node."),
+        Some(version) => {
+            options.reinstall_from = Some(version.to_owned());
+            return Ok(true);
+        }
+    };
+    Err(CliError::InvalidOptions(message))
 }
 
 /// # Errors
@@ -67,6 +98,7 @@
                 let message = "arguments with `---` are not supported - this is likely a typo";
                 return Err(CliError::Unsupported(message.to_owned()));
             }
+            other if read_reinstall(&mut options, other)? => {}
             other => return Err(unsupported(other)),
         }
     }
@@ -77,8 +109,8 @@
     for option in rest {
         if option == "--skip-default-packages" {
             options.skip_default_packages = true;
-        } else if is_not_yet(option, &NOT_YET_WITH_VALUE)
-            || NOT_YET_AFTER_VERSION.contains(&option.as_str())
+        } else if !read_reinstall(&mut options, option)?
+            && NOT_YET_AFTER_VERSION.contains(&option.as_str())
         {
             return Err(unsupported(option));
         }
@@ -97,7 +129,7 @@
         || arg.starts_with("--alias=")
         || arg.starts_with("---")
         || NOT_YET.contains(&arg)
-        || is_not_yet(arg, &NOT_YET_WITH_VALUE)
+        || reinstall_option(arg).is_some()
         || NOT_YET_AFTER_VERSION.contains(&arg)
 }
 
```

Apply to `src/commands/npm/mod.rs` (above the test module):

```diff
--- a/src/commands/npm/mod.rs
+++ b/src/commands/npm/mod.rs
@@ -4,6 +4,7 @@
 //! in front of the `PATH` of each run instead.
 
 pub mod latest;
+pub mod packages;
 
 use std::path::PathBuf;
 
@@ -73,10 +74,34 @@
     }
 
     /// Runs `npm` for its output (`npm list -g`), which is returned instead.
+    /// `npm list` exits with an error when a package has problems and still
+    /// lists the others, so the output counts whatever the status.
     #[must_use]
     pub fn output(&self, context: &Context<'_>, args: &[&str]) -> Option<String> {
         let done = context.process().execute(&self.invocation(args)).ok()?;
-        done.success.then_some(done.stdout)
+        Some(done.stdout)
+    }
+
+    /// Like [`Self::run`], inside `directory`.
+    pub fn run_in(
+        &self,
+        context: &Context<'_>,
+        directory: &std::path::Path,
+        args: &[&str],
+        transcript: &mut Transcript,
+    ) -> bool {
+        let invocation = self.invocation(args).dir(directory);
+        match context.process().execute(&invocation) {
+            Ok(done) => {
+                done.stdout.lines().for_each(|line| transcript.out(line));
+                done.stderr.lines().for_each(|line| transcript.err(line));
+                done.success
+            }
+            Err(error) => {
+                transcript.err(format!("cd: {}: {error}", directory.display()));
+                false
+            }
+        }
     }
 }
 
```

Insert above the `#[cfg(test)]` line of `src/commands/npm/packages/mod.rs`:

```rust
//! Moving the global packages of one node version to another:
//! `nvm reinstall-packages` and `nvm install --reinstall-packages-from`.

use crate::commands::install::place::version_path;
use crate::commands::npm::Npm;
use crate::commands::resolve::system_node;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::npm::global_packages::parse;
use crate::domain::version::Version;
use crate::error::NvmExitCode;

/// Where the packages come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Version(Version),
    /// The `node` outside `$NVM_DIR`.
    System,
    /// Nothing is installed under that name (shown as `N/A`).
    Missing,
}

impl Source {
    fn label(&self) -> String {
        match self {
            Self::Version(version) => version.to_string(),
            Self::System => "system".to_owned(),
            Self::Missing => "N/A".to_owned(),
        }
    }

    fn npm(&self, context: &Context<'_>) -> Option<Npm> {
        match self {
            Self::Version(version) => {
                Npm::in_version(context, &version_path(context, version).ok()?)
            }
            Self::System => {
                let node = system_node(context).ok()??;
                Npm::in_version(context, node.parent()?.parent()?)
            }
            Self::Missing => None,
        }
    }
}

/// What `nvm use <missing>` prints, which `nvm.sh` lets through.
fn not_yet_installed(name: &str) -> String {
    format!(
        "{name}: version \"{name}\" is not yet installed.\n\nYou need to run `nvm install {name}` to install and use it."
    )
}

/// Installs the packages of `source` with `destination`, then links the ones
/// that were linked. The status is that of the last link, as in `nvm.sh`
/// (a package that fails to install does not change it).
pub fn reinstall(
    context: &Context<'_>,
    source: &Source,
    destination: &Npm,
    transcript: &mut Transcript,
) -> NvmExitCode {
    let listing = source
        .npm(context)
        .and_then(|npm| npm.output(context, &["list", "-g", "--depth=0"]))
        .unwrap_or_default();
    if *source == Source::Missing {
        transcript.err(not_yet_installed("N/A"));
    }
    let packages = parse(&listing);
    let label = source.label();
    transcript.out(format!("Reinstalling global packages from {label}..."));
    if packages.installs.is_empty() {
        transcript.out("No installed global packages found...");
    } else {
        let mut args = vec!["install", "-g", "--quiet"];
        args.extend(packages.installs.iter().map(String::as_str));
        destination.run(context, &args, transcript);
    }
    transcript.out(format!("Linking global packages from {label}..."));
    link(context, destination, &packages.links, transcript)
}

fn link(
    context: &Context<'_>,
    destination: &Npm,
    links: &[String],
    transcript: &mut Transcript,
) -> NvmExitCode {
    if links.is_empty() {
        transcript.out("No linked global packages found...");
        return NvmExitCode::Success;
    }
    let root = destination
        .output(context, &["root", "-g"])
        .map(|text| text.trim().to_owned())
        .unwrap_or_default();
    let mut status = NvmExitCode::Success;
    for target in links.iter().filter(|target| !target.is_empty()) {
        let directory = if target.starts_with('/') {
            target.clone()
        } else {
            format!("{root}/../{target}")
        };
        let linked = destination.run_in(
            context,
            std::path::Path::new(&directory),
            &["link"],
            transcript,
        );
        status = if linked {
            NvmExitCode::Success
        } else {
            NvmExitCode::Failure
        };
    }
    status
}

```

Insert above the `#[cfg(test)]` line of `src/commands/reinstall_packages/mod.rs`:

```rust
//! `nvm reinstall-packages <version>` (also `copy-packages`): install in the
//! node in use the global packages of another version.

use crate::commands::Output;
use crate::commands::current;
use crate::commands::npm::Npm;
use crate::commands::npm::packages::{Source, reinstall};
use crate::commands::resolve::{Resolved, resolve_installed, system_node};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};

/// # Errors
/// - [`CliError::Usage`] unless exactly one word is given.
/// - [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(context: &Context<'_>, command: &str, args: &[String]) -> Result<Output, CliError> {
    let [provided] = args else {
        let usage = format!("Usage: nvm {command} <version>\n  Run `nvm --help` for full help.");
        return Err(CliError::Usage(usage));
    };
    let active = current::detect(context)?.to_string();
    let resolved = resolve_installed(context, provided)?;
    let named = match &resolved {
        Resolved::Installed(version) => version.to_string(),
        Resolved::System => "system".to_owned(),
        Resolved::Missing { .. } => "N/A".to_owned(),
    };
    let mut transcript = Transcript::default();
    if *provided == active || named == active {
        transcript.err("Can not reinstall packages from the current version of node.");
        return Ok(transcript.finish(NvmExitCode::MissingTarget));
    }
    let source = match resolved {
        Resolved::Installed(version) => Source::Version(version),
        Resolved::System => Source::System,
        Resolved::Missing { .. } if provided == "system" => Source::System,
        Resolved::Missing { .. } => Source::Missing,
    };
    if source == Source::System && system_node(context)?.is_none() {
        transcript.err("No system version of node or io.js detected.");
        return Ok(transcript.finish(NvmExitCode::InvalidVersion));
    }
    let Some(destination) = Npm::on_path(context) else {
        transcript
            .err("npm was not found on PATH; there is nowhere to reinstall the global packages.");
        return Ok(transcript.finish(NvmExitCode::Failure));
    };
    let status = reinstall(context, &source, &destination, &mut transcript);
    Ok(transcript.finish(status))
}

```

Apply to `src/error.rs` (above the test module):

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -16,6 +16,12 @@
     /// Something that was asked for does not exist: an `lts/<name>` alias, or
     /// the archive of a version whose download failed.
     MissingTarget = 2,
+    /// `nvm install --reinstall-packages-from` the very version being
+    /// installed.
+    SameVersion = 4,
+    /// `nvm install --reinstall-packages-from` a version that is not
+    /// installed.
+    SourceNotInstalled = 5,
     /// Options that cannot be combined, or given twice.
     InvalidOptions = 6,
     /// An option `nvm.sh` does not support, or one used in a combination it
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 520 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(install): reinstall global packages from another version"
```

---

### Task 18: `--offline` and `--save`

**Files:**

- Create: `src/commands/install/resolve.rs`, `src/commands/install/offline/mod.rs`,
  `src/commands/install/offline/tests.rs`, `src/commands/install/nvmrc_file.rs`,
  `src/commands/install/tests/offline_and_save.rs`
- Modify: `src/commands/install/mod.rs`, `src/commands/install/options/mod.rs`,
  `src/commands/install/options/tests.rs`, `src/commands/install/fetch/mod.rs`,
  `src/commands/install/fetch/tests.rs`, `src/commands/install/tests/mod.rs`,
  `src/commands/install/flow.rs`, `src/commands/install/npm_steps/mod.rs`,
  `src/commands/install/npm_steps/tests.rs`

**Interfaces:**

- Step 0 moves `resolve`, `not_found_message` and `check_floor` out of
  `src/commands/install/mod.rs`, which would pass 300 lines, into
  `src/commands/install/resolve.rs` (no behaviour change).
- Produces: `commands::install::flow::Target { version, path, source }`, the
  version an install is about with the `Source` of the packages to reinstall
  (it replaces the loose arguments `npm_steps::run` took); `Options::{offline,
  save}`; `fetch::Artifact::slug` and
  `fetch::fetch(.., offline, ..)`; `commands::install::offline::cached_version`;
  `commands::install::nvmrc_file::write`.
- Behaviour (verified against `nvm.sh` in six scenarios): `--offline` never
  uses the network. The version is what is installed, else the newest cached
  version whose name contains the pattern (the entries of `.cache/bin` for this
  platform and of `.cache/src`); `--lts` needs the local `lts/*` aliases
  (`LTS alias '<name>' not found locally. Run `nvm ls-remote --lts` first to
  populate LTS aliases.`, 3); nothing found is `Version '<v>' not found locally
  or in cache - try `nvm ls` to browse available versions.` (3). A cached
  archive is used without a checksum (`Offline: using cached archive <path>`),
  and a missing one is `Offline: no cached archive found for <slug>`.
  `--save` or `-w`, before the version only (after it the word goes to
  `./configure`, as in `nvm.sh`), writes the resolved version to `$PWD/.nvmrc`
  and says `Wrote version number (<v>) to .nvmrc`; a failure is `Warning: Unable
  to write version number (<v>) to .nvmrc` (3) and skips `--alias`; given twice
  it is `--save and -w may only be provided once` (6). Unlike `nvm.sh`, which
  forgets `--save` after a fresh install, `.nvmrc` is written then too.
- [ ] **Step 0: Move `resolve` out of `install/mod.rs` (no behaviour change)**

`src/commands/install/mod.rs` would pass 300 lines in this task. Create
`src/commands/install/resolve.rs` with the code below and apply the diff to
`mod.rs` (it removes the same functions and declares `mod resolve;`).

Create `src/commands/install/resolve.rs`:

```rust
//! Which version an install is about: the one the mirror says a description
//! stands for, and whether it may be installed (the version floor).

use crate::commands::install::flow::{Halt, Step};
use crate::commands::install::options::Options;
use crate::commands::transcript::Transcript;
use crate::commands::version_remote::lookup;
use crate::context::Context;
use crate::domain::floor::VersionFloor;
use crate::domain::remote::Query;
use crate::domain::version::Version;
use crate::error::NvmExitCode;

pub(super) fn resolve(
    context: &Context<'_>,
    options: &Options,
    transcript: &mut Transcript,
) -> Step<Version> {
    let query = Query {
        pattern: Some(options.version.clone()).filter(|text| !text.is_empty()),
        lts: options.lts.clone(),
    };
    let found = lookup(context, query)?;
    for warning in found.warnings {
        transcript.err(warning);
    }
    found.version.ok_or_else(|| {
        transcript.err(not_found_message(options));
        Halt::Exit(NvmExitCode::InvalidVersion)
    })
}

fn not_found_message(options: &Options) -> String {
    let version = &options.version;
    match options.lts.as_deref() {
        Some("*") => format!(
            "Version '{version}' (with LTS filter) not found - try `nvm ls-remote --lts` to browse available versions."
        ),
        Some(lts) if version.is_empty() => format!(
            "Version with LTS filter '{lts}' not found - try `nvm ls-remote --lts={lts}` to browse available versions."
        ),
        Some(lts) => format!(
            "Version '{version}' (with LTS filter '{lts}') not found - try `nvm ls-remote --lts={lts}` to browse available versions."
        ),
        None => format!(
            "Version '{version}' not found - try `nvm ls-remote` to browse available versions."
        ),
    }
}

pub(super) fn check_floor(
    context: &Context<'_>,
    version: &Version,
    transcript: &mut Transcript,
) -> Step<()> {
    let from_file = context
        .fs
        .read_to_string(&context.nvm_dir()?.join("min-version"))
        .ok();
    let from_env = context.env.var("NVM_MIN_VERSION");
    let floor = VersionFloor::from_sources(from_env.as_deref(), from_file.as_deref())
        .and_then(|floor| floor.map_or(Ok(()), |floor| floor.check(version)));
    let Err(error) = floor else {
        return Ok(());
    };
    transcript.err(error.to_string());
    if matches!(error, crate::error::FloorError::Below { .. }) {
        transcript
            .err("Lower or unset NVM_MIN_VERSION (or edit $NVM_DIR/min-version) to install it.");
    }
    Err(Halt::Exit(NvmExitCode::BelowVersionFloor))
}
```

Apply to `src/commands/install/mod.rs`:

```diff
--- a/src/commands/install/mod.rs
+++ b/src/commands/install/mod.rs
@@ -9,6 +9,7 @@
 mod npm_steps;
 pub mod options;
 pub mod place;
+mod resolve;
 
 use std::time::SystemTime;
 
@@ -16,17 +17,15 @@
 use crate::commands::npm::packages::Source;
 use crate::commands::resolve::{Resolved, resolve_installed};
 use crate::commands::transcript::Transcript;
-use crate::commands::version_remote::lookup;
 use crate::context::Context;
-use crate::domain::floor::VersionFloor;
 use crate::domain::platform::binary_available;
-use crate::domain::remote::Query;
 use crate::domain::version::{Flavor, Version};
 use crate::domain::version_prefix::with_v_prefix;
 use crate::error::{CliError, NvmExitCode};
 use flow::{Halt, Step};
 use lock::{LockRequest, acquire};
 use options::Options;
+use resolve::{check_floor, resolve};
 
 const USAGE: &str = "No version provided and no .nvmrc file found\n\
 Usage: nvm install [<version>]\n  \
@@ -159,58 +158,6 @@
     Ok(())
 }
 
-fn resolve(context: &Context<'_>, options: &Options, transcript: &mut Transcript) -> Step<Version> {
-    let query = Query {
-        pattern: Some(options.version.clone()).filter(|text| !text.is_empty()),
-        lts: options.lts.clone(),
-    };
-    let found = lookup(context, query)?;
-    for warning in found.warnings {
-        transcript.err(warning);
-    }
-    found.version.ok_or_else(|| {
-        transcript.err(not_found_message(options));
-        Halt::Exit(NvmExitCode::InvalidVersion)
-    })
-}
-
-fn not_found_message(options: &Options) -> String {
-    let version = &options.version;
-    match options.lts.as_deref() {
-        Some("*") => format!(
-            "Version '{version}' (with LTS filter) not found - try `nvm ls-remote --lts` to browse available versions."
-        ),
-        Some(lts) if version.is_empty() => format!(
-            "Version with LTS filter '{lts}' not found - try `nvm ls-remote --lts={lts}` to browse available versions."
-        ),
-        Some(lts) => format!(
-            "Version '{version}' (with LTS filter '{lts}') not found - try `nvm ls-remote --lts={lts}` to browse available versions."
-        ),
-        None => format!(
-            "Version '{version}' not found - try `nvm ls-remote` to browse available versions."
-        ),
-    }
-}
-
-fn check_floor(context: &Context<'_>, version: &Version, transcript: &mut Transcript) -> Step<()> {
-    let from_file = context
-        .fs
-        .read_to_string(&context.nvm_dir()?.join("min-version"))
-        .ok();
-    let from_env = context.env.var("NVM_MIN_VERSION");
-    let floor = VersionFloor::from_sources(from_env.as_deref(), from_file.as_deref())
-        .and_then(|floor| floor.map_or(Ok(()), |floor| floor.check(version)));
-    let Err(error) = floor else {
-        return Ok(());
-    };
-    transcript.err(error.to_string());
-    if matches!(error, crate::error::FloorError::Below { .. }) {
-        transcript
-            .err("Lower or unset NVM_MIN_VERSION (or edit $NVM_DIR/min-version) to install it.");
-    }
-    Err(Halt::Exit(NvmExitCode::BelowVersionFloor))
-}
-
 /// Everything from the lock to the unpacked directory.
 fn install_binary(
     context: &Context<'_>,
```

Run: `cargo fmt && cargo test`
Expected: PASS, 520 unit tests plus the end-to-end tests, as at the end of
Task 17.

- [ ] **Step 1: Write the failing tests**

Apply to `src/commands/install/fetch/tests.rs`:

```diff
--- a/src/commands/install/fetch/tests.rs
+++ b/src/commands/install/fetch/tests.rs
@@ -31,7 +31,7 @@
     let env = env();
     let context = Context::new(fs, &env).with_http(http).with_digest(digest);
     let mut transcript = Transcript::default();
-    let result = fetch(&context, &version(), &mut transcript);
+    let result = fetch(&context, &version(), false, &mut transcript);
     (result, transcript)
 }
 
@@ -166,7 +166,10 @@
         .with_http(&http)
         .with_digest(&digest);
     let mut transcript = Transcript::default();
-    assert_eq!(fetch(&context, &version(), &mut transcript), Err(Failed));
+    assert_eq!(
+        fetch(&context, &version(), false, &mut transcript),
+        Err(Failed)
+    );
     assert!(stderr(transcript)[0].contains("may only contain a URL"));
     assert!(http.requests().is_empty());
 }
@@ -185,3 +188,38 @@
     );
     assert!(Artifact::of(&context.with_platform(None), &"v20.10.0".parse().unwrap()).is_none());
 }
+
+fn offline(fs: &FakeFileSystem, http: &FakeHttp) -> (Result<PathBuf, Failed>, Transcript) {
+    let env = env();
+    let digest = FakeDigest::default();
+    let context = Context::new(fs, &env).with_http(http).with_digest(&digest);
+    let mut transcript = Transcript::default();
+    let result = fetch(&context, &version(), true, &mut transcript);
+    (result, transcript)
+}
+
+/// The expectations are what the real `nvm install --offline` printed.
+#[test]
+fn offline_uses_the_cached_archive_without_a_checksum_or_the_network() {
+    let fs = FakeFileSystem::default().with_file(TARBALL, "cached");
+    let http = FakeHttp::default();
+    let (result, transcript) = offline(&fs, &http);
+    assert_eq!(result.unwrap(), PathBuf::from(TARBALL));
+    assert!(http.requests().is_empty());
+    assert_eq!(
+        stderr(transcript),
+        [
+            "Offline: using cached archive ${NVM_DIR}/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz"
+        ]
+    );
+}
+
+#[test]
+fn offline_without_a_cached_archive_fails_naming_the_slug() {
+    let (result, transcript) = offline(&FakeFileSystem::default(), &FakeHttp::default());
+    assert_eq!(result, Err(Failed));
+    assert_eq!(
+        stderr(transcript),
+        ["Offline: no cached archive found for node-v20.10.0-linux-x64"]
+    );
+}
```

Apply to `src/commands/install/npm_steps/tests.rs`:

```diff
--- a/src/commands/install/npm_steps/tests.rs
+++ b/src/commands/install/npm_steps/tests.rs
@@ -9,6 +9,14 @@
 
 fn version() -> Version {
     "v20.10.0".parse().unwrap()
+}
+
+fn target(source: Option<Source>) -> Target {
+    Target {
+        version: version(),
+        path: Path::new(PATH).to_path_buf(),
+        source,
+    }
 }
 
 fn options(latest_npm: bool, skip: bool) -> Options {
@@ -27,15 +35,8 @@
     let env = FakeEnv::default().with_var("NVM_DIR", "/n");
     let context = Context::new(fs, &env).with_process(process);
     let mut transcript = Transcript::default();
-    let status = run(
-        &context,
-        options,
-        &version(),
-        Path::new(PATH),
-        None,
-        &mut transcript,
-    )
-    .unwrap();
+    let target = target(None);
+    let status = run(&context, options, &target, &mut transcript).unwrap();
     let output = transcript.finish(status);
     (status, output.stdout, output.stderr)
 }
@@ -173,15 +174,8 @@
     let env = FakeEnv::default().with_var("NVM_DIR", "/n");
     let context = Context::new(fs, &env).with_process(process);
     let mut transcript = Transcript::default();
-    let status = run(
-        &context,
-        &options(false, true),
-        &version(),
-        Path::new(PATH),
-        Some(source),
-        &mut transcript,
-    )
-    .unwrap();
+    let target = target(Some(source.clone()));
+    let status = run(&context, &options(false, true), &target, &mut transcript).unwrap();
     let output = transcript.finish(status);
     (status, output.stdout, output.stderr)
 }
```

Create `src/commands/install/nvmrc_file.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use crate::ports::FileSystem;

    fn version() -> Version {
        "v20.10.0".parse().unwrap()
    }

    #[test]
    fn it_writes_the_version_and_says_so() {
        let (fs, env) = (
            FakeFileSystem::default(),
            FakeEnv::default().with_var("PWD", "/work"),
        );
        let mut transcript = Transcript::default();
        let status = write(&Context::new(&fs, &env), &version(), &mut transcript);
        assert_eq!(status, NvmExitCode::Success);
        assert_eq!(
            fs.read_to_string(Path::new("/work/.nvmrc")).unwrap(),
            "v20.10.0\n"
        );
        let output = transcript.finish(status);
        assert_eq!(output.stdout, "Wrote version number (v20.10.0) to .nvmrc");
    }

    #[test]
    fn no_current_directory_is_a_warning_and_status_3() {
        let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
        let mut transcript = Transcript::default();
        let status = write(&Context::new(&fs, &env), &version(), &mut transcript);
        assert_eq!(status, NvmExitCode::InvalidVersion);
        let output = transcript.finish(status);
        assert_eq!(
            output.stderr,
            "Warning: Unable to write version number (v20.10.0) to .nvmrc"
        );
    }
}
```

Create `src/commands/install/offline/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/install/offline/tests.rs`:

```rust
use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem};

fn cached(fs: &FakeFileSystem, pattern: &str) -> Option<String> {
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let context = Context::new(fs, &env);
    cached_version(&context, pattern).map(|version| version.to_string())
}

fn cache() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file(
            "/n/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz",
            "t",
        )
        .with_file(
            "/n/.cache/bin/node-v18.19.0-linux-x64/node-v18.19.0-linux-x64.tar.gz",
            "t",
        )
        .with_file(
            "/n/.cache/bin/node-v18.18.0-linux-x64/node-v18.18.0-linux-x64.tar.gz",
            "t",
        )
        .with_file(
            "/n/.cache/bin/node-v22.0.0-darwin-arm64/node-v22.0.0-darwin-arm64.tar.gz",
            "t",
        )
        .with_file(
            "/n/.cache/bin/iojs-v3.3.1-linux-x64/iojs-v3.3.1-linux-x64.tar.gz",
            "t",
        )
        .with_file("/n/.cache/src/node-v16.20.2/node-v16.20.2.tar.gz", "t")
}

/// The expectations are what `nvm_ls_cached` of the real `nvm.sh` lists.
#[test]
fn the_newest_cached_match_wins() {
    let fs = cache();
    assert_eq!(cached(&fs, "18").as_deref(), Some("v18.19.0"));
    assert_eq!(cached(&fs, "20").as_deref(), Some("v20.10.0"));
    assert_eq!(cached(&fs, "v18.18").as_deref(), Some("v18.18.0"));
}

#[test]
fn only_this_platform_and_source_entries_count() {
    let fs = cache();
    assert_eq!(cached(&fs, "22"), None);
    assert_eq!(cached(&fs, "16").as_deref(), Some("v16.20.2"));
}

#[test]
fn io_js_is_found_by_its_name_and_a_pattern_is_a_substring() {
    let fs = cache();
    assert_eq!(cached(&fs, "iojs").as_deref(), Some("iojs-v3.3.1"));
    assert_eq!(cached(&fs, "20.1").as_deref(), Some("v20.10.0"));
    assert_eq!(cached(&fs, "99"), None);
}

#[test]
fn an_empty_cache_has_nothing() {
    assert_eq!(cached(&FakeFileSystem::default(), "20"), None);
}
```

Apply to `src/commands/install/options/tests.rs`:

```diff
--- a/src/commands/install/options/tests.rs
+++ b/src/commands/install/options/tests.rs
@@ -91,6 +91,31 @@
 }
 
 #[test]
+fn offline_and_save_are_read_before_the_version() {
+    let options = parsed("--offline --save 20").unwrap();
+    assert!(options.offline && options.save);
+    assert!(parsed("-w 20").unwrap().save);
+    let neither = parsed("20").unwrap();
+    assert!(!neither.offline && !neither.save);
+}
+
+#[test]
+fn save_after_the_version_is_ignored_like_nvm_sh_does() {
+    let options = parsed("20 --save -w").unwrap();
+    assert!(!options.save);
+    assert_eq!(options.version, "20");
+}
+
+#[test]
+fn save_twice_is_status_6() {
+    for line in ["--save --save 20", "-w --save 20", "--save -w 20"] {
+        let error = parsed(line).unwrap_err();
+        assert_eq!(error.exit_code(), NvmExitCode::InvalidOptions, "{line}");
+        assert_eq!(error.to_string(), "--save and -w may only be provided once");
+    }
+}
+
+#[test]
 fn nothing_at_all_is_no_version() {
     let options = parsed("").unwrap();
     assert_eq!(
@@ -155,7 +180,7 @@
 
 #[test]
 fn options_that_need_a_source_build_or_npm_are_not_supported_yet() {
-    for line in ["-s 20", "-j 4 20", "--offline 20", "20 --save"] {
+    for line in ["-s 20", "-j 4 20"] {
         let error = parsed(line).unwrap_err();
         assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption, "{line}");
         assert!(
```

Apply to `src/commands/install/tests/mod.rs`:

```diff
--- a/src/commands/install/tests/mod.rs
+++ b/src/commands/install/tests/mod.rs
@@ -76,6 +76,7 @@
 
 mod failures;
 mod npm;
+mod offline_and_save;
 
 #[test]
 fn a_fresh_install_downloads_unpacks_and_makes_the_default_alias() {
```

Create `src/commands/install/tests/offline_and_save.rs`:

```rust
use super::*;

fn offline_world(fs: FakeFileSystem) -> World {
    World { fs, ..World::new() }
}

/// The expectations are what the real `nvm install --offline` printed.
#[test]
fn offline_nothing_installed_or_cached_is_status_3_and_asks_for_nothing_online() {
    let world = World::new();
    let output = world.run("--offline 20").unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stderr,
        "Version '20' not found locally or in cache - try `nvm ls` to browse available versions."
    );
    assert!(world.http.requests().is_empty());
}

#[test]
fn offline_installs_from_the_cached_archive_without_a_checksum() {
    let world = offline_world(FakeFileSystem::default().with_file(TARBALL, "cached"));
    let output = world.run("--offline 20").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        lines(&output.stdout),
        [
            "Downloading and installing node v20.10.0...",
            "Creating default alias: default -> 20 (-> v20.10.0 *)",
        ]
    );
    assert!(
        output
            .stderr
            .starts_with("Offline: using cached archive ${NVM_DIR}/.cache/bin")
    );
    assert!(world.installed());
    assert!(world.http.requests().is_empty());
}

#[test]
fn offline_an_installed_version_is_just_reported() {
    let world = World::new();
    world.run("20").unwrap();
    let requests = world.http.requests().len();
    let output = world.run("--offline 20").unwrap();
    assert_eq!(output.stderr, "v20.10.0 is already installed.");
    assert_eq!(world.http.requests().len(), requests);
}

#[test]
fn offline_lts_needs_the_local_alias_and_then_the_installed_or_cached_version() {
    let missing = World::new().run("--offline --lts").unwrap();
    assert_eq!(missing.status, NvmExitCode::InvalidVersion);
    assert_eq!(missing.stdout, "Installing latest LTS version.");
    assert_eq!(
        missing.stderr,
        "LTS alias '*' not found locally. Run `nvm ls-remote --lts` first to populate LTS aliases."
    );
    let fs = FakeFileSystem::default()
        .with_file("/n/alias/lts/*", "lts/iron\n")
        .with_file("/n/alias/lts/iron", "v20.10.0\n")
        .with_file(TARBALL, "cached");
    let world = offline_world(fs);
    let output = world.run("--offline lts/iron").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(world.installed());
}

#[test]
fn offline_with_a_missing_archive_fails_like_a_failed_download() {
    let world = offline_world(FakeFileSystem::default().with_dir("/n/.cache/src/node-v20.10.0"));
    let output = world.run("--offline 20").unwrap();
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    assert!(
        output
            .stderr
            .starts_with("Offline: no cached archive found for node-v20.10.0-linux-x64")
    );
}

fn with_pwd(world: World) -> World {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PWD", "/work");
    World { env, ..world }
}

#[test]
fn save_writes_the_resolved_version_of_an_installed_version_to_nvmrc() {
    let world = with_pwd(World::new());
    world.run("20").unwrap();
    let output = world.run("--save 20").unwrap();
    assert_eq!(output.stdout, "Wrote version number (v20.10.0) to .nvmrc");
    assert_eq!(world.text("/work/.nvmrc").as_deref(), Some("v20.10.0\n"));
}

#[test]
fn save_also_writes_after_a_fresh_install_unlike_nvm_sh() {
    let world = with_pwd(World::new());
    let output = world.run("-w 20").unwrap();
    assert!(
        output
            .stdout
            .ends_with("Wrote version number (v20.10.0) to .nvmrc")
    );
    assert_eq!(world.text("/work/.nvmrc").as_deref(), Some("v20.10.0\n"));
}

#[test]
fn save_after_the_version_does_nothing_like_nvm_sh() {
    let world = with_pwd(World::new());
    world.run("20 --save").unwrap();
    assert_eq!(world.text("/work/.nvmrc"), None);
}

#[test]
fn a_save_that_cannot_write_is_status_3_with_a_warning_and_skips_the_alias() {
    let world = World::new();
    world.run("20").unwrap();
    let output = world.run("--save --alias=work 20").unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stderr,
        "v20.10.0 is already installed.\nWarning: Unable to write version number (v20.10.0) to .nvmrc"
    );
    assert_eq!(world.text("/n/alias/work"), None);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "no field `offline` on type
`Options`", "cannot find function `cached_version`" and "cannot find module
`nvmrc_file`".

- [ ] **Step 3: Write the implementation**

Apply to `src/commands/install/fetch/mod.rs` (above the test module):

```diff
--- a/src/commands/install/fetch/mod.rs
+++ b/src/commands/install/fetch/mod.rs
@@ -16,6 +16,8 @@
 
 /// Where an archive lives in the cache.
 pub struct Artifact {
+    /// `node-v20.10.0-linux-x64`.
+    pub slug: String,
     /// `node-v20.10.0-linux-x64.tar.gz`.
     pub file_name: String,
     /// `$NVM_DIR/.cache/bin/<slug>`.
@@ -33,6 +35,7 @@
         let file_name = format!("{slug}.tar.gz");
         let tarball = directory.join(&file_name);
         Some(Self {
+            slug,
             file_name,
             directory,
             tarball,
@@ -69,9 +72,13 @@
 pub fn fetch(
     context: &Context<'_>,
     version: &Version,
+    offline: bool,
     transcript: &mut Transcript,
 ) -> Result<PathBuf, Failed> {
     let artifact = Artifact::of(context, version).ok_or(Failed)?;
+    if offline {
+        return cached_only(context, &artifact, transcript);
+    }
     let mirror = mirror::from_env(context.env, version.flavor).map_err(|error| {
         transcript.err(error.to_string());
         Failed
@@ -88,6 +95,24 @@
     download(context, &mirror, version, &artifact, transcript)?;
     verify(context, &artifact, &expected, transcript)?;
     Ok(artifact.tarball)
+}
+
+/// `--offline`: the cached archive, taken without a checksum, or nothing.
+fn cached_only(
+    context: &Context<'_>,
+    artifact: &Artifact,
+    transcript: &mut Transcript,
+) -> Result<PathBuf, Failed> {
+    if context.fs.file_info(&artifact.tarball).is_ok() {
+        let shown = sanitize(context, &artifact.tarball);
+        transcript.err(format!("Offline: using cached archive {shown}"));
+        return Ok(artifact.tarball.clone());
+    }
+    transcript.err(format!(
+        "Offline: no cached archive found for {}",
+        artifact.slug
+    ));
+    Err(Failed)
 }
 
 /// What `SHASUMS256.txt` lists for the archive; empty when it cannot be had.
```

Apply to `src/commands/install/flow.rs` (above the test module):

```diff
--- a/src/commands/install/flow.rs
+++ b/src/commands/install/flow.rs
@@ -1,5 +1,9 @@
 //! How the steps of an install end early.
 
+use std::path::PathBuf;
+
+use crate::commands::npm::packages::Source;
+use crate::domain::version::Version;
 use crate::error::{CliError, NvmExitCode};
 
 /// Why an install stops before its last step.
@@ -18,3 +22,12 @@
 }
 
 pub type Step<T> = Result<T, Halt>;
+
+/// The version an install is about.
+pub struct Target {
+    pub version: Version,
+    /// `$NVM_DIR/versions/<flavor>/<version>`.
+    pub path: PathBuf,
+    /// Where `--reinstall-packages-from` takes packages from.
+    pub source: Option<Source>,
+}
```

Apply to `src/commands/install/mod.rs` (above the test module):

```diff
--- a/src/commands/install/mod.rs
+++ b/src/commands/install/mod.rs
@@ -7,6 +7,8 @@
 mod flow;
 pub mod lock;
 mod npm_steps;
+mod nvmrc_file;
+mod offline;
 pub mod options;
 pub mod place;
 mod resolve;
@@ -22,7 +24,7 @@
 use crate::domain::version::{Flavor, Version};
 use crate::domain::version_prefix::with_v_prefix;
 use crate::error::{CliError, NvmExitCode};
-use flow::{Halt, Step};
+use flow::{Halt, Step, Target};
 use lock::{LockRequest, acquire};
 use options::Options;
 use resolve::{check_floor, resolve};
@@ -50,42 +52,70 @@
     announce(options, transcript)?;
     let version = resolve(context, options, transcript)?;
     check_floor(context, &version, transcript)?;
-    let version_path = place::version_path(context, &version)?;
+    let path = place::version_path(context, &version)?;
     let source = reinstall_source(context, options, &version, transcript)?;
-    if place::is_valid_install(context, &version_path) {
-        transcript.err(format!("{version} is already installed."));
-        let status = npm_steps::run(
-            context,
-            options,
-            &version,
-            &version_path,
-            source.as_ref(),
-            transcript,
-        )?;
-        defaults::ensure_default(context, &alias_target(options), transcript)?;
-        if status == NvmExitCode::Success {
-            apply_alias(context, options, transcript)?;
-        }
-        return end_with(status);
-    }
-    install_binary(context, &version, &version_path, transcript)?;
+    let target = Target {
+        version,
+        path,
+        source,
+    };
+    if place::is_valid_install(context, &target.path) {
+        already_installed(context, options, &target, transcript)
+    } else {
+        fresh_install(context, options, &target, transcript)
+    }
+}
+
+/// A valid install is left as it is; the steps after the install still run.
+/// The status of the last one is the status of the command: `--save`, which
+/// writes `.nvmrc`, replaces that of the `npm` steps, as in `nvm.sh`.
+fn already_installed(
+    context: &Context<'_>,
+    options: &Options,
+    target: &Target,
+    transcript: &mut Transcript,
+) -> Step<()> {
+    transcript.err(format!("{} is already installed.", target.version));
+    let mut status = npm_steps::run(context, options, target, transcript)?;
+    defaults::ensure_default(context, &alias_target(options), transcript)?;
+    if options.save {
+        status = nvmrc_file::write(context, &target.version, transcript);
+    }
+    if status == NvmExitCode::Success {
+        apply_alias(context, options, transcript)?;
+    }
+    end_with(status)
+}
+
+/// Unlike `nvm.sh`, which forgets `--save` on a fresh install, `.nvmrc` is
+/// written here too, after everything else.
+fn fresh_install(
+    context: &Context<'_>,
+    options: &Options,
+    target: &Target,
+    transcript: &mut Transcript,
+) -> Step<()> {
+    install_binary(
+        context,
+        &target.version,
+        &target.path,
+        options.offline,
+        transcript,
+    )?;
     apply_alias(context, options, transcript)?;
-    if !place::is_valid_install(context, &version_path) {
+    if !place::is_valid_install(context, &target.path) {
         let message = format!(
-            "The install of {version} reported success but failed verification; not activating it."
+            "The install of {} reported success but failed verification; not activating it.",
+            target.version
         );
         transcript.err(message);
         return Err(Halt::Exit(NvmExitCode::Failure));
     }
     defaults::ensure_default(context, &alias_target(options), transcript)?;
-    let status = npm_steps::run(
-        context,
-        options,
-        &version,
-        &version_path,
-        source.as_ref(),
-        transcript,
-    )?;
+    let mut status = npm_steps::run(context, options, target, transcript)?;
+    if options.save && status == NvmExitCode::Success {
+        status = nvmrc_file::write(context, &target.version, transcript);
+    }
     end_with(status)
 }
 
@@ -163,6 +193,7 @@
     context: &Context<'_>,
     version: &Version,
     version_path: &std::path::Path,
+    offline: bool,
     transcript: &mut Transcript,
 ) -> Step<()> {
     let unavailable = !binary_available(version)
@@ -181,8 +212,8 @@
         "Downloading and installing {name} {}...",
         version.directory_name()
     ));
-    let tarball =
-        fetch::fetch(context, version, transcript).map_err(|_| binary_failed(transcript))?;
+    let tarball = fetch::fetch(context, version, offline, transcript)
+        .map_err(|_| binary_failed(transcript))?;
     let artifact =
         fetch::Artifact::of(context, version).ok_or_else(|| binary_failed(transcript))?;
     place::place(context, &tarball, &artifact.files(), version_path).map_err(|message| {
```

Apply to `src/commands/install/npm_steps/mod.rs` (above the test module):

```diff
--- a/src/commands/install/npm_steps/mod.rs
+++ b/src/commands/install/npm_steps/mod.rs
@@ -3,9 +3,7 @@
 //! an `npm` skips these steps with a warning (`nvm.sh` would download an `npm`
 //! installer from the internet and run it; this port never does).
 
-use std::path::Path;
-
-use crate::commands::install::flow::Step;
+use crate::commands::install::flow::{Step, Target};
 use crate::commands::install::options::Options;
 use crate::commands::npm::Npm;
 use crate::commands::npm::latest::{Node, install_latest};
@@ -25,12 +23,11 @@
 pub fn run(
     context: &Context<'_>,
     options: &Options,
-    version: &Version,
-    version_path: &Path,
-    source: Option<&Source>,
+    target: &Target,
     transcript: &mut Transcript,
 ) -> Step<NvmExitCode> {
-    let npm = Npm::in_version(context, version_path);
+    let version = &target.version;
+    let npm = Npm::in_version(context, &target.path);
     if options.latest_npm {
         let status = upgrade(context, npm.as_ref(), version, transcript);
         if status != NvmExitCode::Success {
@@ -43,9 +40,12 @@
             return Ok(status);
         }
     }
-    Ok(source.map_or(NvmExitCode::Success, |source| {
-        copy_packages(context, npm.as_ref(), version, source, transcript)
-    }))
+    Ok(target
+        .source
+        .as_ref()
+        .map_or(NvmExitCode::Success, |source| {
+            copy_packages(context, npm.as_ref(), version, source, transcript)
+        }))
 }
 
 /// `nvm reinstall-packages <source>` into the version just installed.
```

Insert above the `#[cfg(test)]` line of `src/commands/install/nvmrc_file.rs`:

```rust
//! `--save` / `-w`: write the installed version to `.nvmrc` in the current
//! directory (`nvm_write_nvmrc`).

use std::path::PathBuf;

use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::version::Version;
use crate::error::NvmExitCode;

/// Writes `<version>\n` to `$PWD/.nvmrc`. Failing to is status 3, after the
/// warning `nvm.sh` gives.
pub fn write(context: &Context<'_>, version: &Version, transcript: &mut Transcript) -> NvmExitCode {
    let directory = context.env.var_os("PWD").map(PathBuf::from);
    let written = directory
        .map(|directory| directory.join(".nvmrc"))
        .map(|file| context.fs.write_file(&file, &format!("{version}\n")));
    if let Some(Ok(())) = written {
        transcript.out(format!("Wrote version number ({version}) to .nvmrc"));
        return NvmExitCode::Success;
    }
    transcript.err(format!(
        "Warning: Unable to write version number ({version}) to .nvmrc"
    ));
    NvmExitCode::InvalidVersion
}

```

Insert above the `#[cfg(test)]` line of `src/commands/install/offline/mod.rs`:

```rust
//! `--offline`: what is installed or cached is all there is
//! (`nvm_offline_version` and `nvm_ls_cached`).

use crate::context::Context;
use crate::domain::version::Version;
use crate::domain::version_prefix::with_v_prefix;

/// `node-v20.10.0-linux-x64` and `iojs-v3.3.1-linux-x64` hold archives, and
/// `node-v20.10.0` holds a source tree: all versions of the entries of the
/// cache directories `bin` (for this platform) and `src`.
fn cached_versions(context: &Context<'_>) -> Vec<Version> {
    let Ok(cache) = context.cache_dir() else {
        return Vec::new();
    };
    let suffix = context
        .platform()
        .map(|platform| format!("-{}-{}", platform.os.slug(), platform.arch));
    let names = |directory: &str| -> Vec<String> {
        context
            .fs
            .read_dir(&cache.join(directory))
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| entry.is_dir)
            .map(|entry| entry.name)
            .collect()
    };
    let binaries = names("bin")
        .into_iter()
        .filter_map(|name| name.strip_suffix(suffix.as_deref()?).map(str::to_owned));
    binaries
        .chain(names("src"))
        .filter_map(|name| name.strip_prefix("node-").unwrap_or(&name).parse().ok())
        .collect()
}

/// The newest cached version whose name contains `pattern` (with the `v` that
/// a number needs), whether or not it is installed.
#[must_use]
pub fn cached_version(context: &Context<'_>, pattern: &str) -> Option<Version> {
    let wanted = with_v_prefix(pattern);
    cached_versions(context)
        .into_iter()
        .filter(|version| version.to_string().contains(&wanted))
        .max()
}

```

Apply to `src/commands/install/options/mod.rs` (above the test module):

```diff
--- a/src/commands/install/options/mod.rs
+++ b/src/commands/install/options/mod.rs
@@ -1,12 +1,13 @@
 //! The command line of `nvm install`, as `nvm.sh` reads it: options first,
-//! then the version (or `lts/*`, `lts/<name>`), and what follows it is not
-//! looked at, except for the options that this port does not have.
+//! then the version (or `lts/*`, `lts/<name>`). What follows the version is
+//! looked at only for `--skip-default-packages` and the two options that name
+//! a version to take packages from; `--save` there, like any other word, is
+//! ignored, as in `nvm.sh`.
 
 use crate::error::CliError;
 
-/// Options that need a source build, `npm` or a `.nvmrc`: not in this port yet.
-const NOT_YET: [&str; 3] = ["-s", "-j", "--offline"];
-const NOT_YET_AFTER_VERSION: [&str; 2] = ["--save", "-w"];
+/// Options that need a source build: not in this port yet.
+const NOT_YET: [&str; 2] = ["-s", "-j"];
 /// Both options mean the same; `nvm.sh` words its messages after the one used.
 const REINSTALL_OPTIONS: [&str; 2] = ["--reinstall-packages-from", "--copy-packages-from"];
 
@@ -30,6 +31,10 @@
     /// `--reinstall-packages-from=<version>` (or `--copy-packages-from`): the
     /// version, as given, whose global packages are installed again.
     pub reinstall_from: Option<String>,
+    /// `--offline`: use what is installed or cached, and never the network.
+    pub offline: bool,
+    /// `--save` or `-w`: write the version to `.nvmrc` in the current directory.
+    pub save: bool,
 }
 
 fn unsupported(option: &str) -> CliError {
@@ -85,6 +90,8 @@
         match option.as_str() {
             "-b" | "--no-progress" => {}
             "--latest-npm" => options.latest_npm = true,
+            "--offline" => options.offline = true,
+            "--save" | "-w" => set_save(&mut options)?,
             "--skip-default-packages" => options.skip_default_packages = true,
             "--lts" => options.lts = Some("*".to_owned()),
             "--default" => set_alias(&mut options, "default")?,
@@ -109,10 +116,8 @@
     for option in rest {
         if option == "--skip-default-packages" {
             options.skip_default_packages = true;
-        } else if !read_reinstall(&mut options, option)?
-            && NOT_YET_AFTER_VERSION.contains(&option.as_str())
-        {
-            return Err(unsupported(option));
+        } else {
+            read_reinstall(&mut options, option)?;
         }
     }
     options.announce_lts = options.lts.is_some() && options.version.is_empty();
@@ -124,13 +129,28 @@
 fn is_option(arg: &str) -> bool {
     matches!(
         arg,
-        "-b" | "--no-progress" | "--lts" | "--default" | "--latest-npm" | "--skip-default-packages"
+        "-b" | "--no-progress"
+            | "--lts"
+            | "--default"
+            | "--latest-npm"
+            | "--skip-default-packages"
+            | "--offline"
+            | "--save"
+            | "-w"
     ) || arg.starts_with("--lts=")
         || arg.starts_with("--alias=")
         || arg.starts_with("---")
         || NOT_YET.contains(&arg)
         || reinstall_option(arg).is_some()
-        || NOT_YET_AFTER_VERSION.contains(&arg)
+}
+
+fn set_save(options: &mut Options) -> Result<(), CliError> {
+    if options.save {
+        let message = "--save and -w may only be provided once";
+        return Err(CliError::InvalidOptions(message.to_owned()));
+    }
+    options.save = true;
+    Ok(())
 }
 
 fn set_alias(options: &mut Options, name: &str) -> Result<(), CliError> {
```

Apply to `src/commands/install/resolve.rs` (above the test module):

```diff
--- a/src/commands/install/resolve.rs
+++ b/src/commands/install/resolve.rs
@@ -2,10 +2,13 @@
 //! stands for, and whether it may be installed (the version floor).
 
 use crate::commands::install::flow::{Halt, Step};
+use crate::commands::install::offline::cached_version;
 use crate::commands::install::options::Options;
+use crate::commands::resolve::{Resolved, resolve_installed};
 use crate::commands::transcript::Transcript;
 use crate::commands::version_remote::lookup;
 use crate::context::Context;
+use crate::domain::alias::{AliasStore, resolve as resolve_alias};
 use crate::domain::floor::VersionFloor;
 use crate::domain::remote::Query;
 use crate::domain::version::Version;
@@ -16,6 +19,9 @@
     options: &Options,
     transcript: &mut Transcript,
 ) -> Step<Version> {
+    if options.offline {
+        return resolve_offline(context, options, transcript);
+    }
     let query = Query {
         pattern: Some(options.version.clone()).filter(|text| !text.is_empty()),
         lts: options.lts.clone(),
@@ -30,22 +36,77 @@
     })
 }
 
+/// `--offline`: an installed version first, then the newest cached archive.
+fn resolve_offline(
+    context: &Context<'_>,
+    options: &Options,
+    transcript: &mut Transcript,
+) -> Step<Version> {
+    let pattern = offline_pattern(context, options, transcript)?;
+    if let Resolved::Installed(version) = resolve_installed(context, &pattern)? {
+        return Ok(version);
+    }
+    cached_version(context, &pattern).ok_or_else(|| {
+        transcript.err(not_found_message(options));
+        Halt::Exit(NvmExitCode::InvalidVersion)
+    })
+}
+
+/// What to look for offline: the version as given, or what the local `lts/*`
+/// alias stands for, as nothing can be asked of the mirror.
+fn offline_pattern(
+    context: &Context<'_>,
+    options: &Options,
+    transcript: &mut Transcript,
+) -> Step<String> {
+    let Some(lts) = options.lts.as_deref() else {
+        return Ok(options.version.clone());
+    };
+    let name = if lts == "*" {
+        "lts/*".to_owned()
+    } else {
+        format!("lts/{lts}")
+    };
+    let store = context.alias_store()?;
+    let resolved = store
+        .target(&name)
+        .and_then(|_| resolve_alias(&store, &name).ok());
+    resolved.ok_or_else(|| {
+        transcript.err(format!(
+            "LTS alias '{lts}' not found locally. Run `nvm ls-remote --lts` first to populate LTS aliases."
+        ));
+        Halt::Exit(NvmExitCode::InvalidVersion)
+    })
+}
+
+/// What `nvm.sh` says when nothing matches, and where to look instead.
 fn not_found_message(options: &Options) -> String {
     let version = &options.version;
-    match options.lts.as_deref() {
-        Some("*") => format!(
-            "Version '{version}' (with LTS filter) not found - try `nvm ls-remote --lts` to browse available versions."
+    let (filter, command) = match options.lts.as_deref() {
+        Some("*") => (
+            " (with LTS filter)".to_owned(),
+            "nvm ls-remote --lts".to_owned(),
         ),
-        Some(lts) if version.is_empty() => format!(
-            "Version with LTS filter '{lts}' not found - try `nvm ls-remote --lts={lts}` to browse available versions."
+        Some(lts) if version.is_empty() => {
+            return format!(
+                "Version with LTS filter '{lts}' not found - try `nvm ls-remote --lts={lts}` to browse available versions."
+            );
+        }
+        Some(lts) => (
+            format!(" (with LTS filter '{lts}')"),
+            format!("nvm ls-remote --lts={lts}"),
         ),
-        Some(lts) => format!(
-            "Version '{version}' (with LTS filter '{lts}') not found - try `nvm ls-remote --lts={lts}` to browse available versions."
-        ),
-        None => format!(
-            "Version '{version}' not found - try `nvm ls-remote` to browse available versions."
-        ),
-    }
+        None if options.offline => (String::new(), "nvm ls".to_owned()),
+        None => (String::new(), "nvm ls-remote".to_owned()),
+    };
+    let wherever = if options.offline {
+        " locally or in cache"
+    } else {
+        ""
+    };
+    format!(
+        "Version '{version}'{filter} not found{wherever} - try `{command}` to browse available versions."
+    )
 }
 
 pub(super) fn check_floor(
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 540 unit tests plus the end-to-end tests.

- [ ] **Step 5: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(install): work offline and save the version to .nvmrc"
```

---

### Task 19: The foundations of a source build

**Files:**

- Create: `src/domain/source_build/mod.rs`, `src/domain/source_build/tests.rs`,
  `src/adapters/std_cpu.rs`, `src/adapters/no_cpu.rs`, `src/fakes/cpu.rs`
- Modify: `src/domain/platform/mod.rs`, `src/domain/platform/tests.rs`,
  `src/domain/mod.rs`, `src/ports/mod.rs`, `src/adapters/mod.rs`,
  `src/fakes/mod.rs`, `src/context/mod.rs`, `src/context/tests.rs`

**Interfaces:**

- Produces: `ports::Cpu::cores()`, `StdCpu` (the parallelism the operating
  system grants, which respects container limits), `NoCpu`, test-only `FakeCpu`,
  `Context::with_cpu` and `Context::cpu()`; `Os::{FreeBsd, OpenBsd}` and
  `Os::has_binaries()` (the BSDs are built from source);
  `domain::source_build::{jobs, natural_jobs, clang_version, compiler, make,
  Compiler, Jobs}`.
- Behaviour (from `nvm_get_make_jobs` and `nvm_install_source`, checked against
  the real script): `-j <n>` with a natural number is `number of `make` jobs: n`;
  else one core fewer than the machine has when it has more than two
  (`Detected that you have N CPU core(s)`, `Running with N-1 threads to speed up
  the build`), else one job; an invalid `-j` is `<x> is invalid for number of
  `make` jobs, must be a natural number`. `clang --version` is read from the
  word after `version`. `CC=cc CXX=c++` (each `${CC:-cc}`) are given to `make`
  on macOS and the BSDs, and on any system when Clang 3.5 or later is there and
  `CC` or `CXX` is not set; the BSDs and AIX use `gmake`; `node` before 0.12
  gets `SHELL=/bin/sh`.
- [ ] **Step 1: Declare the new modules**

Apply to `src/adapters/mod.rs`:

```diff
--- a/src/adapters/mod.rs
+++ b/src/adapters/mod.rs
@@ -1,11 +1,13 @@
 pub mod fs_alias_store;
 pub mod no_archive;
+pub mod no_cpu;
 pub mod no_digest;
 pub mod no_http;
 pub mod no_process;
 pub mod no_sleeper;
 pub mod retrying_http;
 pub mod sha256_digest;
+pub mod std_cpu;
 pub mod std_env;
 pub mod std_fs;
 pub mod std_process;
```

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -16,5 +16,6 @@
 pub mod platform;
 pub mod remote;
 pub mod remote_format;
+pub mod source_build;
 pub mod version;
 pub mod version_prefix;
```

- [ ] **Step 2: Write the failing tests**

Create `src/adapters/no_cpu.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_nothing() {
        assert_eq!(NoCpu.cores(), None);
    }
}
```

Create `src/adapters/std_cpu.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_has_at_least_one_core() {
        assert!(StdCpu.cores().is_some_and(|cores| cores >= 1));
    }
}
```

Apply to `src/context/tests.rs`:

```diff
--- a/src/context/tests.rs
+++ b/src/context/tests.rs
@@ -1,6 +1,8 @@
 use super::*;
 use crate::domain::platform::Platform;
-use crate::fakes::{FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeSleeper};
+use crate::fakes::{
+    FakeArchive, FakeCpu, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeSleeper,
+};
 
 fn nvm_dir_for(env: &FakeEnv) -> Result<PathBuf, CliError> {
     let fs = FakeFileSystem::default();
@@ -122,6 +124,17 @@
 }
 
 #[test]
+fn a_context_does_not_know_its_processors_until_it_is_given_a_cpu() {
+    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
+    assert_eq!(Context::new(&fs, &env).cpu().cores(), None);
+    let cpu = FakeCpu::with_cores(8);
+    assert_eq!(
+        Context::new(&fs, &env).with_cpu(&cpu).cpu().cores(),
+        Some(8)
+    );
+}
+
+#[test]
 fn a_context_is_linux_x64_until_told_otherwise() {
     let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
     let context = Context::new(&fs, &env);
```

Apply to `src/domain/platform/tests.rs`:

```diff
--- a/src/domain/platform/tests.rs
+++ b/src/domain/platform/tests.rs
@@ -26,7 +26,7 @@
 
 #[test]
 fn an_unsupported_operating_system_has_no_platform() {
-    assert_eq!(Platform::from_host("freebsd", "x86_64", false), None);
+    assert_eq!(Platform::from_host("solaris", "x86_64", false), None);
     assert_eq!(Platform::from_host("windows", "x86_64", false), None);
 }
 
@@ -104,3 +104,13 @@
     assert!(binary_available(&version("v0.8.6")));
     assert!(binary_available(&version("iojs-v1.0.0")));
 }
+
+#[test]
+fn the_bsds_are_built_from_source_because_they_have_no_binaries() {
+    for (host, os) in [("freebsd", Os::FreeBsd), ("openbsd", Os::OpenBsd)] {
+        let platform = Platform::from_host(host, "x86_64", false).unwrap();
+        assert_eq!(platform.os, os);
+        assert!(!os.has_binaries());
+    }
+    assert!(Os::Linux.has_binaries() && Os::Darwin.has_binaries() && Os::Aix.has_binaries());
+}
```

Create `src/domain/source_build/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/domain/source_build/tests.rs`:

```rust
use super::*;

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

/// Every expectation is what `nvm_get_make_jobs` of the real `nvm.sh` prints.
#[test]
fn a_natural_number_of_jobs_is_used_as_it_is() {
    let said = jobs(Some("4"), Some(18));
    assert_eq!(
        (said.jobs, said.stdout, said.stderr),
        (4, vec!["number of `make` jobs: 4".to_owned()], vec![])
    );
}

#[test]
fn without_a_request_one_core_is_left_free_when_there_are_more_than_two() {
    let said = jobs(None, Some(18));
    assert_eq!(said.jobs, 17);
    assert_eq!(
        said.stdout,
        [
            "Detected that you have 18 CPU core(s)",
            "Running with 17 threads to speed up the build"
        ]
    );
    assert_eq!(jobs(None, Some(3)).jobs, 2);
}

#[test]
fn two_cores_or_fewer_build_with_one_job() {
    for cores in [1, 2] {
        let said = jobs(None, Some(cores));
        assert_eq!(said.jobs, 1);
        assert_eq!(
            said.stdout,
            [
                format!("Detected that you have {cores} CPU core(s)"),
                "Number of CPU core(s) less than or equal to 2, running in single-threaded mode"
                    .to_owned()
            ]
        );
    }
}

#[test]
fn a_request_that_is_not_a_natural_number_is_reported_and_the_cores_decide() {
    for bad in ["abc", "0", "-3", "1.5"] {
        let said = jobs(Some(bad), Some(8));
        assert_eq!(said.jobs, 7, "{bad}");
        assert_eq!(
            said.stderr,
            [format!(
                "{bad} is invalid for number of `make` jobs, must be a natural number"
            )]
        );
    }
    assert!(jobs(Some(""), Some(8)).stderr.is_empty());
}

#[test]
fn unknown_cores_mean_one_job_and_a_plea_to_report_it() {
    for cores in [None, Some(0)] {
        let said = jobs(None, cores);
        assert_eq!(said.jobs, 1);
        assert!(said.stdout.is_empty());
        assert_eq!(
            said.stderr,
            [
                "Can not determine how many core(s) are available, running in single-threaded mode.",
                "Please report an issue on GitHub to help us make nvm run faster on your computer!",
            ]
        );
    }
}

#[test]
fn clang_reports_its_version_in_one_of_two_places() {
    assert_eq!(
        clang_version("clang version 15.0.7 (Fedora 15.0.7-1.fc37)\nTarget: x86_64"),
        Some((15, 0))
    );
    assert_eq!(
        clang_version("Apple clang version 15.0.0 (clang-1500.1.0.2.5)\nTarget: arm64"),
        Some((15, 0))
    );
    assert_eq!(
        clang_version("clang version 3.5.2-1ubuntu1 (tags/RELEASE_352/final)"),
        Some((3, 5))
    );
    assert_eq!(clang_version("gcc (GCC) 13.2.0"), None);
    assert_eq!(clang_version(""), None);
}

#[test]
fn macos_and_the_bsds_name_the_compilers_and_linux_does_not() {
    let names = vec!["CC=cc".to_owned(), "CXX=c++".to_owned()];
    for os in [Os::Darwin, Os::FreeBsd, Os::OpenBsd] {
        assert_eq!(compiler(os, None, None, None).words, names);
    }
    assert!(compiler(Os::Linux, None, None, None).words.is_empty());
    assert!(compiler(Os::Aix, None, None, None).words.is_empty());
}

#[test]
fn the_compilers_in_the_environment_are_used_and_empty_ones_are_not() {
    let words = compiler(Os::Darwin, None, Some("gcc-13"), Some("")).words;
    assert_eq!(words, ["CC=gcc-13", "CXX=c++"]);
}

#[test]
fn clang_3_5_or_later_is_used_when_cc_or_cxx_is_missing() {
    let found = compiler(Os::Linux, Some((15, 0)), None, Some("g++"));
    assert!(found.clang_note);
    assert_eq!(found.words, ["CC=cc", "CXX=g++"]);
    assert!(!compiler(Os::Linux, Some((15, 0)), Some("gcc"), Some("g++")).clang_note);
    assert!(!compiler(Os::Linux, Some((3, 4)), None, None).clang_note);
    assert!(!compiler(Os::Linux, None, None, None).clang_note);
}

#[test]
fn the_bsds_and_aix_use_gmake_and_old_node_gets_a_shell() {
    assert_eq!(make(Os::Linux, &version("v20.10.0")), ("make", vec![]));
    assert_eq!(make(Os::FreeBsd, &version("v20.10.0")).0, "gmake");
    assert_eq!(make(Os::Aix, &version("v20.10.0")).0, "gmake");
    assert_eq!(
        make(Os::Darwin, &version("v0.10.48")),
        ("make", vec!["SHELL=/bin/sh".to_owned()])
    );
    assert!(make(Os::Linux, &version("v0.12.0")).1.is_empty());
}
```

Create `src/fakes/cpu.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_what_it_was_given() {
        assert_eq!(FakeCpu::with_cores(4).cores(), Some(4));
        assert_eq!(FakeCpu::default().cores(), None);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find module
`source_build`", "no variant named `FreeBsd` found" and "cannot find trait
`Cpu`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/adapters/no_cpu.rs`:

```rust
use crate::ports::Cpu;

/// The `Cpu` of a `Context` that was not given one: it knows nothing.
pub struct NoCpu;

impl Cpu for NoCpu {
    fn cores(&self) -> Option<usize> {
        None
    }
}

```

Insert above the `#[cfg(test)]` line of `src/adapters/std_cpu.rs`:

```rust
use crate::ports::Cpu;

/// The real [`Cpu`]: the parallelism the operating system grants this
/// process (which respects container limits, unlike `/proc/cpuinfo`).
pub struct StdCpu;

impl Cpu for StdCpu {
    fn cores(&self) -> Option<usize> {
        std::thread::available_parallelism().ok().map(usize::from)
    }
}

```

Apply to `src/context/mod.rs` (above the test module):

```diff
--- a/src/context/mod.rs
+++ b/src/context/mod.rs
@@ -4,6 +4,7 @@
 
 use crate::adapters::fs_alias_store::FsAliasStore;
 use crate::adapters::no_archive::NoArchive;
+use crate::adapters::no_cpu::NoCpu;
 use crate::adapters::no_digest::NoDigest;
 use crate::adapters::no_http::NoHttp;
 use crate::adapters::no_process::NoProcess;
@@ -12,7 +13,7 @@
 use crate::domain::platform::{Os, Platform};
 use crate::domain::version::Version;
 use crate::error::CliError;
-use crate::ports::{Archive, Digest, Env, FileSystem, Http, Process, Sleeper};
+use crate::ports::{Archive, Cpu, Digest, Env, FileSystem, Http, Process, Sleeper};
 
 pub struct Context<'a> {
     pub fs: &'a dyn FileSystem,
@@ -22,6 +23,7 @@
     digest: &'a dyn Digest,
     archive: &'a dyn Archive,
     sleeper: &'a dyn Sleeper,
+    cpu: &'a dyn Cpu,
     platform: Option<Platform>,
 }
 
@@ -38,6 +40,7 @@
             digest: &NoDigest,
             archive: &NoArchive,
             sleeper: &NoSleeper,
+            cpu: &NoCpu,
             platform: Some(Platform {
                 os: Os::Linux,
                 arch: "x64".to_owned(),
@@ -54,6 +57,17 @@
     #[must_use]
     pub fn process(&self) -> &dyn Process {
         self.process
+    }
+
+    #[must_use]
+    pub fn with_cpu(mut self, cpu: &'a dyn Cpu) -> Self {
+        self.cpu = cpu;
+        self
+    }
+
+    #[must_use]
+    pub fn cpu(&self) -> &dyn Cpu {
+        self.cpu
     }
 
     /// The machine the binaries are for; `None` when it has no official ones.
```

Apply to `src/domain/platform/mod.rs` (above the test module):

```diff
--- a/src/domain/platform/mod.rs
+++ b/src/domain/platform/mod.rs
@@ -8,6 +8,10 @@
     Linux,
     Darwin,
     Aix,
+    /// No official binaries: it is built from source.
+    FreeBsd,
+    /// No official binaries: it is built from source.
+    OpenBsd,
 }
 
 impl Os {
@@ -18,7 +22,15 @@
             Self::Linux => "linux",
             Self::Darwin => "darwin",
             Self::Aix => "aix",
+            Self::FreeBsd => "freebsd",
+            Self::OpenBsd => "openbsd",
         }
+    }
+
+    /// Whether nodejs.org publishes binaries for it.
+    #[must_use]
+    pub fn has_binaries(self) -> bool {
+        !matches!(self, Self::FreeBsd | Self::OpenBsd)
     }
 }
 
@@ -39,6 +51,8 @@
             "linux" => Os::Linux,
             "macos" => Os::Darwin,
             "aix" => Os::Aix,
+            "freebsd" => Os::FreeBsd,
+            "openbsd" => Os::OpenBsd,
             _ => return None,
         };
         let mut arch = match arch {
```

Insert above the `#[cfg(test)]` line of `src/domain/source_build/mod.rs`:

```rust
//! How a version is built from source: the number of `make` jobs
//! (`nvm_get_make_jobs`), the compiler words and the `make` program of
//! `nvm_install_source`, as pure rules.

use crate::domain::platform::Os;
use crate::domain::version::Version;

/// How many jobs `make` gets, and what is said about it on the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jobs {
    pub jobs: usize,
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
}

/// `nvm_is_natural_num`: digits, and not zero.
fn natural(text: &str) -> Option<usize> {
    let number: usize = text.parse().ok()?;
    (number > 0 && text.bytes().all(|byte| byte.is_ascii_digit())).then_some(number)
}

/// The jobs for a build: `requested` (from `-j`) when it is a natural number,
/// else one fewer than the cores when there are more than two.
#[must_use]
pub fn jobs(requested: Option<&str>, cores: Option<usize>) -> Jobs {
    let mut said = Jobs {
        jobs: 1,
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    if let Some(number) = requested.and_then(natural) {
        said.jobs = number;
        said.stdout.push(format!("number of `make` jobs: {number}"));
        return said;
    }
    if let Some(bad) = requested.filter(|text| !text.is_empty()) {
        said.stderr.push(format!(
            "{bad} is invalid for number of `make` jobs, must be a natural number"
        ));
    }
    let Some(cores) = cores.filter(|cores| *cores > 0) else {
        said.stderr.push(
            "Can not determine how many core(s) are available, running in single-threaded mode."
                .to_owned(),
        );
        said.stderr.push(
            "Please report an issue on GitHub to help us make nvm run faster on your computer!"
                .to_owned(),
        );
        return said;
    };
    said.stdout
        .push(format!("Detected that you have {cores} CPU core(s)"));
    if cores > 2 {
        said.jobs = cores - 1;
        said.stdout.push(format!(
            "Running with {} threads to speed up the build",
            said.jobs
        ));
    } else {
        said.stdout.push(
            "Number of CPU core(s) less than or equal to 2, running in single-threaded mode"
                .to_owned(),
        );
    }
    said
}

/// The version `clang --version` reports as `(major, minor)`: the word after
/// `version`, as `nvm_clang_version` reads it, without a `-suffix`.
#[must_use]
pub fn clang_version(output: &str) -> Option<(u64, u64)> {
    let line = output.lines().next()?;
    let words: Vec<&str> = line.split_whitespace().collect();
    let at = words.iter().position(|word| *word == "version")?;
    if at != 1 && at != 2 {
        return None;
    }
    let number = words.get(at + 1)?.split('-').next()?;
    let mut parts = number.split('.').map(|part| part.parse::<u64>().ok());
    Some((parts.next()??, parts.next().flatten().unwrap_or(0)))
}

/// What `nvm.sh` does about the compiler: the `CC=` and `CXX=` words that
/// `make` gets, and whether to say that it picked Clang.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Compiler {
    pub words: Vec<String>,
    pub clang_note: bool,
}

/// `cc` and `c++` are named on macOS and the BSDs, and when Clang 3.5 or later
/// is there and `CC` or `CXX` is not set.
#[must_use]
pub fn compiler(
    os: Os,
    clang: Option<(u64, u64)>,
    cc: Option<&str>,
    cxx: Option<&str>,
) -> Compiler {
    let cc = cc.filter(|text| !text.is_empty());
    let cxx = cxx.filter(|text| !text.is_empty());
    let words = |_: ()| {
        vec![
            format!("CC={}", cc.unwrap_or("cc")),
            format!("CXX={}", cxx.unwrap_or("c++")),
        ]
    };
    let clang_note =
        clang.is_some_and(|version| version >= (3, 5)) && (cc.is_none() || cxx.is_none());
    let named = matches!(os, Os::Darwin | Os::FreeBsd | Os::OpenBsd) || clang_note;
    Compiler {
        words: if named { words(()) } else { Vec::new() },
        clang_note,
    }
}

/// `make`, or `gmake` where `make` is not GNU, and for `node` before 0.12 the
/// word that stops a shell from expanding globs in its Makefiles.
#[must_use]
pub fn make(os: Os, version: &Version) -> (&'static str, Vec<String>) {
    let program = if matches!(os, Os::FreeBsd | Os::OpenBsd | Os::Aix) {
        "gmake"
    } else {
        "make"
    };
    let shell = (version.triple() < (0, 12, 0)).then(|| "SHELL=/bin/sh".to_owned());
    (program, shell.into_iter().collect())
}

```

Insert above the `#[cfg(test)]` line of `src/fakes/cpu.rs`:

```rust
use crate::ports::Cpu;

/// A machine with a fixed number of processors, or none known.
#[derive(Default)]
pub struct FakeCpu(Option<usize>);

impl FakeCpu {
    #[must_use]
    pub fn with_cores(cores: usize) -> Self {
        Self(Some(cores))
    }
}

impl Cpu for FakeCpu {
    fn cores(&self) -> Option<usize> {
        self.0
    }
}

```

Apply to `src/fakes/mod.rs` (above the test module):

```diff
--- a/src/fakes/mod.rs
+++ b/src/fakes/mod.rs
@@ -1,6 +1,7 @@
 //! In-memory implementations of the ports, for unit tests only.
 
 mod archive;
+mod cpu;
 mod digest;
 mod env;
 mod file_system;
@@ -9,6 +10,7 @@
 mod sleeper;
 
 pub use archive::FakeArchive;
+pub use cpu::FakeCpu;
 pub use digest::FakeDigest;
 pub use env::FakeEnv;
 pub use file_system::FakeFileSystem;
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -211,6 +211,11 @@
     fn extract(&self, archive: &Path, destination: &Path) -> io::Result<()>;
 }
 
+pub trait Cpu {
+    /// How many processors this machine has for a build to use, when known.
+    fn cores(&self) -> Option<usize>;
+}
+
 pub trait Sleeper {
     /// Waits for `duration` (between retries).
     fn sleep(&self, duration: Duration);
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 555 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(domain): decide jobs, compiler and make for a source build"
```

---

### Task 20: Building from source, `-s`, `-b`, `-j` and the install hook

**Files:**

- Create: `src/commands/install/acquire/mod.rs`,
  `src/commands/install/acquire/hook.rs`, `src/commands/install/source/mod.rs`,
  `src/commands/install/source/tests.rs`, `src/commands/install/tests/source.rs`
- Modify: `src/error.rs`, `src/ports/mod.rs`, `src/adapters/std_process.rs`,
  `src/fakes/process.rs`, `src/cli/mod.rs`, `src/commands/install/mod.rs`,
  `src/commands/install/flow.rs`, `src/commands/install/fetch/mod.rs`,
  `src/commands/install/fetch/tests.rs`, `src/commands/install/place/mod.rs`,
  `src/commands/install/options/mod.rs`, `src/commands/install/options/tests.rs`,
  `src/commands/install/npm_steps/mod.rs`,
  `src/commands/install/npm_steps/tests.rs`, `src/commands/install/tests/mod.rs`,
  `src/commands/install/tests/failures.rs`,
  `src/commands/install/tests/offline_and_save.rs`,
  `src/commands/install/tests/npm.rs`, `src/commands/npm/tests.rs`,
  `src/commands/npm/packages/tests.rs`, `tests/install_cli.rs`,
  `src/domain/source_build/mod.rs`

**Interfaces:**

- Produces: `NvmExitCode` is no longer a C-like enum: `NvmExitCode::Passed(u8)`
  carries the status of a program that ran and `NvmExitCode::passing_on(Option<
  i32>)` builds it, `NvmExitCode::HookClaimedSuccess` is 33; `Completed::code`;
  `fetch::Artifact::source_of` and `fetch::fetch(.., &Artifact, ..)`;
  `place::unpack`; `Options::{no_binary, no_source, make_jobs, extra}`;
  `commands::install::acquire::acquire`; `commands::install::source::{build,
  Build, BuildFailed}`; `Target::make_jobs`.
- Behaviour (verified against `nvm.sh` in twelve scenarios, with a fake
  `./configure`, a fake `make` that logs its arguments and the directory it ran
  in, and fake install hooks): the version is installed by, in order, the hook
  `NVM_INSTALL_THIRD_PARTY_HOOK` if it is set (called as `<hook> <version>
  <node|iojs> std <binary|source> <path>`, with the warning `** $NVM_INSTALL_
  THIRD_PARTY_HOOK env var set; dispatching to third-party installation method
  **`; its status is passed on when it fails, 33 when it succeeds and installs
  nothing); else the binary (unless `-s`); and when the binary cannot be had
  (`Binary download failed, trying source.`, or silently when there is none for
  this machine or version) the source, unless `-b` or `NVM_NO_SOURCE_FALLBACK=1`,
  which end with `Binary download failed. Download from source aborted.` (2) or
  `Binary download is not available for <v>` (3). `-s` with `-b` or with
  `NVM_NO_SOURCE_FALLBACK=1` is status 6. The source is `<mirror>/<v>/<node|
  iojs>-<v>.tar.gz` in `.cache/src/<name>/` (checksummed like a binary, and
  unpacked into `files/` with the top-level folder stripped); the build prints
  the jobs (Task 19), `Additional options while compiling: <options>` (the words
  after the version, each with a space in front, so two after the colon),
  `Clang v3.5+ detected! CC or CXX not specified, will use Clang as C/C++
  compiler!`, and `$>./configure --prefix=<path> <options><` (the `<` sticks to
  the last option), then runs `./configure --prefix=<path> <options>`, `make -j
  <jobs> [CC=.. CXX=..]`, removes the version path if it is a file and runs `make
  ... install`, all in `files/`; a failure is `nvm: install <v> failed!` (1) and
  removes `files/`. `NVM_MAKE_JOBS` is used silently when it is a natural number.
  `-j <n>` is read where it stands, so its message comes first. Unlike `nvm.sh`,
  `--alias` is applied after a source build and a hook install too.
- [ ] **Step 1: Write the failing tests**

Apply to `src/commands/install/fetch/tests.rs`:

```diff
--- a/src/commands/install/fetch/tests.rs
+++ b/src/commands/install/fetch/tests.rs
@@ -8,6 +8,10 @@
     "/home/me/.nvm/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz";
 const GOOD: &str = "aa11";
 
+fn artifact_of(context: &Context<'_>) -> Artifact {
+    Artifact::of(context, &version()).unwrap()
+}
+
 fn version() -> Version {
     "v20.10.0".parse().unwrap()
 }
@@ -31,7 +35,13 @@
     let env = env();
     let context = Context::new(fs, &env).with_http(http).with_digest(digest);
     let mut transcript = Transcript::default();
-    let result = fetch(&context, &version(), false, &mut transcript);
+    let result = fetch(
+        &context,
+        &artifact_of(&context),
+        &version(),
+        false,
+        &mut transcript,
+    );
     (result, transcript)
 }
 
@@ -167,7 +177,13 @@
         .with_digest(&digest);
     let mut transcript = Transcript::default();
     assert_eq!(
-        fetch(&context, &version(), false, &mut transcript),
+        fetch(
+            &context,
+            &artifact_of(&context),
+            &version(),
+            false,
+            &mut transcript
+        ),
         Err(Failed)
     );
     assert!(stderr(transcript)[0].contains("may only contain a URL"));
@@ -194,7 +210,13 @@
     let digest = FakeDigest::default();
     let context = Context::new(fs, &env).with_http(http).with_digest(&digest);
     let mut transcript = Transcript::default();
-    let result = fetch(&context, &version(), true, &mut transcript);
+    let result = fetch(
+        &context,
+        &artifact_of(&context),
+        &version(),
+        true,
+        &mut transcript,
+    );
     (result, transcript)
 }
 
@@ -223,3 +245,46 @@
         ["Offline: no cached archive found for node-v20.10.0-linux-x64"]
     );
 }
+
+#[test]
+fn a_source_archive_is_named_after_the_version_alone_and_lives_in_the_src_cache() {
+    let fs = FakeFileSystem::default();
+    let env = env();
+    let context = Context::new(&fs, &env);
+    let artifact = Artifact::source_of(&context, &version()).unwrap();
+    assert_eq!(artifact.slug, "node-v20.10.0");
+    assert_eq!(artifact.file_name, "node-v20.10.0.tar.gz");
+    assert_eq!(
+        artifact.tarball,
+        PathBuf::from("/home/me/.nvm/.cache/src/node-v20.10.0/node-v20.10.0.tar.gz")
+    );
+    let iojs = Artifact::source_of(&context, &"iojs-v3.3.1".parse().unwrap()).unwrap();
+    assert_eq!(iojs.slug, "iojs-v3.3.1");
+}
+
+#[test]
+fn a_source_archive_is_downloaded_and_checked_like_a_binary_one() {
+    let fs = FakeFileSystem::default();
+    let url = "http://127.0.0.1:1/v20.10.0/node-v20.10.0.tar.gz";
+    let sums_text = format!("{GOOD}  node-v20.10.0.tar.gz\n");
+    let http = FakeHttp::default()
+        .with_body(SUMS, &sums_text)
+        .with_bytes(url, b"source");
+    let tarball = "/home/me/.nvm/.cache/src/node-v20.10.0/node-v20.10.0.tar.gz";
+    let digest = FakeDigest::default().with_digest(tarball, GOOD);
+    let env = env();
+    let context = Context::new(&fs, &env)
+        .with_http(&http)
+        .with_digest(&digest);
+    let artifact = Artifact::source_of(&context, &version()).unwrap();
+    let mut transcript = Transcript::default();
+    let got = fetch(&context, &artifact, &version(), false, &mut transcript).unwrap();
+    assert_eq!(got, PathBuf::from(tarball));
+    assert_eq!(
+        stderr(transcript),
+        [
+            format!("Downloading {url}..."),
+            "Checksums matched!".to_owned()
+        ]
+    );
+}
```

Apply to `src/commands/install/npm_steps/tests.rs`:

```diff
--- a/src/commands/install/npm_steps/tests.rs
+++ b/src/commands/install/npm_steps/tests.rs
@@ -16,6 +16,7 @@
         version: version(),
         path: Path::new(PATH).to_path_buf(),
         source,
+        make_jobs: None,
     }
 }
 
@@ -110,6 +111,7 @@
         success: false,
         stdout: String::new(),
         stderr: "E404\n".to_owned(),
+        ..Completed::default()
     };
     let process = FakeProcess::default().with_execution(NPM, "install -g --quiet yarn", failed);
     let (status, _, stderr) = run_steps(&world(Some("yarn\n")), &process, &options(false, false));
```

Apply to `src/commands/install/options/tests.rs`:

```diff
--- a/src/commands/install/options/tests.rs
+++ b/src/commands/install/options/tests.rs
@@ -100,9 +100,10 @@
 }
 
 #[test]
-fn save_after_the_version_is_ignored_like_nvm_sh_does() {
+fn save_after_the_version_is_not_an_option_like_in_nvm_sh() {
     let options = parsed("20 --save -w").unwrap();
     assert!(!options.save);
+    assert_eq!(options.extra, ["--save", "-w"]);
     assert_eq!(options.version, "20");
 }
 
@@ -164,7 +165,7 @@
 
 #[test]
 fn harmless_options_are_accepted() {
-    let options = parsed("-b --no-progress 20").unwrap();
+    let options = parsed("--no-progress 20").unwrap();
     assert_eq!(options.version, "20");
 }
 
@@ -179,15 +180,35 @@
 }
 
 #[test]
-fn options_that_need_a_source_build_or_npm_are_not_supported_yet() {
-    for line in ["-s 20", "-j 4 20"] {
-        let error = parsed(line).unwrap_err();
-        assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption, "{line}");
-        assert!(
-            error.to_string().ends_with("is not supported yet."),
-            "{line}"
+fn s_b_and_j_choose_how_to_build() {
+    assert!(parsed("-s 20").unwrap().no_binary);
+    assert!(parsed("-b 20").unwrap().no_source);
+    let jobs = parsed("-j 4 20").unwrap();
+    assert_eq!(
+        (jobs.make_jobs.as_deref(), jobs.version.as_str()),
+        (Some("4"), "20")
+    );
+    assert_eq!(parsed("-j").unwrap().make_jobs.as_deref(), Some(""));
+}
+
+#[test]
+fn s_and_b_together_are_status_6() {
+    for line in ["-s -b 20", "-b -s 20"] {
+        let error = parsed(line).unwrap_err();
+        assert_eq!(error.exit_code(), NvmExitCode::InvalidOptions, "{line}");
+        assert_eq!(
+            error.to_string(),
+            "-s and -b cannot be set together since they would skip install from both binary and source"
         );
     }
+}
+
+#[test]
+fn the_words_after_the_version_that_no_option_takes_go_to_configure() {
+    let options = parsed("-s 20 --with-intl=full-icu --save --ninja").unwrap();
+    assert_eq!(options.extra, ["--with-intl=full-icu", "--save", "--ninja"]);
+    assert!(!options.save);
+    assert!(parsed("20").unwrap().extra.is_empty());
 }
 
 #[test]
```

Create `src/commands/install/source/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/install/source/tests.rs`:

```rust
use std::path::PathBuf;

use super::*;
use crate::domain::platform::Platform;
use crate::fakes::{FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeProcess};
use crate::ports::{Completed, FileSystem};

const SUMS: &str = "https://nodejs.org/dist/v20.10.0/SHASUMS256.txt";
const TARBALL_URL: &str = "https://nodejs.org/dist/v20.10.0/node-v20.10.0.tar.gz";
const TARBALL: &str = "/n/.cache/src/node-v20.10.0/node-v20.10.0.tar.gz";
const TOP: &str = "/n/.cache/src/node-v20.10.0/files";
const PREFIX: &str = "--prefix=/n/versions/node/v20.10.0";

struct World {
    fs: FakeFileSystem,
    http: FakeHttp,
    digest: FakeDigest,
    env: FakeEnv,
    process: FakeProcess,
}

impl World {
    fn new() -> Self {
        Self {
            fs: FakeFileSystem::default(),
            http: FakeHttp::default()
                .with_body(SUMS, "aa11  node-v20.10.0.tar.gz\n")
                .with_bytes(TARBALL_URL, b"source"),
            digest: FakeDigest::default().with_digest(TARBALL, "aa11"),
            env: FakeEnv::default().with_var("NVM_DIR", "/n"),
            process: FakeProcess::default()
                .with_success(&format!("{TOP}/configure"), PREFIX, "configured\n")
                .with_success("make", "-j 7", "built\n")
                .with_success("make", "-j 7 install", "installed\n"),
        }
    }

    fn build(
        &self,
        extra: &[&str],
        platform: Option<Platform>,
    ) -> (Result<(), BuildFailed>, String, String) {
        let archive = FakeArchive::new(&self.fs)
            .with_archive(TARBALL, &[("node-v20.10.0/configure", "#!/bin/sh", true)]);
        let context = Context::new(&self.fs, &self.env)
            .with_http(&self.http)
            .with_digest(&self.digest)
            .with_archive(&archive)
            .with_process(&self.process)
            .with_platform(platform);
        let version: Version = "v20.10.0".parse().unwrap();
        let path = PathBuf::from("/n/versions/node/v20.10.0");
        let extra: Vec<String> = extra.iter().map(|word| (*word).to_owned()).collect();
        let job = Build {
            version: &version,
            version_path: &path,
            jobs: 7,
            extra: &extra,
            offline: false,
        };
        let mut transcript = Transcript::default();
        let result = build(&context, &job, &mut transcript);
        let output = transcript.finish(crate::error::NvmExitCode::Success);
        (result, output.stdout, output.stderr)
    }

    fn ran(&self) -> Vec<String> {
        self.process
            .executed()
            .iter()
            .map(|i| format!("{} {}", i.program.display(), i.args.join(" ")))
            .collect()
    }
}

fn linux() -> Option<Platform> {
    Platform::from_host("linux", "x86_64", false)
}

/// The expectations are what `nvm install -s` of the real `nvm.sh` printed
/// and ran, with a fake `./configure` and `make`.
#[test]
fn it_downloads_unpacks_configures_makes_and_installs() {
    let world = World::new();
    let (result, stdout, stderr) = world.build(&[], linux());
    assert_eq!(result, Ok(()));
    assert_eq!(
        stdout,
        format!("$>./configure {PREFIX} <\nconfigured\nbuilt\ninstalled")
    );
    assert_eq!(
        stderr,
        format!("Downloading {TARBALL_URL}...\nChecksums matched!")
    );
    assert_eq!(
        world.ran(),
        [
            format!("{TOP}/configure {PREFIX}"),
            "make -j 7".to_owned(),
            "make -j 7 install".to_owned()
        ]
    );
    let first = &world.process.executed()[0];
    assert_eq!(first.dir.as_deref(), Some(std::path::Path::new(TOP)));
}

#[test]
fn the_words_after_the_version_go_to_configure_and_are_announced_with_a_leading_space() {
    let world = World {
        process: FakeProcess::default()
            .with_success(
                &format!("{TOP}/configure"),
                &format!("{PREFIX} --with-intl=full-icu --ninja"),
                "",
            )
            .with_success("make", "-j 7", "")
            .with_success("make", "-j 7 install", ""),
        ..World::new()
    };
    let (result, stdout, _) = world.build(&["--with-intl=full-icu", "--ninja"], linux());
    assert_eq!(result, Ok(()));
    assert!(stdout.starts_with(
        "Additional options while compiling:  --with-intl=full-icu --ninja\n$>./configure --prefix=/n/versions/node/v20.10.0 --with-intl=full-icu --ninja<"
    ));
}

#[test]
fn a_32_bit_arm_gets_without_snapshot() {
    let arm = Platform::from_host("linux", "arm", false);
    let world = World {
        process: FakeProcess::default()
            .with_success(
                &format!("{TOP}/configure"),
                &format!("{PREFIX} --without-snapshot"),
                "",
            )
            .with_success("make", "-j 7", "")
            .with_success("make", "-j 7 install", ""),
        ..World::new()
    };
    let (_, stdout, _) = world.build(&[], arm.clone());
    assert!(stdout.starts_with("Additional options while compiling: --without-snapshot\n"));
    let world = World {
        process: FakeProcess::default().with_success(
            &format!("{TOP}/configure"),
            &format!("{PREFIX} --without-snapshot --a"),
            "",
        ),
        ..World::new()
    };
    let (_, stdout, _) = world.build(&["--a"], arm);
    assert!(stdout.starts_with("Additional options while compiling: --without-snapshot  --a\n"));
}

#[test]
fn macos_names_the_compilers_and_clang_is_announced() {
    let mac = Platform::from_host("macos", "aarch64", false);
    let fs = FakeFileSystem::default()
        .with_executable("/usr/bin/clang", "")
        .with_executable("/usr/bin/clang++", "");
    let process = FakeProcess::default()
        .with_output(
            "/usr/bin/clang",
            "Apple clang version 15.0.0 (clang-1500.1.0.2.5)\n",
        )
        .with_success(&format!("{TOP}/configure"), PREFIX, "")
        .with_success("make", "-j 7 CC=cc CXX=c++", "")
        .with_success("make", "-j 7 CC=cc CXX=c++ install", "");
    let world = World {
        fs,
        env: FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", "/usr/bin"),
        process,
        ..World::new()
    };
    let (result, stdout, _) = world.build(&[], mac);
    assert_eq!(result, Ok(()));
    assert!(stdout.starts_with("Clang v3.5+ detected! CC or CXX not specified, will use Clang as C/C++ compiler!\n$>./configure"));
    assert!(
        world
            .ran()
            .contains(&"make -j 7 CC=cc CXX=c++ install".to_owned())
    );
}

#[test]
fn a_failing_configure_is_reported_and_the_unpacked_tree_is_removed() {
    let failed = Completed {
        success: false,
        stdout: String::new(),
        stderr: "no compiler\n".to_owned(),
        ..Completed::default()
    };
    let world = World {
        process: FakeProcess::default().with_execution(&format!("{TOP}/configure"), PREFIX, failed),
        ..World::new()
    };
    let (result, stdout, stderr) = world.build(&[], linux());
    assert_eq!(result, Err(BuildFailed));
    assert_eq!(stdout, format!("$>./configure {PREFIX} <"));
    assert!(stderr.ends_with("no compiler\nnvm: install v20.10.0 failed!"));
    assert!(
        world
            .fs
            .file_info(std::path::Path::new("/n/.cache/src/node-v20.10.0/files"))
            .is_err()
    );
    assert_eq!(world.ran().len(), 1);
}

#[test]
fn a_failing_make_install_is_a_failure_too() {
    let failed = Completed::default();
    let world = World {
        process: FakeProcess::default()
            .with_success(&format!("{TOP}/configure"), PREFIX, "")
            .with_success("make", "-j 7", "")
            .with_execution("make", "-j 7 install", failed),
        ..World::new()
    };
    let (result, _, stderr) = world.build(&[], linux());
    assert_eq!(result, Err(BuildFailed));
    assert!(stderr.ends_with("nvm: install v20.10.0 failed!"));
}

#[test]
fn a_failed_download_fails_without_saying_the_install_failed() {
    let world = World {
        http: FakeHttp::default().with_status(TARBALL_URL, 404),
        ..World::new()
    };
    let (result, _, stderr) = world.build(&[], linux());
    assert_eq!(result, Err(BuildFailed));
    assert!(stderr.ends_with(&format!("download from {TARBALL_URL} failed")));
    assert!(world.ran().is_empty());
}

#[test]
fn old_node_gets_a_shell_for_make_and_the_bsds_use_gmake() {
    let freebsd = Platform::from_host("freebsd", "x86_64", false);
    let world = World {
        process: FakeProcess::default()
            .with_success(&format!("{TOP}/configure"), PREFIX, "")
            .with_success("gmake", "-j 7 CC=cc CXX=c++", "")
            .with_success("gmake", "-j 7 CC=cc CXX=c++ install", ""),
        ..World::new()
    };
    let (result, _, _) = world.build(&[], freebsd);
    assert_eq!(result, Ok(()));
}
```

Apply to `src/commands/install/tests/failures.rs`:

```diff
--- a/src/commands/install/tests/failures.rs
+++ b/src/commands/install/tests/failures.rs
@@ -57,7 +57,7 @@
         .with_body(NODE_INDEX, &index_text(&[("v20.10.0", "Iron")]))
         .with_body(IOJS_INDEX, &index_text(&[]))
         .with_status(TARBALL_URL, 404);
-    let output = world.run("20").unwrap();
+    let output = world.run("-b 20").unwrap();
     assert_eq!(output.status, NvmExitCode::MissingTarget);
     assert!(
         output
@@ -71,7 +71,7 @@
 fn a_wrong_checksum_is_status_2_and_installs_nothing() {
     let mut world = World::new();
     world.digest = FakeDigest::default().with_digest(TARBALL, "bad0");
-    let output = world.run("20").unwrap();
+    let output = world.run("-b 20").unwrap();
     assert_eq!(output.status, NvmExitCode::MissingTarget);
     assert!(
         output
@@ -89,7 +89,7 @@
         .with_http(&world.http)
         .with_archive(&archive)
         .with_platform(None);
-    let output = super::run(&context, &["20".to_owned()]).unwrap();
+    let output = super::run(&context, &["-b".to_owned(), "20".to_owned()]).unwrap();
     assert_eq!(output.status, NvmExitCode::InvalidVersion);
     assert_eq!(
         output.stderr,
```

Apply to `src/commands/install/tests/mod.rs`:

```diff
--- a/src/commands/install/tests/mod.rs
+++ b/src/commands/install/tests/mod.rs
@@ -77,6 +77,7 @@
 mod failures;
 mod npm;
 mod offline_and_save;
+mod source;
 
 #[test]
 fn a_fresh_install_downloads_unpacks_and_makes_the_default_alias() {
```

Apply to `src/commands/install/tests/offline_and_save.rs`:

```diff
--- a/src/commands/install/tests/offline_and_save.rs
+++ b/src/commands/install/tests/offline_and_save.rs
@@ -70,7 +70,7 @@
 #[test]
 fn offline_with_a_missing_archive_fails_like_a_failed_download() {
     let world = offline_world(FakeFileSystem::default().with_dir("/n/.cache/src/node-v20.10.0"));
-    let output = world.run("--offline 20").unwrap();
+    let output = world.run("--offline -b 20").unwrap();
     assert_eq!(output.status, NvmExitCode::MissingTarget);
     assert!(
         output
```

Create `src/commands/install/tests/source.rs`:

```rust
use std::rc::Rc;

use super::*;
use crate::fakes::FakeCpu;

const SRC_URL: &str = "https://nodejs.org/dist/v20.10.0/node-v20.10.0.tar.gz";
const SRC_TARBALL: &str = "/n/.cache/src/node-v20.10.0/node-v20.10.0.tar.gz";
const TOP: &str = "/n/.cache/src/node-v20.10.0/files";
const PREFIX: &str = "--prefix=/n/versions/node/v20.10.0";

/// A world where the binary cannot be had and the source can.
struct Built {
    fs: Rc<FakeFileSystem>,
    http: FakeHttp,
    digest: FakeDigest,
    env: FakeEnv,
    cpu: FakeCpu,
}

impl Built {
    fn new() -> Self {
        let node = index_text(&[("v20.10.0", "Iron")]);
        let sums =
            format!("{GOOD}  node-v20.10.0-linux-x64.tar.gz\n{GOOD}  node-v20.10.0.tar.gz\n");
        Self {
            fs: Rc::new(FakeFileSystem::default()),
            http: FakeHttp::default()
                .with_body(NODE_INDEX, &node)
                .with_body(IOJS_INDEX, &index_text(&[]))
                .with_body(SUMS, &sums)
                .with_status(TARBALL_URL, 404)
                .with_bytes(SRC_URL, b"source"),
            digest: FakeDigest::default().with_digest(SRC_TARBALL, GOOD),
            env: FakeEnv::default().with_var("NVM_DIR", "/n"),
            cpu: FakeCpu::with_cores(8),
        }
    }

    fn with_env(self, name: &str, value: &str) -> Self {
        let mut vars = vec![("NVM_DIR", "/n")];
        vars.push((name, value));
        let env = vars
            .iter()
            .fold(FakeEnv::default(), |env, (k, v)| env.with_var(k, v));
        Self { env, ..self }
    }

    fn run(&self, line: &str) -> Output {
        let fs = Rc::clone(&self.fs);
        let make_node = move || {
            fs.write_file(Path::new(NODE), "binary").unwrap();
            fs.set_executable(Path::new(NODE));
        };
        let process = FakeProcess::default()
            .with_success(&format!("{TOP}/configure"), PREFIX, "configured\n")
            .with_success("make", "-j 7", "built\n")
            .with_success("make", "-j 7 install", "installed\n")
            .with_effect("make", "-j 7 install", make_node)
            .with_success("make", "-j 4", "")
            .with_success("make", "-j 4 install", "")
            .with_effect("make", "-j 4 install", {
                let fs = Rc::clone(&self.fs);
                move || {
                    fs.write_file(Path::new(NODE), "binary").unwrap();
                    fs.set_executable(Path::new(NODE));
                }
            })
            .with_success(
                &format!("{TOP}/configure"),
                &format!("{PREFIX} --with-intl=full-icu"),
                "",
            );
        let archive = FakeArchive::new(&self.fs).with_archive(
            SRC_TARBALL,
            &[("node-v20.10.0/configure", "#!/bin/sh", true)],
        );
        let context = Context::new(self.fs.as_ref(), &self.env)
            .with_http(&self.http)
            .with_digest(&self.digest)
            .with_archive(&archive)
            .with_process(&process)
            .with_cpu(&self.cpu);
        let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        super::super::run(&context, &words).unwrap()
    }
}

/// The expectations are what `nvm install` of the real `nvm.sh` printed with
/// a mirror that has no binary, and a fake `./configure` and `make`.
#[test]
fn a_failed_binary_falls_back_to_building_from_source() {
    let world = Built::new();
    let output = world.run("20");
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        lines(&output.stdout),
        [
            "Downloading and installing node v20.10.0...",
            "Detected that you have 8 CPU core(s)",
            "Running with 7 threads to speed up the build",
            "$>./configure --prefix=/n/versions/node/v20.10.0 <",
            "configured",
            "built",
            "installed",
            "Creating default alias: default -> 20 (-> v20.10.0 *)",
        ]
    );
    assert!(
        output
            .stderr
            .contains("Binary download failed, trying source.")
    );
    assert!(output.stderr.contains(&format!("Downloading {SRC_URL}...")));
    assert!(world.fs.file_info(Path::new(NODE)).is_ok());
}

#[test]
fn s_goes_straight_to_source_and_skips_the_binary() {
    let world = Built::new();
    let output = world.run("-s 20");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(!output.stdout.contains("Downloading and installing"));
    assert!(!world.http.requests().iter().any(|url| url == TARBALL_URL));
}

#[test]
fn b_ends_a_failed_binary_instead_of_building() {
    let world = Built::new();
    let output = world.run("-b 20");
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    assert!(
        output
            .stderr
            .ends_with("Binary download failed. Download from source aborted.")
    );
    assert!(!world.http.requests().iter().any(|url| url == SRC_URL));
}

#[test]
fn nvm_no_source_fallback_is_b_for_every_install() {
    let world = Built::new().with_env("NVM_NO_SOURCE_FALLBACK", "1");
    let output = world.run("20");
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    let value_that_is_not_one = Built::new()
        .with_env("NVM_NO_SOURCE_FALLBACK", "yes")
        .run("20");
    assert_eq!(value_that_is_not_one.status, NvmExitCode::Success);
}

#[test]
fn s_with_nvm_no_source_fallback_is_status_6() {
    let world = Built::new().with_env("NVM_NO_SOURCE_FALLBACK", "1");
    let fs = Rc::clone(&world.fs);
    let context = Context::new(&*fs, &world.env);
    let error = super::super::run(&context, &["-s".to_owned(), "20".to_owned()]).unwrap_err();
    assert_eq!(error.exit_code(), NvmExitCode::InvalidOptions);
    assert_eq!(
        error.to_string(),
        "-s cannot be combined with NVM_NO_SOURCE_FALLBACK=1 since that would skip install from both binary and source"
    );
}

#[test]
fn b_on_a_machine_without_binaries_is_status_3_with_the_binary_message() {
    let mut world = Built::new();
    world.http = FakeHttp::default()
        .with_body(NODE_INDEX, &index_text(&[("v20.10.0", "Iron")]))
        .with_body(IOJS_INDEX, &index_text(&[]))
        .with_body(SUMS, &format!("{GOOD}  node-v20.10.0.tar.gz\n"))
        .with_bytes(SRC_URL, b"source");
    let no_binary = Rc::clone(&world.fs);
    let context = Context::new(&*no_binary, &world.env)
        .with_platform(None)
        .with_http(&world.http);
    let words = ["-b".to_owned(), "20".to_owned()];
    let output = super::super::run(&context, &words).unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stderr,
        "Binary download is not available for v20.10.0"
    );
}

#[test]
fn j_sets_the_jobs_and_says_so_first() {
    let world = Built::new();
    let output = world.run("-j 4 -s 20");
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        output.stdout.lines().next(),
        Some("number of `make` jobs: 4")
    );
    assert!(!output.stdout.contains("Detected that you have"));
}

#[test]
fn j_that_is_not_natural_is_reported_and_the_cores_decide() {
    let world = Built::new();
    let output = world.run("-j abc -s 20");
    assert!(
        output
            .stderr
            .starts_with("abc is invalid for number of `make` jobs, must be a natural number")
    );
    assert!(
        output
            .stdout
            .contains("Running with 7 threads to speed up the build")
    );
}

#[test]
fn nvm_make_jobs_is_used_silently_when_it_is_a_natural_number() {
    let world = Built::new().with_env("NVM_MAKE_JOBS", "4");
    let output = world.run("-s 20");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(!output.stdout.contains("Detected that you have"));
    let ignored = Built::new().with_env("NVM_MAKE_JOBS", "many").run("-s 20");
    assert!(
        ignored
            .stdout
            .contains("Detected that you have 8 CPU core(s)")
    );
}

#[test]
fn the_words_after_the_version_are_passed_to_configure() {
    let world = Built::new();
    let output = world.run("-s 20 --with-intl=full-icu");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(
        output
            .stdout
            .contains("Additional options while compiling:  --with-intl=full-icu")
    );
}

#[test]
fn a_failed_build_is_status_1_and_installs_nothing() {
    let world = Built::new();
    let broken = World::new();
    let _ = broken;
    let process = FakeProcess::default();
    let archive = FakeArchive::new(&world.fs).with_archive(
        SRC_TARBALL,
        &[("node-v20.10.0/configure", "#!/bin/sh", true)],
    );
    let context = Context::new(world.fs.as_ref(), &world.env)
        .with_http(&world.http)
        .with_digest(&world.digest)
        .with_archive(&archive)
        .with_process(&process)
        .with_cpu(&world.cpu);
    let output = super::super::run(&context, &["-s".to_owned(), "20".to_owned()]).unwrap();
    assert_eq!(output.status, NvmExitCode::Failure);
    assert!(output.stderr.ends_with("nvm: install v20.10.0 failed!"));
    assert!(world.fs.file_info(Path::new(NODE)).is_err());
}

#[test]
fn a_hook_installs_in_place_of_nvm_and_is_told_how() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let fs = Rc::clone(&world.fs);
    let process = FakeProcess::default()
        .with_success(
            "/opt/hook",
            "v20.10.0 node std binary /n/versions/node/v20.10.0",
            "",
        )
        .with_effect(
            "/opt/hook",
            "v20.10.0 node std binary /n/versions/node/v20.10.0",
            move || {
                fs.write_file(Path::new(NODE), "binary").unwrap();
                fs.set_executable(Path::new(NODE));
            },
        );
    let context = Context::new(world.fs.as_ref(), &world.env)
        .with_http(&world.http)
        .with_process(&process);
    let output = super::super::run(&context, &["20".to_owned()]).unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        output.stderr,
        "** $NVM_INSTALL_THIRD_PARTY_HOOK env var set; dispatching to third-party installation method **"
    );
    assert!(
        !world
            .http
            .requests()
            .iter()
            .any(|url| url.contains(".tar.gz"))
    );
}

#[test]
fn a_hook_that_fails_or_installs_nothing_ends_the_install() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let run_with = |process: &FakeProcess| {
        let context = Context::new(world.fs.as_ref(), &world.env)
            .with_http(&world.http)
            .with_process(process);
        super::super::run(&context, &["20".to_owned()]).unwrap()
    };
    let missing = run_with(&FakeProcess::default());
    assert_eq!(missing.status, NvmExitCode::Failure);
    assert!(
        missing.stderr.ends_with(
            "*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var failed to install! ***"
        )
    );
    let silent = FakeProcess::default().with_success(
        "/opt/hook",
        "v20.10.0 node std binary /n/versions/node/v20.10.0",
        "",
    );
    let claimed = run_with(&silent);
    assert_eq!(claimed.status, NvmExitCode::HookClaimedSuccess);
    assert!(claimed.stderr.ends_with("*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var claimed to succeed, but failed to install! ***"));
}

#[test]
fn the_hook_is_told_source_when_s_is_given() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let process = FakeProcess::default();
    let context = Context::new(world.fs.as_ref(), &world.env)
        .with_http(&world.http)
        .with_process(&process);
    super::super::run(&context, &["-s".to_owned(), "20".to_owned()]).unwrap();
    let ran = process.executed();
    assert_eq!(
        ran[0].args,
        [
            "v20.10.0",
            "node",
            "std",
            "source",
            "/n/versions/node/v20.10.0"
        ]
    );
}

#[test]
fn a_hook_that_fails_passes_its_own_status_on() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let failed = crate::ports::Completed {
        code: Some(7),
        ..crate::ports::Completed::default()
    };
    let process = FakeProcess::default().with_execution(
        "/opt/hook",
        "v20.10.0 node std binary /n/versions/node/v20.10.0",
        failed,
    );
    let context = Context::new(world.fs.as_ref(), &world.env)
        .with_http(&world.http)
        .with_process(&process);
    let output = super::super::run(&context, &["20".to_owned()]).unwrap();
    assert_eq!(output.status, NvmExitCode::Passed(7));
    assert_eq!(output.status.code(), 7);
}
```

Apply to `src/commands/npm/packages/tests.rs`:

```diff
--- a/src/commands/npm/packages/tests.rs
+++ b/src/commands/npm/packages/tests.rs
@@ -113,6 +113,7 @@
         success: false,
         stdout: String::new(),
         stderr: String::new(),
+        ..Completed::default()
     };
     let process = FakeProcess::default()
         .with_success(
```

Apply to `src/commands/npm/tests.rs`:

```diff
--- a/src/commands/npm/tests.rs
+++ b/src/commands/npm/tests.rs
@@ -67,6 +67,7 @@
         success: false,
         stdout: "out1\nout2\n".to_owned(),
         stderr: "oops\n".to_owned(),
+        ..Completed::default()
     };
     let process = FakeProcess::default().with_execution(NPM, "install -g left-pad", done);
     let context = context(&fs, &env, &process);
@@ -107,6 +108,7 @@
         success: false,
         stdout: "tree\n".to_owned(),
         stderr: String::new(),
+        ..Completed::default()
     };
     let process = FakeProcess::default().with_execution(NPM, "list -g --depth=0", done);
     let context = context(&fs, &env, &process);
```

Apply to the test module of `src/error.rs`:

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -12,8 +12,19 @@
         assert_eq!(NvmExitCode::SameVersion.code(), 4);
         assert_eq!(NvmExitCode::SourceNotInstalled.code(), 5);
         assert_eq!(NvmExitCode::InvalidOptions.code(), 6);
+        assert_eq!(NvmExitCode::HookClaimedSuccess.code(), 33);
+        assert_eq!(NvmExitCode::Passed(7).code(), 7);
         assert_eq!(NvmExitCode::UnsupportedOption.code(), 55);
         assert_eq!(NvmExitCode::NotFound.code(), 127);
+    }
+
+    #[test]
+    fn a_program_that_ran_passes_its_status_on() {
+        assert_eq!(NvmExitCode::passing_on(Some(7)), NvmExitCode::Passed(7));
+        assert_eq!(NvmExitCode::passing_on(Some(0)), NvmExitCode::Success);
+        assert_eq!(NvmExitCode::passing_on(None), NvmExitCode::Failure);
+        assert_eq!(NvmExitCode::passing_on(Some(-1)), NvmExitCode::Failure);
+        assert_eq!(NvmExitCode::passing_on(Some(300)), NvmExitCode::Failure);
     }
 
     #[test]
```

Apply to the test module of `src/fakes/process.rs`:

```diff
--- a/src/fakes/process.rs
+++ b/src/fakes/process.rs
@@ -15,6 +15,21 @@
     }
 
     #[test]
+    fn fake_process_runs_the_effect_of_an_invocation() {
+        use std::cell::Cell;
+        use std::rc::Rc;
+        let ran = Rc::new(Cell::new(0));
+        let seen = Rc::clone(&ran);
+        let process = FakeProcess::default()
+            .with_success("make", "install", "")
+            .with_effect("make", "install", move || seen.set(seen.get() + 1));
+        process
+            .execute(&Invocation::new("make").args(&["install"]))
+            .unwrap();
+        assert_eq!(ran.get(), 1);
+    }
+
+    #[test]
     fn fake_process_answers_by_program_path() {
         let process = FakeProcess::default()
             .with_output("/usr/bin/node", "v22.1.0\n")
```

Apply to `tests/install_cli.rs`:

```diff
--- a/tests/install_cli.rs
+++ b/tests/install_cli.rs
@@ -200,7 +200,7 @@
         return;
     };
     let home = tempfile::tempdir().unwrap();
-    let output = nvm(home.path(), &mirror, &["install", "20"]);
+    let output = nvm(home.path(), &mirror, &["install", "-b", "20"]);
     assert_eq!(output.status.code(), Some(2));
     assert!(stderr(&output).contains("Checksums do not match:"));
     assert!(stderr(&output).ends_with("Binary download failed. Download from source aborted.\n"));
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "no field `no_binary` on type
`Options`", "cannot find module `source`" and "no variant named
`HookClaimedSuccess` found".

- [ ] **Step 3: Write the implementation**

Apply to `src/adapters/std_process.rs` (above the test module):

```diff
--- a/src/adapters/std_process.rs
+++ b/src/adapters/std_process.rs
@@ -52,6 +52,7 @@
         let status = child.wait()?;
         Ok(Completed {
             success: status.success(),
+            code: status.code(),
             stdout: stdout.join().unwrap_or_default(),
             stderr: stderr.join().unwrap_or_default(),
         })
```

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -8,6 +8,7 @@
 
 use crate::adapters::retrying_http::RetryingHttp;
 use crate::adapters::sha256_digest::Sha256Digest;
+use crate::adapters::std_cpu::StdCpu;
 use crate::adapters::std_env::StdEnv;
 use crate::adapters::std_fs::StdFileSystem;
 use crate::adapters::std_process::StdProcess;
@@ -208,6 +209,7 @@
         .with_digest(&Sha256Digest)
         .with_archive(&TarGzArchive)
         .with_sleeper(&StdSleeper)
+        .with_cpu(&StdCpu)
         .with_platform(platform);
     run(
         std::env::args_os(),
```

Create `src/commands/install/acquire/hook.rs`:

```rust
//! `NVM_INSTALL_THIRD_PARTY_HOOK`: a program that installs the version in
//! place of `nvm`, called as `<hook> <version> <flavor> std <method> <path>`.

use crate::commands::install::flow::{Halt, Step, Target};
use crate::commands::install::options::Options;
use crate::commands::install::place::is_valid_install;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::version::Flavor;
use crate::error::NvmExitCode;
use crate::ports::Invocation;

/// Runs the hook, and checks that it left a version behind.
///
/// # Errors
/// [`Halt`]: the hook's own status when it fails (1 when it could not run), 33
/// when it succeeds and installs nothing.
pub fn run(
    context: &Context<'_>,
    program: &str,
    options: &Options,
    target: &Target,
    transcript: &mut Transcript,
) -> Step<()> {
    transcript.err("** $NVM_INSTALL_THIRD_PARTY_HOOK env var set; dispatching to third-party installation method **");
    let flavor = match target.version.flavor {
        Flavor::Node => "node",
        Flavor::IoJs => "iojs",
    };
    let method = if options.no_binary {
        "source"
    } else {
        "binary"
    };
    let version = target.version.to_string();
    let path = target.path.display().to_string();
    let invocation = Invocation::new(program).args(&[&version, flavor, "std", method, &path]);
    let done = context.process().execute(&invocation);
    if let Ok(done) = &done {
        done.stdout.lines().for_each(|line| transcript.out(line));
        done.stderr.lines().for_each(|line| transcript.err(line));
    }
    if !done.as_ref().is_ok_and(|done| done.success) {
        transcript
            .err("*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var failed to install! ***");
        let code = done.ok().and_then(|done| done.code);
        return Err(Halt::Exit(NvmExitCode::passing_on(code)));
    }
    if !is_valid_install(context, &target.path) {
        transcript.err("*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var claimed to succeed, but failed to install! ***");
        return Err(Halt::Exit(NvmExitCode::HookClaimedSuccess));
    }
    Ok(())
}
```

Create `src/commands/install/acquire/mod.rs`:

```rust
//! Getting a version onto the disk: the third-party hook if there is one,
//! else the prebuilt binary, and from source when the binary cannot be had
//! and `-b` (or `NVM_NO_SOURCE_FALLBACK=1`) did not forbid it.

mod hook;

use std::time::SystemTime;

use crate::commands::install::fetch::{self, Artifact};
use crate::commands::install::flow::{Halt, Step, Target};
use crate::commands::install::lock::{InstallLock, LockRequest, acquire as take};
use crate::commands::install::options::Options;
use crate::commands::install::place;
use crate::commands::install::source::{Build, build};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::platform::binary_available;
use crate::domain::source_build::jobs as plan_jobs;
use crate::domain::version::{Flavor, Version};
use crate::error::NvmExitCode;

const DEFAULT_LOCK_TIMEOUT_SECONDS: u64 = 600;

/// What became of the attempt to install the binary.
enum Binary {
    Installed,
    /// There is no binary for this version on this machine.
    Unavailable,
    Failed,
}

/// Everything from the lock to a version directory that is in place.
///
/// # Errors
/// [`Halt`] with the status the install ends with, after the transcript says
/// why.
pub fn acquire(
    context: &Context<'_>,
    options: &Options,
    target: &Target,
    transcript: &mut Transcript,
) -> Step<()> {
    let version = &target.version;
    if version.flavor == Flavor::Node && version.triple() < (0, 12, 0) {
        transcript.err(format!(
            "Versions before v0.12.0 use the legacy layout, which is not supported: {version}"
        ));
        return Err(Halt::Exit(NvmExitCode::InvalidVersion));
    }
    if let Some(program) = context
        .env
        .var("NVM_INSTALL_THIRD_PARTY_HOOK")
        .filter(|hook| !hook.is_empty())
    {
        return hook::run(context, &program, options, target, transcript);
    }
    let _lock = take_lock(context, version, transcript)?;
    let binary = if options.no_binary {
        None
    } else {
        Some(install_binary(context, target, options.offline, transcript))
    };
    if matches!(binary, Some(Binary::Installed)) {
        return Ok(());
    }
    if options.no_source {
        return Err(without_source(binary, version, transcript));
    }
    if matches!(binary, Some(Binary::Failed)) {
        transcript.err("Binary download failed, trying source.");
    }
    from_source(context, options, target, transcript)
}

/// `-b`: the end of the road, with what `nvm.sh` says about the binary.
fn without_source(binary: Option<Binary>, version: &Version, transcript: &mut Transcript) -> Halt {
    if matches!(binary, Some(Binary::Failed)) {
        transcript.err("Binary download failed. Download from source aborted.");
        return Halt::Exit(NvmExitCode::MissingTarget);
    }
    transcript.err(format!("Binary download is not available for {version}"));
    Halt::Exit(NvmExitCode::InvalidVersion)
}

fn install_binary(
    context: &Context<'_>,
    target: &Target,
    offline: bool,
    transcript: &mut Transcript,
) -> Binary {
    let version = &target.version;
    let has_binaries = context
        .platform()
        .is_some_and(|platform| platform.os.has_binaries());
    let Some(artifact) =
        Artifact::of(context, version).filter(|_| has_binaries && binary_available(version))
    else {
        return Binary::Unavailable;
    };
    let name = match version.flavor {
        Flavor::Node => "node",
        Flavor::IoJs => "io.js",
    };
    transcript.out(format!(
        "Downloading and installing {name} {}...",
        version.directory_name()
    ));
    let Ok(tarball) = fetch::fetch(context, &artifact, version, offline, transcript) else {
        return Binary::Failed;
    };
    match place::place(context, &tarball, &artifact.files(), &target.path) {
        Ok(()) => Binary::Installed,
        Err(message) => {
            transcript.err(message);
            Binary::Failed
        }
    }
}

fn from_source(
    context: &Context<'_>,
    options: &Options,
    target: &Target,
    transcript: &mut Transcript,
) -> Step<()> {
    let jobs = make_jobs(context, target.make_jobs, transcript);
    let job = Build {
        version: &target.version,
        version_path: &target.path,
        jobs,
        extra: &options.extra,
        offline: options.offline,
    };
    build(context, &job, transcript).map_err(|_| Halt::Exit(NvmExitCode::Failure))
}

/// `-j`, or `NVM_MAKE_JOBS` when it is a natural number, or one fewer than
/// the cores, with the messages of `nvm_get_make_jobs` for the last.
fn make_jobs(
    context: &Context<'_>,
    requested: Option<usize>,
    transcript: &mut Transcript,
) -> usize {
    if let Some(jobs) = requested {
        return jobs;
    }
    let from_env = context
        .env
        .var("NVM_MAKE_JOBS")
        .and_then(|text| text.trim().parse::<usize>().ok())
        .filter(|jobs| *jobs > 0);
    if let Some(jobs) = from_env {
        return jobs;
    }
    let planned = plan_jobs(None, context.cpu().cores());
    planned.stdout.iter().for_each(|line| transcript.out(line));
    planned.stderr.iter().for_each(|line| transcript.err(line));
    planned.jobs
}

fn take_lock<'a>(
    context: &'a Context<'_>,
    version: &Version,
    transcript: &mut Transcript,
) -> Step<Option<InstallLock<'a>>> {
    let number = |name: &str, default: u64| {
        context
            .env
            .var(name)
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(default)
    };
    let root = context.cache_dir()?.join("locks");
    let text = version.to_string();
    let request = LockRequest {
        root: &root,
        version: &text,
        timeout_seconds: number("NVM_INSTALL_LOCK_TIMEOUT", DEFAULT_LOCK_TIMEOUT_SECONDS),
        stale_minutes: number("NVM_INSTALL_LOCK_STALE", 0),
        now: SystemTime::now(),
    };
    let mut notes = Vec::new();
    let lock = take(context.fs, context.sleeper(), &request, &mut notes);
    for note in notes {
        transcript.err(note);
    }
    Ok(lock?)
}
```

Apply to `src/commands/install/fetch/mod.rs` (above the test module):

```diff
--- a/src/commands/install/fetch/mod.rs
+++ b/src/commands/install/fetch/mod.rs
@@ -8,7 +8,7 @@
 use crate::context::Context;
 use crate::domain::checksum::{compare, expected_digest};
 use crate::domain::mirror::{self, MirrorUrl};
-use crate::domain::version::Version;
+use crate::domain::version::{Flavor, Version};
 
 /// A failure whose messages are in the transcript already.
 #[derive(Debug, PartialEq, Eq)]
@@ -42,6 +42,26 @@
         })
     }
 
+    /// The source archive of `version`: `node-v20.10.0` in `.cache/src`, the
+    /// same for every platform.
+    #[must_use]
+    pub fn source_of(context: &Context<'_>, version: &Version) -> Option<Self> {
+        let flavor = match version.flavor {
+            Flavor::Node => "node",
+            Flavor::IoJs => "iojs",
+        };
+        let slug = format!("{flavor}-{}", version.directory_name());
+        let directory = context.cache_dir().ok()?.join("src").join(&slug);
+        let file_name = format!("{slug}.tar.gz");
+        let tarball = directory.join(&file_name);
+        Some(Self {
+            slug,
+            file_name,
+            directory,
+            tarball,
+        })
+    }
+
     /// Where the archive is unpacked.
     #[must_use]
     pub fn files(&self) -> PathBuf {
@@ -71,30 +91,30 @@
 /// [`Failed`], after the transcript says why.
 pub fn fetch(
     context: &Context<'_>,
+    artifact: &Artifact,
     version: &Version,
     offline: bool,
     transcript: &mut Transcript,
 ) -> Result<PathBuf, Failed> {
-    let artifact = Artifact::of(context, version).ok_or(Failed)?;
     if offline {
-        return cached_only(context, &artifact, transcript);
+        return cached_only(context, artifact, transcript);
     }
     let mirror = mirror::from_env(context.env, version.flavor).map_err(|error| {
         transcript.err(error.to_string());
         Failed
     })?;
-    let expected = expected_checksum(context, &mirror, version, &artifact);
+    let expected = expected_checksum(context, &mirror, version, artifact);
     let files = artifact.files();
     context.fs.create_dir_all(&files).map_err(|_| {
         transcript.err(format!("creating directory {} failed", files.display()));
         Failed
     })?;
-    if reuse_cache(context, &artifact, &expected, transcript) {
-        return Ok(artifact.tarball);
-    }
-    download(context, &mirror, version, &artifact, transcript)?;
-    verify(context, &artifact, &expected, transcript)?;
-    Ok(artifact.tarball)
+    if reuse_cache(context, artifact, &expected, transcript) {
+        return Ok(artifact.tarball.clone());
+    }
+    download(context, &mirror, version, artifact, transcript)?;
+    verify(context, artifact, &expected, transcript)?;
+    Ok(artifact.tarball.clone())
 }
 
 /// `--offline`: the cached archive, taken without a checksum, or nothing.
```

Apply to `src/commands/install/flow.rs` (above the test module):

```diff
--- a/src/commands/install/flow.rs
+++ b/src/commands/install/flow.rs
@@ -30,4 +30,6 @@
     pub path: PathBuf,
     /// Where `--reinstall-packages-from` takes packages from.
     pub source: Option<Source>,
+    /// `make -j`, from `-j` when it is a natural number.
+    pub make_jobs: Option<usize>,
 }
```

Apply to `src/commands/install/mod.rs` (above the test module):

```diff
--- a/src/commands/install/mod.rs
+++ b/src/commands/install/mod.rs
@@ -2,6 +2,7 @@
 //! `$NVM_DIR/versions`. Only prebuilt binaries (`.tar.gz`) are installed; the
 //! install is not activated, which is the shell's job (`nvm use`).
 
+mod acquire;
 mod defaults;
 pub mod fetch;
 mod flow;
@@ -12,20 +13,18 @@
 pub mod options;
 pub mod place;
 mod resolve;
-
-use std::time::SystemTime;
+mod source;
 
 use crate::commands::Output;
 use crate::commands::npm::packages::Source;
 use crate::commands::resolve::{Resolved, resolve_installed};
 use crate::commands::transcript::Transcript;
 use crate::context::Context;
-use crate::domain::platform::binary_available;
-use crate::domain::version::{Flavor, Version};
+use crate::domain::source_build::natural_jobs;
+use crate::domain::version::Version;
 use crate::domain::version_prefix::with_v_prefix;
 use crate::error::{CliError, NvmExitCode};
 use flow::{Halt, Step, Target};
-use lock::{LockRequest, acquire};
 use options::Options;
 use resolve::{check_floor, resolve};
 
@@ -33,7 +32,6 @@
 Usage: nvm install [<version>]\n  \
 Provide a <version>, or run from a directory containing an .nvmrc file.\n  \
 Run `nvm --help` for full help.";
-const DEFAULT_LOCK_TIMEOUT_SECONDS: u64 = 600;
 
 /// # Errors
 /// As [`options::parse`], [`CliError::Usage`] without a version, and
@@ -48,7 +46,9 @@
     }
 }
 
-fn install(context: &Context<'_>, options: &Options, transcript: &mut Transcript) -> Step<()> {
+fn install(context: &Context<'_>, given: &Options, transcript: &mut Transcript) -> Step<()> {
+    let make_jobs = requested_jobs(given, transcript);
+    let options = &no_source_fallback(context, given)?;
     announce(options, transcript)?;
     let version = resolve(context, options, transcript)?;
     check_floor(context, &version, transcript)?;
@@ -58,6 +58,7 @@
         version,
         path,
         source,
+        make_jobs,
     };
     if place::is_valid_install(context, &target.path) {
         already_installed(context, options, &target, transcript)
@@ -95,13 +96,7 @@
     target: &Target,
     transcript: &mut Transcript,
 ) -> Step<()> {
-    install_binary(
-        context,
-        &target.version,
-        &target.path,
-        options.offline,
-        transcript,
-    )?;
+    acquire::acquire(context, options, target, transcript)?;
     apply_alias(context, options, transcript)?;
     if !place::is_valid_install(context, &target.path) {
         let message = format!(
@@ -148,6 +143,35 @@
     }
 }
 
+/// `-j`: what `nvm.sh` says when the option is read, and the number if it is
+/// a natural one.
+fn requested_jobs(options: &Options, transcript: &mut Transcript) -> Option<usize> {
+    let text = options.make_jobs.as_deref()?;
+    let jobs = natural_jobs(text);
+    match jobs {
+        Some(jobs) => transcript.out(format!("number of `make` jobs: {jobs}")),
+        None if !text.is_empty() => transcript.err(format!(
+            "{text} is invalid for number of `make` jobs, must be a natural number"
+        )),
+        None => {}
+    }
+    jobs
+}
+
+/// `NVM_NO_SOURCE_FALLBACK=1` is `-b` for every install, and is at odds with
+/// `-s`.
+fn no_source_fallback(context: &Context<'_>, given: &Options) -> Step<Options> {
+    let mut options = given.clone();
+    if context.env.var("NVM_NO_SOURCE_FALLBACK").as_deref() == Some("1") && !options.no_source {
+        if options.no_binary {
+            let message = "-s cannot be combined with NVM_NO_SOURCE_FALLBACK=1 since that would skip install from both binary and source";
+            return Err(Halt::Error(CliError::InvalidOptions(message.to_owned())));
+        }
+        options.no_source = true;
+    }
+    Ok(options)
+}
+
 /// The install ends with the status of its last step.
 fn end_with(status: NvmExitCode) -> Step<()> {
     match status {
@@ -188,71 +212,3 @@
     Ok(())
 }
 
-/// Everything from the lock to the unpacked directory.
-fn install_binary(
-    context: &Context<'_>,
-    version: &Version,
-    version_path: &std::path::Path,
-    offline: bool,
-    transcript: &mut Transcript,
-) -> Step<()> {
-    let unavailable = !binary_available(version)
-        || context.platform().is_none()
-        || (version.flavor == Flavor::Node && version.triple() < (0, 12, 0));
-    if unavailable {
-        transcript.err(format!("Binary download is not available for {version}"));
-        return Err(Halt::Exit(NvmExitCode::InvalidVersion));
-    }
-    let _lock = take_lock(context, version, transcript)?;
-    let name = match version.flavor {
-        Flavor::Node => "node",
-        Flavor::IoJs => "io.js",
-    };
-    transcript.out(format!(
-        "Downloading and installing {name} {}...",
-        version.directory_name()
-    ));
-    let tarball = fetch::fetch(context, version, offline, transcript)
-        .map_err(|_| binary_failed(transcript))?;
-    let artifact =
-        fetch::Artifact::of(context, version).ok_or_else(|| binary_failed(transcript))?;
-    place::place(context, &tarball, &artifact.files(), version_path).map_err(|message| {
-        transcript.err(message);
-        binary_failed(transcript)
-    })
-}
-
-fn binary_failed(transcript: &mut Transcript) -> Halt {
-    transcript.err("Binary download failed. Download from source aborted.");
-    Halt::Exit(NvmExitCode::MissingTarget)
-}
-
-fn take_lock<'a>(
-    context: &'a Context<'_>,
-    version: &Version,
-    transcript: &mut Transcript,
-) -> Step<Option<lock::InstallLock<'a>>> {
-    let number = |name: &str, default: u64| {
-        context
-            .env
-            .var(name)
-            .and_then(|text| text.trim().parse().ok())
-            .unwrap_or(default)
-    };
-    let root = context.cache_dir()?.join("locks");
-    let text = version.to_string();
-    let request = LockRequest {
-        root: &root,
-        version: &text,
-        timeout_seconds: number("NVM_INSTALL_LOCK_TIMEOUT", DEFAULT_LOCK_TIMEOUT_SECONDS),
-        stale_minutes: number("NVM_INSTALL_LOCK_STALE", 0),
-        now: SystemTime::now(),
-    };
-    let mut notes = Vec::new();
-    let lock = acquire(context.fs, context.sleeper(), &request, &mut notes);
-    for note in notes {
-        transcript.err(note);
-    }
-    Ok(lock?)
-}
-
```

Apply to `src/commands/install/options/mod.rs` (above the test module):

```diff
--- a/src/commands/install/options/mod.rs
+++ b/src/commands/install/options/mod.rs
@@ -1,17 +1,16 @@
 //! The command line of `nvm install`, as `nvm.sh` reads it: options first,
 //! then the version (or `lts/*`, `lts/<name>`). What follows the version is
 //! looked at only for `--skip-default-packages` and the two options that name
-//! a version to take packages from; `--save` there, like any other word, is
-//! ignored, as in `nvm.sh`.
+//! a version to take packages from; every other word there (and `--save` is
+//! one) is handed to `./configure` when the version is built from source, as
+//! in `nvm.sh`.
 
 use crate::error::CliError;
 
-/// Options that need a source build: not in this port yet.
-const NOT_YET: [&str; 2] = ["-s", "-j"];
 /// Both options mean the same; `nvm.sh` words its messages after the one used.
 const REINSTALL_OPTIONS: [&str; 2] = ["--reinstall-packages-from", "--copy-packages-from"];
 
-#[derive(Debug, Default, PartialEq, Eq)]
+#[derive(Debug, Clone, Default, PartialEq, Eq)]
 pub struct Options {
     /// The version, partial version or alias: what follows the options.
     pub version: String,
@@ -35,11 +34,19 @@
     pub offline: bool,
     /// `--save` or `-w`: write the version to `.nvmrc` in the current directory.
     pub save: bool,
+    /// `-s`: build from source, whatever there is a binary for.
+    pub no_binary: bool,
+    /// `-b`: never build from source, not even when the binary fails.
+    pub no_source: bool,
+    /// `-j <jobs>`: how many `make` jobs, as given.
+    pub make_jobs: Option<String>,
+    /// The words after the version that no option takes: arguments for
+    /// `./configure`.
+    pub extra: Vec<String>,
 }
 
-fn unsupported(option: &str) -> CliError {
-    CliError::Unsupported(format!("The option \"{option}\" is not supported yet."))
-}
+pub const BOTH_OFF: &str =
+    "-s and -b cannot be set together since they would skip install from both binary and source";
 
 fn already_given() -> CliError {
     let message =
@@ -88,7 +95,10 @@
     let mut rest = args.iter().peekable();
     while let Some(option) = rest.next_if(|arg| is_option(arg)) {
         match option.as_str() {
-            "-b" | "--no-progress" => {}
+            "--no-progress" => {}
+            "-s" => set_build_mode(&mut options, true)?,
+            "-b" => set_build_mode(&mut options, false)?,
+            "-j" => options.make_jobs = Some(rest.next().cloned().unwrap_or_default()),
             "--latest-npm" => options.latest_npm = true,
             "--offline" => options.offline = true,
             "--save" | "-w" => set_save(&mut options)?,
@@ -105,8 +115,9 @@
                 let message = "arguments with `---` are not supported - this is likely a typo";
                 return Err(CliError::Unsupported(message.to_owned()));
             }
-            other if read_reinstall(&mut options, other)? => {}
-            other => return Err(unsupported(other)),
+            other => {
+                read_reinstall(&mut options, other)?;
+            }
         }
     }
     if let Some(version) = rest.next() {
@@ -116,8 +127,8 @@
     for option in rest {
         if option == "--skip-default-packages" {
             options.skip_default_packages = true;
-        } else {
-            read_reinstall(&mut options, option)?;
+        } else if !read_reinstall(&mut options, option)? {
+            options.extra.push(option.clone());
         }
     }
     options.announce_lts = options.lts.is_some() && options.version.is_empty();
@@ -129,7 +140,9 @@
 fn is_option(arg: &str) -> bool {
     matches!(
         arg,
-        "-b" | "--no-progress"
+        "-b" | "-s"
+            | "-j"
+            | "--no-progress"
             | "--lts"
             | "--default"
             | "--latest-npm"
@@ -140,8 +153,19 @@
     ) || arg.starts_with("--lts=")
         || arg.starts_with("--alias=")
         || arg.starts_with("---")
-        || NOT_YET.contains(&arg)
         || reinstall_option(arg).is_some()
+}
+
+fn set_build_mode(options: &mut Options, binary_off: bool) -> Result<(), CliError> {
+    if (binary_off && options.no_source) || (!binary_off && options.no_binary) {
+        return Err(CliError::InvalidOptions(BOTH_OFF.to_owned()));
+    }
+    if binary_off {
+        options.no_binary = true;
+    } else {
+        options.no_source = true;
+    }
+    Ok(())
 }
 
 fn set_save(options: &mut Options) -> Result<(), CliError> {
```

Apply to `src/commands/install/place/mod.rs` (above the test module):

```diff
--- a/src/commands/install/place/mod.rs
+++ b/src/commands/install/place/mod.rs
@@ -41,15 +41,7 @@
     version_path: &Path,
 ) -> Result<(), String> {
     let fs = context.fs;
-    fs.remove_dir_all(files)
-        .map_err(|error| error.to_string())?;
-    fs.create_dir_all(files)
-        .map_err(|error| error.to_string())?;
-    context
-        .archive()
-        .extract(tarball, files)
-        .map_err(|error| error.to_string())?;
-    let top = single_directory(context, files)?;
+    let top = unpack(context, tarball, files)?;
     fs.remove_dir_all(version_path)
         .map_err(|error| error.to_string())?;
     if let Some(parent) = version_path.parent() {
@@ -60,6 +52,24 @@
         .map_err(|error| error.to_string())?;
     let _ = fs.remove_dir_all(files);
     Ok(())
+}
+
+/// Unpacks `tarball` into an empty `files` and returns the one directory the
+/// archive holds.
+///
+/// # Errors
+/// What went wrong, as a message for stderr.
+pub fn unpack(context: &Context<'_>, tarball: &Path, files: &Path) -> Result<PathBuf, String> {
+    let fs = context.fs;
+    fs.remove_dir_all(files)
+        .map_err(|error| error.to_string())?;
+    fs.create_dir_all(files)
+        .map_err(|error| error.to_string())?;
+    context
+        .archive()
+        .extract(tarball, files)
+        .map_err(|error| error.to_string())?;
+    single_directory(context, files)
 }
 
 /// The only entry of `directory`, which must be a directory: the archive's
```

Insert above the `#[cfg(test)]` line of `src/commands/install/source/mod.rs`:

```rust
//! Building a version from source: `nvm_install_source`. Download the source
//! archive, unpack it, then `./configure --prefix=<version path>`, `make` and
//! `make install`.

use std::path::Path;

use crate::commands::install::fetch::{Artifact, fetch};
use crate::commands::install::place::unpack;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::path_search::find_in_path;
use crate::domain::platform::Os;
use crate::domain::source_build::{clang_version, compiler, make};
use crate::domain::version::Version;
use crate::ports::Invocation;

/// What the build needs from the command line and the environment.
pub struct Build<'a> {
    pub version: &'a Version,
    pub version_path: &'a Path,
    pub jobs: usize,
    /// The words after the version, for `./configure`.
    pub extra: &'a [String],
    pub offline: bool,
}

/// A build that failed; what it printed is in the transcript.
#[derive(Debug, PartialEq, Eq)]
pub struct BuildFailed;

/// `--without-snapshot` for 32-bit ARM, then what the user passed, each
/// preceded by a space as `nvm.sh` builds the string (so it shows two spaces
/// after the arm option).
fn parameters(context: &Context<'_>, extra: &[String]) -> String {
    let given: String = extra.iter().map(|word| format!(" {word}")).collect();
    let arm = context
        .platform()
        .is_some_and(|platform| matches!(platform.arch.as_str(), "armv6l" | "armv7l"));
    match (arm, given.is_empty()) {
        (true, true) => "--without-snapshot".to_owned(),
        (true, false) => format!("--without-snapshot {given}"),
        (false, _) => given,
    }
}

/// The version Clang reports, when both `clang` and `clang++` are on `PATH`.
fn clang(context: &Context<'_>) -> Option<(u64, u64)> {
    let path = context.env.var_os("PATH").unwrap_or_default();
    find_in_path(context.fs, &path, "clang++")?;
    let program = find_in_path(context.fs, &path, "clang")?;
    let output = context.process().run(&program, &["--version"]).ok()?;
    clang_version(&output.stdout)
}

/// Runs a step of the build in `directory`; false (after printing why) when it
/// does not succeed.
fn run(context: &Context<'_>, invocation: &Invocation, transcript: &mut Transcript) -> bool {
    match context.process().execute(invocation) {
        Ok(done) => {
            done.stdout.lines().for_each(|line| transcript.out(line));
            done.stderr.lines().for_each(|line| transcript.err(line));
            done.success
        }
        Err(error) => {
            transcript.err(format!("{}: {error}", invocation.program.display()));
            false
        }
    }
}

/// # Errors
/// [`BuildFailed`], after the transcript says why.
pub fn build(
    context: &Context<'_>,
    job: &Build<'_>,
    transcript: &mut Transcript,
) -> Result<(), BuildFailed> {
    let params = parameters(context, job.extra);
    if !params.is_empty() {
        transcript.out(format!("Additional options while compiling: {params}"));
    }
    let os = context.platform().map_or(Os::Linux, |platform| platform.os);
    let toolchain = compiler(
        os,
        clang(context),
        context.env.var("CC").as_deref(),
        context.env.var("CXX").as_deref(),
    );
    if toolchain.clang_note {
        transcript.out(
            "Clang v3.5+ detected! CC or CXX not specified, will use Clang as C/C++ compiler!",
        );
    }
    let artifact = Artifact::source_of(context, job.version).ok_or(BuildFailed)?;
    let tarball =
        fetch(context, &artifact, job.version, job.offline, transcript).map_err(|_| BuildFailed)?;
    let files = artifact.files();
    unpack_source(context, &tarball, &files)
        .map_err(|message| failed(context, job, &files, message, transcript))?;
    compile(
        context,
        job,
        &files,
        &params,
        &toolchain.words,
        os,
        transcript,
    )
    .map_err(|()| failed(context, job, &files, String::new(), transcript))
}

/// The source tree is `files` itself, the archive's top-level directory
/// stripped (`tar --strip-components 1`).
fn unpack_source(context: &Context<'_>, tarball: &Path, files: &Path) -> Result<(), String> {
    let top = unpack(context, tarball, files)?;
    let entries = context
        .fs
        .read_dir(&top)
        .map_err(|error| error.to_string())?;
    for entry in entries {
        context
            .fs
            .rename(&top.join(&entry.name), &files.join(&entry.name))
            .map_err(|error| error.to_string())?;
    }
    context
        .fs
        .remove_dir_all(&top)
        .map_err(|error| error.to_string())
}

/// `nvm: install <v> failed!`, and the unpacked tree is removed.
fn failed(
    context: &Context<'_>,
    job: &Build<'_>,
    files: &Path,
    message: String,
    transcript: &mut Transcript,
) -> BuildFailed {
    if !message.is_empty() {
        transcript.err(message);
    }
    transcript.err(format!(
        "nvm: install {} failed!",
        job.version.directory_name()
    ));
    let _ = context.fs.remove_dir_all(files);
    BuildFailed
}

/// `./configure`, `make`, then `make install`.
fn compile(
    context: &Context<'_>,
    job: &Build<'_>,
    top: &Path,
    params: &str,
    compiler_words: &[String],
    os: Os,
    transcript: &mut Transcript,
) -> Result<(), ()> {
    let prefix = format!("--prefix={}", job.version_path.display());
    let words: Vec<&str> = params.split_whitespace().collect();
    // `nvm.sh` writes `$ADDITIONAL_PARAMETERS'<'`: the `<` sticks to the last
    // option, and stands alone when there are none.
    let mut shown = vec!["$>./configure".to_owned(), prefix.clone()];
    shown.extend(words.iter().map(|word| (*word).to_owned()));
    if words.is_empty() {
        shown.push("<".to_owned());
    } else if let Some(last) = shown.last_mut() {
        last.push('<');
    }
    transcript.out(shown.join(" "));
    let configure = Invocation::new(top.join("configure"))
        .args(&[&prefix])
        .args(&words)
        .dir(top);
    let (program, shell) = make(os, job.version);
    let jobs = job.jobs.to_string();
    let make_args = |goal: Option<&str>| {
        let mut args: Vec<&str> = shell.iter().map(String::as_str).collect();
        args.extend(["-j", &jobs]);
        args.extend(compiler_words.iter().map(String::as_str));
        args.extend(goal);
        Invocation::new(program).args(&args).dir(top)
    };
    if !run(context, &configure, transcript) || !run(context, &make_args(None), transcript) {
        return Err(());
    }
    let _ = context.fs.remove_file(job.version_path);
    run(context, &make_args(Some("install")), transcript)
        .then_some(())
        .ok_or(())
}

```

Apply to `src/domain/source_build/mod.rs` (above the test module):

```diff
--- a/src/domain/source_build/mod.rs
+++ b/src/domain/source_build/mod.rs
@@ -14,7 +14,8 @@
 }
 
 /// `nvm_is_natural_num`: digits, and not zero.
-fn natural(text: &str) -> Option<usize> {
+#[must_use]
+pub fn natural_jobs(text: &str) -> Option<usize> {
     let number: usize = text.parse().ok()?;
     (number > 0 && text.bytes().all(|byte| byte.is_ascii_digit())).then_some(number)
 }
@@ -28,7 +29,7 @@
         stdout: Vec::new(),
         stderr: Vec::new(),
     };
-    if let Some(number) = requested.and_then(natural) {
+    if let Some(number) = requested.and_then(natural_jobs) {
         said.jobs = number;
         said.stdout.push(format!("number of `make` jobs: {number}"));
         return said;
```

Apply to `src/error.rs` (above the test module):

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -8,33 +8,62 @@
 #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
 pub enum NvmExitCode {
     #[default]
-    Success = 0,
-    Failure = 1,
-    InvalidVersion = 3,
-    BelowVersionFloor = 7,
-    AliasLoop = 8,
+    Success,
+    Failure,
+    InvalidVersion,
+    BelowVersionFloor,
+    AliasLoop,
     /// Something that was asked for does not exist: an `lts/<name>` alias, or
     /// the archive of a version whose download failed.
-    MissingTarget = 2,
+    MissingTarget,
     /// `nvm install --reinstall-packages-from` the very version being
     /// installed.
-    SameVersion = 4,
+    SameVersion,
     /// `nvm install --reinstall-packages-from` a version that is not
     /// installed.
-    SourceNotInstalled = 5,
+    SourceNotInstalled,
     /// Options that cannot be combined, or given twice.
-    InvalidOptions = 6,
+    InvalidOptions,
+    /// `NVM_INSTALL_THIRD_PARTY_HOOK` succeeded and installed nothing.
+    HookClaimedSuccess,
     /// An option `nvm.sh` does not support, or one used in a combination it
     /// does not support.
-    UnsupportedOption = 55,
+    UnsupportedOption,
     /// A usage error, or a requested system version that does not exist.
-    NotFound = 127,
+    NotFound,
+    /// The status of a program that `nvm` ran and passes on, such as the
+    /// third-party install hook.
+    Passed(u8),
 }
 
 impl NvmExitCode {
     #[must_use]
     pub fn code(self) -> u8 {
-        self as u8
+        match self {
+            Self::Success => 0,
+            Self::Failure => 1,
+            Self::MissingTarget => 2,
+            Self::InvalidVersion => 3,
+            Self::SameVersion => 4,
+            Self::SourceNotInstalled => 5,
+            Self::InvalidOptions => 6,
+            Self::BelowVersionFloor => 7,
+            Self::AliasLoop => 8,
+            Self::HookClaimedSuccess => 33,
+            Self::UnsupportedOption => 55,
+            Self::NotFound => 127,
+            Self::Passed(code) => code,
+        }
+    }
+
+    /// The status of a program that ran: 1 when it has none (a signal).
+    #[must_use]
+    pub fn passing_on(code: Option<i32>) -> Self {
+        match code.and_then(|code| u8::try_from(code).ok()) {
+            Some(0) => Self::Success,
+            Some(code) => Self::Passed(code),
+            None => Self::Failure,
+        }
     }
 }
 
```

Apply to `src/fakes/process.rs` (above the test module):

```diff
--- a/src/fakes/process.rs
+++ b/src/fakes/process.rs
@@ -10,6 +10,7 @@
 pub struct FakeProcess {
     outputs: BTreeMap<PathBuf, ProcessOutput>,
     executions: BTreeMap<(PathBuf, String), Completed>,
+    effects: BTreeMap<(PathBuf, String), Box<dyn Fn()>>,
     executed: RefCell<Vec<Invocation>>,
 }
 
@@ -52,8 +53,18 @@
             success: true,
             stdout: stdout.to_owned(),
             stderr: String::new(),
+            ..Completed::default()
         };
         self.with_execution(program, args, done)
+    }
+
+    /// Something that happens when `execute` runs `program` with exactly
+    /// `args`: a build that leaves files behind, say.
+    #[must_use]
+    pub fn with_effect(mut self, program: &str, args: &str, effect: impl Fn() + 'static) -> Self {
+        self.effects
+            .insert((PathBuf::from(program), args.to_owned()), Box::new(effect));
+        self
     }
 
     /// Every invocation `execute` was given, in order.
@@ -67,6 +78,9 @@
     fn execute(&self, invocation: &Invocation) -> io::Result<Completed> {
         self.executed.borrow_mut().push(invocation.clone());
         let key = (invocation.program.clone(), invocation.args.join(" "));
+        if let Some(effect) = self.effects.get(&key) {
+            effect();
+        }
         self.executions
             .get(&key)
             .cloned()
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -146,6 +146,8 @@
 #[derive(Debug, Clone, Default, PartialEq, Eq)]
 pub struct Completed {
     pub success: bool,
+    /// The exit status, when the program ended by itself.
+    pub code: Option<i32>,
     pub stdout: String,
     pub stderr: String,
 }
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 584 unit tests plus the end-to-end tests (the earlier end-to-
end tests now pass `-b`).

- [ ] **Step 5: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Expected: formatting clean, zero clippy warnings.

```bash
git add src tests
git commit -S -m "feat(install): build from source, fall back to it and run the install hook"
```

---

### Task 21: Limits, the `npm` and source end-to-end tests, and the gate

**Files:**

- Create: `tests/common/mod.rs`, `tests/install_npm_cli.rs`,
  `tests/install_source_cli.rs`, `src/cli/tests/mod.rs`,
  `src/cli/tests/install.rs`, `src/commands/install/tests/built.rs`,
  `src/commands/install/tests/hook.rs`
- Modify: `tests/install_cli.rs`, `src/commands/install/tests/source.rs`,
  `src/commands/install/tests/mod.rs`,
  `src/commands/install/acquire/hook.rs`, `src/commands/install/acquire/mod.rs`,
  `src/commands/install/lock/mod.rs`, `src/commands/install/npm_steps/mod.rs`,
  `src/commands/install/options/mod.rs`, `src/commands/install/source/mod.rs`,
  `src/commands/npm/latest/mod.rs`, `src/commands/npm/packages/mod.rs`,
  `src/commands/reinstall_packages/mod.rs`, `src/domain/npm/upgrade/mod.rs`,
  `src/domain/source_build/mod.rs`, `src/fakes/file_system/mod.rs`
- Delete: `src/cli/tests.rs`

**Interfaces:**

- Produces: nothing new for users. Functions that grew past 30 lines are split
  (`parse` into `read_leading`, `read_valued` and `read_trailing`; `build` and
  `compile` into `choose_compiler`, `configure_line` and `make_invocation`; the
  lock, the hook, `acquire`, the default packages, `install_latest`, `link`,
  `jobs`, `steps` and the fake `rename` likewise), two test files that passed
  300 lines are split, and the end-to-end tests of Plans 4 and 5 share
  `tests/common/mod.rs`. New acceptance tests run the real binary against a
  mirror whose archives hold a fake `npm` (`--latest-npm`, the default packages,
  `--skip-default-packages`, `--reinstall-packages-from`, `install-latest-npm`,
  a version with no `npm`) and a source archive with a fake `./configure` and a
  fake `make` on `PATH` (`-s`, the fallback, `-b`, the words for
  `./configure`, a failing `make`, an install hook whose status is passed on). Both
  files are Unix only.

- [ ] **Step 1: Share the end-to-end helpers and add the new acceptance tests**

Delete `src/cli/tests.rs` (`git rm src/cli/tests.rs`).

Create `src/cli/tests/install.rs`:

```rust
use super::*;

#[test]
fn the_npm_commands_have_their_usages_with_exit_127() {
    let (code, _, err) = run_cli(&["nvm", "install-latest-npm", "x"]);
    assert_eq!(code, 127);
    assert_eq!(
        err,
        "Usage: nvm install-latest-npm\n  Run `nvm --help` for full help.\n"
    );
    for command in ["reinstall-packages", "copy-packages"] {
        let (code, _, err) = run_cli(&["nvm", command]);
        assert_eq!(code, 127);
        assert_eq!(
            err,
            format!("Usage: nvm {command} <version>\n  Run `nvm --help` for full help.\n")
        );
    }
}

#[test]
fn conflicting_install_options_exit_6_and_the_message_comes_on_stderr() {
    let (code, out, err) = run_cli(&["nvm", "install", "-s", "-b", "20"]);
    assert_eq!((code, out.as_str()), (6, ""));
    assert!(err.starts_with("-s and -b cannot be set together"));
}
```

Create `src/cli/tests/mod.rs`:

```rust
use super::*;
use crate::domain::fixtures::index_text;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};
use crate::ports::FileSystem;

fn run_cli(args: &[&str]) -> (u8, String, String) {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(args, &Context::new(&fs, &env), &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn version_prints_the_match_and_exits_zero() {
    assert_eq!(
        run_cli(&["nvm", "version", "20"]),
        (0, "v20.1.0\n".into(), String::new())
    );
}

#[test]
fn which_prints_the_binary_path() {
    assert_eq!(
        run_cli(&["nvm", "which", "20"]),
        (
            0,
            "/n/versions/node/v20.1.0/bin/node\n".into(),
            String::new()
        )
    );
}

#[test]
fn which_of_a_missing_version_fails_on_stderr_with_exit_1() {
    let (code, out, err) = run_cli(&["nvm", "which", "16"]);
    assert_eq!(code, 1);
    assert!(out.is_empty());
    assert!(err.starts_with("N/A: version \"v16\" is not yet installed."));
}

#[test]
fn which_without_an_argument_is_a_usage_error_with_exit_127() {
    let (code, out, err) = run_cli(&["nvm", "which"]);
    assert_eq!(code, 127);
    assert!(out.is_empty() && err.starts_with("Usage: nvm which"));
}

#[test]
fn unalias_removes_the_alias_and_says_how_to_restore_it() {
    let fs = FakeFileSystem::default().with_file("/n/alias/work", "v18");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args = ["nvm", "unalias", "work"];
    let code = run(args, &Context::new(&fs, &env), &mut out, &mut err);
    assert_eq!(code, 0);
    let printed = String::from_utf8(out).unwrap();
    assert!(printed.starts_with("Deleted alias work - restore it"));
    assert!(!fs.is_file(std::path::Path::new("/n/alias/work")));
}

#[test]
fn alias_creates_the_file_and_prints_the_formatted_line() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args = ["nvm", "alias", "work", "20"];
    let code = run(args, &Context::new(&fs, &env), &mut out, &mut err);
    assert_eq!(
        (code, out, err),
        (0, b"work -> 20 (-> v20.1.0 *)\n".to_vec(), Vec::new())
    );
    let stored = fs.read_to_string(std::path::Path::new("/n/alias/work"));
    assert_eq!(stored.unwrap(), "20\n");
}

#[test]
fn unalias_without_a_name_is_a_usage_error_with_exit_127() {
    let (code, out, err) = run_cli(&["nvm", "unalias"]);
    assert_eq!(code, 127);
    assert!(out.is_empty() && err.starts_with("Usage: nvm unalias <name>"));
}

#[test]
fn current_prints_the_active_version() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/n/versions/node/v20.1.0/bin");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(
        ["nvm", "current"],
        &Context::new(&fs, &env),
        &mut out,
        &mut err,
    );
    assert_eq!((code, out, err), (0, b"v20.1.0\n".to_vec(), Vec::new()));
}

#[test]
fn ls_prints_the_rows_then_the_aliases_and_list_is_the_same_command() {
    let rows = "        v20.1.0 *\n";
    let aliases = "iojs -> N/A (default)\n\
                   node -> stable (-> v20.1.0 *) (default)\n\
                   stable -> 20.1 (-> v20.1.0 *) (default)\n\
                   unstable -> N/A (default)\n";
    let expected = (0, format!("{rows}{aliases}"), String::new());
    assert_eq!(run_cli(&["nvm", "ls"]), expected);
    assert_eq!(run_cli(&["nvm", "list"]), expected);
    let without = (0, rows.to_owned(), String::new());
    assert_eq!(run_cli(&["nvm", "ls", "--no-alias"]), without);
    assert_eq!(
        run_cli(&["nvm", "ls", "--no-colors", "--no-alias"]),
        without
    );
}

#[test]
fn unsupported_options_exit_55_with_the_nvm_sh_message() {
    assert_eq!(
        run_cli(&["nvm", "ls", "--bogus"]),
        (
            55,
            String::new(),
            "Unsupported option \"--bogus\".\n".to_owned()
        )
    );
    assert_eq!(
        run_cli(&["nvm", "alias", "--bogus"]),
        (
            55,
            String::new(),
            "Unsupported option \"--bogus\".\n".to_owned()
        )
    );
    let (code, out, err) = run_cli(&["nvm", "ls", "20", "--no-alias"]);
    assert_eq!(code, 55);
    assert!(out.is_empty());
    assert_eq!(
        err,
        "`--no-alias` is not supported when a pattern is provided.\n"
    );
}

#[test]
fn alias_without_arguments_lists_the_aliases() {
    let (code, out, _) = run_cli(&["nvm", "alias"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("iojs -> N/A (default)\n"), "{out}");
}

#[test]
fn ls_of_a_missing_version_prints_na_on_stdout_and_exits_3() {
    assert_eq!(
        run_cli(&["nvm", "ls", "16"]),
        (3, "            N/A\n".to_owned(), String::new())
    );
}

#[test]
fn version_without_an_argument_means_current() {
    assert_eq!(
        run_cli(&["nvm", "version"]),
        (0, "none\n".into(), String::new())
    );
}

#[test]
fn version_reports_not_installed_on_stdout_with_exit_3() {
    assert_eq!(
        run_cli(&["nvm", "version", "16"]),
        (3, "N/A\n".into(), String::new())
    );
}

#[test]
fn version_of_a_non_version_name_is_na_on_stdout_with_exit_3() {
    assert_eq!(
        run_cli(&["nvm", "version", "foo"]),
        (3, "N/A\n".into(), String::new())
    );
}

#[test]
fn an_alias_loop_is_reported_on_stderr_with_exit_8() {
    let fs = FakeFileSystem::default()
        .with_file("/n/alias/a", "b")
        .with_file("/n/alias/b", "a");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args = ["nvm", "version", "a"];
    let code = run(args, &Context::new(&fs, &env), &mut out, &mut err);
    assert_eq!(code, 8);
    assert!(out.is_empty() && !err.is_empty());
}

struct BrokenPipe;

impl Write for BrokenPipe {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }
}

#[test]
fn a_failed_stdout_write_exits_1() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let mut err = Vec::new();
    let args = ["nvm", "version", "20"];
    let code = run(args, &Context::new(&fs, &env), &mut BrokenPipe, &mut err);
    assert_eq!(code, 1);
}

#[test]
fn a_failed_stderr_write_keeps_the_exit_code() {
    let fs = FakeFileSystem::default()
        .with_file("/n/alias/a", "b")
        .with_file("/n/alias/b", "a");
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let mut out = Vec::new();
    let args = ["nvm", "version", "a"];
    let code = run(args, &Context::new(&fs, &env), &mut out, &mut BrokenPipe);
    assert_eq!(code, 8);
}

#[test]
fn a_usage_error_exits_127_without_stdout() {
    for args in [&["nvm", "bogus"][..], &["nvm", "which", "--silent", "20"]] {
        let (code, out, err) = run_cli(args);
        assert_eq!(code, 127, "{args:?}");
        assert!(out.is_empty() && !err.is_empty(), "{args:?}");
    }
}

#[test]
fn help_and_version_go_to_stdout_with_exit_0() {
    for flag in ["--help", "--version"] {
        let (code, out, err) = run_cli(&["nvm", flag]);
        assert_eq!(code, 0, "{flag}");
        assert!(!out.is_empty() && err.is_empty(), "{flag}");
    }
}

#[test]
fn ls_remote_and_list_remote_print_the_releases_with_the_exit_status() {
    let index = index_text(&[("v20.10.0", "Iron"), ("v20.9.0", "Iron")]);
    let http = FakeHttp::default()
        .with_body("https://nodejs.org/dist/index.tab", &index)
        .with_status("https://iojs.org/dist/index.tab", 404);
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    for command in ["ls-remote", "list-remote"] {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let context = Context::new(&fs, &env).with_http(&http);
        let code = run(["nvm", command, "--lts"], &context, &mut out, &mut err);
        let expected = "        v20.9.0   (LTS: Iron)\n       v20.10.0   (Latest LTS: Iron)\n";
        assert_eq!(String::from_utf8(out).unwrap(), expected);
        assert_eq!(code, 0);
    }
}

#[test]
fn install_and_its_alias_i_without_a_version_are_a_usage_error_with_exit_127() {
    for command in ["install", "i"] {
        let (code, out, err) = run_cli(&["nvm", command]);
        assert_eq!((code, out.as_str()), (127, ""));
        assert!(
            err.starts_with("No version provided and no .nvmrc file found\nUsage: nvm install")
        );
    }
}

#[test]
fn uninstall_needs_one_word_and_a_missing_version_is_only_a_message() {
    let (code, _, err) = run_cli(&["nvm", "uninstall"]);
    assert_eq!(code, 127);
    assert!(err.starts_with("Usage: nvm uninstall <version>"));
    let (code, out, err) = run_cli(&["nvm", "uninstall", "99"]);
    assert_eq!((code, out.as_str()), (0, ""));
    assert_eq!(err, "Version '99' is not installed.\n");
}

mod install;
```

Create `src/commands/install/tests/built.rs`:

```rust
use std::rc::Rc;

use super::*;
use crate::fakes::FakeCpu;

pub(super) const SRC_URL: &str = "https://nodejs.org/dist/v20.10.0/node-v20.10.0.tar.gz";
pub(super) const SRC_TARBALL: &str = "/n/.cache/src/node-v20.10.0/node-v20.10.0.tar.gz";
pub(super) const TOP: &str = "/n/.cache/src/node-v20.10.0/files";
pub(super) const PREFIX: &str = "--prefix=/n/versions/node/v20.10.0";

/// A world where the binary cannot be had and the source can.
pub(super) struct Built {
    pub(super) fs: Rc<FakeFileSystem>,
    pub(super) http: FakeHttp,
    pub(super) digest: FakeDigest,
    pub(super) env: FakeEnv,
    pub(super) cpu: FakeCpu,
}

impl Built {
    pub(super) fn new() -> Self {
        let node = index_text(&[("v20.10.0", "Iron")]);
        let sums =
            format!("{GOOD}  node-v20.10.0-linux-x64.tar.gz\n{GOOD}  node-v20.10.0.tar.gz\n");
        Self {
            fs: Rc::new(FakeFileSystem::default()),
            http: FakeHttp::default()
                .with_body(NODE_INDEX, &node)
                .with_body(IOJS_INDEX, &index_text(&[]))
                .with_body(SUMS, &sums)
                .with_status(TARBALL_URL, 404)
                .with_bytes(SRC_URL, b"source"),
            digest: FakeDigest::default().with_digest(SRC_TARBALL, GOOD),
            env: FakeEnv::default().with_var("NVM_DIR", "/n"),
            cpu: FakeCpu::with_cores(8),
        }
    }

    pub(super) fn with_env(self, name: &str, value: &str) -> Self {
        let mut vars = vec![("NVM_DIR", "/n")];
        vars.push((name, value));
        let env = vars
            .iter()
            .fold(FakeEnv::default(), |env, (k, v)| env.with_var(k, v));
        Self { env, ..self }
    }

    pub(super) fn run(&self, line: &str) -> Output {
        let fs = Rc::clone(&self.fs);
        let make_node = move || {
            fs.write_file(Path::new(NODE), "binary").unwrap();
            fs.set_executable(Path::new(NODE));
        };
        let process = FakeProcess::default()
            .with_success(&format!("{TOP}/configure"), PREFIX, "configured\n")
            .with_success("make", "-j 7", "built\n")
            .with_success("make", "-j 7 install", "installed\n")
            .with_effect("make", "-j 7 install", make_node)
            .with_success("make", "-j 4", "")
            .with_success("make", "-j 4 install", "")
            .with_effect("make", "-j 4 install", {
                let fs = Rc::clone(&self.fs);
                move || {
                    fs.write_file(Path::new(NODE), "binary").unwrap();
                    fs.set_executable(Path::new(NODE));
                }
            })
            .with_success(
                &format!("{TOP}/configure"),
                &format!("{PREFIX} --with-intl=full-icu"),
                "",
            );
        let archive = FakeArchive::new(&self.fs).with_archive(
            SRC_TARBALL,
            &[("node-v20.10.0/configure", "#!/bin/sh", true)],
        );
        let context = Context::new(self.fs.as_ref(), &self.env)
            .with_http(&self.http)
            .with_digest(&self.digest)
            .with_archive(&archive)
            .with_process(&process)
            .with_cpu(&self.cpu);
        let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        super::super::run(&context, &words).unwrap()
    }
}
```

Create `src/commands/install/tests/hook.rs`:

```rust
use std::rc::Rc;

use super::built::*;
use super::*;

#[test]
fn a_hook_installs_in_place_of_nvm_and_is_told_how() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let fs = Rc::clone(&world.fs);
    let process = FakeProcess::default()
        .with_success(
            "/opt/hook",
            "v20.10.0 node std binary /n/versions/node/v20.10.0",
            "",
        )
        .with_effect(
            "/opt/hook",
            "v20.10.0 node std binary /n/versions/node/v20.10.0",
            move || {
                fs.write_file(Path::new(NODE), "binary").unwrap();
                fs.set_executable(Path::new(NODE));
            },
        );
    let context = Context::new(world.fs.as_ref(), &world.env)
        .with_http(&world.http)
        .with_process(&process);
    let output = super::super::run(&context, &["20".to_owned()]).unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        output.stderr,
        "** $NVM_INSTALL_THIRD_PARTY_HOOK env var set; dispatching to third-party installation method **"
    );
    assert!(
        !world
            .http
            .requests()
            .iter()
            .any(|url| url.contains(".tar.gz"))
    );
}

#[test]
fn a_hook_that_fails_or_installs_nothing_ends_the_install() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let run_with = |process: &FakeProcess| {
        let context = Context::new(world.fs.as_ref(), &world.env)
            .with_http(&world.http)
            .with_process(process);
        super::super::run(&context, &["20".to_owned()]).unwrap()
    };
    let missing = run_with(&FakeProcess::default());
    assert_eq!(missing.status, NvmExitCode::Failure);
    assert!(
        missing.stderr.ends_with(
            "*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var failed to install! ***"
        )
    );
    let silent = FakeProcess::default().with_success(
        "/opt/hook",
        "v20.10.0 node std binary /n/versions/node/v20.10.0",
        "",
    );
    let claimed = run_with(&silent);
    assert_eq!(claimed.status, NvmExitCode::HookClaimedSuccess);
    assert!(claimed.stderr.ends_with("*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var claimed to succeed, but failed to install! ***"));
}

#[test]
fn the_hook_is_told_source_when_s_is_given() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let process = FakeProcess::default();
    let context = Context::new(world.fs.as_ref(), &world.env)
        .with_http(&world.http)
        .with_process(&process);
    super::super::run(&context, &["-s".to_owned(), "20".to_owned()]).unwrap();
    let ran = process.executed();
    assert_eq!(
        ran[0].args,
        [
            "v20.10.0",
            "node",
            "std",
            "source",
            "/n/versions/node/v20.10.0"
        ]
    );
}

#[test]
fn a_hook_that_fails_passes_its_own_status_on() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let failed = crate::ports::Completed {
        code: Some(7),
        ..crate::ports::Completed::default()
    };
    let process = FakeProcess::default().with_execution(
        "/opt/hook",
        "v20.10.0 node std binary /n/versions/node/v20.10.0",
        failed,
    );
    let context = Context::new(world.fs.as_ref(), &world.env)
        .with_http(&world.http)
        .with_process(&process);
    let output = super::super::run(&context, &["20".to_owned()]).unwrap();
    assert_eq!(output.status, NvmExitCode::Passed(7));
    assert_eq!(output.status.code(), 7);
}
```

Apply to `src/commands/install/tests/mod.rs`:

```diff
--- a/src/commands/install/tests/mod.rs
+++ b/src/commands/install/tests/mod.rs
@@ -74,7 +74,9 @@
     text.lines().collect()
 }
 
+mod built;
 mod failures;
+mod hook;
 mod npm;
 mod offline_and_save;
 mod source;
```

Apply to `src/commands/install/tests/source.rs`:

```diff
--- a/src/commands/install/tests/source.rs
+++ b/src/commands/install/tests/source.rs
@@ -1,89 +1,7 @@
 use std::rc::Rc;
 
+use super::built::*;
 use super::*;
-use crate::fakes::FakeCpu;
-
-const SRC_URL: &str = "https://nodejs.org/dist/v20.10.0/node-v20.10.0.tar.gz";
-const SRC_TARBALL: &str = "/n/.cache/src/node-v20.10.0/node-v20.10.0.tar.gz";
-const TOP: &str = "/n/.cache/src/node-v20.10.0/files";
-const PREFIX: &str = "--prefix=/n/versions/node/v20.10.0";
-
-/// A world where the binary cannot be had and the source can.
-struct Built {
-    fs: Rc<FakeFileSystem>,
-    http: FakeHttp,
-    digest: FakeDigest,
-    env: FakeEnv,
-    cpu: FakeCpu,
-}
-
-impl Built {
-    fn new() -> Self {
-        let node = index_text(&[("v20.10.0", "Iron")]);
-        let sums =
-            format!("{GOOD}  node-v20.10.0-linux-x64.tar.gz\n{GOOD}  node-v20.10.0.tar.gz\n");
-        Self {
-            fs: Rc::new(FakeFileSystem::default()),
-            http: FakeHttp::default()
-                .with_body(NODE_INDEX, &node)
-                .with_body(IOJS_INDEX, &index_text(&[]))
-                .with_body(SUMS, &sums)
-                .with_status(TARBALL_URL, 404)
-                .with_bytes(SRC_URL, b"source"),
-            digest: FakeDigest::default().with_digest(SRC_TARBALL, GOOD),
-            env: FakeEnv::default().with_var("NVM_DIR", "/n"),
-            cpu: FakeCpu::with_cores(8),
-        }
-    }
-
-    fn with_env(self, name: &str, value: &str) -> Self {
-        let mut vars = vec![("NVM_DIR", "/n")];
-        vars.push((name, value));
-        let env = vars
-            .iter()
-            .fold(FakeEnv::default(), |env, (k, v)| env.with_var(k, v));
-        Self { env, ..self }
-    }
-
-    fn run(&self, line: &str) -> Output {
-        let fs = Rc::clone(&self.fs);
-        let make_node = move || {
-            fs.write_file(Path::new(NODE), "binary").unwrap();
-            fs.set_executable(Path::new(NODE));
-        };
-        let process = FakeProcess::default()
-            .with_success(&format!("{TOP}/configure"), PREFIX, "configured\n")
-            .with_success("make", "-j 7", "built\n")
-            .with_success("make", "-j 7 install", "installed\n")
-            .with_effect("make", "-j 7 install", make_node)
-            .with_success("make", "-j 4", "")
-            .with_success("make", "-j 4 install", "")
-            .with_effect("make", "-j 4 install", {
-                let fs = Rc::clone(&self.fs);
-                move || {
-                    fs.write_file(Path::new(NODE), "binary").unwrap();
-                    fs.set_executable(Path::new(NODE));
-                }
-            })
-            .with_success(
-                &format!("{TOP}/configure"),
-                &format!("{PREFIX} --with-intl=full-icu"),
-                "",
-            );
-        let archive = FakeArchive::new(&self.fs).with_archive(
-            SRC_TARBALL,
-            &[("node-v20.10.0/configure", "#!/bin/sh", true)],
-        );
-        let context = Context::new(self.fs.as_ref(), &self.env)
-            .with_http(&self.http)
-            .with_digest(&self.digest)
-            .with_archive(&archive)
-            .with_process(&process)
-            .with_cpu(&self.cpu);
-        let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
-        super::super::run(&context, &words).unwrap()
-    }
-}
 
 /// The expectations are what `nvm install` of the real `nvm.sh` printed with
 /// a mirror that has no binary, and a fake `./configure` and `make`.
@@ -256,106 +174,3 @@
     assert!(output.stderr.ends_with("nvm: install v20.10.0 failed!"));
     assert!(world.fs.file_info(Path::new(NODE)).is_err());
 }
-
-#[test]
-fn a_hook_installs_in_place_of_nvm_and_is_told_how() {
-    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
-    let fs = Rc::clone(&world.fs);
-    let process = FakeProcess::default()
-        .with_success(
-            "/opt/hook",
-            "v20.10.0 node std binary /n/versions/node/v20.10.0",
-            "",
-        )
-        .with_effect(
-            "/opt/hook",
-            "v20.10.0 node std binary /n/versions/node/v20.10.0",
-            move || {
-                fs.write_file(Path::new(NODE), "binary").unwrap();
-                fs.set_executable(Path::new(NODE));
-            },
-        );
-    let context = Context::new(world.fs.as_ref(), &world.env)
-        .with_http(&world.http)
-        .with_process(&process);
-    let output = super::super::run(&context, &["20".to_owned()]).unwrap();
-    assert_eq!(output.status, NvmExitCode::Success);
-    assert_eq!(
-        output.stderr,
-        "** $NVM_INSTALL_THIRD_PARTY_HOOK env var set; dispatching to third-party installation method **"
-    );
-    assert!(
-        !world
-            .http
-            .requests()
-            .iter()
-            .any(|url| url.contains(".tar.gz"))
-    );
-}
-
-#[test]
-fn a_hook_that_fails_or_installs_nothing_ends_the_install() {
-    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
-    let run_with = |process: &FakeProcess| {
-        let context = Context::new(world.fs.as_ref(), &world.env)
-            .with_http(&world.http)
-            .with_process(process);
-        super::super::run(&context, &["20".to_owned()]).unwrap()
-    };
-    let missing = run_with(&FakeProcess::default());
-    assert_eq!(missing.status, NvmExitCode::Failure);
-    assert!(
-        missing.stderr.ends_with(
-            "*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var failed to install! ***"
-        )
-    );
-    let silent = FakeProcess::default().with_success(
-        "/opt/hook",
-        "v20.10.0 node std binary /n/versions/node/v20.10.0",
-        "",
-    );
-    let claimed = run_with(&silent);
-    assert_eq!(claimed.status, NvmExitCode::HookClaimedSuccess);
-    assert!(claimed.stderr.ends_with("*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var claimed to succeed, but failed to install! ***"));
-}
-
-#[test]
-fn the_hook_is_told_source_when_s_is_given() {
-    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
-    let process = FakeProcess::default();
-    let context = Context::new(world.fs.as_ref(), &world.env)
-        .with_http(&world.http)
-        .with_process(&process);
-    super::super::run(&context, &["-s".to_owned(), "20".to_owned()]).unwrap();
-    let ran = process.executed();
-    assert_eq!(
-        ran[0].args,
-        [
-            "v20.10.0",
-            "node",
-            "std",
-            "source",
-            "/n/versions/node/v20.10.0"
-        ]
-    );
-}
-
-#[test]
-fn a_hook_that_fails_passes_its_own_status_on() {
-    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
-    let failed = crate::ports::Completed {
-        code: Some(7),
-        ..crate::ports::Completed::default()
-    };
-    let process = FakeProcess::default().with_execution(
-        "/opt/hook",
-        "v20.10.0 node std binary /n/versions/node/v20.10.0",
-        failed,
-    );
-    let context = Context::new(world.fs.as_ref(), &world.env)
-        .with_http(&world.http)
-        .with_process(&process);
-    let output = super::super::run(&context, &["20".to_owned()]).unwrap();
-    assert_eq!(output.status, NvmExitCode::Passed(7));
-    assert_eq!(output.status.code(), 7);
-}
```

Create `tests/common/mod.rs`:

```rust
//! What the end-to-end tests of `install` share: a mirror served from a local
//! socket that holds real archives, and the binary run against it.
//!
//! Every test crate uses some of this, so what one of them leaves out is not
//! dead code.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};
use std::thread;

use flate2::Compression;
use flate2::write::GzEncoder;
use nvmrc::domain::platform::Platform;
use sha2::{Digest, Sha256};

pub const NODE_INDEX: &str = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n\
v20.10.0\t2023-11-22\tx\t10.2.3\t11.3\t1.46\t1.3\t3.0\t115\tIron\t-\n\
v18.19.0\t2023-11-29\tx\t10.2.3\t10.2\t1.44\t1.3\t3.0\t108\tHydrogen\t-\n";
pub const IOJS_INDEX: &str =
    "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n";
const INHERITED_NETWORK_SETTINGS: [&str; 7] = [
    "ALL_PROXY",
    "all_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
    "NVM_AUTH_HEADER",
];

/// A file of an archive: its path, contents and permissions.
pub type Entry<'a> = (String, &'a str, u32);

/// The archive name nvmrc will ask for on this machine, or `None` where it has
/// no binaries (the tests then have nothing to check).
pub fn slug(version: &str) -> Option<String> {
    let platform = Platform::from_host(std::env::consts::OS, std::env::consts::ARCH, false)?;
    Some(platform.download_slug(&version.parse().unwrap()))
}

/// A gzip-compressed tar of `entries`.
pub fn tar_gz(entries: &[Entry<'_>]) -> Vec<u8> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    for (path, contents, mode) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(*mode);
        header.set_cksum();
        builder
            .append_data(&mut header, path, contents.as_bytes())
            .unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

/// `<slug>/bin/node` (executable, prints its version) and `<slug>/lib/README`,
/// plus `extra`, which is under `<slug>/` too.
pub fn tarball_with(slug: &str, version: &str, extra: &[(&str, &str, u32)]) -> Vec<u8> {
    let script = format!("#!/bin/sh\necho {version}\n");
    let mut entries: Vec<Entry<'_>> = vec![
        (format!("{slug}/bin/node"), &script, 0o755),
        (format!("{slug}/lib/README"), "readme", 0o644),
    ];
    entries.extend(
        extra
            .iter()
            .map(|(path, contents, mode)| (format!("{slug}/{path}"), *contents, *mode)),
    );
    tar_gz(&entries)
}

pub fn tarball(slug: &str, version: &str) -> Vec<u8> {
    tarball_with(slug, version, &[])
}

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Serves each path with its body and 404 for the rest; returns the URL.
pub fn serve(files: BTreeMap<String, Vec<u8>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).unwrap_or(0);
            let text = String::from_utf8_lossy(&request[..read]).into_owned();
            let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
            let (status, body) = match files.get(&path) {
                Some(body) => ("200 OK", body.clone()),
                None => ("404 Not Found", Vec::new()),
            };
            let head = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    format!("http://127.0.0.1:{port}")
}

/// A mirror under construction: the index, and per version the archives and
/// their checksums.
pub struct Mirror {
    files: BTreeMap<String, Vec<u8>>,
    sums: BTreeMap<String, String>,
}

impl Mirror {
    pub fn new() -> Self {
        let files = BTreeMap::from([("/index.tab".to_owned(), NODE_INDEX.as_bytes().to_vec())]);
        Self {
            files,
            sums: BTreeMap::new(),
        }
    }

    fn add(&mut self, version: &str, name: &str, bytes: Vec<u8>, listed: Option<&str>) {
        let checksum = listed.map_or_else(|| digest(&bytes), str::to_owned);
        let sums = self.sums.entry(version.to_owned()).or_default();
        sums.push_str(&format!("{checksum}  {name}\n"));
        self.files.insert(format!("/{version}/{name}"), bytes);
    }

    /// The prebuilt binary of `version`, with `extra` files in it; `listed`
    /// replaces the checksum in `SHASUMS256.txt`. `None` on a machine with no
    /// binaries.
    pub fn binary(
        mut self,
        version: &str,
        extra: &[(&str, &str, u32)],
        listed: Option<&str>,
    ) -> Option<Self> {
        let slug = slug(version)?;
        let archive = tarball_with(&slug, version, extra);
        self.add(version, &format!("{slug}.tar.gz"), archive, listed);
        Some(self)
    }

    /// The source archive of `version`, holding `files` under `node-<version>/`.
    pub fn source(mut self, version: &str, files: &[(&str, &str, u32)]) -> Self {
        let top = format!("node-{version}");
        let entries: Vec<Entry<'_>> = files
            .iter()
            .map(|(path, contents, mode)| (format!("{top}/{path}"), *contents, *mode))
            .collect();
        self.add(version, &format!("{top}.tar.gz"), tar_gz(&entries), None);
        self
    }

    pub fn serve(mut self) -> String {
        for (version, text) in std::mem::take(&mut self.sums) {
            self.files
                .insert(format!("/{version}/SHASUMS256.txt"), text.into_bytes());
        }
        serve(self.files)
    }
}

/// A mirror with Node 20.10.0 whose checksum is `listed` (or the right one).
pub fn mirror(listed: Option<&str>) -> Option<String> {
    Some(Mirror::new().binary("v20.10.0", &[], listed)?.serve())
}

/// Runs the binary with the environment `extra` added, and `path` as `PATH`.
pub fn nvm_with(
    nvm_dir: &Path,
    mirror: &str,
    args: &[&str],
    path: &str,
    extra: &[(&str, &str)],
) -> Output {
    let iojs = serve(BTreeMap::from([(
        "/index.tab".to_owned(),
        IOJS_INDEX.as_bytes().to_vec(),
    )]));
    let mut command = Command::new(env!("CARGO_BIN_EXE_nvm"));
    command
        .args(args)
        .env("NVM_DIR", nvm_dir)
        .env("PWD", nvm_dir)
        .env("PATH", path)
        .env("NVM_NODEJS_ORG_MIRROR", mirror)
        .env("NVM_IOJS_ORG_MIRROR", iojs)
        .env("NO_PROXY", "127.0.0.1")
        .current_dir(nvm_dir);
    for name in INHERITED_NETWORK_SETTINGS {
        command.env_remove(name);
    }
    for (name, value) in extra {
        command.env(name, value);
    }
    command.output().expect("run the binary")
}

pub fn nvm(nvm_dir: &Path, mirror: &str, args: &[&str]) -> Output {
    nvm_with(nvm_dir, mirror, args, "/nonexistent", &[])
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
```

Apply to `tests/install_cli.rs`:

```diff
--- a/tests/install_cli.rs
+++ b/tests/install_cli.rs
@@ -2,135 +2,12 @@
 //! temporary `$NVM_DIR` and a mirror served on a local port that holds a real
 //! `.tar.gz`.
 
-use std::collections::BTreeMap;
+mod common;
+
 use std::fs;
-use std::io::{Read, Write};
-use std::net::TcpListener;
-use std::path::Path;
-use std::process::{Command, Output};
-use std::thread;
+use std::process::Command;
 
-use flate2::Compression;
-use flate2::write::GzEncoder;
-use nvmrc::domain::platform::Platform;
-use sha2::{Digest, Sha256};
-
-const NODE_INDEX: &str = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n\
-v20.10.0\t2023-11-22\tx\t10.2.3\t11.3\t1.46\t1.3\t3.0\t115\tIron\t-\n\
-v18.19.0\t2023-11-29\tx\t10.2.3\t10.2\t1.44\t1.3\t3.0\t108\tHydrogen\t-\n";
-const IOJS_INDEX: &str =
-    "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n";
-const INHERITED_NETWORK_SETTINGS: [&str; 7] = [
-    "ALL_PROXY",
-    "all_proxy",
-    "HTTPS_PROXY",
-    "https_proxy",
-    "HTTP_PROXY",
-    "http_proxy",
-    "NVM_AUTH_HEADER",
-];
-
-/// The archive name nvmrc will ask for on this machine, or `None` where it has
-/// no binaries (the tests then have nothing to check).
-fn slug(version: &str) -> Option<String> {
-    let platform = Platform::from_host(std::env::consts::OS, std::env::consts::ARCH, false)?;
-    Some(platform.download_slug(&version.parse().unwrap()))
-}
-
-/// `<slug>/bin/node` (executable) and `<slug>/lib/README`, gzip-compressed.
-fn tarball(slug: &str, version: &str) -> Vec<u8> {
-    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
-    let script = format!("#!/bin/sh\necho {version}\n");
-    for (path, contents, mode) in [
-        (format!("{slug}/bin/node"), script.as_str(), 0o755),
-        (format!("{slug}/lib/README"), "readme", 0o644),
-    ] {
-        let mut header = tar::Header::new_gnu();
-        header.set_size(contents.len() as u64);
-        header.set_mode(mode);
-        header.set_cksum();
-        builder
-            .append_data(&mut header, path, contents.as_bytes())
-            .unwrap();
-    }
-    builder.into_inner().unwrap().finish().unwrap()
-}
-
-fn digest(bytes: &[u8]) -> String {
-    Sha256::digest(bytes)
-        .iter()
-        .map(|byte| format!("{byte:02x}"))
-        .collect()
-}
-
-/// Serves each path with its body and 404 for the rest; returns the URL.
-fn serve(files: BTreeMap<String, Vec<u8>>) -> String {
-    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
-    let port = listener.local_addr().unwrap().port();
-    thread::spawn(move || {
-        for stream in listener.incoming() {
-            let Ok(mut stream) = stream else { return };
-            let mut request = [0_u8; 2048];
-            let read = stream.read(&mut request).unwrap_or(0);
-            let text = String::from_utf8_lossy(&request[..read]).into_owned();
-            let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
-            let (status, body) = match files.get(&path) {
-                Some(body) => ("200 OK", body.clone()),
-                None => ("404 Not Found", Vec::new()),
-            };
-            let head = format!(
-                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
-                body.len()
-            );
-            let _ = stream.write_all(head.as_bytes());
-            let _ = stream.write_all(&body);
-        }
-    });
-    format!("http://127.0.0.1:{port}")
-}
-
-/// A mirror with Node 20.10.0 whose checksum is `listed` (or the right one).
-fn mirror(listed: Option<&str>) -> Option<String> {
-    let slug = slug("v20.10.0")?;
-    let archive = tarball(&slug, "v20.10.0");
-    let name = format!("{slug}.tar.gz");
-    let checksum = listed.map_or_else(|| digest(&archive), str::to_owned);
-    let mut files = BTreeMap::new();
-    files.insert("/index.tab".to_owned(), NODE_INDEX.as_bytes().to_vec());
-    files.insert(
-        "/v20.10.0/SHASUMS256.txt".to_owned(),
-        format!("{checksum}  {name}\n").into_bytes(),
-    );
-    files.insert(format!("/v20.10.0/{name}"), archive);
-    Some(serve(files))
-}
-
-fn nvm(nvm_dir: &Path, mirror: &str, args: &[&str]) -> Output {
-    let iojs = serve(BTreeMap::from([(
-        "/index.tab".to_owned(),
-        IOJS_INDEX.as_bytes().to_vec(),
-    )]));
-    let mut command = Command::new(env!("CARGO_BIN_EXE_nvm"));
-    command
-        .args(args)
-        .env("NVM_DIR", nvm_dir)
-        .env("PATH", "/nonexistent")
-        .env("NVM_NODEJS_ORG_MIRROR", mirror)
-        .env("NVM_IOJS_ORG_MIRROR", iojs)
-        .env("NO_PROXY", "127.0.0.1");
-    for name in INHERITED_NETWORK_SETTINGS {
-        command.env_remove(name);
-    }
-    command.output().expect("run the binary")
-}
-
-fn stdout(output: &Output) -> String {
-    String::from_utf8_lossy(&output.stdout).into_owned()
-}
-
-fn stderr(output: &Output) -> String {
-    String::from_utf8_lossy(&output.stderr).into_owned()
-}
+use common::{mirror, nvm, slug, stderr, stdout};
 
 #[test]
 fn install_puts_a_working_version_in_place_and_ls_sees_it() {
```

Create `tests/install_npm_cli.rs`:

```rust
//! End-to-end: what `install` does with `npm` once a version is in place
//! (`--latest-npm`, `default-packages`, `--reinstall-packages-from`) and the
//! commands `install-latest-npm` and `reinstall-packages`, against a mirror
//! whose archives hold a small shell script for `npm`.
#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use common::{Mirror, nvm_with, stderr, stdout};

/// An `npm` that remembers its version and every `install` it was asked for,
/// both next to itself, and lists the packages in `../packages.txt`.
const FAKE_NPM: &str = r#"#!/bin/sh
HERE=$(cd "$(dirname "$0")" && pwd)
STATE="$HERE/../npm-state"
mkdir -p "$STATE"
case "$1" in
  --version) cat "$STATE/version" 2>/dev/null || echo 10.2.3 ;;
  install)
    shift; shift
    echo "$*" >> "$STATE/installs"
    case "$1" in npm@*) echo "${1#npm@}.99.0" > "$STATE/version" ;; esac ;;
  list) if [ -f "$HERE/../packages.txt" ]; then cat "$HERE/../packages.txt"; else echo "$HERE/../lib"; fi ;;
  root) echo "$HERE/../lib/node_modules" ;;
  link) echo "linked $PWD" >> "$STATE/links" ;;
esac
"#;

fn mirror() -> Option<String> {
    let extra = [("bin/npm", FAKE_NPM, 0o755)];
    Some(Mirror::new().binary("v20.10.0", &extra, None)?.serve())
}

fn installs(home: &Path, version: &str) -> String {
    let file = home
        .join("versions/node")
        .join(version)
        .join("npm-state/installs");
    fs::read_to_string(file).unwrap_or_default()
}

fn path_of(home: &Path, version: &str) -> String {
    let bin = home.join("versions/node").join(version).join("bin");
    format!("{}:/usr/bin:/bin", bin.display())
}

#[test]
fn latest_npm_upgrades_the_npm_of_the_new_version() {
    let Some(mirror) = mirror() else { return };
    let home = tempfile::tempdir().unwrap();
    let output = nvm_with(
        home.path(),
        &mirror,
        &["install", "--latest-npm", "20"],
        "/usr/bin:/bin",
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let out = stdout(&output);
    assert!(out.contains("Attempting to upgrade to the latest working version of npm..."));
    assert!(out.contains("* `npm` `v10.x` is the last version that works on `node` `< v20.17`"));
    assert!(out.contains("* npm upgraded to: v10.99.0"));
    assert_eq!(installs(home.path(), "v20.10.0"), "npm@10\n");
}

#[test]
fn the_default_packages_are_installed_in_one_command() {
    let Some(mirror) = mirror() else { return };
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join("default-packages"),
        "# mine\nyarn\n\n@scope/tool\n",
    )
    .unwrap();
    let output = nvm_with(
        home.path(),
        &mirror,
        &["install", "20"],
        "/usr/bin:/bin",
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("npm install -g --quiet  yarn @scope/tool"));
    assert_eq!(
        installs(home.path(), "v20.10.0"),
        "--quiet yarn @scope/tool\n"
    );
}

#[test]
fn skip_default_packages_does_not_install_them() {
    let Some(mirror) = mirror() else { return };
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("default-packages"), "yarn\n").unwrap();
    let args = ["install", "--skip-default-packages", "20"];
    nvm_with(home.path(), &mirror, &args, "/usr/bin:/bin", &[]);
    assert_eq!(installs(home.path(), "v20.10.0"), "");
}

#[test]
fn reinstall_packages_from_another_version_installs_and_links_them() {
    let Some(mirror) = mirror() else { return };
    let home = tempfile::tempdir().unwrap();
    let old = home.path().join("versions/node/v18.19.0");
    fs::create_dir_all(old.join("bin")).unwrap();
    fs::write(old.join("bin/node"), "#!/bin/sh\necho v18.19.0\n").unwrap();
    fs::write(old.join("bin/npm"), FAKE_NPM).unwrap();
    for name in ["node", "npm"] {
        let path = old.join("bin").join(name);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let link_target = home.path().join("src/mylink");
    fs::create_dir_all(&link_target).unwrap();
    let listing = format!(
        "/x/lib\n├── yarn@1.22.19\n├── mylink@1.0.0 -> {}\n",
        link_target.display()
    );
    fs::write(old.join("packages.txt"), listing).unwrap();
    let args = ["install", "--reinstall-packages-from=18", "20"];
    let output = nvm_with(home.path(), &mirror, &args, "/usr/bin:/bin", &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("Reinstalling global packages from v18.19.0..."));
    assert_eq!(installs(home.path(), "v20.10.0"), "--quiet yarn@1.22.19\n");
    let links =
        fs::read_to_string(home.path().join("versions/node/v20.10.0/npm-state/links")).unwrap();
    assert!(links.starts_with("linked "));
}

#[test]
fn install_latest_npm_upgrades_the_npm_in_use() {
    let Some(mirror) = mirror() else { return };
    let home = tempfile::tempdir().unwrap();
    nvm_with(
        home.path(),
        &mirror,
        &["install", "20"],
        "/usr/bin:/bin",
        &[],
    );
    let path = path_of(home.path(), "v20.10.0");
    let output = nvm_with(home.path(), &mirror, &["install-latest-npm"], &path, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("* npm upgraded to: v10.99.0"));
}

#[test]
fn a_version_without_npm_skips_the_npm_steps_with_a_warning() {
    let Some(mirror) = common::mirror(None) else {
        return;
    };
    let home = tempfile::tempdir().unwrap();
    let args = ["install", "--latest-npm", "20"];
    let output = nvm_with(home.path(), &mirror, &args, "/usr/bin:/bin", &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stderr(&output).contains("npm was not found in v20.10.0; skipping the npm upgrade."));
}
```

Create `tests/install_source_cli.rs`:

```rust
//! End-to-end: building a version from source (`-s`, the fallback after a
//! failed binary, `-b`, `-j`) with a fake `./configure` in the source archive
//! and a fake `make` on `PATH`.
#![cfg(unix)]

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use common::{Mirror, nvm_with, stderr, stdout};

/// Remembers the prefix it was given, for the fake `make`.
const CONFIGURE: &str = r#"#!/bin/sh
echo "configure args: $*" >&2
PREFIX=
for arg in "$@"; do case "$arg" in --prefix=*) PREFIX="${arg#--prefix=}" ;; esac; done
printf 'PREFIX=%s\n' "$PREFIX" > Makefile
"#;

/// Logs what it was asked, and `install` puts a `node` in the prefix.
const MAKE: &str = r#"#!/bin/sh
echo "make $*" >> "$MAKE_LOG"
case " $* " in
  *" install "*)
    PREFIX=$(sed -n 's/^PREFIX=//p' Makefile)
    mkdir -p "$PREFIX/bin"
    printf '#!/bin/sh\necho built-from-source\n' > "$PREFIX/bin/node"
    chmod +x "$PREFIX/bin/node" ;;
esac
"#;

struct Setup {
    home: tempfile::TempDir,
    tools: tempfile::TempDir,
}

impl Setup {
    fn new() -> Self {
        let tools = tempfile::tempdir().unwrap();
        let make = tools.path().join("make");
        fs::write(&make, MAKE).unwrap();
        fs::set_permissions(&make, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            home: tempfile::tempdir().unwrap(),
            tools,
        }
    }

    fn log(&self) -> String {
        fs::read_to_string(self.tools.path().join("make.log")).unwrap_or_default()
    }

    fn run(&self, mirror: &str, args: &[&str]) -> std::process::Output {
        let path = format!("{}:/usr/bin:/bin", self.tools.path().display());
        let log = self.tools.path().join("make.log");
        let extra = [("MAKE_LOG", log.to_str().unwrap())];
        nvm_with(self.home.path(), mirror, args, &path, &extra)
    }
}

fn source_only() -> String {
    Mirror::new()
        .source("v20.10.0", &[("configure", CONFIGURE, 0o755)])
        .serve()
}

fn node(home: &Path) -> String {
    let node = home.join("versions/node/v20.10.0/bin/node");
    let ran = Command::new(node).output().unwrap();
    String::from_utf8_lossy(&ran.stdout).into_owned()
}

#[test]
fn s_builds_from_source_and_the_result_runs() {
    let setup = Setup::new();
    let output = setup.run(&source_only(), &["install", "-j", "3", "-s", "20"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let prefix = setup.home.path().join("versions/node/v20.10.0");
    let out = stdout(&output);
    assert!(out.starts_with("number of `make` jobs: 3\n"));
    assert!(out.contains(&format!("$>./configure --prefix={} <", prefix.display())));
    // macOS and the BSDs add `CC=cc CXX=c++`.
    let log = setup.log();
    let calls: Vec<&str> = log.lines().collect();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].starts_with("make -j 3") && !calls[0].ends_with("install"));
    assert!(calls[1].starts_with("make -j 3") && calls[1].ends_with(" install"));
    assert_eq!(node(setup.home.path()), "built-from-source\n");
}

#[test]
fn a_failed_binary_falls_back_to_source() {
    let setup = Setup::new();
    let output = setup.run(&source_only(), &["install", "-j", "2", "20"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stderr(&output).contains("Binary download failed, trying source."));
    assert_eq!(node(setup.home.path()), "built-from-source\n");
}

#[test]
fn b_does_not_build_after_a_failed_binary() {
    let setup = Setup::new();
    let output = setup.run(&source_only(), &["install", "-b", "20"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).ends_with("Binary download failed. Download from source aborted.\n"));
    assert_eq!(setup.log(), "");
}

#[test]
fn the_words_after_the_version_reach_configure() {
    let setup = Setup::new();
    let output = setup.run(
        &source_only(),
        &["install", "-j", "2", "-s", "20", "--ninja"],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("Additional options while compiling:  --ninja"));
    assert!(stderr(&output).contains("--ninja"));
}

#[test]
fn a_failing_make_is_status_1_and_installs_nothing() {
    let setup = Setup::new();
    let make = setup.tools.path().join("make");
    fs::write(&make, "#!/bin/sh\necho no good >&2\nexit 2\n").unwrap();
    let output = setup.run(&source_only(), &["install", "-j", "2", "-s", "20"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).ends_with("no good\nnvm: install v20.10.0 failed!\n"));
    assert!(!setup.home.path().join("versions").exists());
}

#[test]
fn the_third_party_hook_installs_in_place_of_nvm_and_its_status_is_passed_on() {
    let setup = Setup::new();
    let hook = setup.tools.path().join("hook");
    fs::write(&hook, "#!/bin/sh\necho \"hook: $*\"\nexit 9\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:/usr/bin:/bin", setup.tools.path().display());
    let extra = [("NVM_INSTALL_THIRD_PARTY_HOOK", hook.to_str().unwrap())];
    let output = nvm_with(
        setup.home.path(),
        &source_only(),
        &["install", "20"],
        &path,
        &extra,
    );
    assert_eq!(output.status.code(), Some(9));
    assert!(stdout(&output).starts_with("hook: v20.10.0 node std binary "));
    assert!(stderr(&output).ends_with(
        "*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var failed to install! ***\n"
    ));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile: the unit tests that moved into
`src/commands/install/tests/built.rs` and `hook.rs` use helpers (`split_off`,
`read_valued`, `choose_compiler` and others) that do not exist yet, and
`tests/common/mod.rs` is not declared by the old test files.

- [ ] **Step 3: Split the long functions and files**

Apply to `src/commands/install/acquire/hook.rs` (above the test module):

```diff
--- a/src/commands/install/acquire/hook.rs
+++ b/src/commands/install/acquire/hook.rs
@@ -9,6 +9,22 @@
 use crate::domain::version::Flavor;
 use crate::error::NvmExitCode;
 use crate::ports::Invocation;
+
+/// `<hook> <version> <flavor> std <method> <path>`.
+fn invocation(program: &str, options: &Options, target: &Target) -> Invocation {
+    let flavor = match target.version.flavor {
+        Flavor::Node => "node",
+        Flavor::IoJs => "iojs",
+    };
+    let method = if options.no_binary {
+        "source"
+    } else {
+        "binary"
+    };
+    let version = target.version.to_string();
+    let path = target.path.display().to_string();
+    Invocation::new(program).args(&[&version, flavor, "std", method, &path])
+}
 
 /// Runs the hook, and checks that it left a version behind.
 ///
@@ -23,19 +39,9 @@
     transcript: &mut Transcript,
 ) -> Step<()> {
     transcript.err("** $NVM_INSTALL_THIRD_PARTY_HOOK env var set; dispatching to third-party installation method **");
-    let flavor = match target.version.flavor {
-        Flavor::Node => "node",
-        Flavor::IoJs => "iojs",
-    };
-    let method = if options.no_binary {
-        "source"
-    } else {
-        "binary"
-    };
-    let version = target.version.to_string();
-    let path = target.path.display().to_string();
-    let invocation = Invocation::new(program).args(&[&version, flavor, "std", method, &path]);
-    let done = context.process().execute(&invocation);
+    let done = context
+        .process()
+        .execute(&invocation(program, options, target));
     if let Ok(done) = &done {
         done.stdout.lines().for_each(|line| transcript.out(line));
         done.stderr.lines().for_each(|line| transcript.err(line));
```

Apply to `src/commands/install/acquire/mod.rs` (above the test module):

```diff
--- a/src/commands/install/acquire/mod.rs
+++ b/src/commands/install/acquire/mod.rs
@@ -41,35 +41,54 @@
     transcript: &mut Transcript,
 ) -> Step<()> {
     let version = &target.version;
+    check_layout(version, transcript)?;
+    if let Some(program) = third_party_hook(context) {
+        return hook::run(context, &program, options, target, transcript);
+    }
+    let _lock = take_lock(context, version, transcript)?;
+    let binary = if options.no_binary {
+        None
+    } else {
+        Some(install_binary(context, target, options.offline, transcript))
+    };
+    if matches!(binary, Some(Binary::Installed)) {
+        return Ok(());
+    }
+    if options.no_source {
+        return Err(without_source(binary, version, transcript));
+    }
+    if matches!(binary, Some(Binary::Failed)) {
+        transcript.err("Binary download failed, trying source.");
+    }
+    from_source(context, options, target, transcript)
+}
+
+/// `NVM_INSTALL_THIRD_PARTY_HOOK`, when it is set to something.
+fn third_party_hook(context: &Context<'_>) -> Option<String> {
+    context
+        .env
+        .var("NVM_INSTALL_THIRD_PARTY_HOOK")
+        .filter(|hook| !hook.is_empty())
+}
+
+/// Node before 0.12 lives in `$NVM_DIR/<version>`, a layout `nvm` lists
+/// nowhere else here.
+fn check_layout(version: &Version, transcript: &mut Transcript) -> Step<()> {
     if version.flavor == Flavor::Node && version.triple() < (0, 12, 0) {
         transcript.err(format!(
             "Versions before v0.12.0 use the legacy layout, which is not supported: {version}"
         ));
         return Err(Halt::Exit(NvmExitCode::InvalidVersion));
     }
-    if let Some(program) = context
-        .env
-        .var("NVM_INSTALL_THIRD_PARTY_HOOK")
-        .filter(|hook| !hook.is_empty())
-    {
-        return hook::run(context, &program, options, target, transcript);
-    }
-    let _lock = take_lock(context, version, transcript)?;
-    let binary = if options.no_binary {
-        None
-    } else {
-        Some(install_binary(context, target, options.offline, transcript))
-    };
-    if matches!(binary, Some(Binary::Installed)) {
-        return Ok(());
-    }
-    if options.no_source {
-        return Err(without_source(binary, version, transcript));
-    }
-    if matches!(binary, Some(Binary::Failed)) {
-        transcript.err("Binary download failed, trying source.");
-    }
-    from_source(context, options, target, transcript)
+    Ok(())
+}
+
+/// The binary archive of `version`, when this machine and version have one.
+fn binary_artifact(context: &Context<'_>, version: &Version) -> Option<Artifact> {
+    let has_binaries = context
+        .platform()
+        .is_some_and(|platform| platform.os.has_binaries());
+    Artifact::of(context, version).filter(|_| has_binaries && binary_available(version))
 }
 
 /// `-b`: the end of the road, with what `nvm.sh` says about the binary.
@@ -89,12 +108,7 @@
     transcript: &mut Transcript,
 ) -> Binary {
     let version = &target.version;
-    let has_binaries = context
-        .platform()
-        .is_some_and(|platform| platform.os.has_binaries());
-    let Some(artifact) =
-        Artifact::of(context, version).filter(|_| has_binaries && binary_available(version))
-    else {
+    let Some(artifact) = binary_artifact(context, version) else {
         return Binary::Unavailable;
     };
     let name = match version.flavor {
```

Apply to `src/commands/install/lock/mod.rs` (above the test module):

```diff
--- a/src/commands/install/lock/mod.rs
+++ b/src/commands/install/lock/mod.rs
@@ -77,7 +77,8 @@
     request: &LockRequest<'_>,
     notes: &mut Vec<String>,
 ) -> Result<Option<InstallLock<'a>>, CliError> {
-    if fs.create_dir_all(request.root).is_err() {
+    let root_exists = fs.create_dir_all(request.root).is_ok();
+    if !root_exists {
         return Ok(None);
     }
     let path = request.root.join(lock_name(request.version));
@@ -88,26 +89,44 @@
             Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Ok(None),
             Err(_) => {}
         }
-        if is_stale(fs, &path, request, waited) {
-            notes.push(format!(
-                "Removing stale install lock for {} (older than {} minute(s))",
-                request.version, request.stale_minutes
-            ));
-            let _ = fs.remove_dir_all(&path);
+        if steal_if_stale(fs, &path, request, waited, notes) {
             continue;
         }
         if waited >= request.timeout_seconds {
             return Err(timed_out(request, &path));
         }
         if waited == 0 {
-            notes.push(format!(
-                "Waiting for another install of {} to finish...",
-                request.version
-            ));
+            notes.push(waiting(request));
         }
         sleeper.sleep(Duration::from_secs(1));
         waited += 1;
     }
+}
+
+/// An abandoned lock is removed, with a note; true when it was.
+fn steal_if_stale(
+    fs: &dyn FileSystem,
+    path: &Path,
+    request: &LockRequest<'_>,
+    waited: u64,
+    notes: &mut Vec<String>,
+) -> bool {
+    if !is_stale(fs, path, request, waited) {
+        return false;
+    }
+    notes.push(format!(
+        "Removing stale install lock for {} (older than {} minute(s))",
+        request.version, request.stale_minutes
+    ));
+    let _ = fs.remove_dir_all(path);
+    true
+}
+
+fn waiting(request: &LockRequest<'_>) -> String {
+    format!(
+        "Waiting for another install of {} to finish...",
+        request.version
+    )
 }
 
 fn timed_out(request: &LockRequest<'_>, path: &Path) -> CliError {
```

Apply to `src/commands/install/npm_steps/mod.rs` (above the test module):

```diff
--- a/src/commands/install/npm_steps/mod.rs
+++ b/src/commands/install/npm_steps/mod.rs
@@ -2,6 +2,8 @@
 //! of `nvm.sh`: `--latest-npm`, then the default packages. A version without
 //! an `npm` skips these steps with a warning (`nvm.sh` would download an `npm`
 //! installer from the internet and run it; this port never does).
+
+use std::path::Path;
 
 use crate::commands::install::flow::{Step, Target};
 use crate::commands::install::options::Options;
@@ -84,6 +86,26 @@
     install_latest(context, npm, &Node::Version(&text), transcript)
 }
 
+/// The packages the file lists, joined as `nvm.sh` does; `None` when there is
+/// no file or nothing in it, and the failing status when a line is wrong.
+fn listed_packages(
+    context: &Context<'_>,
+    file: &Path,
+    shown: &str,
+    transcript: &mut Transcript,
+) -> Result<Option<String>, NvmExitCode> {
+    let Ok(contents) = context.fs.read_to_string(file) else {
+        return Ok(None);
+    };
+    match default_packages::parse(&contents, shown) {
+        Ok(joined) => Ok(Some(joined).filter(|joined| !joined.is_empty())),
+        Err(error) => {
+            transcript.err(error.to_string());
+            Err(NvmExitCode::Failure)
+        }
+    }
+}
+
 /// `nvm_install_default_packages`: one `npm install -g --quiet` for the lot.
 fn default_packages(
     context: &Context<'_>,
@@ -92,20 +114,12 @@
     transcript: &mut Transcript,
 ) -> Step<NvmExitCode> {
     let file = context.nvm_dir()?.join("default-packages");
-    let Ok(contents) = context.fs.read_to_string(&file) else {
-        return Ok(NvmExitCode::Success);
+    let shown = file.display().to_string();
+    let joined = match listed_packages(context, &file, &shown, transcript) {
+        Ok(Some(joined)) => joined,
+        Ok(None) => return Ok(NvmExitCode::Success),
+        Err(status) => return Ok(status),
     };
-    let shown = file.display().to_string();
-    let joined = match default_packages::parse(&contents, &shown) {
-        Ok(joined) => joined,
-        Err(error) => {
-            transcript.err(error.to_string());
-            return Ok(NvmExitCode::Failure);
-        }
-    };
-    if joined.is_empty() {
-        return Ok(NvmExitCode::Success);
-    }
     let Some(npm) = npm else {
         return Ok(skip(version, "the default packages", transcript));
     };
```

Apply to `src/commands/install/options/mod.rs` (above the test module):

```diff
--- a/src/commands/install/options/mod.rs
+++ b/src/commands/install/options/mod.rs
@@ -94,46 +94,67 @@
     let mut options = Options::default();
     let mut rest = args.iter().peekable();
     while let Some(option) = rest.next_if(|arg| is_option(arg)) {
-        match option.as_str() {
-            "--no-progress" => {}
-            "-s" => set_build_mode(&mut options, true)?,
-            "-b" => set_build_mode(&mut options, false)?,
-            "-j" => options.make_jobs = Some(rest.next().cloned().unwrap_or_default()),
-            "--latest-npm" => options.latest_npm = true,
-            "--offline" => options.offline = true,
-            "--save" | "-w" => set_save(&mut options)?,
-            "--skip-default-packages" => options.skip_default_packages = true,
-            "--lts" => options.lts = Some("*".to_owned()),
-            "--default" => set_alias(&mut options, "default")?,
-            other if other.starts_with("--lts=") => {
-                options.lts = Some(other["--lts=".len()..].to_owned());
-            }
-            other if other.starts_with("--alias=") => {
-                set_alias(&mut options, &other["--alias=".len()..])?;
-            }
-            other if other.starts_with("---") => {
-                let message = "arguments with `---` are not supported - this is likely a typo";
-                return Err(CliError::Unsupported(message.to_owned()));
-            }
-            other => {
-                read_reinstall(&mut options, other)?;
-            }
-        }
+        read_leading(&mut options, option, &mut rest)?;
     }
     if let Some(version) = rest.next() {
         options.version.clone_from(version);
         options.version_given = true;
     }
     for option in rest {
-        if option == "--skip-default-packages" {
-            options.skip_default_packages = true;
-        } else if !read_reinstall(&mut options, option)? {
-            options.extra.push(option.clone());
-        }
+        read_trailing(&mut options, option)?;
     }
     options.announce_lts = options.lts.is_some() && options.version.is_empty();
     take_lts_version(&mut options);
     Ok(options)
+}
+
+/// One option before the version; `-j` takes the next word as its value.
+fn read_leading(
+    options: &mut Options,
+    option: &str,
+    rest: &mut std::iter::Peekable<std::slice::Iter<'_, String>>,
+) -> Result<(), CliError> {
+    match option {
+        "--no-progress" => {}
+        "-s" => set_build_mode(options, true)?,
+        "-b" => set_build_mode(options, false)?,
+        "-j" => options.make_jobs = Some(rest.next().cloned().unwrap_or_default()),
+        "--latest-npm" => options.latest_npm = true,
+        "--offline" => options.offline = true,
+        "--save" | "-w" => set_save(options)?,
+        "--skip-default-packages" => options.skip_default_packages = true,
+        "--lts" => options.lts = Some("*".to_owned()),
+        "--default" => set_alias(options, "default")?,
+        other => read_valued(options, other)?,
+    }
+    Ok(())
+}
+
+/// The options that carry a value (`--lts=`, `--alias=`, the reinstall pair),
+/// and `---x`, which is a typo.
+fn read_valued(options: &mut Options, option: &str) -> Result<(), CliError> {
+    if let Some(name) = option.strip_prefix("--lts=") {
+        options.lts = Some(name.to_owned());
+    } else if let Some(name) = option.strip_prefix("--alias=") {
+        set_alias(options, name)?;
+    } else if option.starts_with("---") {
+        let message = "arguments with `---` are not supported - this is likely a typo";
+        return Err(CliError::Unsupported(message.to_owned()));
+    } else {
+        read_reinstall(options, option)?;
+    }
+    Ok(())
+}
+
+/// One word after the version: an option that is read there, or an argument
+/// for `./configure`.
+fn read_trailing(options: &mut Options, option: &str) -> Result<(), CliError> {
+    if option == "--skip-default-packages" {
+        options.skip_default_packages = true;
+    } else if !read_reinstall(options, option)? {
+        options.extra.push(option.to_owned());
+    }
+    Ok(())
 }
 
 /// A word that `nvm.sh` reads as an option rather than as the version.
```

Apply to `src/commands/install/source/mod.rs` (above the test module):

```diff
--- a/src/commands/install/source/mod.rs
+++ b/src/commands/install/source/mod.rs
@@ -10,7 +10,7 @@
 use crate::context::Context;
 use crate::domain::path_search::find_in_path;
 use crate::domain::platform::Os;
-use crate::domain::source_build::{clang_version, compiler, make};
+use crate::domain::source_build::{Compiler, clang_version, compiler, make};
 use crate::domain::version::Version;
 use crate::ports::Invocation;
 
@@ -80,6 +80,23 @@
         transcript.out(format!("Additional options while compiling: {params}"));
     }
     let os = context.platform().map_or(Os::Linux, |platform| platform.os);
+    let toolchain = choose_compiler(context, os, transcript);
+    let artifact = Artifact::source_of(context, job.version).ok_or(BuildFailed)?;
+    let tarball =
+        fetch(context, &artifact, job.version, job.offline, transcript).map_err(|_| BuildFailed)?;
+    let files = artifact.files();
+    unpack_source(context, &tarball, &files)
+        .map_err(|message| failed(context, job, &files, message, transcript))?;
+    let tools = Toolchain {
+        os,
+        compiler_words: &toolchain.words,
+    };
+    compile(context, job, &files, &params, &tools, transcript)
+        .map_err(|()| failed(context, job, &files, String::new(), transcript))
+}
+
+/// The `CC=` and `CXX=` words, and the line about Clang when it was chosen.
+fn choose_compiler(context: &Context<'_>, os: Os, transcript: &mut Transcript) -> Compiler {
     let toolchain = compiler(
         os,
         clang(context),
@@ -91,22 +108,7 @@
             "Clang v3.5+ detected! CC or CXX not specified, will use Clang as C/C++ compiler!",
         );
     }
-    let artifact = Artifact::source_of(context, job.version).ok_or(BuildFailed)?;
-    let tarball =
-        fetch(context, &artifact, job.version, job.offline, transcript).map_err(|_| BuildFailed)?;
-    let files = artifact.files();
-    unpack_source(context, &tarball, &files)
-        .map_err(|message| failed(context, job, &files, message, transcript))?;
-    compile(
-        context,
-        job,
-        &files,
-        &params,
-        &toolchain.words,
-        os,
-        transcript,
-    )
-    .map_err(|()| failed(context, job, &files, String::new(), transcript))
+    toolchain
 }
 
 /// The source tree is `files` itself, the archive's top-level directory
@@ -148,47 +150,63 @@
     BuildFailed
 }
 
-/// `./configure`, `make`, then `make install`.
-fn compile(
-    context: &Context<'_>,
-    job: &Build<'_>,
-    top: &Path,
-    params: &str,
-    compiler_words: &[String],
+/// What `make` needs to know about the machine.
+struct Toolchain<'a> {
     os: Os,
-    transcript: &mut Transcript,
-) -> Result<(), ()> {
-    let prefix = format!("--prefix={}", job.version_path.display());
-    let words: Vec<&str> = params.split_whitespace().collect();
-    // `nvm.sh` writes `$ADDITIONAL_PARAMETERS'<'`: the `<` sticks to the last
-    // option, and stands alone when there are none.
-    let mut shown = vec!["$>./configure".to_owned(), prefix.clone()];
+    compiler_words: &'a [String],
+}
+
+/// `$>./configure --prefix=<path> <options>`, as `nvm.sh` writes it: the `<`
+/// sticks to the last option, and stands alone when there are none.
+fn configure_line(prefix: &str, words: &[&str]) -> String {
+    let mut shown = vec!["$>./configure".to_owned(), prefix.to_owned()];
     shown.extend(words.iter().map(|word| (*word).to_owned()));
     if words.is_empty() {
         shown.push("<".to_owned());
     } else if let Some(last) = shown.last_mut() {
         last.push('<');
     }
-    transcript.out(shown.join(" "));
+    shown.join(" ")
+}
+
+/// `make -j <jobs> <compiler words> [goal]`, in the source tree.
+fn make_invocation(
+    job: &Build<'_>,
+    tools: &Toolchain<'_>,
+    top: &Path,
+    goal: Option<&str>,
+) -> Invocation {
+    let (program, shell) = make(tools.os, job.version);
+    let jobs = job.jobs.to_string();
+    let mut args: Vec<&str> = shell.iter().map(String::as_str).collect();
+    args.extend(["-j", &jobs]);
+    args.extend(tools.compiler_words.iter().map(String::as_str));
+    args.extend(goal);
+    Invocation::new(program).args(&args).dir(top)
+}
+
+/// `./configure`, `make`, then `make install`.
+fn compile(
+    context: &Context<'_>,
+    job: &Build<'_>,
+    top: &Path,
+    params: &str,
+    tools: &Toolchain<'_>,
+    transcript: &mut Transcript,
+) -> Result<(), ()> {
+    let prefix = format!("--prefix={}", job.version_path.display());
+    let words: Vec<&str> = params.split_whitespace().collect();
+    transcript.out(configure_line(&prefix, &words));
     let configure = Invocation::new(top.join("configure"))
         .args(&[&prefix])
         .args(&words)
         .dir(top);
-    let (program, shell) = make(os, job.version);
-    let jobs = job.jobs.to_string();
-    let make_args = |goal: Option<&str>| {
-        let mut args: Vec<&str> = shell.iter().map(String::as_str).collect();
-        args.extend(["-j", &jobs]);
-        args.extend(compiler_words.iter().map(String::as_str));
-        args.extend(goal);
-        Invocation::new(program).args(&args).dir(top)
-    };
-    if !run(context, &configure, transcript) || !run(context, &make_args(None), transcript) {
+    let make_all = make_invocation(job, tools, top, None);
+    if !run(context, &configure, transcript) || !run(context, &make_all, transcript) {
         return Err(());
     }
     let _ = context.fs.remove_file(job.version_path);
-    run(context, &make_args(Some("install")), transcript)
-        .then_some(())
-        .ok_or(())
-}
-
+    let install = make_invocation(job, tools, top, Some("install"));
+    run(context, &install, transcript).then_some(()).ok_or(())
+}
+
```

Apply to `src/commands/npm/latest/mod.rs` (above the test module):

```diff
--- a/src/commands/npm/latest/mod.rs
+++ b/src/commands/npm/latest/mod.rs
@@ -31,16 +31,7 @@
 ) -> NvmExitCode {
     transcript.out("Attempting to upgrade to the latest working version of npm...");
     let npm_version = npm.and_then(|npm| npm.version(context));
-    let node_text = match node {
-        Node::Version(text) => Some(text.strip_prefix("iojs-").unwrap_or(text)),
-        Node::None => {
-            let shown = npm_version.as_deref().unwrap_or_default();
-            transcript.out(format!("Detected node version none, npm version v{shown}"));
-            None
-        }
-    };
-    let Some(node_text) = node_text.filter(|text| triple(text).is_some()) else {
-        transcript.err("Unable to obtain node version.");
+    let Some(node_text) = known_node(node, npm_version.as_deref(), transcript) else {
         return NvmExitCode::Failure;
     };
     let (Some(npm), Some(npm_version)) = (npm, npm_version) else {
@@ -53,9 +44,46 @@
             "Detected node version {node_text}, npm version v{npm_version}"
         ));
     }
+    upgrade(context, npm, (node_text, &npm_version), debug, transcript);
+    let upgraded = npm.version(context).unwrap_or_default();
+    transcript.out(format!("* npm upgraded to: v{upgraded}"));
+    NvmExitCode::Success
+}
+
+/// The node version to go by, without an `iojs-`; none (after saying so) when
+/// there is no node, or what it says is not a version.
+fn known_node<'a>(
+    node: &Node<'a>,
+    npm_version: Option<&str>,
+    transcript: &mut Transcript,
+) -> Option<&'a str> {
+    let text = match node {
+        Node::Version(text) => Some(text.strip_prefix("iojs-").unwrap_or(text)),
+        Node::None => {
+            let shown = npm_version.unwrap_or_default();
+            transcript.out(format!("Detected node version none, npm version v{shown}"));
+            None
+        }
+    };
+    let text = text.filter(|text| triple(text).is_some());
+    if text.is_none() {
+        transcript.err("Unable to obtain node version.");
+    }
+    text
+}
+
+/// Says why and installs, step by step, for the `(node, npm)` versions.
+fn upgrade(
+    context: &Context<'_>,
+    npm: &Npm,
+    versions: (&str, &str),
+    debug: bool,
+    transcript: &mut Transcript,
+) {
+    let (node, current) = versions;
     let plan = steps(
-        triple(node_text).unwrap_or_default(),
-        triple(&npm_version).unwrap_or_default(),
+        triple(node).unwrap_or_default(),
+        triple(current).unwrap_or_default(),
     );
     for step in plan {
         transcript.out(step.note);
@@ -65,9 +93,6 @@
             Install::Spec(spec) => install(context, npm, spec, debug, transcript),
         }
     }
-    let upgraded = npm.version(context).unwrap_or_default();
-    transcript.out(format!("* npm upgraded to: v{upgraded}"));
-    NvmExitCode::Success
 }
 
 fn install(context: &Context<'_>, npm: &Npm, spec: &str, debug: bool, transcript: &mut Transcript) {
```

Apply to `src/commands/npm/packages/mod.rs` (above the test module):

```diff
--- a/src/commands/npm/packages/mod.rs
+++ b/src/commands/npm/packages/mod.rs
@@ -21,7 +21,7 @@
 }
 
 impl Source {
-    fn label(&self) -> String {
+    pub fn label(&self) -> String {
         match self {
             Self::Version(version) => version.to_string(),
             Self::System => "system".to_owned(),
@@ -80,6 +80,23 @@
     link(context, destination, &packages.links, transcript)
 }
 
+/// Where `npm link` runs for a linked package: an absolute target as it is, a
+/// relative one from the global `node_modules` (as `nvm.sh` joins them).
+fn link_directory(root: &str, target: &str) -> String {
+    if target.starts_with('/') {
+        target.to_owned()
+    } else {
+        format!("{root}/../{target}")
+    }
+}
+
+/// `npm root -g`: where the global packages are.
+fn global_root(context: &Context<'_>, npm: &Npm) -> String {
+    npm.output(context, &["root", "-g"])
+        .map(|text| text.trim().to_owned())
+        .unwrap_or_default()
+}
+
 fn link(
     context: &Context<'_>,
     destination: &Npm,
@@ -90,17 +107,10 @@
         transcript.out("No linked global packages found...");
         return NvmExitCode::Success;
     }
-    let root = destination
-        .output(context, &["root", "-g"])
-        .map(|text| text.trim().to_owned())
-        .unwrap_or_default();
+    let root = global_root(context, destination);
     let mut status = NvmExitCode::Success;
     for target in links.iter().filter(|target| !target.is_empty()) {
-        let directory = if target.starts_with('/') {
-            target.clone()
-        } else {
-            format!("{root}/../{target}")
-        };
+        let directory = link_directory(&root, target);
         let linked = destination.run_in(
             context,
             std::path::Path::new(&directory),
```

Apply to `src/commands/reinstall_packages/mod.rs` (above the test module):

```diff
--- a/src/commands/reinstall_packages/mod.rs
+++ b/src/commands/reinstall_packages/mod.rs
@@ -19,23 +19,12 @@
         return Err(CliError::Usage(usage));
     };
     let active = current::detect(context)?.to_string();
-    let resolved = resolve_installed(context, provided)?;
-    let named = match &resolved {
-        Resolved::Installed(version) => version.to_string(),
-        Resolved::System => "system".to_owned(),
-        Resolved::Missing { .. } => "N/A".to_owned(),
-    };
+    let source = source_of(provided, resolve_installed(context, provided)?);
     let mut transcript = Transcript::default();
-    if *provided == active || named == active {
+    if *provided == active || source.label() == active {
         transcript.err("Can not reinstall packages from the current version of node.");
         return Ok(transcript.finish(NvmExitCode::MissingTarget));
     }
-    let source = match resolved {
-        Resolved::Installed(version) => Source::Version(version),
-        Resolved::System => Source::System,
-        Resolved::Missing { .. } if provided == "system" => Source::System,
-        Resolved::Missing { .. } => Source::Missing,
-    };
     if source == Source::System && system_node(context)?.is_none() {
         transcript.err("No system version of node or io.js detected.");
         return Ok(transcript.finish(NvmExitCode::InvalidVersion));
@@ -49,3 +38,13 @@
     Ok(transcript.finish(status))
 }
 
+/// Where the packages come from: what was asked for, once resolved.
+fn source_of(provided: &str, resolved: Resolved) -> Source {
+    match resolved {
+        Resolved::Installed(version) => Source::Version(version),
+        Resolved::System => Source::System,
+        Resolved::Missing { .. } if provided == "system" => Source::System,
+        Resolved::Missing { .. } => Source::Missing,
+    }
+}
+
```

Apply to `src/domain/npm/upgrade/mod.rs` (above the test module):

```diff
--- a/src/domain/npm/upgrade/mod.rs
+++ b/src/domain/npm/upgrade/mod.rs
@@ -42,25 +42,32 @@
     } else if !is_0_9 {
         plan.extend(first_jump(npm));
     }
-    if is_0_6 || is_0_9 {
-        plan.push(Step {
+    plan.extend(then_the_newest(node, npm, is_0_6 || is_0_9));
+    plan
+}
+
+/// What follows the first hop: nothing for `node` 0.6 and 0.9, a fixed `npm`
+/// for the oldest ones, and the table for `node` 4 and later.
+fn then_the_newest(node: Triple, npm: Triple, cannot_go_further: bool) -> Vec<Step> {
+    if cannot_go_further {
+        return vec![Step {
             note: "* node v0.6 and v0.9 are unable to upgrade further",
             install: Install::Nothing,
-        });
-    } else if node < (1, 1, 0) {
-        plan.push(step(
+        }];
+    }
+    if node < (1, 1, 0) {
+        return vec![step(
             "* `npm` v4.5.x is the last version that works on `node` versions < v1.1.0",
             "npm@4.5",
-        ));
-    } else if node < (4, 0, 0) {
-        plan.push(step(
+        )];
+    }
+    if node < (4, 0, 0) {
+        return vec![step(
             "* `npm` v5 and higher do not work on `node` versions below v4.0.0",
             "npm@4",
-        ));
-    } else {
-        plan.extend(modern(node, npm));
+        )];
     }
-    plan
+    modern(node, npm)
 }
 
 /// `npm` 1.x and 2.x have to hop through their last release first.
```

Apply to `src/domain/source_build/mod.rs` (above the test module):

```diff
--- a/src/domain/source_build/mod.rs
+++ b/src/domain/source_build/mod.rs
@@ -24,30 +24,32 @@
 /// else one fewer than the cores when there are more than two.
 #[must_use]
 pub fn jobs(requested: Option<&str>, cores: Option<usize>) -> Jobs {
+    if let Some(number) = requested.and_then(natural_jobs) {
+        return Jobs {
+            jobs: number,
+            stdout: vec![format!("number of `make` jobs: {number}")],
+            stderr: Vec::new(),
+        };
+    }
+    let mut said = from_cores(cores);
+    if let Some(bad) = requested.filter(|text| !text.is_empty()) {
+        said.stderr.insert(
+            0,
+            format!("{bad} is invalid for number of `make` jobs, must be a natural number"),
+        );
+    }
+    said
+}
+
+/// What to do when the number of jobs is left to the machine.
+fn from_cores(cores: Option<usize>) -> Jobs {
     let mut said = Jobs {
         jobs: 1,
         stdout: Vec::new(),
         stderr: Vec::new(),
     };
-    if let Some(number) = requested.and_then(natural_jobs) {
-        said.jobs = number;
-        said.stdout.push(format!("number of `make` jobs: {number}"));
-        return said;
-    }
-    if let Some(bad) = requested.filter(|text| !text.is_empty()) {
-        said.stderr.push(format!(
-            "{bad} is invalid for number of `make` jobs, must be a natural number"
-        ));
-    }
     let Some(cores) = cores.filter(|cores| *cores > 0) else {
-        said.stderr.push(
-            "Can not determine how many core(s) are available, running in single-threaded mode."
-                .to_owned(),
-        );
-        said.stderr.push(
-            "Please report an issue on GitHub to help us make nvm run faster on your computer!"
-                .to_owned(),
-        );
+        said.stderr.extend(unknown_cores());
         return said;
     };
     said.stdout
@@ -65,6 +67,15 @@
         );
     }
     said
+}
+
+fn unknown_cores() -> [String; 2] {
+    [
+        "Can not determine how many core(s) are available, running in single-threaded mode."
+            .to_owned(),
+        "Please report an issue on GitHub to help us make nvm run faster on your computer!"
+            .to_owned(),
+    ]
 }
 
 /// The version `clang --version` reports as `(major, minor)`: the word after
```

Apply to `src/fakes/file_system/mod.rs` (above the test module):

```diff
--- a/src/fakes/file_system/mod.rs
+++ b/src/fakes/file_system/mod.rs
@@ -47,6 +47,19 @@
     pub fn with_dir(mut self, path: &str) -> Self {
         self.dirs.get_mut().insert(PathBuf::from(path));
         self
+    }
+}
+
+/// The paths of `all` that are `from` or inside it.
+fn under<'a>(all: impl Iterator<Item = &'a PathBuf>, from: &Path) -> Vec<PathBuf> {
+    all.filter(|path| path.starts_with(from)).cloned().collect()
+}
+
+/// Renames, in a set of paths, those that are `from` or inside it.
+fn move_paths(set: &mut BTreeSet<PathBuf>, from: &Path, moved: &dyn Fn(&Path) -> PathBuf) {
+    for name in under(set.iter(), from) {
+        set.remove(&name);
+        set.insert(moved(&name));
     }
 }
 
@@ -143,36 +156,13 @@
         }
         let moved = |old: &Path| to.join(old.strip_prefix(from).unwrap_or(old));
         let mut files = self.files.borrow_mut();
-        let names: Vec<PathBuf> = files
-            .keys()
-            .filter(|f| f.starts_with(from))
-            .cloned()
-            .collect();
-        for name in names {
+        for name in under(files.keys(), from) {
             if let Some(contents) = files.remove(&name) {
                 files.insert(moved(&name), contents);
             }
         }
-        let mut executables = self.executables.borrow_mut();
-        let names: Vec<PathBuf> = executables
-            .iter()
-            .filter(|e| e.starts_with(from))
-            .cloned()
-            .collect();
-        for name in names {
-            executables.remove(&name);
-            executables.insert(moved(&name));
-        }
-        let mut dirs = self.dirs.borrow_mut();
-        let names: Vec<PathBuf> = dirs
-            .iter()
-            .filter(|d| d.starts_with(from))
-            .cloned()
-            .collect();
-        for name in names {
-            dirs.remove(&name);
-            dirs.insert(moved(&name));
-        }
+        move_paths(&mut self.executables.borrow_mut(), from, &moved);
+        move_paths(&mut self.dirs.borrow_mut(), from, &moved);
         Ok(())
     }
 
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 586 unit tests plus 7 + 6 + 6 + 6 + 10 + 5 + 1 end-to-end
tests on Linux (the `install_npm_cli` and `install_source_cli` files are Unix
only).

- [ ] **Step 5: Run the full quality gate**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
rustup run 1.88 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings, every test passing, no
advisories, and the crate still builds on MSRV 1.88.

Also check that no file is over 300 lines and that no function is over 30:

```bash
find src tests -name '*.rs' | xargs wc -l | sort -n | tail -3
```

- [ ] **Step 6: Run the tests on Linux**

Run:

```bash
docker buildx build -f Dockerfile -t nvmrc:trixie .
docker run --rm nvmrc:trixie
```

Expected: the same test counts, all passing.

- [ ] **Step 7: Commit**

```bash
git add src tests
git commit -S -m "refactor: keep functions under 30 lines and share the end-to-end helpers"
```

---

## Environment variables

Every variable below was read in `nvm.sh` and, where it changes what is
printed or run, exercised against the real script. "Makes sense" is a verdict
on whether this port should honour it as `nvm.sh` does.

- `NVM_DIR`: the root of everything (versions, aliases, cache, locks,
  `default-packages`, `min-version`). It defaults to `$HOME/.nvm` here;
  `nvm.sh` derives it from where the script lives, which has no equivalent for
  a binary. Makes sense.
- `HOME`: the default `NVM_DIR`, and shown as `${HOME}` in the paths of cache
  messages (after `${NVM_DIR}`, as `nvm.sh` substitutes them). Makes sense.
  Because the substitution is a plain replace, an `NVM_DIR` that is a prefix of
  other words (`/n`) garbles the message; `nvm.sh` does the same.
- `PATH`: where `node`, `npm`, `clang`, `clang++` and `make` are found; the
  `bin` of a version is put first for each `npm` run. Makes sense.
- `PWD`: where `--save` writes `.nvmrc`. Makes sense.
- `NVM_NODEJS_ORG_MIRROR`, `NVM_IOJS_ORG_MIRROR`: where indexes, checksums and
  archives come from, validated with the strict character set of
  `nvm_get_mirror` (an invalid one is the error `$NVM_NODEJS_ORG_MIRROR and
  $NVM_IOJS_ORG_MIRROR may only contain a URL` and that flavor lists nothing).
  Makes sense: the value is appended to paths, and a mirror is the only way to
  install behind a firewall.
- `NVM_AUTH_HEADER`: sent as `Authorization: <value>` after the same
  sanitizing as `nvm_sanitize_auth_header`, and followed to a redirect on the
  same host only. Makes sense for private mirrors.
- `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`: honoured by the HTTP
  client, as `curl` does. Makes sense.
- `NVM_MIN_VERSION` and `$NVM_DIR/min-version`: the lowest version `install`
  accepts (the variable wins). Makes sense as a policy lever for teams.
- `NVM_INSTALL_LOCK_TIMEOUT` (600 seconds) and `NVM_INSTALL_LOCK_STALE` (0
  minutes, never): how long to wait for another install of the same version,
  and when to take a lock over. Makes sense. `nvm.sh` fails with a shell error
  on a value that is not a number; here it is the default.
- `NVM_NO_SOURCE_FALLBACK`: exactly `1` is `-b` for every install, and is at
  odds with `-s` (status 6), as printed by `nvm.sh`. Makes sense: the policy
  of a machine that must never compile.
- `NVM_MAKE_JOBS`: a natural number is the number of `make` jobs, without the
  CPU messages. Makes sense. `nvm.sh` hands any value to `make`, which then
  fails on `-j abc`; here such a value is ignored.
- `NVM_NO_PROGRESS`: makes `curl` quiet. There is no progress bar here, so
  it is accepted and has no effect. Makes sense only as compatibility.
- `NVM_DEBUG`: with `1`, `install-latest-npm` prints the `npm` commands
  instead of running them. Makes sense as a dry run of the upgrade (the one
  thing it does in `nvm.sh` that matters here).
- `NVM_INSTALL_THIRD_PARTY_HOOK`: a program that installs instead of `nvm`
  (see Task 20). It is started directly, without a shell, so its path is never
  parsed; it runs with the rights of the user, which is also what setting the
  variable already requires. Makes sense for companies that provision node
  their own way, and is the one variable here that runs arbitrary code, so
  it is listed in the security notes of the README (Plan 9).
- `CC`, `CXX`: the compilers of a source build (`CC=${CC:-cc}`
  `CXX=${CXX:-c++}`), and the reason `Clang v3.5+ detected!` is or is not said.
  Makes sense.
- Not inputs: `NVM_OFFLINE` (`nvm.sh` sets it from `--offline` and ignores
  the environment), `NVM_LTS` and the other internal variables, and the ones of
  later plans: `NVM_BIN`, `NVM_INC`, `NVM_SYMLINK_CURRENT`, `NVM_CD_FLAGS`,
  `NVM_NPM_*` (Plan 6, `use`) and `NVM_COLORS`, `NVM_NO_COLORS` (Plan 7).

## Self-review against the spec

- **Spec coverage:** section 3 layering is kept (`ports/` gains `Digest`,
  `Archive`, `Cpu` and a `Process::execute`; `adapters/` holds the only
  hashing, unpacking and child-process code; `domain/` has the platform,
  checksum, `npm` and source-build rules; `commands/install` assembles them);
  section 4.4 steps 1 to 9 are implemented except the `.nvmrc` lookup of step 1
  (Plan 6) and `--reinstall`; the lock of step 4 honours
  `NVM_INSTALL_LOCK_TIMEOUT` and `NVM_INSTALL_LOCK_STALE`; step 7 extracts into
  a temporary directory and moves it with one rename; step 8 is the `npm`
  upgrade, the default packages and the copy of global packages; section 6
  gains exits 4, 5, 6 and 33; section 10 gains `sha2`, `flate2` and `tar`;
  section 11 step 5 is complete for `install`, `uninstall`,
  `install-latest-npm` and `reinstall-packages`.
- **Deliberate deviations from `nvm.sh`, each to be pinned by the Plan 9
  compatibility contract:**
  - Only `.tar.gz` archives are used; `nvm.sh` prefers `.tar.xz` wherever it
    can, so the archive and its checksum differ.
  - The install is not activated and there is no `Now using node ...` line;
    `nvm use` belongs to the shell (Plan 6). `nvm install` with no version
    does not read a `.nvmrc`, and a command with options only is a usage error.
  - There is no progress bar and no `Computing checksum with ...` line (the
    hash is computed natively). The archive is read into memory (up to 1 GiB)
    before it is written, instead of being streamed to disk.
  - A checksum that cannot be computed fails the install; `nvm.sh` warns and
    goes on without verification.
  - A version whose `node` has no `npm` is not given the one from
    `https://npmjs.org/install.sh`, which `nvm.sh` downloads and runs; the
    `npm` steps are skipped with a warning.
  - `--default` or `--alias=<name>` together with `--lts` and no version make
    an alias to `lts/<name>`; `nvm.sh` runs `nvm alias <name> ""`, which
    deletes the alias. `--alias` is also applied after a source build or a
    hook install, which `nvm.sh` skips.
  - `--save` writes `.nvmrc` after a fresh install too; `nvm.sh` forgets it.
  - Versions before v0.12.0 (the legacy `$NVM_DIR/vX.Y.Z` layout) are not
    installed. `nvm install` of a version below v0.8.6, or on a platform with
    no official binary and no source, is status 3.
  - `nvm uninstall` does not check file permissions before removing, and
    matches aliases by literal text where `nvm.sh` uses `grep` with a regular
    expression.
  - The number of cores comes from the operating system's parallelism, not
    from `nproc`, `sysctl` or `getconf`; an invalid `NVM_MAKE_JOBS` is ignored.
  - The output of `npm`, `./configure` and `make` is shown when the command
    ends, not as it is produced.
- **Type consistency:** names match across tasks (`Platform`, `FileInfo`,
  `Digest`, `Archive`, `Cpu`, `Http::get_bytes`, `Invocation`, `Completed`,
  `LockRequest`, `Lookup`, `Transcript`, `Options`, `Artifact`, `Failed`,
  `Halt`, `Step`, `Target`, `Npm`, `NvmExitCode::{MissingTarget, SameVersion,
  SourceNotInstalled, InvalidOptions, HookClaimedSuccess, Passed}`).
