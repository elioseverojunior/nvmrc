# nvmrc Plan 1: Foundation and Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the typed core of nvmrc (versions, version floor, aliases,
`.nvmrc`, ports) and ship a working `nvm version <pattern>` command in two
binaries, `nvmrc` and `nvm`.

**Architecture:** One crate with a library and thin binaries. Pure rules live
in `domain/`, I/O sits behind the `ports` traits (`FileSystem`, `Env`) with
real adapters and in-memory fakes, and one `From` mapping turns typed errors
into the nvm exit codes.

**Tech Stack:** Rust 2024 edition (MSRV 1.88), `clap` (derive), `thiserror`,
`tempfile` (dev only).

**Spec:** `docs/superpowers/specs/2026-10-02-nvmrc-design.md`, covering
implementation-order steps 1 and 2 (sections 3, 4.2, 4.3, 5, 6 and 11).

**Roadmap (later plans, each written on top of this one):** 2 read-only
commands (`ls`, `current`, `which`, `alias`, `unalias`); 3 network
(`ls-remote`, `cache`, mirror, checksum); 4 writes (`install`, `uninstall`);
5 shell (`use`, `deactivate`, `exec`, `run`, `init`, `nvm-exec`); 6 conflicts
(`doctor`, `migrate`); 7 compatibility contract and CI.

## Global Constraints

- Rust edition 2024, `rust-version = "1.88.0"`; no APIs newer than 1.88. The
  development toolchain is pinned separately by `rust-toolchain.toml`
  (channel 1.99) and is deliberately decoupled from `rust-version`.
- Run cargo through the rustup proxies. Homebrew's `rust` formula installs its
  own `cargo` that ignores `rust-toolchain.toml`; check with
  `rustup show active-toolchain` and `command -v cargo`.
- `.cargo/config.toml` is the single source of truth for build profiles and
  already sets `-D warnings` for every workspace build, test and check, so
  `Cargo.toml` must not declare any `[profile.*]` table.
- No async I/O, no workspace, no plugin system.
- Files under 300 lines, functions under 30 lines, cyclomatic complexity
  under 10, meaningful names without abbreviations.
- `cargo fmt --check` and
  `cargo clippy --all-targets --all-features -- -D warnings` must be clean.
  Because of the global `-D warnings`, a dead-code warning in any intermediate
  task is a build error: every item must be used by the end of its own task.
- `cargo audit` is configured in `.cargo/audit.toml` (warnings are errors).
- Errors use `thiserror` in the library; only the binaries touch
  `std::process::ExitCode`.
- Exit codes: 0 success, 1 generic failure, 3 invalid or unknown version,
  7 below the version floor or invalid floor, 8 alias loop.
- Layout compatible with nvm: `$NVM_DIR/versions/node/vX.Y.Z`,
  `$NVM_DIR/versions/io.js/vX.Y.Z` (the `iojs-` prefix is stripped),
  `$NVM_DIR/alias/<name>`, `$NVM_DIR/min-version`.
- Path-traversal safety: alias names may contain `/` but never `..` or an
  absolute path.
- Commits: Conventional Commits, GPG-signed with `git commit -S`, and no AI
  attribution or `Co-Authored-By` trailer in the message.
- Do not keep test scripts or test-output directories in the repository.

---

### Task 1: Scaffold and exit-code contract

**Files:**

- Modify: `Cargo.toml`, `.gitignore` (append only)
- Create: `src/lib.rs`, `src/error.rs`
- Already present, committed here: `rust-toolchain.toml`,
  `.cargo/config.toml`, `.cargo/audit.toml`, `rumdl.toml`
- Modify: `src/main.rs` (replaced in Task 9)

**Interfaces:**

- Produces: `error::NvmExitCode` (`Success`, `Failure`, `InvalidVersion`,
  `BelowVersionFloor`, `AliasLoop`; `fn code(self) -> u8`),
  `error::VersionError::Invalid(String)`,
  `error::FloorError::{Invalid(String), Below { version: String, floor: String }}`,
  `error::AliasError::Loop(String)`,
  `error::CliError::{Version, Floor, Alias, NotInstalled}` with
  `fn exit_code(&self) -> NvmExitCode`.

The repository has no commits yet, so this task's commit is the initial one.
It includes `docs/` (the spec and this plan): `~/.gitignore_global` ignores
`docs/superpowers/**`, and the project `.gitignore` un-ignores it with
`!/docs/**/*`.

- [ ] **Step 1: Add the dependency and ignore `.idea`**

Run:

```bash
cargo add thiserror@2
[ -z "$(tail -c1 .gitignore)" ] || echo >> .gitignore  # ensure trailing newline
grep -qx '.idea' .gitignore || printf '.idea\n' >> .gitignore
```

Expected: `Cargo.toml` now lists `thiserror = "2"` under `[dependencies]`, and
`.gitignore` keeps its existing lines (including the `!/docs/**/*` exception
that un-ignores `docs/`) plus `.idea`. Never overwrite `.gitignore`.

- [ ] **Step 2: Create the library root**

Create `src/lib.rs`:

```rust
//! nvmrc: a native Rust port of nvm.

pub mod error;
```

- [ ] **Step 3: Write the failing tests**

Create `src/error.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_the_nvm_contract() {
        assert_eq!(NvmExitCode::Success.code(), 0);
        assert_eq!(NvmExitCode::InvalidVersion.code(), 3);
        assert_eq!(NvmExitCode::BelowVersionFloor.code(), 7);
        assert_eq!(NvmExitCode::AliasLoop.code(), 8);
    }

    #[test]
    fn errors_map_to_their_exit_codes() {
        let floor = FloorError::Invalid("x".into());
        assert_eq!(
            CliError::from(floor).exit_code(),
            NvmExitCode::BelowVersionFloor
        );
        let alias = AliasError::Loop("a".into());
        assert_eq!(CliError::from(alias).exit_code(), NvmExitCode::AliasLoop);
        let version = VersionError::Invalid("x".into());
        assert_eq!(
            CliError::from(version).exit_code(),
            NvmExitCode::InvalidVersion
        );
        assert_eq!(
            CliError::NotInstalled.exit_code(),
            NvmExitCode::InvalidVersion
        );
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test error::`
Expected: FAIL to compile with "cannot find type `NvmExitCode`" (and the other
error types).

- [ ] **Step 5: Write the implementation**

Insert this above the `#[cfg(test)]` line of `src/error.rs`:

```rust
//! Typed errors and the single mapping from errors to process exit codes.

use thiserror::Error;

/// Public exit-code contract, taken from `nvm.sh`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NvmExitCode {
    Success = 0,
    Failure = 1,
    InvalidVersion = 3,
    BelowVersionFloor = 7,
    AliasLoop = 8,
}

impl NvmExitCode {
    #[must_use]
    pub fn code(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VersionError {
    #[error("invalid version: {0}")]
    Invalid(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FloorError {
    #[error("Invalid minimum version '{0}' (from NVM_MIN_VERSION or $NVM_DIR/min-version).")]
    Invalid(String),
    #[error("Version {version} is below the minimum allowed version {floor}.")]
    Below { version: String, floor: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AliasError {
    #[error("The alias \"{0}\" leads to an infinite loop. Aborting.")]
    Loop(String),
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Version(#[from] VersionError),
    #[error(transparent)]
    Floor(#[from] FloorError),
    #[error(transparent)]
    Alias(#[from] AliasError),
    #[error("N/A")]
    NotInstalled,
}

impl CliError {
    #[must_use]
    pub fn exit_code(&self) -> NvmExitCode {
        match self {
            Self::Version(_) | Self::NotInstalled => NvmExitCode::InvalidVersion,
            Self::Floor(_) => NvmExitCode::BelowVersionFloor,
            Self::Alias(_) => NvmExitCode::AliasLoop,
        }
    }
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test error::`
Expected: PASS, 2 tests.

- [ ] **Step 7: Commit**

```bash
git add .gitignore .cargo Cargo.toml Cargo.lock rumdl.toml \
  rust-toolchain.toml src docs
git commit -S -m "feat(error): add nvm exit-code contract and typed errors"
```

---

### Task 2: Version and VersionPattern

**Files:**

- Create: `src/domain/mod.rs`, `src/domain/version.rs`
- Modify: `src/lib.rs`

**Interfaces:**

- Consumes: `error::VersionError`.
- Produces: `domain::version::{Flavor, Version, VersionPattern}`.
  `Version { flavor, major, minor, patch }` implements `FromStr` (exactly three
  numeric parts), `Display` (`v20.1.2`, `iojs-v3.0.0`), `Ord`, and
  `fn triple(&self) -> (u64, u64, u64)`.
  `VersionPattern { flavor, major, minor: Option<u64>, patch: Option<u64> }`
  implements `FromStr` (one to three parts) with
  `fn matches(&self, &Version) -> bool`, `fn lowest(&self) -> Version` and
  `fn highest_match<'a>(&self, &'a [Version]) -> Option<&'a Version>`.
- [ ] **Step 1: Declare the modules**

Create `src/domain/mod.rs`:

```rust
pub mod version;
```

Add `pub mod domain;` to `src/lib.rs` (keep the list alphabetical).

- [ ] **Step 2: Write the failing tests**

Create `src/domain/version.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        text.parse().expect("valid version")
    }

    #[test]
    fn parses_with_and_without_the_v_prefix() {
        assert_eq!(version("v20.1.2"), version("20.1.2"));
    }

    #[test]
    fn parses_iojs_versions() {
        let parsed = version("iojs-v3.0.0");
        assert_eq!(parsed.flavor, Flavor::IoJs);
        assert_eq!(parsed.to_string(), "iojs-v3.0.0");
    }

    #[test]
    fn displays_node_versions_with_the_v_prefix() {
        assert_eq!(version("20.1.2").to_string(), "v20.1.2");
    }

    #[test]
    fn rejects_malformed_versions() {
        for bad in [
            "", "v", "1.2", "1.2.3.4", "a.b.c", "1..3", "-1.0.0", "v1.0.x",
        ] {
            assert!(bad.parse::<Version>().is_err(), "{bad:?} should be invalid");
        }
    }

    #[test]
    fn orders_numerically_not_lexicographically() {
        assert!(version("v10.0.0") > version("v9.0.0"));
        assert!(version("v1.10.0") > version("v1.9.0"));
    }

    #[test]
    fn pattern_accepts_one_to_three_parts() {
        let major: VersionPattern = "20".parse().unwrap();
        assert_eq!((major.minor, major.patch), (None, None));
        let minor: VersionPattern = "v20.1".parse().unwrap();
        assert_eq!((minor.minor, minor.patch), (Some(1), None));
    }

    #[test]
    fn pattern_lowest_fills_missing_parts_with_zero() {
        let pattern: VersionPattern = "24".parse().unwrap();
        assert_eq!(pattern.lowest(), version("v24.0.0"));
    }

    #[test]
    fn pattern_picks_the_highest_matching_installed_version() {
        let installed = [version("v20.1.0"), version("v20.10.0"), version("v18.9.0")];
        let pattern: VersionPattern = "20".parse().unwrap();
        assert_eq!(pattern.highest_match(&installed), Some(&installed[1]));
        let none: VersionPattern = "16".parse().unwrap();
        assert_eq!(none.highest_match(&installed), None);
    }

    #[test]
    fn pattern_does_not_match_across_flavors() {
        let pattern: VersionPattern = "3".parse().unwrap();
        assert!(!pattern.matches(&version("iojs-v3.0.0")));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test domain::version`
Expected: FAIL to compile with "cannot find type `Version`".

- [ ] **Step 4: Write the implementation**

Insert this above the `#[cfg(test)]` line of `src/domain/version.rs`:

```rust
//! Node and io.js versions, and partial patterns such as `20` or `v20.1`.

use std::fmt;
use std::str::FromStr;

use crate::error::VersionError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Flavor {
    Node,
    IoJs,
}

/// A fully specified version. Ordering is by flavor, then numerically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version {
    pub flavor: Flavor,
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

/// A version with optional minor and patch, used to select installed versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionPattern {
    pub flavor: Flavor,
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
}

fn parse_parts(input: &str) -> Result<(Flavor, Vec<u64>), VersionError> {
    let invalid = || VersionError::Invalid(input.to_owned());
    let (flavor, rest) = match input.strip_prefix("iojs-") {
        Some(rest) => (Flavor::IoJs, rest),
        None => (Flavor::Node, input),
    };
    let rest = rest.strip_prefix('v').unwrap_or(rest);
    let parts = rest
        .split('.')
        .map(|part| {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(invalid());
            }
            part.parse::<u64>().map_err(|_| invalid())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() > 3 {
        return Err(invalid());
    }
    Ok((flavor, parts))
}

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match parse_parts(input)? {
            (flavor, parts) if parts.len() == 3 => Ok(Self {
                flavor,
                major: parts[0],
                minor: parts[1],
                patch: parts[2],
            }),
            _ => Err(VersionError::Invalid(input.to_owned())),
        }
    }
}

impl FromStr for VersionPattern {
    type Err = VersionError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (flavor, parts) = parse_parts(input)?;
        Ok(Self {
            flavor,
            major: parts[0],
            minor: parts.get(1).copied(),
            patch: parts.get(2).copied(),
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self.flavor {
            Flavor::Node => "",
            Flavor::IoJs => "iojs-",
        };
        write!(
            formatter,
            "{prefix}v{}.{}.{}",
            self.major, self.minor, self.patch
        )
    }
}

impl Version {
    /// The numeric part, ignoring flavor (used for floor comparison).
    #[must_use]
    pub fn triple(&self) -> (u64, u64, u64) {
        (self.major, self.minor, self.patch)
    }
}

impl VersionPattern {
    #[must_use]
    pub fn matches(&self, version: &Version) -> bool {
        version.flavor == self.flavor
            && version.major == self.major
            && self.minor.is_none_or(|minor| minor == version.minor)
            && self.patch.is_none_or(|patch| patch == version.patch)
    }

    /// The lowest version the pattern covers: `24` is `v24.0.0`.
    #[must_use]
    pub fn lowest(&self) -> Version {
        Version {
            flavor: self.flavor,
            major: self.major,
            minor: self.minor.unwrap_or(0),
            patch: self.patch.unwrap_or(0),
        }
    }

    #[must_use]
    pub fn highest_match<'a>(&self, installed: &'a [Version]) -> Option<&'a Version> {
        installed
            .iter()
            .filter(|version| self.matches(version))
            .max()
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test domain::version`
Expected: PASS, 9 tests.

- [ ] **Step 6: Commit**

```bash
git add src
git commit -S -m "feat(domain): add Version and VersionPattern with numeric ordering"
```

---

### Task 3: VersionFloor (from the nvm spike branch)

**Files:**

- Create: `src/domain/floor.rs`
- Modify: `src/domain/mod.rs`

**Interfaces:**

- Consumes: `domain::version::{Version, VersionPattern}`, `error::FloorError`.
- Produces: `domain::floor::VersionFloor` with
  `fn parse(&str) -> Result<Self, FloorError>`,
  `fn from_sources(env_value, file_contents)` returning
  `Result<Option<Self>, FloorError>` with both arguments `Option<&str>`
  (the env value wins over the file's first line; empty means no floor) and
  `fn check(&self, &Version) -> Result<(), FloorError>`.

These tests port the cases of the spike's
`test/fast/Unit tests/nvm install NVM_MIN_VERSION`.

- [ ] **Step 1: Declare the module**

Add `pub mod floor;` to `src/domain/mod.rs` (alphabetical, before `version`).

- [ ] **Step 2: Write the failing tests**

Create `src/domain/floor.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        text.parse().expect("valid version")
    }

    fn floor(text: &str) -> VersionFloor {
        VersionFloor::parse(text).expect("valid floor")
    }

    #[test]
    fn a_version_below_a_major_only_floor_is_refused() {
        let error = floor("24").check(&version("v20.0.0")).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("v20.0.0") && message.contains("v24.0.0"),
            "{message}"
        );
    }

    #[test]
    fn the_floor_may_be_written_with_a_leading_v() {
        assert!(floor("v24").check(&version("v20.0.0")).is_err());
    }

    #[test]
    fn equal_and_higher_versions_are_allowed() {
        assert!(floor("24").check(&version("v24.0.0")).is_ok());
        assert!(floor("24").check(&version("v25.1.0")).is_ok());
    }

    #[test]
    fn a_full_version_floor_is_honoured_to_the_patch_level() {
        assert!(floor("v24.1.0").check(&version("v24.0.0")).is_err());
    }

    #[test]
    fn an_invalid_floor_is_an_error_naming_the_value() {
        let error = VersionFloor::parse("not-a-version").unwrap_err();
        assert!(error.to_string().contains("not-a-version"));
    }

    #[test]
    fn no_sources_means_no_floor() {
        assert_eq!(VersionFloor::from_sources(None, None), Ok(None));
        assert_eq!(VersionFloor::from_sources(Some(""), Some("")), Ok(None));
    }

    #[test]
    fn the_file_is_used_when_the_env_var_is_unset() {
        let found = VersionFloor::from_sources(None, Some("24\nignored")).unwrap();
        assert_eq!(found, Some(floor("24")));
    }

    #[test]
    fn the_env_var_overrides_the_file() {
        let found = VersionFloor::from_sources(Some("18"), Some("24")).unwrap();
        assert_eq!(found, Some(floor("18")));
    }

    #[test]
    fn an_invalid_value_is_an_error_not_a_silent_pass() {
        assert!(VersionFloor::from_sources(Some("nope"), Some("24")).is_err());
        assert!(VersionFloor::from_sources(None, Some("nope")).is_err());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test domain::floor`
Expected: FAIL to compile with "cannot find type `VersionFloor`".

- [ ] **Step 4: Write the implementation**

Insert this above the `#[cfg(test)]` line of `src/domain/floor.rs`:

```rust
//! The minimum installable version (`NVM_MIN_VERSION`, `$NVM_DIR/min-version`).

use crate::domain::version::{Version, VersionPattern};
use crate::error::FloorError;

/// A valid version floor. An invalid one can never be constructed, so a bad
/// floor can never let an install through unchecked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionFloor(Version);

impl VersionFloor {
    /// # Errors
    /// Returns [`FloorError::Invalid`] when `raw` is not a version.
    pub fn parse(raw: &str) -> Result<Self, FloorError> {
        let pattern: VersionPattern = raw
            .parse()
            .map_err(|_| FloorError::Invalid(raw.to_owned()))?;
        Ok(Self(pattern.lowest()))
    }

    /// The environment value wins over the first line of the file.
    ///
    /// # Errors
    /// Returns [`FloorError::Invalid`] when the chosen value is not a version.
    pub fn from_sources(
        env_value: Option<&str>,
        file_contents: Option<&str>,
    ) -> Result<Option<Self>, FloorError> {
        let first_line = file_contents.and_then(|contents| contents.lines().next());
        env_value
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| first_line.map(str::trim).filter(|value| !value.is_empty()))
            .map(Self::parse)
            .transpose()
    }

    /// # Errors
    /// Returns [`FloorError::Below`] when `candidate` is lower than the floor.
    pub fn check(&self, candidate: &Version) -> Result<(), FloorError> {
        if candidate.triple() >= self.0.triple() {
            return Ok(());
        }
        Err(FloorError::Below {
            version: candidate.to_string(),
            floor: Version {
                flavor: candidate.flavor,
                ..self.0
            }
            .to_string(),
        })
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test domain::floor`
Expected: PASS, 9 tests.

- [ ] **Step 6: Commit**

```bash
git add src
git commit -S -m "feat(domain): add VersionFloor for NVM_MIN_VERSION and min-version"
```

---

### Task 4: Ports, in-memory fakes and std adapters

**Files:**

- Create: `src/ports/mod.rs`, `src/fakes.rs`, `src/adapters/mod.rs`,
  `src/adapters/std_fs.rs`, `src/adapters/std_env.rs`
- Modify: `src/lib.rs`

**Interfaces:**

- Produces: `ports::FileSystem`
  (`read_to_string(&Path) -> io::Result<String>`,
  `read_dir_names(&Path) -> io::Result<Vec<String>>`,
  `is_file(&Path) -> bool`), `ports::Env` (`var(&str) -> Option<String>`),
  `fakes::FakeFileSystem::default().with_file(path, contents)`,
  `fakes::FakeEnv::default().with_var(key, value)` (test-only),
  `adapters::std_fs::StdFileSystem`, `adapters::std_env::StdEnv`.

- [ ] **Step 1: Create the port traits**

Create `src/ports/mod.rs`:

```rust
//! Traits through which the domain and commands reach the outside world.

use std::io;
use std::path::Path;

pub trait FileSystem {
    /// # Errors
    /// Propagates the underlying I/O error.
    fn read_to_string(&self, path: &Path) -> io::Result<String>;

    /// Names (not paths) of the direct children of `path`.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn read_dir_names(&self, path: &Path) -> io::Result<Vec<String>>;

    fn is_file(&self, path: &Path) -> bool;
}

pub trait Env {
    fn var(&self, key: &str) -> Option<String>;
}
```

Add `pub mod adapters;`, `pub mod ports;` and, after the other `mod` lines,
the test-only fakes module to `src/lib.rs`:

```rust
#[cfg(test)]
pub(crate) mod fakes;
```

- [ ] **Step 2: Write the failing tests**

Create `src/fakes.rs` containing only the test module:

```rust
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
```

Create `src/adapters/mod.rs` with the two real adapters declared:

```rust
pub mod std_env;
pub mod std_fs;
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test fakes::`
Expected: FAIL to compile with "cannot find type `FakeFileSystem`".

- [ ] **Step 4: Write the fakes**

Insert this above the `#[cfg(test)]` line of `src/fakes.rs`:

```rust
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
```

- [ ] **Step 5: Write the real adapters**

Create `src/adapters/std_fs.rs`:

```rust
use std::fs;
use std::io;
use std::path::Path;

use crate::ports::FileSystem;

pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn read_dir_names(&self, path: &Path) -> io::Result<Vec<String>> {
        fs::read_dir(path)?
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
            .collect()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }
}
```

Create `src/adapters/std_env.rs`:

```rust
use crate::ports::Env;

pub struct StdEnv;

impl Env for StdEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}
```

These are thin wrappers over `std`; the end-to-end test in Task 9 covers them.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS (3 fakes tests) and no clippy warnings. `FakeEnv` is only used
by its own test until Task 7, which is why that test exists.

- [ ] **Step 7: Commit**

```bash
git add src
git commit -S -m "feat(ports): add FileSystem and Env ports with fakes and std adapters"
```

---

### Task 5: Alias resolution and the file-backed alias store

**Files:**

- Create: `src/domain/alias.rs`, `src/adapters/fs_alias_store.rs`
- Modify: `src/domain/mod.rs`, `src/adapters/mod.rs`

**Interfaces:**

- Consumes: `ports::FileSystem`, `fakes::FakeFileSystem` (tests),
  `error::AliasError`.
- Produces: `domain::alias::AliasStore` (`fn target(&self, &str) -> Option<String>`),
  `domain::alias::resolve(&dyn AliasStore, &str) -> Result<String, AliasError>`
  (a name that is not an alias resolves to itself; a cycle is
  `AliasError::Loop`), and `adapters::fs_alias_store::FsAliasStore`
  (`new(&dyn FileSystem, &Path)`) reading `<nvm_dir>/alias/<name>`.
- [ ] **Step 1: Declare the modules**

Add `pub mod alias;` to `src/domain/mod.rs` (first line) and
`pub mod fs_alias_store;` to `src/adapters/mod.rs` (first line).

- [ ] **Step 2: Write the failing tests**

Create `src/domain/alias.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    struct MapStore(HashMap<&'static str, &'static str>);

    impl AliasStore for MapStore {
        fn target(&self, name: &str) -> Option<String> {
            self.0.get(name).map(|target| (*target).to_owned())
        }
    }

    fn store(pairs: &[(&'static str, &'static str)]) -> MapStore {
        MapStore(pairs.iter().copied().collect())
    }

    #[test]
    fn a_name_that_is_not_an_alias_resolves_to_itself() {
        assert_eq!(resolve(&store(&[]), "v20.0.0").unwrap(), "v20.0.0");
    }

    #[test]
    fn follows_a_chain_to_its_end() {
        let aliases = store(&[("default", "lts"), ("lts", "v20.0.0")]);
        assert_eq!(resolve(&aliases, "default").unwrap(), "v20.0.0");
    }

    #[test]
    fn a_cycle_is_an_alias_loop_error() {
        let aliases = store(&[("a", "b"), ("b", "a")]);
        assert_eq!(resolve(&aliases, "a"), Err(AliasError::Loop("a".into())));
    }

    #[test]
    fn an_alias_pointing_at_itself_is_a_loop() {
        let aliases = store(&[("a", "a")]);
        assert_eq!(resolve(&aliases, "a"), Err(AliasError::Loop("a".into())));
    }
}
```

Create `src/adapters/fs_alias_store.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::FakeFileSystem;

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
    fn rejects_path_traversal_in_alias_names() {
        let fs = FakeFileSystem::default().with_file("/nvm/secret", "v1.0.0");
        assert_eq!(store(&fs).target("../secret"), None);
        assert_eq!(store(&fs).target("/nvm/secret"), None);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test alias`
Expected: FAIL to compile with "cannot find trait `AliasStore`" and
"cannot find type `FsAliasStore`".

- [ ] **Step 4: Write the implementations**

Insert this above the `#[cfg(test)]` line of `src/domain/alias.rs`:

```rust
//! Alias resolution: follow `a -> b -> v20.0.0`, detecting cycles.

use std::collections::HashSet;

use crate::error::AliasError;

pub trait AliasStore {
    /// The target an alias points to, or `None` when `name` is not an alias.
    fn target(&self, name: &str) -> Option<String>;
}

/// Follows the alias chain from `name` to its final target. A name that is not
/// an alias resolves to itself.
///
/// # Errors
/// Returns [`AliasError::Loop`] when the chain revisits a name.
pub fn resolve(store: &dyn AliasStore, name: &str) -> Result<String, AliasError> {
    let mut seen = HashSet::from([name.to_owned()]);
    let mut current = name.to_owned();
    while let Some(next) = store.target(&current) {
        if !seen.insert(next.clone()) {
            return Err(AliasError::Loop(name.to_owned()));
        }
        current = next;
    }
    Ok(current)
}
```

Insert this above the `#[cfg(test)]` line of `src/adapters/fs_alias_store.rs`:

```rust
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
        let line = contents.lines().next()?.trim();
        (!line.is_empty()).then(|| line.to_owned())
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test alias`
Expected: PASS, 8 tests (4 in each module).

- [ ] **Step 6: Commit**

```bash
git add src
git commit -S -m "feat(alias): resolve alias chains with loop detection"
```

---

### Task 6: `.nvmrc` parsing and upward search

**Files:**

- Create: `src/domain/nvmrc.rs`
- Modify: `src/domain/mod.rs`

**Interfaces:**

- Consumes: `ports::FileSystem`, `fakes::FakeFileSystem` (tests).
- Produces: `domain::nvmrc::NVMRC_FILE_NAME`,
  `domain::nvmrc::parse(&str) -> Option<String>` (first line, CR and
  surrounding spaces removed), and
  `domain::nvmrc::find_up(&dyn FileSystem, &Path) -> Option<PathBuf>`.
- [ ] **Step 1: Declare the module**

Add `pub mod nvmrc;` to `src/domain/mod.rs` (between `floor` and `version`).

- [ ] **Step 2: Write the failing tests**

Create `src/domain/nvmrc.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::FakeFileSystem;

    #[test]
    fn parse_takes_the_first_line() {
        assert_eq!(parse("v20.0.0\nother"), Some("v20.0.0".to_owned()));
    }

    #[test]
    fn parse_drops_carriage_returns_and_spaces() {
        assert_eq!(parse("v20.0.0\r\n"), Some("v20.0.0".to_owned()));
        assert_eq!(parse("  lts/iron \n"), Some("lts/iron".to_owned()));
    }

    #[test]
    fn parse_of_an_empty_file_is_none() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("\r\n"), None);
    }

    #[test]
    fn find_up_walks_to_parent_directories() {
        let fs = FakeFileSystem::default().with_file("/work/proj/.nvmrc", "20");
        let found = find_up(&fs, Path::new("/work/proj/src/deep"));
        assert_eq!(found, Some(PathBuf::from("/work/proj/.nvmrc")));
    }

    #[test]
    fn find_up_prefers_the_nearest_file() {
        let fs = FakeFileSystem::default()
            .with_file("/work/.nvmrc", "18")
            .with_file("/work/proj/.nvmrc", "20");
        let found = find_up(&fs, Path::new("/work/proj"));
        assert_eq!(found, Some(PathBuf::from("/work/proj/.nvmrc")));
    }

    #[test]
    fn find_up_returns_none_when_absent() {
        let fs = FakeFileSystem::default();
        assert_eq!(find_up(&fs, Path::new("/work")), None);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test domain::nvmrc`
Expected: FAIL to compile with "cannot find function `parse`".

- [ ] **Step 4: Write the implementation**

Insert this above the `#[cfg(test)]` line of `src/domain/nvmrc.rs`:

```rust
//! `.nvmrc` files: parsing, and the upward search from a directory.

use std::path::{Path, PathBuf};

use crate::ports::FileSystem;

pub const NVMRC_FILE_NAME: &str = ".nvmrc";

/// The version requested by a `.nvmrc`: its first line, without CR or spaces.
#[must_use]
pub fn parse(contents: &str) -> Option<String> {
    let first_line = contents.split('\n').next()?.replace('\r', "");
    let trimmed = first_line.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// The nearest `.nvmrc`, looking in `start` and then each parent directory.
#[must_use]
pub fn find_up(fs: &dyn FileSystem, start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|directory| directory.join(NVMRC_FILE_NAME))
        .find(|candidate| fs.is_file(candidate))
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test domain::nvmrc`
Expected: PASS, 6 tests.

- [ ] **Step 6: Commit**

```bash
git add src
git commit -S -m "feat(domain): parse .nvmrc files and find them upward"
```

---

### Task 7: Context (`$NVM_DIR` and installed versions)

**Files:**

- Create: `src/context.rs`
- Modify: `src/lib.rs`

**Interfaces:**

- Consumes: `ports::{Env, FileSystem}`, `domain::version::Version`, the fakes.
- Produces: `context::Context { fs: &dyn FileSystem, env: &dyn Env }` with
  `fn nvm_dir(&self) -> PathBuf` (`$NVM_DIR` without trailing slash, else
  `$HOME/.nvm`) and `fn installed_versions(&self) -> Vec<Version>` (reads
  `versions/node` and `versions/io.js`, ignoring non-version names).
- [ ] **Step 1: Declare the module**

Add `pub mod context;` to `src/lib.rs` (alphabetical, before `domain`).

- [ ] **Step 2: Write the failing tests**

Create `src/context.rs` containing only the test module:

```rust
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
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test context::`
Expected: FAIL to compile with "cannot find struct `Context`".

- [ ] **Step 4: Write the implementation**

Insert this above the `#[cfg(test)]` line of `src/context.rs`:

```rust
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
        PathBuf::from(dir.trim_end_matches('/'))
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
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test context::`
Expected: PASS, 3 tests.

- [ ] **Step 6: Commit**

```bash
git add src
git commit -S -m "feat(context): derive NVM_DIR and list installed versions"
```

---

### Task 8: The `version` command

**Files:**

- Create: `src/commands/mod.rs`, `src/commands/version.rs`
- Modify: `src/lib.rs`

**Interfaces:**

- Consumes: `context::Context`, `adapters::fs_alias_store::FsAliasStore`,
  `domain::alias::resolve`, `domain::version::VersionPattern`,
  `error::CliError`.
- Produces: `commands::version::run(&Context, &str) -> Result<String, CliError>`.
  It resolves aliases, then returns the highest installed match. No match is
  `CliError::NotInstalled` (displayed as `N/A`, exit code 3); an alias loop is
  exit code 8.
- [ ] **Step 1: Declare the modules**

Create `src/commands/mod.rs`:

```rust
pub mod version;
```

Add `pub mod commands;` to `src/lib.rs` (alphabetical, before `context`).

- [ ] **Step 2: Write the failing tests**

Create `src/commands/version.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn run_with(fs: &FakeFileSystem, name: &str) -> Result<String, CliError> {
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        run(&Context { fs, env: &env }, name)
    }

    fn installed() -> FakeFileSystem {
        FakeFileSystem::default()
            .with_file("/n/versions/node/v20.1.0/bin/node", "")
            .with_file("/n/versions/node/v20.10.0/bin/node", "")
            .with_file("/n/versions/node/v18.9.0/bin/node", "")
    }

    #[test]
    fn resolves_a_major_to_the_highest_installed_version() {
        assert_eq!(run_with(&installed(), "20").unwrap(), "v20.10.0");
    }

    #[test]
    fn resolves_through_an_alias() {
        let fs = installed().with_file("/n/alias/default", "v18");
        assert_eq!(run_with(&fs, "default").unwrap(), "v18.9.0");
    }

    #[test]
    fn a_missing_version_is_not_installed_with_exit_code_3() {
        let error = run_with(&installed(), "16").unwrap_err();
        assert_eq!(error.to_string(), "N/A");
        assert_eq!(error.exit_code(), NvmExitCode::InvalidVersion);
    }

    #[test]
    fn an_alias_loop_has_exit_code_8() {
        let fs = installed()
            .with_file("/n/alias/a", "b")
            .with_file("/n/alias/b", "a");
        let error = run_with(&fs, "a").unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::AliasLoop);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test commands::version`
Expected: FAIL to compile with "cannot find function `run`".

- [ ] **Step 4: Write the implementation**

Insert this above the `#[cfg(test)]` line of `src/commands/version.rs`:

```rust
//! `nvm version <pattern>`: the highest installed version matching a pattern.

use crate::adapters::fs_alias_store::FsAliasStore;
use crate::context::Context;
use crate::domain::alias;
use crate::domain::version::VersionPattern;
use crate::error::CliError;

/// # Errors
/// - [`CliError::Alias`] when the alias chain loops.
/// - [`CliError::Version`] when the resolved name is not a version pattern.
/// - [`CliError::NotInstalled`] when no installed version matches.
pub fn run(context: &Context<'_>, name: &str) -> Result<String, CliError> {
    let store = FsAliasStore::new(context.fs, &context.nvm_dir());
    let resolved = alias::resolve(&store, name)?;
    let pattern: VersionPattern = resolved.parse()?;
    let installed = context.installed_versions();
    pattern
        .highest_match(&installed)
        .map(ToString::to_string)
        .ok_or(CliError::NotInstalled)
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test commands::version`
Expected: PASS, 4 tests.

- [ ] **Step 6: Commit**

```bash
git add src
git commit -S -m "feat(commands): add version command resolving aliases and patterns"
```

---

### Task 9: CLI, the `nvmrc` and `nvm` binaries, and the end-to-end test

**Files:**

- Create: `src/cli.rs`, `src/bin/nvm.rs`, `tests/version_cli.rs`
- Modify: `Cargo.toml`, `src/lib.rs`, `src/main.rs`

**Interfaces:**

- Consumes: `commands::version::run`, `context::Context`, `StdFileSystem`,
  `StdEnv`, `error::{CliError, NvmExitCode}`.
- Produces: `cli::run(args, &Context, &mut dyn Write, &mut dyn Write) -> u8`
  and `cli::run_from_env() -> u8`, shared by both binaries.
- [ ] **Step 1: Add dependencies and declare both binaries**

Run:

```bash
cargo add clap@4 --features derive
cargo add --dev tempfile@3
```

Append to `Cargo.toml`:

```toml
[[bin]]
name = "nvmrc"
path = "src/main.rs"

[[bin]]
name = "nvm"
path = "src/bin/nvm.rs"
```

Add `pub mod cli;` to `src/lib.rs` (alphabetical, before `commands`).

- [ ] **Step 2: Write the failing tests**

Create `src/cli.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::{FakeEnv, FakeFileSystem};

    fn run_cli(args: &[&str]) -> (u8, String, String) {
        let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(args, &Context { fs: &fs, env: &env }, &mut out, &mut err);
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
    fn version_reports_not_installed_on_stderr_with_exit_3() {
        assert_eq!(
            run_cli(&["nvm", "version", "16"]),
            (3, String::new(), "N/A\n".into())
        );
    }

    #[test]
    fn a_usage_error_exits_non_zero_without_stdout() {
        let (code, out, err) = run_cli(&["nvm", "bogus"]);
        assert_ne!(code, 0);
        assert!(out.is_empty() && !err.is_empty());
    }
}
```

Create the end-to-end test `tests/version_cli.rs`:

```rust
//! End-to-end: the real `nvm` binary against a real temporary `$NVM_DIR`.

use std::fs;
use std::process::Command;

fn nvm(nvm_dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_nvm"))
        .args(args)
        .env("NVM_DIR", nvm_dir)
        .output()
        .expect("run nvm")
}

#[test]
fn version_resolves_installed_versions_and_aliases() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("versions/node/v20.1.0/bin")).unwrap();
    fs::create_dir_all(dir.path().join("alias")).unwrap();
    fs::write(dir.path().join("alias/default"), "v20\n").unwrap();

    let output = nvm(dir.path(), &["version", "default"]);
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "v20.1.0\n");

    let missing = nvm(dir.path(), &["version", "16"]);
    assert_eq!(missing.status.code(), Some(3));
    assert_eq!(String::from_utf8_lossy(&missing.stderr), "N/A\n");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test cli::`
Expected: FAIL to compile with "cannot find function `run`".

- [ ] **Step 4: Write the implementation and both binaries**

Insert this above the `#[cfg(test)]` line of `src/cli.rs`:

```rust
//! Argument parsing and the translation of results into streams and exit codes.

use std::ffi::OsString;
use std::io::Write;

use clap::{Parser, Subcommand};

use crate::adapters::std_env::StdEnv;
use crate::adapters::std_fs::StdFileSystem;
use crate::commands;
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};

#[derive(Parser)]
#[command(name = "nvm", version, about = "Node Version Manager, in Rust")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the highest installed version matching a version or alias.
    Version { pattern: String },
}

fn dispatch(command: &Command, context: &Context<'_>) -> Result<String, CliError> {
    match command {
        Command::Version { pattern } => commands::version::run(context, pattern),
    }
}

/// Runs the CLI and returns the process exit code. Output goes to the given
/// writers so tests can capture it.
pub fn run<I, T>(args: I, context: &Context<'_>, out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            if error.use_stderr() {
                let _ = write!(err, "{error}");
                return NvmExitCode::Failure.code();
            }
            let _ = write!(out, "{error}");
            return NvmExitCode::Success.code();
        }
    };
    match dispatch(&cli.command, context) {
        Ok(text) => {
            let _ = writeln!(out, "{text}");
            NvmExitCode::Success.code()
        }
        Err(error) => {
            let _ = writeln!(err, "{error}");
            error.exit_code().code()
        }
    }
}

/// Entry point shared by the `nvmrc` and `nvm` binaries.
#[must_use]
pub fn run_from_env() -> u8 {
    let context = Context {
        fs: &StdFileSystem,
        env: &StdEnv,
    };
    run(
        std::env::args_os(),
        &context,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
}
```

Replace `src/main.rs`:

```rust
fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(nvmrc::cli::run_from_env())
}
```

Create `src/bin/nvm.rs` with the identical content:

```rust
fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(nvmrc::cli::run_from_env())
}
```

- [ ] **Step 5: Run the full quality gate**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
rustup toolchain install 1.88 --profile minimal
rustup run 1.88 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings, all tests pass (47 unit
tests plus the end-to-end test), no advisories, and the crate still builds on
the declared MSRV 1.88. `cargo deny check` is deferred until a `deny.toml`
exists (planned for the CI plan).

- [ ] **Step 6: Try it by hand**

Run:

```bash
export NVM_DIR="$(mktemp -d)" && mkdir -p "$NVM_DIR/versions/node/v20.1.0/bin"
cargo run --bin nvm -- version 20; echo "exit=$?"
cargo run --bin nvm -- version 16; echo "exit=$?"
rm -rf "$NVM_DIR"; unset NVM_DIR
```

Expected: `v20.1.0` with `exit=0`, then `N/A` on stderr with `exit=3`.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src tests
git commit -S -m "feat(cli): add nvmrc and nvm binaries with the version command"
```

---

## Self-review against the spec

- **Spec coverage:** section 4.2 types (`Version`, `VersionPattern`,
  `VersionFloor`, alias targets as plain strings for now), 4.3 resolution with
  loop exit 8, section 5 floor (all spike cases are tests), section 6 exit
  codes 0, 1, 3, 7 and 8, section 3 layering (`domain`, `ports`, `adapters`,
  `commands`, `cli`) and the `nvmrc` and `nvm` binaries from section 2.
  Deferred on purpose to later plans: `AliasTarget` as an enum with `System`
  and `Lts` (needs the `lts/*` aliases from `ls-remote`), the built-in `node`
  and `stable` aliases, `nvm-exec`, `init`, install, conflict detection.
- **Known deviation to pin later:** `nvm version <missing>` prints `N/A` on
  stdout in `nvm.sh`; this plan prints it on stderr. Plan 7's compatibility
  contract test decides which is right and fixes it.
- **Spec correction made while planning:** io.js directories are
  `versions/io.js/vX.Y.Z` (prefix stripped), not `iojs-vX.Y.Z`; the spec was
  updated.
- **Type consistency:** names match across tasks (`NvmExitCode`, `Version`,
  `VersionPattern`, `VersionFloor`, `AliasStore`, `FsAliasStore`, `Context`).
