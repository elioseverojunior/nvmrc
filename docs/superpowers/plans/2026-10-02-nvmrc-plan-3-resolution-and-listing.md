# nvmrc Plan 3: Resolution Model and Local Listing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish the local half of nvm: `ls` (with patterns, `--no-alias`,
the system node) and the alias listing (`nvm alias`, `nvm alias <prefix>`),
on a resolution model that knows `stable`, `unstable`, `iojs` and `system`.

**Architecture:** Same layering as Plans 1 and 2. New pure modules in
`domain/` format rows and alias lines and derive the implicit aliases; a new
`Process` port runs the system `node --version`; `commands::ls` and
`commands::aliases` assemble the output; `Output` now carries the exit status
so `nvm ls` can print rows and still exit 3.

**Tech Stack:** Rust 2024 edition (MSRV 1.88), `clap`, `thiserror`,
`tempfile` (dev only). No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-02-nvmrc-design.md` (sections 3, 4,
6 and 11). This plan starts from the final state of
`2026-10-02-nvmrc-plan-2-ports-and-local-commands.md` (branch
`feat/plan-2-ports-and-local-commands`).

**Ground truth:** every expected output in this plan was captured by running
the real `nvm.sh` (the reference repository) against the same fixtures, with
stdout piped so it prints without colors. The end-to-end tests of Task 8 repeat
those fixtures byte for byte.

**Revised roadmap:** Plan 4 network (`ls-remote`, `cache`, mirror, checksum,
the `lts/*` aliases); Plan 5 `install` and `uninstall`; Plan 6 shell (`use`,
`deactivate`, `exec`, `run`, `init`, `nvm-exec`, `.nvmrc` semantics); Plan 7
colors and terminal detection (`set-colors`, `tput`/TTY checks, italics; until
then every listing is plain, which is what `nvm.sh` prints to a pipe); Plan 8
`doctor` and `migrate`; Plan 9 compatibility contract and CI.

**Carried over from the Plan 2 final review:** alias files with comments or a
leading blank line, the `v` prefix for io.js names, I/O errors that name their
path, the `"N/A"`/`"∞"` sentinel strings (now the `Shown` type), and the
`which 3` / `ls 3` behaviour where a plain number also matches io.js. Still
deferred: a `Layout` value for version paths (Plan 5), a stricter
`FakeFileSystem` and symlinks (Plan 5), non-UTF-8 components in `classify`,
and the legacy `$NVM_DIR/vX.Y.Z` layout.

## Global Constraints

- Rust edition 2024, `rust-version = "1.88.0"`; no APIs newer than 1.88. The
  development toolchain is pinned separately by `rust-toolchain.toml`.
- Run cargo through the rustup proxies (Homebrew's `rust` formula ships a
  `cargo` that ignores `rust-toolchain.toml`).
- `.cargo/config.toml` sets `-D warnings` for every workspace build, test and
  check, so a dead-code or unused-import warning in any intermediate task is a
  build error. `Cargo.toml` must not declare any `[profile.*]` table and does
  not change in this plan.
- No async I/O, no workspace, no plugin system, no new dependencies.
- Files under 300 lines (tests included), functions under 30 lines, cyclomatic
  complexity under 10, meaningful names without abbreviations.
- Errors use `thiserror` in the library; only the binaries touch
  `std::process::ExitCode`. Exit codes: 0 success, 1 generic failure, 2 no
  such `lts/*` alias, 3 invalid or unknown version (and `nvm ls` finding
  nothing), 7 below the version floor, 8 alias loop, 55 unsupported option,
  127 usage error or missing system node.
- Commands return an `Output { stdout, stderr, status }` or a `CliError`; the
  CLI prints stderr first, then stdout, adds a newline to each non-empty
  stream, finishes with `Output::status`, and a failed stdout write exits 1.
- `Context::new(fs, env)` builds a context that cannot run programs;
  `.with_process(&dyn Process)` adds the `Process` port. Nothing else
  constructs a `Context` by struct literal.
- Layout compatible with nvm: `$NVM_DIR/versions/node/vX.Y.Z`,
  `$NVM_DIR/versions/io.js/vX.Y.Z` (the `iojs-` prefix is stripped on disk),
  `$NVM_DIR/alias/<name>` and `$NVM_DIR/alias/lts/<name>`.
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

### Task 1: Alias files like nvm, `iojs-` prefix, I/O errors with paths

**Files:**

- Modify: `src/adapters/fs_alias_store.rs`, `src/commands/which.rs`,
  `src/commands/unalias.rs`, `src/commands/alias.rs`, `src/error.rs`

**Interfaces:**

- Produces: `FsAliasStore::target` now reads the first meaningful line of an
  alias file (everything after `#` is a comment, blank lines are skipped,
  surrounding spaces are trimmed); `CliError::Io { path, source }` with the
  constructor `CliError::io(&Path, io::Error)` replaces `CliError::Io(io::Error)`
  and displays as `<path>: <os error>`; `which`'s message shows `iojs-3` as
  `iojs-v3`.

- [ ] **Step 1: Write the failing tests**

Apply to the test module of `src/adapters/fs_alias_store.rs`:

```diff
--- a/src/adapters/fs_alias_store.rs
+++ b/src/adapters/fs_alias_store.rs
@@ -30,6 +30,17 @@
     }
 
     #[test]
+    fn comments_and_blank_lines_are_skipped() {
+        let fs = FakeFileSystem::default()
+            .with_file("/nvm/alias/commented", "v20 # the current LTS\n")
+            .with_file("/nvm/alias/blank-first", "\n\n  v18  \nv16\n")
+            .with_file("/nvm/alias/only-comment", "# nothing here\n\n");
+        assert_eq!(store(&fs).target("commented"), Some("v20".to_owned()));
+        assert_eq!(store(&fs).target("blank-first"), Some("v18".to_owned()));
+        assert_eq!(store(&fs).target("only-comment"), None);
+    }
+
+    #[test]
     fn rejects_path_traversal_in_alias_names() {
         let fs = FakeFileSystem::default().with_file("/nvm/secret", "v1.0.0");
         assert_eq!(store(&fs).target("../secret"), None);
```

Apply to the test module of `src/commands/which.rs`:

```diff
--- a/src/commands/which.rs
+++ b/src/commands/which.rs
@@ -32,6 +32,14 @@
     fn node_is_the_latest_installed_node() {
         let output = which("node").unwrap();
         assert_eq!(output, Output::stdout("/n/versions/node/v20.1.0/bin/node"));
+    }
+
+    #[test]
+    fn the_v_prefix_is_added_after_the_iojs_prefix() {
+        assert_eq!(with_v_prefix("20"), "v20");
+        assert_eq!(with_v_prefix("iojs-3"), "iojs-v3");
+        assert_eq!(with_v_prefix("iojs-v3"), "iojs-v3");
+        assert_eq!(with_v_prefix("lts/iron"), "lts/iron");
     }
 
     #[test]
```

Apply to the test module of `src/error.rs`:

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -38,7 +38,15 @@
         assert_eq!(no_system_node.exit_code(), NvmExitCode::NotFound);
         let invalid = CliError::InvalidArgument("x".into());
         assert_eq!(invalid.exit_code(), NvmExitCode::Failure);
-        let io_error = CliError::from(std::io::Error::from(std::io::ErrorKind::NotFound));
+        let source = std::io::Error::from(std::io::ErrorKind::NotFound);
+        let io_error = CliError::io(Path::new("/n/alias/work"), source);
         assert_eq!(io_error.exit_code(), NvmExitCode::Failure);
     }
+
+    #[test]
+    fn io_errors_name_the_path() {
+        let source = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
+        let message = CliError::io(Path::new("/n/alias/work"), source).to_string();
+        assert!(message.starts_with("/n/alias/work: "), "{message}");
+    }
 }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with an error such as "no function or associated
item named `io` found for enum `CliError`".

- [ ] **Step 3: Write the implementation**

Apply to `src/adapters/fs_alias_store.rs` (above the test module):

```diff
--- a/src/adapters/fs_alias_store.rs
+++ b/src/adapters/fs_alias_store.rs
@@ -34,8 +34,17 @@
             return None;
         }
         let contents = self.fs.read_to_string(&self.alias_dir.join(name)).ok()?;
-        let line = contents.lines().next()?.trim();
-        (!line.is_empty()).then(|| line.to_owned())
+        first_meaningful_line(&contents)
     }
 }
 
+/// Like `nvm_print_alias_file`: everything after a `#` is a comment, blank
+/// lines are skipped, and the first line left is the target.
+fn first_meaningful_line(contents: &str) -> Option<String> {
+    contents
+        .lines()
+        .map(|line| line.split('#').next().unwrap_or_default().trim())
+        .find(|line| !line.is_empty())
+        .map(str::to_owned)
+}
+
```

Apply to `src/commands/alias.rs` (above the test module):

```diff
--- a/src/commands/alias.rs
+++ b/src/commands/alias.rs
@@ -22,10 +22,15 @@
     validate_name(name)?;
     let version = version_text(context, target)?;
     let alias_dir = context.alias_dir()?;
-    context.fs.create_dir_all(&alias_dir)?;
     context
         .fs
-        .write_file(&alias_dir.join(name), &format!("{target}\n"))?;
+        .create_dir_all(&alias_dir)
+        .map_err(|source| CliError::io(&alias_dir, source))?;
+    let alias_file = alias_dir.join(name);
+    context
+        .fs
+        .write_file(&alias_file, &format!("{target}\n"))
+        .map_err(|source| CliError::io(&alias_file, source))?;
     let output = Output::stdout(format_line(name, target, &version));
     if version == "N/A" {
         return Ok(output.with_stderr(format!("! WARNING: Version '{target}' does not exist.")));
```

Apply to `src/commands/unalias.rs` (above the test module):

```diff
--- a/src/commands/unalias.rs
+++ b/src/commands/unalias.rs
@@ -26,7 +26,10 @@
         return missing(name);
     }
     let original = context.alias_store()?.target(name).unwrap_or_default();
-    context.fs.remove_file(&path)?;
+    context
+        .fs
+        .remove_file(&path)
+        .map_err(|source| CliError::io(&path, source))?;
     Ok(Output::stdout(format!(
         "Deleted alias {name} - restore it with `nvm alias \"{name}\" \"{original}\"`"
     )))
```

Apply to `src/commands/which.rs` (above the test module):

```diff
--- a/src/commands/which.rs
+++ b/src/commands/which.rs
@@ -69,10 +69,15 @@
         .join("node"))
 }
 
-/// `20` is shown as `v20`, like `nvm_ensure_version_prefix`.
+/// `20` is shown as `v20` and `iojs-3` as `iojs-v3`, like
+/// `nvm_ensure_version_prefix`.
 fn with_v_prefix(name: &str) -> String {
-    if name.starts_with(|first: char| first.is_ascii_digit()) {
-        format!("v{name}")
+    let (prefix, rest) = match name.strip_prefix("iojs-") {
+        Some(rest) => ("iojs-", rest),
+        None => ("", name),
+    };
+    if rest.starts_with(|first: char| first.is_ascii_digit()) {
+        format!("{prefix}v{rest}")
     } else {
         name.to_owned()
     }
```

Apply to `src/error.rs` (above the test module):

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -1,4 +1,6 @@
 //! Typed errors and the single mapping from errors to process exit codes.
+
+use std::path::{Path, PathBuf};
 
 use thiserror::Error;
 
@@ -64,11 +66,23 @@
     /// A rejected argument, with the message to print.
     #[error("{0}")]
     InvalidArgument(String),
-    #[error(transparent)]
-    Io(#[from] std::io::Error),
+    /// A failed file-system change, naming the path it was made on.
+    #[error("{}: {source}", path.display())]
+    Io {
+        path: PathBuf,
+        source: std::io::Error,
+    },
 }
 
 impl CliError {
+    #[must_use]
+    pub fn io(path: &Path, source: std::io::Error) -> Self {
+        Self::Io {
+            path: path.to_path_buf(),
+            source,
+        }
+    }
+
     #[must_use]
     pub fn exit_code(&self) -> NvmExitCode {
         match self {
@@ -76,7 +90,7 @@
             Self::NvmDirUnresolved
             | Self::VersionNotInstalled(_)
             | Self::InvalidArgument(_)
-            | Self::Io(_) => NvmExitCode::Failure,
+            | Self::Io { .. } => NvmExitCode::Failure,
             Self::Usage(_) | Self::SystemNodeNotFound => NvmExitCode::NotFound,
             Self::Floor(_) => NvmExitCode::BelowVersionFloor,
             Self::Alias(_) => NvmExitCode::AliasLoop,
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 143 unit tests plus the end-to-end tests.

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
git commit -S -m "fix(aliases): read alias files like nvm and name the path in I/O errors"
```

---

### Task 2: Order by number across flavors; plain patterns match both

**Files:**

- Modify: `src/domain/version.rs`

**Interfaces:**

- Produces: `Version`'s `Ord` compares major, minor and patch first and the
  flavor only to break a tie, so `iojs-v3.0.0` sorts between `v2.9.0` and
  `v4.0.0` (what `nvm ls` prints); `VersionPattern.flavor` is
  `Option<Flavor>` (`Some(IoJs)` only with an `iojs-` prefix), and a plain
  pattern such as `3` or `v3.0.0` matches io.js versions too, as in `nvm.sh`
  (`nvm ls 3`, `nvm which 3`).

- [ ] **Step 1: Write the failing tests**

Apply to the test module of `src/domain/version.rs`:

```diff
--- a/src/domain/version.rs
+++ b/src/domain/version.rs
@@ -74,8 +74,30 @@
     }
 
     #[test]
-    fn pattern_does_not_match_across_flavors() {
+    fn a_plain_pattern_matches_both_flavors() {
         let pattern: VersionPattern = "3".parse().unwrap();
-        assert!(!pattern.matches(&version("iojs-v3.0.0")));
+        assert!(pattern.matches(&version("iojs-v3.0.0")));
+        assert!(pattern.matches(&version("v3.1.0")));
+    }
+
+    #[test]
+    fn an_iojs_pattern_only_matches_iojs() {
+        let pattern: VersionPattern = "iojs-3".parse().unwrap();
+        assert!(pattern.matches(&version("iojs-v3.0.0")));
+        assert!(!pattern.matches(&version("v3.1.0")));
+        assert_eq!(pattern.flavor, Some(Flavor::IoJs));
+    }
+
+    #[test]
+    fn iojs_sorts_among_node_versions_by_number() {
+        let mut versions = [version("v4.0.0"), version("iojs-v3.0.0"), version("v2.9.0")];
+        versions.sort();
+        let sorted: Vec<String> = versions.iter().map(ToString::to_string).collect();
+        assert_eq!(sorted, ["v2.9.0", "iojs-v3.0.0", "v4.0.0"]);
+    }
+
+    #[test]
+    fn equal_numbers_are_ordered_node_before_iojs() {
+        assert!(version("v3.0.0") < version("iojs-v3.0.0"));
     }
 }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with a mismatched type: `VersionPattern::flavor` is
still a `Flavor`, not an `Option<Flavor>`.

- [ ] **Step 3: Write the implementation**

Apply to `src/domain/version.rs` (above the test module):

```diff
--- a/src/domain/version.rs
+++ b/src/domain/version.rs
@@ -1,5 +1,6 @@
 //! Node and io.js versions, and partial patterns such as `20` or `v20.1`.
 
+use std::cmp::Ordering;
 use std::fmt;
 use std::str::FromStr;
 
@@ -11,8 +12,9 @@
     IoJs,
 }
 
-/// A fully specified version. Ordering is by flavor, then numerically.
-#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
+/// A fully specified version. Ordering is numeric, with the flavor only
+/// breaking ties, so `iojs-v3.0.0` sorts between `v2.9.0` and `v4.0.0`.
+#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
 pub struct Version {
     pub flavor: Flavor,
     pub major: u64,
@@ -21,19 +23,21 @@
 }
 
 /// A version with optional minor and patch, used to select installed versions.
+/// Without an `iojs-` prefix the flavor is `None` and the pattern matches both
+/// flavors, as in `nvm.sh` (`nvm ls 3` lists `iojs-v3.0.0`).
 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 pub struct VersionPattern {
-    pub flavor: Flavor,
+    pub flavor: Option<Flavor>,
     pub major: u64,
     pub minor: Option<u64>,
     pub patch: Option<u64>,
 }
 
-fn parse_parts(input: &str) -> Result<(Flavor, Vec<u64>), VersionError> {
+fn parse_parts(input: &str) -> Result<(Option<Flavor>, Vec<u64>), VersionError> {
     let invalid = || VersionError::Invalid(input.to_owned());
     let (flavor, rest) = match input.strip_prefix("iojs-") {
-        Some(rest) => (Flavor::IoJs, rest),
-        None => (Flavor::Node, input),
+        Some(rest) => (Some(Flavor::IoJs), rest),
+        None => (None, input),
     };
     let rest = rest.strip_prefix('v').unwrap_or(rest);
     let parts = rest
@@ -57,7 +61,7 @@
     fn from_str(input: &str) -> Result<Self, Self::Err> {
         match parse_parts(input)? {
             (flavor, parts) if parts.len() == 3 => Ok(Self {
-                flavor,
+                flavor: flavor.unwrap_or(Flavor::Node),
                 major: parts[0],
                 minor: parts[1],
                 patch: parts[2],
@@ -78,6 +82,20 @@
             minor: parts.get(1).copied(),
             patch: parts.get(2).copied(),
         })
+    }
+}
+
+impl Ord for Version {
+    fn cmp(&self, other: &Self) -> Ordering {
+        self.triple()
+            .cmp(&other.triple())
+            .then(self.flavor.cmp(&other.flavor))
+    }
+}
+
+impl PartialOrd for Version {
+    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
+        Some(self.cmp(other))
     }
 }
 
@@ -119,7 +137,7 @@
 impl VersionPattern {
     #[must_use]
     pub fn matches(&self, version: &Version) -> bool {
-        version.flavor == self.flavor
+        self.flavor.is_none_or(|flavor| flavor == version.flavor)
             && version.major == self.major
             && self.minor.is_none_or(|minor| minor == version.minor)
             && self.patch.is_none_or(|patch| patch == version.patch)
@@ -129,7 +147,7 @@
     #[must_use]
     pub fn lowest(&self) -> Version {
         Version {
-            flavor: self.flavor,
+            flavor: self.flavor.unwrap_or(Flavor::Node),
             major: self.major,
             minor: self.minor.unwrap_or(0),
             patch: self.patch.unwrap_or(0),
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 146 unit tests plus the end-to-end tests.

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
git commit -S -m "refactor(version): order by number across flavors and match plain patterns in both"
```

---

### Task 3: Implicit aliases and the `Shown` type

**Files:**

- Create: `src/domain/implicit.rs`
- Modify: `src/domain/mod.rs`, `src/commands/resolve.rs` (becomes
  `src/commands/resolve/mod.rs` plus `src/commands/resolve/tests.rs`),
  `src/commands/alias.rs`

**Interfaces:**

- Consumes: `Version`, `Flavor`, `commands::resolve::resolve_installed`.
- Produces: `domain::implicit::{IMPLICIT_ALIASES, MajorMinor, Implicit,
  derive, highest_in, destination}`; `commands::resolve::{Shown, shown}` where
  `Shown` is `Version(Version)`, `System`, `NotAvailable` or `Infinite`
  (displayed as the version, `system`, `N/A`, `∞`) with `is_available()`.
- Behaviour (verified against `nvm.sh`): from 1.0 on every Node release line
  is stable and the highest wins; below 1.0 even minors are stable and odd
  ones unstable; `node` resolves through `stable`; `stable` and `unstable`
  resolve to the highest patch of their line; `iojs` to the highest io.js.
  `destination` is what `nvm alias <name>` shows as the target: `node` is
  `stable`, `stable` is `20.10`, `unstable` is `N/A` when no odd 0.x line is
  installed, `iojs` is `iojs-v3.0`; `stable` shows nothing when nothing is
  installed.
- [ ] **Step 0: Split the resolve tests into their own file (no behaviour change)**

`src/commands/resolve.rs` would pass 300 lines in this task. Run:

```bash
mkdir src/commands/resolve && git mv src/commands/resolve.rs src/commands/resolve/mod.rs
```

Cut everything inside the `mod tests { ... }` block of
`src/commands/resolve/mod.rs`, remove one level of indentation from it, and
paste it into the new file `src/commands/resolve/tests.rs` (it starts with
`use super::*;`). Replace the block in `src/commands/resolve/mod.rs` with:

```rust
#[cfg(test)]
mod tests;
```

Run: `cargo fmt && cargo test`
Expected: PASS, 146 unit tests plus the end-to-end tests, as at the end of
Task 2.

- [ ] **Step 1: Declare the new modules**

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -1,6 +1,7 @@
 pub mod alias;
 pub mod current;
 pub mod floor;
+pub mod implicit;
 pub mod nvmrc;
 pub mod path_search;
 pub mod version;
```

- [ ] **Step 2: Write the failing tests**

Apply to `src/commands/resolve/tests.rs`:

```diff
--- a/src/commands/resolve/tests.rs
+++ b/src/commands/resolve/tests.rs
@@ -44,6 +44,54 @@
         .with_file("/n/versions/io.js/v2.5.0/bin/node", "");
     let resolved = resolve_with(&fs, "iojs").unwrap();
     assert_eq!(resolved, Resolved::Installed(version("iojs-v3.0.0")));
+}
+
+#[test]
+fn stable_is_the_highest_patch_of_the_highest_release_line() {
+    let resolved = resolve_with(&installed(), "stable").unwrap();
+    assert_eq!(resolved, Resolved::Installed(version("v20.10.0")));
+}
+
+#[test]
+fn unstable_needs_an_old_odd_release_line() {
+    let modern = resolve_with(&installed(), "unstable").unwrap();
+    assert_eq!(
+        modern,
+        Resolved::Missing {
+            resolved: "unstable".into()
+        }
+    );
+    let old = FakeFileSystem::default()
+        .with_file("/n/versions/node/v0.10.48/bin/node", "")
+        .with_file("/n/versions/node/v0.11.16/bin/node", "")
+        .with_file("/n/versions/node/v4.2.0/bin/node", "");
+    assert_eq!(
+        resolve_with(&old, "unstable").unwrap(),
+        Resolved::Installed(version("v0.11.16"))
+    );
+    assert_eq!(
+        resolve_with(&old, "node").unwrap(),
+        Resolved::Installed(version("v4.2.0"))
+    );
+}
+
+#[test]
+fn shown_tells_versions_system_missing_and_loops_apart() {
+    let fs = installed()
+        .with_file("/n/alias/loop", "loop")
+        .with_file("/usr/bin/node", "");
+    let env = FakeEnv::default()
+        .with_var("NVM_DIR", "/n")
+        .with_var("PATH", "/usr/bin");
+    let context = Context { fs: &fs, env: &env };
+    let texts: Vec<String> = ["20", "system", "16", "loop"]
+        .iter()
+        .map(|name| shown(&context, name).unwrap().to_string())
+        .collect();
+    assert_eq!(texts, ["v20.10.0", "system", "N/A", "∞"]);
+    assert!(shown(&context, "20").unwrap().is_available());
+    assert!(!shown(&context, "loop").unwrap().is_available());
+    assert!(!shown(&context, "16").unwrap().is_available());
 }
 
 #[test]
```

Create `src/domain/implicit.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn versions(names: &[&str]) -> Vec<Version> {
        names.iter().map(|name| name.parse().unwrap()).collect()
    }

    fn line(major: u64, minor: u64) -> Option<MajorMinor> {
        Some(MajorMinor { major, minor })
    }

    #[test]
    fn every_modern_release_line_is_stable_and_the_highest_wins() {
        let installed = versions(&["v20.1.0", "v20.10.0", "v18.9.0"]);
        let implicit = derive(&installed);
        assert_eq!(implicit.stable, line(20, 10));
        assert_eq!(implicit.unstable, None);
    }

    #[test]
    fn below_1_0_even_minors_are_stable_and_odd_ones_unstable() {
        let installed = versions(&["v0.10.48", "v0.11.16", "v0.12.18", "v4.2.0"]);
        let implicit = derive(&installed);
        assert_eq!(implicit.stable, line(4, 2));
        assert_eq!(implicit.unstable, line(0, 11));
    }

    #[test]
    fn only_old_versions_leave_the_highest_even_minor_stable() {
        let installed = versions(&["v0.10.48", "v0.11.16", "v0.12.18"]);
        let implicit = derive(&installed);
        assert_eq!(implicit.stable, line(0, 12));
        assert_eq!(implicit.unstable, line(0, 11));
    }

    #[test]
    fn iojs_uses_its_highest_release_line() {
        let installed = versions(&["iojs-v2.5.0", "iojs-v3.0.0", "v20.1.0"]);
        assert_eq!(derive(&installed).iojs, line(3, 0));
    }

    #[test]
    fn nothing_installed_derives_nothing() {
        assert_eq!(derive(&[]), Implicit::default());
    }

    #[test]
    fn highest_in_picks_the_newest_patch_of_one_line() {
        let installed = versions(&["v20.1.0", "v20.10.0", "v20.10.3", "v20.2.0"]);
        let found = highest_in(
            &installed,
            Flavor::Node,
            MajorMinor {
                major: 20,
                minor: 10,
            },
        );
        assert_eq!(found, Some("v20.10.3".parse().unwrap()));
    }

    #[test]
    fn destinations_match_nvm_alias_output() {
        let installed = versions(&["v20.1.0", "v20.10.0", "iojs-v3.0.0"]);
        assert_eq!(destination(&installed, "node"), Some("stable".to_owned()));
        assert_eq!(destination(&installed, "stable"), Some("20.10".to_owned()));
        assert_eq!(destination(&installed, "unstable"), Some("N/A".to_owned()));
        assert_eq!(
            destination(&installed, "iojs"),
            Some("iojs-v3.0".to_owned())
        );
        assert_eq!(destination(&installed, "default"), None);
    }

    #[test]
    fn with_nothing_installed_stable_shows_nothing_and_the_rest_show_na() {
        assert_eq!(destination(&[], "stable"), None);
        assert_eq!(destination(&[], "node"), Some("stable".to_owned()));
        assert_eq!(destination(&[], "iojs"), Some("N/A".to_owned()));
        assert_eq!(destination(&[], "unstable"), Some("N/A".to_owned()));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "unresolved import
`crate::domain::implicit`" and "cannot find function `shown`".

- [ ] **Step 4: Write the implementation**

Apply to `src/commands/alias.rs` (above the test module):

```diff
--- a/src/commands/alias.rs
+++ b/src/commands/alias.rs
@@ -4,7 +4,7 @@
 //! shares its formatting. Output is always plain, as `nvm.sh` prints when
 //! stdout is not a terminal.
 
-use crate::commands::resolve::{Resolved, resolve_installed};
+use crate::commands::resolve::{Shown, shown};
 use crate::commands::{Output, unalias};
 use crate::context::Context;
 use crate::error::CliError;
@@ -20,7 +20,7 @@
         return unalias::run(context, &[name.to_owned()]);
     }
     validate_name(name)?;
-    let version = version_text(context, target)?;
+    let version = shown(context, target)?;
     let alias_dir = context.alias_dir()?;
     context
         .fs
@@ -32,7 +32,7 @@
         .write_file(&alias_file, &format!("{target}\n"))
         .map_err(|source| CliError::io(&alias_file, source))?;
     let output = Output::stdout(format_line(name, target, &version));
-    if version == "N/A" {
+    if version == Shown::NotAvailable {
         return Ok(output.with_stderr(format!("! WARNING: Version '{target}' does not exist.")));
     }
     Ok(output)
@@ -51,24 +51,9 @@
     Err(CliError::InvalidArgument(message))
 }
 
-/// What `target` resolves to now: a version, `system`, `N/A` when nothing
-/// installed matches, or `∞` when its alias chain loops.
-fn version_text(context: &Context<'_>, target: &str) -> Result<String, CliError> {
-    match resolve_installed(context, target) {
-        Ok(Resolved::Installed(version)) => Ok(version.to_string()),
-        Ok(Resolved::System) => Ok("system".to_owned()),
-        Ok(Resolved::Missing { .. }) => Ok("N/A".to_owned()),
-        Err(CliError::Alias(_)) => Ok("∞".to_owned()),
-        Err(error) => Err(error),
-    }
-}
-
-fn format_line(alias: &str, target: &str, version: &str) -> String {
-    let marker = if matches!(version, "N/A" | "∞") {
-        ""
-    } else {
-        " *"
-    };
+fn format_line(alias: &str, target: &str, shown: &Shown) -> String {
+    let version = shown.to_string();
+    let marker = if shown.is_available() { " *" } else { "" };
     if target == version {
         format!("{alias} -> {version}{marker}")
     } else {
```

Apply to `src/commands/resolve/mod.rs` (above the test module):

```diff
--- a/src/commands/resolve/mod.rs
+++ b/src/commands/resolve/mod.rs
@@ -4,7 +4,10 @@
 use std::path::PathBuf;
 
 use crate::context::Context;
+use std::fmt;
+
 use crate::domain::alias;
+use crate::domain::implicit::{derive, highest_in};
 use crate::domain::path_search::find_in_dirs;
 use crate::domain::version::{Flavor, Version, VersionPattern};
 use crate::error::CliError;
@@ -48,12 +51,15 @@
     Ok(find_in_dirs(context.fs, outside_nvm, "node"))
 }
 
-/// `node` and `iojs` are built-in aliases for the latest installed version of
-/// that flavor; anything else is matched as a version pattern.
+/// `node`, `stable`, `unstable` and `iojs` are implicit aliases for the latest
+/// installed release line of their kind; anything else is matched as a version
+/// pattern.
 fn find_installed(resolved: &str, installed: &[Version]) -> Option<Version> {
+    let implicit = derive(installed);
     match resolved {
-        "node" => latest_of(installed, Flavor::Node),
-        "iojs" => latest_of(installed, Flavor::IoJs),
+        "node" | "stable" => highest_in(installed, Flavor::Node, implicit.stable?),
+        "unstable" => highest_in(installed, Flavor::Node, implicit.unstable?),
+        "iojs" => highest_in(installed, Flavor::IoJs, implicit.iojs?),
         other => other
             .parse::<VersionPattern>()
             .ok()
@@ -61,11 +67,47 @@
     }
 }
 
-fn latest_of(installed: &[Version], flavor: Flavor) -> Option<Version> {
-    installed
-        .iter()
-        .filter(|version| version.flavor == flavor)
-        .max()
-        .copied()
+/// What a name resolves to, as `nvm alias` and `nvm ls` print it.
+#[derive(Debug, PartialEq, Eq)]
+pub enum Shown {
+    Version(Version),
+    System,
+    /// Nothing installed matches: `N/A`.
+    NotAvailable,
+    /// The alias chain loops: `∞`.
+    Infinite,
 }
 
+impl fmt::Display for Shown {
+    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
+        match self {
+            Self::Version(version) => version.fmt(formatter),
+            Self::System => formatter.write_str("system"),
+            Self::NotAvailable => formatter.write_str("N/A"),
+            Self::Infinite => formatter.write_str("∞"),
+        }
+    }
+}
+
+impl Shown {
+    /// Resolved names get a trailing ` *` in `nvm alias` and `nvm ls` output.
+    #[must_use]
+    pub fn is_available(&self) -> bool {
+        matches!(self, Self::Version(_) | Self::System)
+    }
+}
+
+/// Like [`resolve_installed`], but a loop is a value, not an error.
+///
+/// # Errors
+/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
+pub fn shown(context: &Context<'_>, name: &str) -> Result<Shown, CliError> {
+    match resolve_installed(context, name) {
+        Ok(Resolved::Installed(version)) => Ok(Shown::Version(version)),
+        Ok(Resolved::System) => Ok(Shown::System),
+        Ok(Resolved::Missing { .. }) => Ok(Shown::NotAvailable),
+        Err(CliError::Alias(_)) => Ok(Shown::Infinite),
+        Err(error) => Err(error),
+    }
+}
+
```

Insert above the `#[cfg(test)]` line of `src/domain/implicit.rs`:

```rust
//! The implicit aliases `node`, `stable`, `unstable` and `iojs`: they have no
//! alias file and are derived from what is installed.

use std::fmt;

use crate::domain::version::{Flavor, Version};

/// The order `nvm alias` lists them in.
pub const IMPLICIT_ALIASES: [&str; 4] = ["iojs", "node", "stable", "unstable"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MajorMinor {
    pub major: u64,
    pub minor: u64,
}

impl fmt::Display for MajorMinor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)
    }
}

impl MajorMinor {
    fn of(version: &Version) -> Self {
        Self {
            major: version.major,
            minor: version.minor,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Implicit {
    pub stable: Option<MajorMinor>,
    pub unstable: Option<MajorMinor>,
    pub iojs: Option<MajorMinor>,
}

/// From 1.0 on every Node release line is stable. Below it, as in the old
/// scheme, even minors are stable and odd minors are unstable. The highest
/// line of each kind wins; io.js has a single kind.
#[must_use]
pub fn derive(installed: &[Version]) -> Implicit {
    let mut implicit = Implicit {
        iojs: lines_of(installed, Flavor::IoJs).into_iter().max(),
        ..Implicit::default()
    };
    for line in lines_of(installed, Flavor::Node) {
        if line.major >= 1 || line.minor % 2 == 0 {
            implicit.stable = Some(line);
        } else {
            implicit.unstable = Some(line);
        }
    }
    implicit
}

/// The release lines of one flavor, ascending and without repeats.
fn lines_of(installed: &[Version], flavor: Flavor) -> Vec<MajorMinor> {
    let mut lines: Vec<MajorMinor> = installed
        .iter()
        .filter(|version| version.flavor == flavor)
        .map(MajorMinor::of)
        .collect();
    lines.sort();
    lines.dedup();
    lines
}

/// The highest installed version of `flavor` on the release line `line`.
#[must_use]
pub fn highest_in(installed: &[Version], flavor: Flavor, line: MajorMinor) -> Option<Version> {
    installed
        .iter()
        .filter(|version| version.flavor == flavor && MajorMinor::of(version) == line)
        .max()
        .copied()
}

/// What `nvm alias <name>` shows as the target of an implicit alias, or `None`
/// when nothing is shown (`stable` before anything is installed).
#[must_use]
pub fn destination(installed: &[Version], name: &str) -> Option<String> {
    let implicit = derive(installed);
    match name {
        "node" => Some("stable".to_owned()),
        "stable" => implicit.stable.map(|line| line.to_string()),
        "unstable" => Some(
            implicit
                .unstable
                .map_or_else(|| "N/A".to_owned(), |line| line.to_string()),
        ),
        "iojs" => Some(
            implicit
                .iojs
                .map_or_else(|| "N/A".to_owned(), |line| format!("iojs-v{line}")),
        ),
        _ => None,
    }
}

```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 157 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(resolve): resolve stable and unstable and show resolution results as a type"
```

---

### Task 4: The `Process` port and the system node version

**Files:**

- Create: `src/adapters/std_process.rs`, `src/adapters/no_process.rs`
- Modify: `src/ports/mod.rs`, `src/adapters/mod.rs`, `src/fakes.rs`,
  `src/context.rs`, `src/commands/resolve/mod.rs`,
  `src/commands/resolve/tests.rs`, `src/cli/mod.rs`, and every file that
  builds a `Context` (mechanical, Step 0)

**Interfaces:**

- Produces: `ports::{Process, ProcessOutput { success, stdout }}` with
  `Process::run(&self, &Path, &[&str]) -> io::Result<ProcessOutput>`;
  `adapters::std_process::StdProcess`, `adapters::no_process::NoProcess` (runs
  nothing, `ErrorKind::Unsupported`); test-only `fakes::FakeProcess` with
  `with_output(program, stdout)` and `with_failure(program)`;
  `Context::new(fs, env)`, `Context::with_process(&dyn Process)`,
  `Context::process()`; `commands::resolve::system_version(&Context) ->
  Result<Option<String>, CliError>` (runs the system node with `--version`).
- `run_from_env` builds `Context::new(&StdFileSystem, &StdEnv)
  .with_process(&StdProcess)`.
- [ ] **Step 0: Replace every `Context { fs, env }` literal with `Context::new`**

`Context` gets a third, private field in this task, so no struct literal can
build it any more. The edit is mechanical (24 places, in tests and in
`run_from_env`). Run from the repository root:

```bash
perl -0pi -e 's|Context\s*\{\s*fs(?::\s*([^,]+?))?\s*,\s*env(?::\s*([^,}]+?))?\s*,?\s*\}|my ($f,$e)=(defined $1 ? $1 : "fs", defined $2 ? $2 : "env"); "Context::new($f, $e)"|ge' \
  $(grep -rl 'Context {' src tests)
cargo fmt
grep -rn 'Context {' src tests
```

Expected: the `grep` prints nothing. The crate does not compile until the
steps below add `Context::new`; the diffs of this task are shown against that
migrated state.

- [ ] **Step 1: Declare the new modules**

Apply to `src/adapters/mod.rs`:

```diff
--- a/src/adapters/mod.rs
+++ b/src/adapters/mod.rs
@@ -1,3 +1,5 @@
 pub mod fs_alias_store;
+pub mod no_process;
 pub mod std_env;
 pub mod std_fs;
+pub mod std_process;
```

- [ ] **Step 2: Write the failing tests**

Create `src/adapters/no_process.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_nothing() {
        let error = NoProcess.run(Path::new("/bin/true"), &[]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}
```

Apply to `src/commands/resolve/tests.rs`:

```diff
--- a/src/commands/resolve/tests.rs
+++ b/src/commands/resolve/tests.rs
@@ -1,6 +1,6 @@
 use super::*;
 use crate::error::AliasError;
-use crate::fakes::{FakeEnv, FakeFileSystem};
+use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};
 
 fn resolve_with(fs: &FakeFileSystem, name: &str) -> Result<Resolved, CliError> {
     resolve_on_path(fs, "/nonexistent", name)
@@ -44,6 +44,38 @@
         .with_file("/n/versions/io.js/v2.5.0/bin/node", "");
     let resolved = resolve_with(&fs, "iojs").unwrap();
     assert_eq!(resolved, Resolved::Installed(version("iojs-v3.0.0")));
+}
+
+fn version_of_system_node(process: &FakeProcess) -> Option<String> {
+    let fs = FakeFileSystem::default().with_file("/usr/bin/node", "");
+    let env = FakeEnv::default()
+        .with_var("NVM_DIR", "/n")
+        .with_var("PATH", "/usr/bin");
+    let context = Context::new(&fs, &env).with_process(process);
+    system_version(&context).unwrap()
+}
+
+#[test]
+fn the_system_version_is_what_node_prints() {
+    let process = FakeProcess::default().with_output("/usr/bin/node", "v22.1.0\n");
+    assert_eq!(version_of_system_node(&process), Some("v22.1.0".to_owned()));
+}
+
+#[test]
+fn the_system_version_is_none_when_node_fails_or_cannot_run() {
+    let failing = FakeProcess::default().with_failure("/usr/bin/node");
+    assert_eq!(version_of_system_node(&failing), None);
+    assert_eq!(version_of_system_node(&FakeProcess::default()), None);
+}
+
+#[test]
+fn without_a_system_node_there_is_no_system_version() {
+    let fs = FakeFileSystem::default();
+    let env = FakeEnv::default()
+        .with_var("NVM_DIR", "/n")
+        .with_var("PATH", "/usr/bin");
+    let context = Context::new(&fs, &env);
+    assert_eq!(system_version(&context).unwrap(), None);
 }
 
 #[test]
```

Apply to the test module of `src/fakes.rs`:

```diff
--- a/src/fakes.rs
+++ b/src/fakes.rs
@@ -72,6 +72,20 @@
     }
 
     #[test]
+    fn fake_process_answers_by_program_path() {
+        let process = FakeProcess::default()
+            .with_output("/usr/bin/node", "v22.1.0\n")
+            .with_failure("/usr/bin/broken");
+        let ok = process
+            .run(Path::new("/usr/bin/node"), &["--version"])
+            .unwrap();
+        assert_eq!((ok.success, ok.stdout.as_str()), (true, "v22.1.0\n"));
+        let failed = process.run(Path::new("/usr/bin/broken"), &[]).unwrap();
+        assert!(!failed.success);
+        assert!(process.run(Path::new("/missing"), &[]).is_err());
+    }
+
+    #[test]
     fn fake_env_returns_the_variables_it_was_given() {
         let env = FakeEnv::default().with_var("HOME", "/home/me");
         assert_eq!(env.var("HOME"), Some("/home/me".to_owned()));
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find trait `Process`",
"cannot find struct `FakeProcess`" and "no function or associated item named
`new` found for struct `Context`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/adapters/no_process.rs`:

```rust
use std::io;
use std::path::Path;

use crate::ports::{Process, ProcessOutput};

/// The `Process` of a `Context` that was not given one: it runs nothing.
pub struct NoProcess;

impl Process for NoProcess {
    fn run(&self, _program: &Path, _args: &[&str]) -> io::Result<ProcessOutput> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

```

Create `src/adapters/std_process.rs`:

```rust
use std::io;
use std::path::Path;
use std::process::Command;

use crate::ports::{Process, ProcessOutput};

pub struct StdProcess;

impl Process for StdProcess {
    fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput> {
        let output = Command::new(program).args(args).output()?;
        Ok(ProcessOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        })
    }
}
```

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -7,6 +7,7 @@
 
 use crate::adapters::std_env::StdEnv;
 use crate::adapters::std_fs::StdFileSystem;
+use crate::adapters::std_process::StdProcess;
 use crate::commands::{self, Output};
 use crate::context::Context;
 use crate::error::{CliError, NvmExitCode};
@@ -109,7 +110,7 @@
 /// Entry point shared by the `nvmrc` and `nvm` binaries.
 #[must_use]
 pub fn run_from_env() -> u8 {
-    let context = Context::new(&StdFileSystem, &StdEnv);
+    let context = Context::new(&StdFileSystem, &StdEnv).with_process(&StdProcess);
     run(
         std::env::args_os(),
         &context,
```

Apply to `src/commands/resolve/mod.rs` (above the test module):

```diff
--- a/src/commands/resolve/mod.rs
+++ b/src/commands/resolve/mod.rs
@@ -49,6 +49,25 @@
         .filter(|directory| !directory.starts_with(&nvm_dir))
         .collect::<Vec<_>>();
     Ok(find_in_dirs(context.fs, outside_nvm, "node"))
+}
+
+/// The version the system `node` reports, or `None` when there is no system
+/// `node` or it does not answer `--version`.
+///
+/// # Errors
+/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
+pub fn system_version(context: &Context<'_>) -> Result<Option<String>, CliError> {
+    let Some(node) = system_node(context)? else {
+        return Ok(None);
+    };
+    let version = context
+        .process()
+        .run(&node, &["--version"])
+        .ok()
+        .filter(|output| output.success)
+        .map(|output| output.stdout.trim().to_owned())
+        .filter(|version| !version.is_empty());
+    Ok(version)
 }
 
 /// `node`, `stable`, `unstable` and `iojs` are implicit aliases for the latest
```

Apply to `src/context.rs` (above the test module):

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -3,14 +3,39 @@
 use std::path::{Path, PathBuf};
 
 use crate::adapters::fs_alias_store::FsAliasStore;
+use crate::adapters::no_process::NoProcess;
 use crate::domain::alias::AliasStore;
 use crate::domain::version::Version;
 use crate::error::CliError;
-use crate::ports::{Env, FileSystem};
+use crate::ports::{Env, FileSystem, Process};
 
 pub struct Context<'a> {
     pub fs: &'a dyn FileSystem,
     pub env: &'a dyn Env,
+    process: &'a dyn Process,
+}
+
+impl<'a> Context<'a> {
+    /// A context that cannot run programs; add that with [`Self::with_process`].
+    #[must_use]
+    pub fn new(fs: &'a dyn FileSystem, env: &'a dyn Env) -> Self {
+        Self {
+            fs,
+            env,
+            process: &NoProcess,
+        }
+    }
+
+    #[must_use]
+    pub fn with_process(mut self, process: &'a dyn Process) -> Self {
+        self.process = process;
+        self
+    }
+
+    #[must_use]
+    pub fn process(&self) -> &dyn Process {
+        self.process
+    }
 }
 
 impl Context<'_> {
```

Apply to `src/fakes.rs` (above the test module):

```diff
--- a/src/fakes.rs
+++ b/src/fakes.rs
@@ -6,7 +6,7 @@
 use std::io;
 use std::path::{Path, PathBuf};
 
-use crate::ports::{DirEntry, Env, FileSystem};
+use crate::ports::{DirEntry, Env, FileSystem, Process, ProcessOutput};
 
 #[derive(Default)]
 pub struct FakeFileSystem {
@@ -93,6 +93,43 @@
     }
 }
 
+/// Programs by path: each prints a fixed output, or fails when it has none.
+#[derive(Default)]
+pub struct FakeProcess {
+    outputs: BTreeMap<PathBuf, ProcessOutput>,
+}
+
+impl FakeProcess {
+    #[must_use]
+    pub fn with_output(mut self, program: &str, stdout: &str) -> Self {
+        let output = ProcessOutput {
+            success: true,
+            stdout: stdout.to_owned(),
+        };
+        self.outputs.insert(PathBuf::from(program), output);
+        self
+    }
+
+    #[must_use]
+    pub fn with_failure(mut self, program: &str) -> Self {
+        let output = ProcessOutput {
+            success: false,
+            stdout: String::new(),
+        };
+        self.outputs.insert(PathBuf::from(program), output);
+        self
+    }
+}
+
+impl Process for FakeProcess {
+    fn run(&self, program: &Path, _args: &[&str]) -> io::Result<ProcessOutput> {
+        self.outputs
+            .get(program)
+            .cloned()
+            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
+    }
+}
+
 #[derive(Default)]
 pub struct FakeEnv {
     vars: BTreeMap<String, OsString>,
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -42,6 +42,21 @@
     fn create_dir_all(&self, path: &Path) -> io::Result<()>;
 }
 
+/// What a finished child process left behind.
+#[derive(Debug, Clone, PartialEq, Eq)]
+pub struct ProcessOutput {
+    pub success: bool,
+    pub stdout: String,
+}
+
+pub trait Process {
+    /// Runs `program` with `args` and waits for it.
+    ///
+    /// # Errors
+    /// Fails when the program cannot be started.
+    fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput>;
+}
+
 pub trait Env {
     /// The variable as text; `None` when unset or not valid UTF-8.
     fn var(&self, key: &str) -> Option<String>;
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 162 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(ports): add the Process port and read the system node version"
```

---

### Task 5: `nvm ls` (versions)

**Files:**

- Create: `src/domain/listing.rs`, `src/commands/ls/mod.rs`,
  `src/commands/ls/tests.rs`
- Modify: `src/domain/mod.rs`, `src/commands/mod.rs`, `src/error.rs`,
  `src/cli/mod.rs`, `src/cli/tests.rs`

**Interfaces:**

- Consumes: `Context::installed_versions`, `commands::current::detect`,
  `commands::resolve::{resolve_installed, system_node, system_version}`.
- Produces: `commands::Output::status` and `Output::with_status`
  (`NvmExitCode` now derives `Default` as `Success`); `domain::listing::
  {RowKind, format_row, format_system_row}`; `commands::ls::run(&Context,
  Option<&str>) -> Result<Output, CliError>`; the `ls` (alias `list`)
  subcommand.
- Behaviour (verified against `nvm.sh`): rows are right-aligned in 15 columns
  counted in bytes, `v18.9.0 *`; the version in use is
  `->      v20.1.0 *`; the system node is the last row,
  `system * (-> v22.1.0)` (the version comes from running the system
  node with `--version`); a pattern lists all matching versions, a plain number
  also matches io.js (`nvm ls 3`), `node` and `iojs` list one flavor, `current`
  lists the active version, an alias lists what it resolves to; nothing found
  prints `N/A` and exits 3, and with nothing installed at all it
  prints `N/A *` (an `nvm.sh` quirk kept on purpose).
- Deviations: no phantom blank row when only a system node exists, both
  io.js and Node versions that share a number are listed, and `ls unstable`
  prints one `N/A` row where `nvm.sh` prints two.
- Housekeeping: `commands/ls` is a directory module so its tests stay out of
  the 300-line limit.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -1,5 +1,6 @@
 pub mod alias;
 pub mod current;
+pub mod ls;
 pub mod resolve;
 pub mod unalias;
 pub mod version;
```

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -2,6 +2,7 @@
 pub mod current;
 pub mod floor;
 pub mod implicit;
+pub mod listing;
 pub mod nvmrc;
 pub mod path_search;
 pub mod version;
```

- [ ] **Step 2: Write the failing tests**

Apply to `src/cli/tests.rs`:

```diff
--- a/src/cli/tests.rs
+++ b/src/cli/tests.rs
@@ -98,6 +98,21 @@
         &mut err,
     );
     assert_eq!((code, out, err), (0, b"v20.1.0\n".to_vec(), Vec::new()));
+}
+
+#[test]
+fn ls_prints_the_rows_and_list_is_the_same_command() {
+    let expected = (0, "        v20.1.0 *\n".to_owned(), String::new());
+    assert_eq!(run_cli(&["nvm", "ls"]), expected);
+    assert_eq!(run_cli(&["nvm", "list"]), expected);
+}
+
+#[test]
+fn ls_of_a_missing_version_prints_na_on_stdout_and_exits_3() {
+    assert_eq!(
+        run_cli(&["nvm", "ls", "16"]),
+        (3, "            N/A\n".to_owned(), String::new())
+    );
 }
 
 #[test]
```

Create `src/commands/ls/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/ls/tests.rs`:

```rust
use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};

fn installed() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/n/versions/node/v20.1.0/bin/node", "")
        .with_file("/n/versions/node/v20.10.0/bin/node", "")
        .with_file("/n/versions/node/v18.9.0/bin/node", "")
        .with_file("/n/versions/io.js/v3.0.0/bin/node", "")
        .with_file("/n/versions/io.js/v2.5.0/bin/node", "")
        .with_file("/n/alias/work", "v18")
        .with_file("/n/alias/loop", "loop")
        .with_file("/n/alias/default", "node")
        .with_file("/n/alias/lts/iron", "v20.10.0")
        .with_file("/n/alias/lts/gallium", "v16.20.2")
}

fn ls_on(fs: &FakeFileSystem, path: &str, pattern: Option<&str>) -> Output {
    let process = FakeProcess::default().with_output("/sys/node", "v22.1.0\n");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path);
    let context = Context::new(fs, &env).with_process(&process);
    run(&context, pattern).unwrap()
}

fn ls(pattern: Option<&str>) -> Output {
    ls_on(&installed(), "/nonexistent", pattern)
}

fn rows(lines: &[&str]) -> String {
    lines.join("\n")
}

#[test]
fn lists_every_version_sorted_by_number_with_io_js_in_between() {
    let expected = rows(&[
        "    iojs-v2.5.0 *",
        "    iojs-v3.0.0 *",
        "        v18.9.0 *",
        "        v20.1.0 *",
        "       v20.10.0 *",
    ]);
    assert_eq!(ls(None), Output::stdout(expected));
}

#[test]
fn the_version_in_use_gets_an_arrow() {
    let output = ls_on(&installed(), "/n/versions/node/v18.9.0/bin", None);
    assert!(
        output.stdout.contains("\n->      v18.9.0 *\n"),
        "{}",
        output.stdout
    );
}

#[test]
fn the_system_node_is_listed_last_with_its_version() {
    let fs = installed().with_file("/sys/node", "");
    let output = ls_on(&fs, "/sys", None);
    let last = output.stdout.lines().last().unwrap();
    assert_eq!(last, "->       system * (-> v22.1.0)");
    let elsewhere = ls_on(&fs, "/n/versions/node/v18.9.0/bin:/sys", None);
    let last = elsewhere.stdout.lines().last().unwrap();
    assert_eq!(last, "         system * (-> v22.1.0)");
}

#[test]
fn a_pattern_lists_the_versions_it_matches() {
    let expected = rows(&["        v20.1.0 *", "       v20.10.0 *"]);
    assert_eq!(ls(Some("20")), Output::stdout(expected.clone()));
    assert_eq!(ls(Some("v20")), Output::stdout(expected));
    assert_eq!(ls(Some("20.1")), Output::stdout("        v20.1.0 *"));
    assert_eq!(ls(Some("v18.9.0")), Output::stdout("        v18.9.0 *"));
}

#[test]
fn a_plain_pattern_also_finds_io_js_like_nvm_sh() {
    assert_eq!(ls(Some("3")), Output::stdout("    iojs-v3.0.0 *"));
    assert_eq!(ls(Some("v3.0.0")), Output::stdout("    iojs-v3.0.0 *"));
}

#[test]
fn node_and_iojs_list_only_their_own_flavor() {
    let node = rows(&[
        "        v18.9.0 *",
        "        v20.1.0 *",
        "       v20.10.0 *",
    ]);
    assert_eq!(ls(Some("node")), Output::stdout(node));
    let iojs = rows(&["    iojs-v2.5.0 *", "    iojs-v3.0.0 *"]);
    assert_eq!(ls(Some("iojs")), Output::stdout(iojs));
}

#[test]
fn an_alias_lists_the_version_it_resolves_to() {
    assert_eq!(ls(Some("work")), Output::stdout("        v18.9.0 *"));
    assert_eq!(ls(Some("default")), Output::stdout("       v20.10.0 *"));
    assert_eq!(ls(Some("lts/iron")), Output::stdout("       v20.10.0 *"));
    assert_eq!(ls(Some("stable")), Output::stdout("       v20.10.0 *"));
}

#[test]
fn nothing_matching_prints_na_and_exits_3() {
    let expected = Output::stdout("            N/A").with_status(NvmExitCode::InvalidVersion);
    for pattern in ["99", "foo", "lts/gallium", "unstable", "system"] {
        assert_eq!(ls(Some(pattern)), expected, "{pattern}");
    }
}

#[test]
fn an_alias_loop_lists_infinity() {
    assert_eq!(ls(Some("loop")), Output::stdout("            ∞"));
}

#[test]
fn current_lists_the_active_version_or_none() {
    assert_eq!(ls(Some("current")), Output::stdout("->         none *"));
    let output = ls_on(
        &installed(),
        "/n/versions/node/v18.9.0/bin",
        Some("current"),
    );
    assert_eq!(output, Output::stdout("->      v18.9.0 *"));
}

#[test]
fn system_lists_the_system_node_when_there_is_one() {
    let fs = FakeFileSystem::default().with_file("/sys/node", "");
    let output = ls_on(&fs, "/sys", Some("system"));
    assert_eq!(output, Output::stdout("->       system * (-> v22.1.0)"));
}

#[test]
fn nothing_installed_prints_na_as_if_installed_and_exits_3() {
    let fs = FakeFileSystem::default();
    let expected = Output::stdout("            N/A *").with_status(NvmExitCode::InvalidVersion);
    assert_eq!(ls_on(&fs, "/nonexistent", None), expected);
}

#[test]
fn only_a_system_node_lists_just_that_row() {
    let fs = FakeFileSystem::default().with_file("/sys/node", "");
    let output = ls_on(&fs, "/sys", None);
    assert_eq!(output, Output::stdout("->       system * (-> v22.1.0)"));
}

#[test]
fn a_system_node_that_does_not_answer_is_listed_without_a_version() {
    let fs = FakeFileSystem::default().with_file("/other/node", "");
    let output = ls_on(&fs, "/other", None);
    assert_eq!(output, Output::stdout("->       system *"));
}
```

Apply to the test module of `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -1,6 +1,13 @@
 #[cfg(test)]
 mod tests {
     use super::*;
+
+    #[test]
+    fn the_default_status_is_success_and_can_be_changed() {
+        assert_eq!(Output::stdout("x").status, NvmExitCode::Success);
+        let output = Output::stdout("x").with_status(NvmExitCode::InvalidVersion);
+        assert_eq!(output.status, NvmExitCode::InvalidVersion);
+    }
 
     #[test]
     fn stdout_output_has_an_empty_stderr() {
```

Create `src/domain/listing.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_versions_are_right_aligned_with_a_star() {
        assert_eq!(
            format_row("v18.9.0", RowKind::Installed),
            "        v18.9.0 *"
        );
        assert_eq!(
            format_row("iojs-v2.5.0", RowKind::Installed),
            "    iojs-v2.5.0 *"
        );
    }

    #[test]
    fn the_current_version_has_an_arrow_and_the_same_width() {
        let row = format_row("v20.1.0", RowKind::Current);
        assert_eq!(row, "->      v20.1.0 *");
        assert_eq!(row.len(), format_row("v20.1.0", RowKind::Installed).len());
    }

    #[test]
    fn a_plain_row_has_no_star() {
        assert_eq!(format_row("N/A", RowKind::Plain), "            N/A");
    }

    #[test]
    fn padding_counts_bytes_so_infinity_gets_twelve_spaces() {
        assert_eq!(format_row("∞", RowKind::Plain), "            ∞");
    }

    #[test]
    fn a_name_longer_than_the_column_is_not_cut() {
        let row = format_row("iojs-v100.100.100", RowKind::Installed);
        assert_eq!(row, "iojs-v100.100.100 *");
    }

    #[test]
    fn the_system_row_shows_the_version_of_the_system_node() {
        let installed = format_system_row(RowKind::Installed, Some("v22.1.0"));
        assert_eq!(installed, "         system * (-> v22.1.0)");
        let current = format_system_row(RowKind::Current, Some("v22.1.0"));
        assert_eq!(current, "->       system * (-> v22.1.0)");
        assert_eq!(
            format_system_row(RowKind::Installed, None),
            "         system *"
        );
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "unresolved import
`crate::domain::listing`", "cannot find module `ls`" and "no method named
`with_status` found".

- [ ] **Step 4: Write the implementation**

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -26,6 +26,9 @@
     Version { pattern: Option<String> },
     /// Print the version of the node that is active in this shell.
     Current,
+    /// List the installed versions, optionally only those matching a pattern.
+    #[command(visible_alias = "list")]
+    Ls { pattern: Option<String> },
     /// Print the path to the node binary of a version or alias.
     Which { version: Option<String> },
     /// Create an alias for a version (an empty target deletes the alias).
@@ -40,6 +43,7 @@
             commands::version::run(context, pattern.as_deref().unwrap_or("current"))
         }
         Command::Current => commands::current::run(context),
+        Command::Ls { pattern } => commands::ls::run(context, pattern.as_deref()),
         Command::Which { version } => commands::which::run(context, version.as_deref()),
         Command::Alias { name, target } => commands::alias::run(context, name, target),
         Command::Unalias { names } => commands::unalias::run(context, names),
@@ -79,32 +83,32 @@
         }
     };
     match dispatch(&cli.command, context) {
-        Ok(output) => finish(&output, out, err, NvmExitCode::Success),
+        Ok(output) => finish(&output, out, err),
         // nvm.sh prints N/A on stdout, not stderr.
-        Err(error @ CliError::NotInstalled) => finish(
-            &Output::stdout(error.to_string()),
-            out,
-            err,
-            error.exit_code(),
-        ),
+        Err(error @ CliError::NotInstalled) => {
+            let output = Output::stdout(error.to_string()).with_status(error.exit_code());
+            finish(&output, out, err)
+        }
         Err(error) => {
-            let output = Output::default().with_stderr(error.to_string());
-            finish(&output, out, err, error.exit_code())
+            let output = Output::default()
+                .with_stderr(error.to_string())
+                .with_status(error.exit_code());
+            finish(&output, out, err)
         }
     }
 }
 
-/// Prints diagnostics first, then the result, and maps a failed stdout write
-/// to a failure exit code.
-fn finish(output: &Output, out: &mut dyn Write, err: &mut dyn Write, success: NvmExitCode) -> u8 {
+/// Prints diagnostics first, then the result, and finishes with the status the
+/// command asked for, unless writing the result failed.
+fn finish(output: &Output, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
     if !output.stderr.is_empty() {
         // Nowhere left to report a failed stderr write.
         let _ = emit(err, &format!("{}\n", output.stderr));
     }
     if output.stdout.is_empty() {
-        return success.code();
+        return output.status.code();
     }
-    exit_code_for_write(emit(out, &format!("{}\n", output.stdout)), success)
+    exit_code_for_write(emit(out, &format!("{}\n", output.stdout)), output.status)
 }
 
 /// Entry point shared by the `nvmrc` and `nvm` binaries.
```

Insert above the `#[cfg(test)]` line of `src/commands/ls/mod.rs`:

```rust
//! `nvm ls [pattern]`: the installed versions, one row each.
//!
//! Output is always plain, as `nvm.sh` prints when stdout is not a terminal.
//! Unlike `nvm.sh` it does not print a blank row when only a system node
//! exists, and it keeps both io.js and Node versions that share a number.

use crate::commands::Output;
use crate::commands::current;
use crate::commands::resolve::{Resolved, resolve_installed, system_node, system_version};
use crate::context::Context;
use crate::domain::alias::AliasStore;
use crate::domain::listing::{RowKind, format_row, format_system_row};
use crate::domain::version::{Flavor, Version, VersionPattern};
use crate::error::{CliError, NvmExitCode};

enum Entry {
    Version(Version),
    /// The system node, with its version when it answers `--version`.
    System(Option<String>),
    Raw(String, RowKind),
}

struct Selection {
    entries: Vec<Entry>,
    /// Nothing matched: the status is 3, like `nvm.sh`.
    missing: bool,
}

/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn run(context: &Context<'_>, pattern: Option<&str>) -> Result<Output, CliError> {
    let current = current::detect(context)?.to_string();
    let selection = select(context, pattern.filter(|text| !text.is_empty()), &current)?;
    let rows: Vec<String> = selection
        .entries
        .iter()
        .map(|entry| render(entry, &current))
        .collect();
    let output = Output::stdout(rows.join("\n"));
    Ok(if selection.missing {
        output.with_status(NvmExitCode::InvalidVersion)
    } else {
        output
    })
}

fn select(
    context: &Context<'_>,
    pattern: Option<&str>,
    current: &str,
) -> Result<Selection, CliError> {
    let mut installed = context.installed_versions()?;
    installed.sort();
    match pattern {
        None => everything(context, installed),
        Some("current") => Ok(found(vec![Entry::Raw(
            current.to_owned(),
            RowKind::Current,
        )])),
        Some("system") => system_only(context),
        Some("node") => Ok(of_flavor(installed, Flavor::Node)),
        Some("iojs") => Ok(of_flavor(installed, Flavor::IoJs)),
        Some(name) => by_name(context, installed, name),
    }
}

fn found(entries: Vec<Entry>) -> Selection {
    Selection {
        entries,
        missing: false,
    }
}

fn nothing_found() -> Selection {
    Selection {
        entries: vec![Entry::Raw("N/A".to_owned(), RowKind::Plain)],
        missing: true,
    }
}

fn some_or_nothing(entries: Vec<Entry>) -> Selection {
    if entries.is_empty() {
        nothing_found()
    } else {
        found(entries)
    }
}

fn versions(installed: impl IntoIterator<Item = Version>) -> Vec<Entry> {
    installed.into_iter().map(Entry::Version).collect()
}

fn system_entry(context: &Context<'_>) -> Result<Option<Entry>, CliError> {
    if system_node(context)?.is_none() {
        return Ok(None);
    }
    Ok(Some(Entry::System(system_version(context)?)))
}

/// With nothing installed `nvm.sh` prints `N/A` as if it were an installed
/// version (`            N/A *`), and so does this.
fn everything(context: &Context<'_>, installed: Vec<Version>) -> Result<Selection, CliError> {
    let mut entries = versions(installed);
    entries.extend(system_entry(context)?);
    if entries.is_empty() {
        let raw = Entry::Raw("N/A".to_owned(), RowKind::Installed);
        return Ok(Selection {
            entries: vec![raw],
            missing: true,
        });
    }
    Ok(found(entries))
}

fn system_only(context: &Context<'_>) -> Result<Selection, CliError> {
    Ok(some_or_nothing(
        system_entry(context)?.into_iter().collect(),
    ))
}

fn of_flavor(installed: Vec<Version>, flavor: Flavor) -> Selection {
    some_or_nothing(versions(
        installed
            .into_iter()
            .filter(|version| version.flavor == flavor),
    ))
}

/// An alias (including `stable` and `unstable`) shows what it resolves to;
/// anything else is a version pattern.
fn by_name(
    context: &Context<'_>,
    installed: Vec<Version>,
    name: &str,
) -> Result<Selection, CliError> {
    let is_alias =
        context.alias_store()?.target(name).is_some() || matches!(name, "stable" | "unstable");
    if is_alias {
        return alias_selection(context, name);
    }
    let Ok(pattern) = name.parse::<VersionPattern>() else {
        return Ok(nothing_found());
    };
    Ok(some_or_nothing(versions(
        installed
            .into_iter()
            .filter(|version| pattern.matches(version)),
    )))
}

fn alias_selection(context: &Context<'_>, name: &str) -> Result<Selection, CliError> {
    match resolve_installed(context, name) {
        Ok(Resolved::Installed(version)) => Ok(found(vec![Entry::Version(version)])),
        Ok(Resolved::System) => Ok(found(vec![Entry::System(system_version(context)?)])),
        Ok(Resolved::Missing { .. }) => Ok(nothing_found()),
        Err(CliError::Alias(_)) => Ok(found(vec![Entry::Raw("∞".to_owned(), RowKind::Plain)])),
        Err(error) => Err(error),
    }
}

fn kind_for(text: &str, current: &str) -> RowKind {
    if text == current {
        RowKind::Current
    } else {
        RowKind::Installed
    }
}

fn render(entry: &Entry, current: &str) -> String {
    match entry {
        Entry::Version(version) => {
            let text = version.to_string();
            format_row(&text, kind_for(&text, current))
        }
        Entry::System(version) => {
            format_system_row(kind_for("system", current), version.as_deref())
        }
        Entry::Raw(text, kind) => format_row(text, *kind),
    }
}

```

Apply to `src/commands/mod.rs` (above the test module):

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -6,12 +6,15 @@
 pub mod version;
 pub mod which;
 
-/// What a command wants printed. Text carries no trailing newline: the CLI
-/// adds one to each non-empty stream.
+use crate::error::NvmExitCode;
+
+/// What a command wants printed, and the exit status to finish with. Text
+/// carries no trailing newline: the CLI adds one to each non-empty stream.
 #[derive(Debug, Default, PartialEq, Eq)]
 pub struct Output {
     pub stdout: String,
     pub stderr: String,
+    pub status: NvmExitCode,
 }
 
 impl Output {
@@ -19,7 +22,7 @@
     pub fn stdout(text: impl Into<String>) -> Self {
         Self {
             stdout: text.into(),
-            stderr: String::new(),
+            ..Self::default()
         }
     }
 
@@ -28,5 +31,13 @@
         self.stderr = text.into();
         self
     }
+
+    /// For a result that is printed and still not a success, like `nvm ls`
+    /// finding nothing.
+    #[must_use]
+    pub fn with_status(mut self, status: NvmExitCode) -> Self {
+        self.status = status;
+        self
+    }
 }
 
```

Insert above the `#[cfg(test)]` line of `src/domain/listing.rs`:

```rust
//! The rows `nvm ls` prints, as `nvm.sh` prints them when stdout is not a
//! terminal (no colors): the version right-aligned in 15 columns.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// An installed version: `        v18.9.0 *`.
    Installed,
    /// The version in use: `->      v20.1.0 *`.
    Current,
    /// Not installed, so no marker: `            N/A`.
    Plain,
}

/// Right-aligns by bytes, like the `awk` of `nvm.sh`: the 3-byte `∞` is
/// padded with 12 spaces, not 14.
fn pad_left(text: &str, width: usize) -> String {
    format!("{}{text}", " ".repeat(width.saturating_sub(text.len())))
}

#[must_use]
pub fn format_row(text: &str, kind: RowKind) -> String {
    match kind {
        RowKind::Installed => format!("{} *", pad_left(text, 15)),
        RowKind::Current => format!("->{} *", pad_left(text, 13)),
        RowKind::Plain => pad_left(text, 15),
    }
}

/// The `system` row, with the version of the system node when it is known.
#[must_use]
pub fn format_system_row(kind: RowKind, version: Option<&str>) -> String {
    let row = format_row("system", kind);
    match version {
        Some(version) => format!("{row} (-> {version})"),
        None => row,
    }
}

```

Apply to `src/error.rs` (above the test module):

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -5,8 +5,9 @@
 use thiserror::Error;
 
 /// Public exit-code contract, taken from `nvm.sh`.
-#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
 pub enum NvmExitCode {
+    #[default]
     Success = 0,
     Failure = 1,
     InvalidVersion = 3,
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 185 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(commands): add ls"
```

---

### Task 6: Alias listing

**Files:**

- Create: `src/domain/alias_format.rs`, `src/commands/aliases/mod.rs`,
  `src/commands/aliases/tests.rs`
- Modify: `src/domain/mod.rs`, `src/commands/mod.rs`, `src/error.rs`,
  `src/commands/alias.rs`

**Interfaces:**

- Consumes: `commands::resolve::shown`, `domain::implicit::{IMPLICIT_ALIASES,
  destination}`, `Context::{alias_dir, alias_store, installed_versions}`.
- Produces: `domain::alias_format::format_line(alias, target, version,
  available, default) -> String` (shared by `alias` creation and listing);
  `NvmExitCode::NoSuchAlias` (2); `commands::aliases::list(&Context,
  Option<&str>) -> Result<Output, CliError>`.
- Behaviour (verified against `nvm.sh`): three groups, each sorted by line: the
  alias files, then the implicit aliases that have no file (`iojs`, `node`,
  `stable`, `unstable`, each ending in `(default)`), then the `lts/*` files.
  A prefix selects alias files and `lts/` files by name and an implicit alias
  only by its exact name. `nvm alias lts/<name>` prints the file's target, or
  `Alias does not exist.` on stderr with exit 2. Hidden files and directories
  are not aliases. Loops print `(-> ∞)`.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -1,4 +1,5 @@
 pub mod alias;
+pub mod aliases;
 pub mod current;
 pub mod ls;
 pub mod resolve;
```

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -1,4 +1,5 @@
 pub mod alias;
+pub mod alias_format;
 pub mod current;
 pub mod floor;
 pub mod implicit;
```

- [ ] **Step 2: Write the failing tests**

Create `src/commands/aliases/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/aliases/tests.rs`:

```rust
use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem};

/// The fixture the golden outputs were captured from, with the real `nvm.sh`.
fn fixture() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/n/versions/node/v20.1.0/bin/node", "")
        .with_file("/n/versions/node/v20.10.0/bin/node", "")
        .with_file("/n/versions/node/v18.9.0/bin/node", "")
        .with_file("/n/versions/io.js/v3.0.0/bin/node", "")
        .with_file("/n/versions/io.js/v2.5.0/bin/node", "")
        .with_file("/n/alias/default", "node\n")
        .with_file("/n/alias/work", "v18\n")
        .with_file("/n/alias/lts/*", "lts/iron\n")
        .with_file("/n/alias/lts/iron", "v20.10.0\n")
        .with_file("/n/alias/lts/gallium", "v16.20.2\n")
}

fn list_on(fs: &FakeFileSystem, path: &str, prefix: Option<&str>) -> Output {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path);
    list(&Context::new(fs, &env), prefix).unwrap()
}

fn list_with(fs: &FakeFileSystem, prefix: Option<&str>) -> Output {
    list_on(fs, "/nonexistent", prefix)
}

fn lines(output: &Output) -> Vec<&str> {
    output.stdout.lines().collect()
}

#[test]
fn lists_the_files_then_the_implicit_aliases_then_the_lts_aliases() {
    let output = list_with(&fixture(), None);
    assert_eq!(
        lines(&output),
        [
            "default -> node (-> v20.10.0 *)",
            "work -> v18 (-> v18.9.0 *)",
            "iojs -> iojs-v3.0 (-> iojs-v3.0.0 *) (default)",
            "node -> stable (-> v20.10.0 *) (default)",
            "stable -> 20.10 (-> v20.10.0 *) (default)",
            "unstable -> N/A (default)",
            "lts/* -> lts/iron (-> v20.10.0 *)",
            "lts/gallium -> v16.20.2 (-> N/A)",
            "lts/iron -> v20.10.0 *",
        ]
    );
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn a_prefix_selects_alias_files_and_the_exactly_named_implicit_alias() {
    let fs = fixture();
    assert_eq!(
        lines(&list_with(&fs, Some("wor"))),
        ["work -> v18 (-> v18.9.0 *)"]
    );
    assert_eq!(
        lines(&list_with(&fs, Some("default"))),
        ["default -> node (-> v20.10.0 *)"]
    );
    assert_eq!(
        lines(&list_with(&fs, Some("iojs"))),
        ["iojs -> iojs-v3.0 (-> iojs-v3.0.0 *) (default)"]
    );
    assert_eq!(
        lines(&list_with(&fs, Some("stable"))),
        ["stable -> 20.10 (-> v20.10.0 *) (default)"]
    );
}

#[test]
fn a_prefix_also_selects_lts_aliases_by_name() {
    let output = list_with(&fixture(), Some("i"));
    assert_eq!(lines(&output), ["lts/iron -> v20.10.0 *"]);
}

#[test]
fn an_unknown_prefix_prints_nothing_and_succeeds() {
    let output = list_with(&fixture(), Some("nope"));
    assert_eq!(output, Output::default());
}

#[test]
fn an_lts_name_prints_the_alias_file_target_as_is() {
    let fs = fixture();
    assert_eq!(list_with(&fs, Some("lts/iron")), Output::stdout("v20.10.0"));
    assert_eq!(list_with(&fs, Some("lts/*")), Output::stdout("lts/iron"));
}

#[test]
fn a_missing_lts_alias_is_exit_2_with_a_message_on_stderr() {
    let fs = fixture();
    let expected = Output::default()
        .with_stderr("Alias does not exist.")
        .with_status(NvmExitCode::NoSuchAlias);
    assert_eq!(list_with(&fs, Some("lts/nope")), expected);
    assert_eq!(list_with(&fs, Some("lts/")), expected);
}

#[test]
fn an_alias_file_hides_the_implicit_alias_of_the_same_name() {
    let fs = fixture().with_file("/n/alias/node", "v18\n");
    let output = list_with(&fs, None);
    let node: Vec<&str> = lines(&output)
        .into_iter()
        .filter(|line| line.starts_with("node "))
        .collect();
    assert_eq!(node, ["node -> v18 (-> v18.9.0 *)"]);
}

#[test]
fn nothing_installed_lists_the_implicit_aliases_without_stable() {
    let output = list_with(&FakeFileSystem::default(), None);
    assert_eq!(
        lines(&output),
        [
            "iojs -> N/A (default)",
            "node -> stable (-> N/A) (default)",
            "unstable -> N/A (default)",
        ]
    );
}

#[test]
fn old_release_lines_make_stable_and_unstable_differ() {
    let fs = FakeFileSystem::default()
        .with_file("/n/versions/node/v0.10.48/bin/node", "")
        .with_file("/n/versions/node/v0.11.16/bin/node", "")
        .with_file("/n/versions/node/v0.12.18/bin/node", "")
        .with_file("/n/versions/node/v4.2.0/bin/node", "");
    let output = list_with(&fs, None);
    assert_eq!(
        lines(&output),
        [
            "iojs -> N/A (default)",
            "node -> stable (-> v4.2.0 *) (default)",
            "stable -> 4.2 (-> v4.2.0 *) (default)",
            "unstable -> 0.11 (-> v0.11.16 *) (default)",
        ]
    );
}

#[test]
fn loops_comments_blank_lines_and_system_targets_are_shown_like_nvm_sh() {
    let fs = FakeFileSystem::default()
        .with_file("/n/versions/node/v4.2.0/bin/node", "")
        .with_file("/n/versions/node/v0.10.48/bin/node", "")
        .with_file("/n/alias/a", "b\n")
        .with_file("/n/alias/b", "a\n")
        .with_file("/n/alias/loop", "loop\n")
        .with_file("/n/alias/commented", "v4 # four\n")
        .with_file("/n/alias/blankfirst", "\n\nv0.10\n")
        .with_file("/n/alias/sys", "system\n");
    let output = list_with(&fs, None);
    assert_eq!(
        &lines(&output)[..6],
        [
            "a -> b (-> ∞)",
            "b -> a (-> ∞)",
            "blankfirst -> v0.10 (-> v0.10.48 *)",
            "commented -> v4 (-> v4.2.0 *)",
            "loop -> loop (-> ∞)",
            "sys -> system (-> N/A)",
        ]
    );
}

#[test]
fn a_system_node_makes_a_system_alias_resolve() {
    let fs = FakeFileSystem::default()
        .with_file("/sys/node", "")
        .with_file("/n/alias/sys", "system\n");
    let output = list_on(&fs, "/sys", Some("sys"));
    assert_eq!(lines(&output), ["sys -> system *"]);
}

#[test]
fn hidden_files_and_directories_are_not_aliases() {
    let fs = fixture()
        .with_file("/n/alias/.DS_Store", "v20\n")
        .with_file("/n/alias/other/inner", "v20\n");
    let output = list_with(&fs, None);
    assert!(lines(&output).iter().all(|line| !line.contains("DS_Store")));
    assert!(lines(&output).iter().all(|line| !line.starts_with("other")));
}
```

Create `src/domain/alias_format.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_that_is_the_version_prints_one_arrow() {
        let line = format_line("lts/iron", "v20.10.0", "v20.10.0", true, false);
        assert_eq!(line, "lts/iron -> v20.10.0 *");
    }

    #[test]
    fn a_target_that_resolves_shows_the_version_after_a_second_arrow() {
        let line = format_line("work", "v18", "v18.9.0", true, false);
        assert_eq!(line, "work -> v18 (-> v18.9.0 *)");
    }

    #[test]
    fn unresolved_targets_carry_no_star() {
        assert_eq!(
            format_line("old", "v16", "N/A", false, false),
            "old -> v16 (-> N/A)"
        );
        assert_eq!(format_line("a", "b", "∞", false, false), "a -> b (-> ∞)");
    }

    #[test]
    fn implicit_aliases_end_with_default() {
        let line = format_line("node", "stable", "v20.10.0", true, true);
        assert_eq!(line, "node -> stable (-> v20.10.0 *) (default)");
        let missing = format_line("unstable", "N/A", "N/A", false, true);
        assert_eq!(missing, "unstable -> N/A (default)");
    }
}
```

Apply to the test module of `src/error.rs`:

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -8,6 +8,7 @@
         assert_eq!(NvmExitCode::InvalidVersion.code(), 3);
         assert_eq!(NvmExitCode::BelowVersionFloor.code(), 7);
         assert_eq!(NvmExitCode::AliasLoop.code(), 8);
+        assert_eq!(NvmExitCode::NoSuchAlias.code(), 2);
         assert_eq!(NvmExitCode::NotFound.code(), 127);
     }
 
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "unresolved import
`crate::domain::alias_format`", "cannot find module `aliases`" and "no variant
named `NoSuchAlias` found".

- [ ] **Step 4: Write the implementation**

Apply to `src/commands/alias.rs` (above the test module):

```diff
--- a/src/commands/alias.rs
+++ b/src/commands/alias.rs
@@ -7,6 +7,7 @@
 use crate::commands::resolve::{Shown, shown};
 use crate::commands::{Output, unalias};
 use crate::context::Context;
+use crate::domain::alias_format::format_line;
 use crate::error::CliError;
 
 /// # Errors
@@ -31,7 +32,14 @@
         .fs
         .write_file(&alias_file, &format!("{target}\n"))
         .map_err(|source| CliError::io(&alias_file, source))?;
-    let output = Output::stdout(format_line(name, target, &version));
+    let line = format_line(
+        name,
+        target,
+        &version.to_string(),
+        version.is_available(),
+        false,
+    );
+    let output = Output::stdout(line);
     if version == Shown::NotAvailable {
         return Ok(output.with_stderr(format!("! WARNING: Version '{target}' does not exist.")));
     }
@@ -51,13 +59,3 @@
     Err(CliError::InvalidArgument(message))
 }
 
-fn format_line(alias: &str, target: &str, shown: &Shown) -> String {
-    let version = shown.to_string();
-    let marker = if shown.is_available() { " *" } else { "" };
-    if target == version {
-        format!("{alias} -> {version}{marker}")
-    } else {
-        format!("{alias} -> {target} (-> {version}{marker})")
-    }
-}
-
```

Insert above the `#[cfg(test)]` line of `src/commands/aliases/mod.rs`:

```rust
//! `nvm alias` and `nvm alias <prefix>`: list aliases.
//!
//! Three groups, each sorted: the alias files, the implicit aliases that have
//! no file (`iojs`, `node`, `stable`, `unstable`), and the `lts/*` files.
//! Output is always plain, as `nvm.sh` prints when stdout is not a terminal.

use std::path::Path;

use crate::commands::Output;
use crate::commands::resolve::shown;
use crate::context::Context;
use crate::domain::alias::AliasStore;
use crate::domain::alias_format::format_line;
use crate::domain::implicit::{IMPLICIT_ALIASES, destination};
use crate::error::{CliError, NvmExitCode};

/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn list(context: &Context<'_>, prefix: Option<&str>) -> Result<Output, CliError> {
    let prefix = prefix.unwrap_or_default();
    if prefix.starts_with("lts/") {
        return lts_target(context, prefix);
    }
    let alias_dir = context.alias_dir()?;
    let mut lines = directory_lines(context, &alias_dir, "", prefix)?;
    lines.extend(implicit_lines(context, &alias_dir, prefix)?);
    lines.extend(directory_lines(
        context,
        &alias_dir.join("lts"),
        "lts/",
        prefix,
    )?);
    Ok(Output::stdout(lines.join("\n")))
}

/// `nvm alias lts/iron` prints the alias file's target as is.
fn lts_target(context: &Context<'_>, name: &str) -> Result<Output, CliError> {
    match context.alias_store()?.target(name) {
        Some(target) => Ok(Output::stdout(target)),
        None => Ok(Output::default()
            .with_stderr("Alias does not exist.")
            .with_status(NvmExitCode::NoSuchAlias)),
    }
}

fn line(
    context: &Context<'_>,
    name: &str,
    target: &str,
    default: bool,
) -> Result<String, CliError> {
    let version = shown(context, target)?;
    let text = version.to_string();
    Ok(format_line(
        name,
        target,
        &text,
        version.is_available(),
        default,
    ))
}

/// The alias files directly inside `directory` whose name starts with
/// `prefix`, named `label` + the file name. Hidden files only match a hidden
/// prefix, like a shell glob.
fn directory_lines(
    context: &Context<'_>,
    directory: &Path,
    label: &str,
    prefix: &str,
) -> Result<Vec<String>, CliError> {
    let store = context.alias_store()?;
    let mut lines = Vec::new();
    for entry in context.fs.read_dir(directory).unwrap_or_default() {
        let hidden = entry.name.starts_with('.') && !prefix.starts_with('.');
        if entry.is_dir || hidden || !entry.name.starts_with(prefix) {
            continue;
        }
        let name = format!("{label}{}", entry.name);
        if let Some(target) = store.target(&name) {
            lines.push(line(context, &name, &target, false)?);
        }
    }
    lines.sort();
    Ok(lines)
}

/// The implicit aliases that have no file of their own, and that match the
/// prefix exactly (a prefix does not select among them).
fn implicit_lines(
    context: &Context<'_>,
    alias_dir: &Path,
    prefix: &str,
) -> Result<Vec<String>, CliError> {
    let installed = context.installed_versions()?;
    let mut lines = Vec::new();
    for name in IMPLICIT_ALIASES {
        let wanted = prefix.is_empty() || prefix == name;
        if !wanted || context.fs.is_file(&alias_dir.join(name)) {
            continue;
        }
        if let Some(target) = destination(&installed, name) {
            lines.push(line(context, name, &target, true)?);
        }
    }
    lines.sort();
    Ok(lines)
}

```

Insert above the `#[cfg(test)]` line of `src/domain/alias_format.rs`:

```rust
//! The line `nvm alias` prints for one alias, as `nvm.sh` prints it when stdout
//! is not a terminal (no colors).

/// `name -> target *` when the target already is the version, else
/// `name -> target (-> version *)`. The `*` marks a version that resolved; the
/// `(default)` suffix marks the implicit aliases.
#[must_use]
pub fn format_line(
    alias: &str,
    target: &str,
    version: &str,
    available: bool,
    default: bool,
) -> String {
    let marker = if available { " *" } else { "" };
    let line = if target == version {
        format!("{alias} -> {version}{marker}")
    } else {
        format!("{alias} -> {target} (-> {version}{marker})")
    };
    if default {
        format!("{line} (default)")
    } else {
        line
    }
}

```

Apply to `src/error.rs` (above the test module):

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -13,6 +13,8 @@
     InvalidVersion = 3,
     BelowVersionFloor = 7,
     AliasLoop = 8,
+    /// `nvm alias lts/<name>` for an alias that does not exist.
+    NoSuchAlias = 2,
     /// A usage error, or a requested system version that does not exist.
     NotFound = 127,
 }
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 201 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(commands): list aliases"
```

---

### Task 7: Options, `ls` with aliases, and the CLI wiring

**Files:**

- Modify: `src/commands/ls/mod.rs`, `src/commands/ls/tests.rs`,
  `src/commands/aliases/mod.rs`, `src/commands/aliases/tests.rs`,
  `src/error.rs`, `src/cli/mod.rs`, `src/cli/tests.rs`

**Interfaces:**

- Produces: `NvmExitCode::UnsupportedOption` (55) and
  `CliError::Unsupported(String)`; `commands::ls::{Options, parse_options,
  run_command}`; `commands::aliases::run(&Context, &[String])`. The `ls` and
  `alias` subcommands take their words verbatim (`allow_hyphen_values`).
- Behaviour (from `nvm.sh`): `ls` takes `--no-colors` (accepted, output is
  already plain), `--no-alias`, `--`, and the first non-empty word as the
  pattern; any other `--option` is `Unsupported option "--x".` with exit 55;
  `--no-alias` with a pattern is "`--no-alias` is not supported when a pattern
  is provided." with exit 55. Without a pattern and without `--no-alias`, `ls`
  prints the aliases after the versions and keeps the versions' exit status.
  `alias` with no words lists, with one word lists the aliases starting with
  it, with two creates the alias (extra words ignored), and an explicitly empty
  target deletes it; a `#` in the name is rejected even without a target.
- [ ] **Step 1: Write the failing tests**

Apply to `src/cli/tests.rs`:

```diff
--- a/src/cli/tests.rs
+++ b/src/cli/tests.rs
@@ -101,10 +101,55 @@
 }
 
 #[test]
-fn ls_prints_the_rows_and_list_is_the_same_command() {
-    let expected = (0, "        v20.1.0 *\n".to_owned(), String::new());
+fn ls_prints_the_rows_then_the_aliases_and_list_is_the_same_command() {
+    let rows = "        v20.1.0 *\n";
+    let aliases = "iojs -> N/A (default)\n\
+                   node -> stable (-> v20.1.0 *) (default)\n\
+                   stable -> 20.1 (-> v20.1.0 *) (default)\n\
+                   unstable -> N/A (default)\n";
+    let expected = (0, format!("{rows}{aliases}"), String::new());
     assert_eq!(run_cli(&["nvm", "ls"]), expected);
     assert_eq!(run_cli(&["nvm", "list"]), expected);
+    let without = (0, rows.to_owned(), String::new());
+    assert_eq!(run_cli(&["nvm", "ls", "--no-alias"]), without);
+    assert_eq!(
+        run_cli(&["nvm", "ls", "--no-colors", "--no-alias"]),
+        without
+    );
+}
+
+#[test]
+fn unsupported_options_exit_55_with_the_nvm_sh_message() {
+    assert_eq!(
+        run_cli(&["nvm", "ls", "--bogus"]),
+        (
+            55,
+            String::new(),
+            "Unsupported option \"--bogus\".\n".to_owned()
+        )
+    );
+    assert_eq!(
+        run_cli(&["nvm", "alias", "--bogus"]),
+        (
+            55,
+            String::new(),
+            "Unsupported option \"--bogus\".\n".to_owned()
+        )
+    );
+    let (code, out, err) = run_cli(&["nvm", "ls", "20", "--no-alias"]);
+    assert_eq!(code, 55);
+    assert!(out.is_empty());
+    assert_eq!(
+        err,
+        "`--no-alias` is not supported when a pattern is provided.\n"
+    );
+}
+
+#[test]
+fn alias_without_arguments_lists_the_aliases() {
+    let (code, out, _) = run_cli(&["nvm", "alias"]);
+    assert_eq!(code, 0);
+    assert!(out.starts_with("iojs -> N/A (default)\n"), "{out}");
 }
 
 #[test]
@@ -187,12 +232,7 @@
 
 #[test]
 fn a_usage_error_exits_127_without_stdout() {
-    for args in [
-        &["nvm", "bogus"][..],
-        &["nvm", "alias", "x"],
-        &["nvm", "alias", "x", "y", "z"],
-        &["nvm", "which", "--silent", "20"],
-    ] {
+    for args in [&["nvm", "bogus"][..], &["nvm", "which", "--silent", "20"]] {
         let (code, out, err) = run_cli(args);
         assert_eq!(code, 127, "{args:?}");
         assert!(out.is_empty() && !err.is_empty(), "{args:?}");
```

Apply to `src/commands/aliases/tests.rs`:

```diff
--- a/src/commands/aliases/tests.rs
+++ b/src/commands/aliases/tests.rs
@@ -178,6 +178,66 @@
     assert_eq!(lines(&output), ["sys -> system *"]);
 }
 
+fn words(args: &[&str]) -> Vec<String> {
+    args.iter().map(ToString::to_string).collect()
+}
+
+fn run_words(fs: &FakeFileSystem, args: &[&str]) -> Result<Output, CliError> {
+    let env = FakeEnv::default()
+        .with_var("NVM_DIR", "/n")
+        .with_var("PATH", "/nonexistent");
+    run(&Context::new(fs, &env), &words(args))
+}
+
+#[test]
+fn no_words_list_and_one_word_is_a_prefix() {
+    let fs = fixture();
+    assert_eq!(run_words(&fs, &[]).unwrap(), list_with(&fs, None));
+    assert_eq!(
+        run_words(&fs, &["--no-colors"]).unwrap(),
+        list_with(&fs, None)
+    );
+    assert_eq!(
+        run_words(&fs, &["--", "default"]).unwrap(),
+        list_with(&fs, Some("default"))
+    );
+}
+
+#[test]
+fn two_words_create_the_alias_and_extra_words_are_ignored() {
+    let fs = fixture();
+    let output = run_words(&fs, &["fresh", "stable", "ignored"]).unwrap();
+    assert_eq!(output, Output::stdout("fresh -> stable (-> v20.10.0 *)"));
+    assert_eq!(
+        list_with(&fs, Some("fresh")),
+        Output::stdout("fresh -> stable (-> v20.10.0 *)")
+    );
+}
+
+#[test]
+fn an_explicitly_empty_target_deletes_the_alias() {
+    let fs = fixture();
+    let output = run_words(&fs, &["work", ""]).unwrap();
+    assert!(output.stdout.starts_with("Deleted alias work"));
+}
+
+#[test]
+fn unknown_options_are_exit_55() {
+    let error = run_words(&fixture(), &["--bogus"]).unwrap_err();
+    assert_eq!(error.to_string(), "Unsupported option \"--bogus\".");
+    assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption);
+}
+
+#[test]
+fn a_comment_delimiter_in_the_name_is_rejected_even_without_a_target() {
+    let error = run_words(&fixture(), &["a#b"]).unwrap_err();
+    assert_eq!(
+        error.to_string(),
+        "Aliases with a comment delimiter (#) are not supported."
+    );
+    assert_eq!(error.exit_code(), NvmExitCode::Failure);
+}
+
 #[test]
 fn hidden_files_and_directories_are_not_aliases() {
     let fs = fixture()
```

Apply to `src/commands/ls/tests.rs`:

```diff
--- a/src/commands/ls/tests.rs
+++ b/src/commands/ls/tests.rs
@@ -131,6 +131,87 @@
     assert_eq!(output, Output::stdout("->       system * (-> v22.1.0)"));
 }
 
+fn words(args: &[&str]) -> Vec<String> {
+    args.iter().map(ToString::to_string).collect()
+}
+
+#[test]
+fn options_are_read_like_nvm_sh() {
+    let parsed = parse_options(&words(&["--no-colors", "--", "20", "18"])).unwrap();
+    assert_eq!(
+        parsed,
+        Options {
+            pattern: Some("20".to_owned()),
+            no_alias: false
+        }
+    );
+    let no_alias = parse_options(&words(&["--no-alias"])).unwrap();
+    assert!(no_alias.no_alias && no_alias.pattern.is_none());
+    let empty_first = parse_options(&words(&["", "20"])).unwrap();
+    assert_eq!(empty_first.pattern, Some("20".to_owned()));
+}
+
+#[test]
+fn bad_options_are_exit_55() {
+    let unknown = parse_options(&words(&["--bogus"])).unwrap_err();
+    assert_eq!(unknown.to_string(), "Unsupported option \"--bogus\".");
+    assert_eq!(unknown.exit_code(), NvmExitCode::UnsupportedOption);
+    let both = parse_options(&words(&["20", "--no-alias"])).unwrap_err();
+    assert_eq!(
+        both.to_string(),
+        "`--no-alias` is not supported when a pattern is provided."
+    );
+    assert_eq!(both.exit_code(), NvmExitCode::UnsupportedOption);
+}
+
+fn command(fs: &FakeFileSystem, args: &[&str]) -> Output {
+    let env = FakeEnv::default()
+        .with_var("NVM_DIR", "/n")
+        .with_var("PATH", "/nonexistent");
+    run_command(&Context::new(fs, &env), &words(args)).unwrap()
+}
+
+#[test]
+fn the_aliases_follow_the_versions() {
+    let fs = FakeFileSystem::default()
+        .with_file("/n/versions/node/v20.10.0/bin/node", "")
+        .with_file("/n/alias/default", "node");
+    let expected = [
+        "       v20.10.0 *",
+        "default -> node (-> v20.10.0 *)",
+        "iojs -> N/A (default)",
+        "node -> stable (-> v20.10.0 *) (default)",
+        "stable -> 20.10 (-> v20.10.0 *) (default)",
+        "unstable -> N/A (default)",
+    ];
+    assert_eq!(command(&fs, &[]), Output::stdout(expected.join("\n")));
+    let versions_only = command(&fs, &["--no-alias"]);
+    assert_eq!(versions_only, Output::stdout(expected[0]));
+}
+
+#[test]
+fn a_pattern_lists_no_aliases_and_keeps_the_status() {
+    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.10.0/bin/node", "");
+    let output = command(&fs, &["99"]);
+    assert_eq!(
+        output,
+        Output::stdout("            N/A").with_status(NvmExitCode::InvalidVersion)
+    );
+}
+
+#[test]
+fn nothing_installed_still_lists_the_aliases_with_status_3() {
+    let output = command(&FakeFileSystem::default(), &[]);
+    assert_eq!(output.status, NvmExitCode::InvalidVersion);
+    assert_eq!(
+        output.stdout,
+        "            N/A *\n\
+         iojs -> N/A (default)\n\
+         node -> stable (-> N/A) (default)\n\
+         unstable -> N/A (default)"
+    );
+}
+
 #[test]
 fn nothing_installed_prints_na_as_if_installed_and_exits_3() {
     let fs = FakeFileSystem::default();
```

Apply to the test module of `src/error.rs`:

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -9,6 +9,7 @@
         assert_eq!(NvmExitCode::BelowVersionFloor.code(), 7);
         assert_eq!(NvmExitCode::AliasLoop.code(), 8);
         assert_eq!(NvmExitCode::NoSuchAlias.code(), 2);
+        assert_eq!(NvmExitCode::UnsupportedOption.code(), 55);
         assert_eq!(NvmExitCode::NotFound.code(), 127);
     }
 
@@ -37,6 +38,8 @@
         assert_eq!(usage.exit_code(), NvmExitCode::NotFound);
         let no_system_node = CliError::SystemNodeNotFound;
         assert_eq!(no_system_node.exit_code(), NvmExitCode::NotFound);
+        let unsupported = CliError::Unsupported("x".into());
+        assert_eq!(unsupported.exit_code(), NvmExitCode::UnsupportedOption);
         let invalid = CliError::InvalidArgument("x".into());
         assert_eq!(invalid.exit_code(), NvmExitCode::Failure);
         let source = std::io::Error::from(std::io::ErrorKind::NotFound);
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function
`parse_options`", "cannot find function `run_command`" and "no variant named
`UnsupportedOption` found".

- [ ] **Step 3: Write the implementation**

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -26,13 +26,21 @@
     Version { pattern: Option<String> },
     /// Print the version of the node that is active in this shell.
     Current,
-    /// List the installed versions, optionally only those matching a pattern.
+    /// List the installed versions and the aliases (`--no-alias` omits them;
+    /// a pattern lists only the versions matching it).
     #[command(visible_alias = "list")]
-    Ls { pattern: Option<String> },
+    Ls {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
     /// Print the path to the node binary of a version or alias.
     Which { version: Option<String> },
-    /// Create an alias for a version (an empty target deletes the alias).
-    Alias { name: String, target: String },
+    /// List aliases, show those starting with a name, or create an alias for
+    /// a version (an empty target deletes the alias).
+    Alias {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
     /// Delete an alias.
     Unalias { names: Vec<String> },
 }
@@ -43,9 +51,9 @@
             commands::version::run(context, pattern.as_deref().unwrap_or("current"))
         }
         Command::Current => commands::current::run(context),
-        Command::Ls { pattern } => commands::ls::run(context, pattern.as_deref()),
+        Command::Ls { args } => commands::ls::run_command(context, args),
         Command::Which { version } => commands::which::run(context, version.as_deref()),
-        Command::Alias { name, target } => commands::alias::run(context, name, target),
+        Command::Alias { args } => commands::aliases::run(context, args),
         Command::Unalias { names } => commands::unalias::run(context, names),
     }
 }
```

Apply to `src/commands/aliases/mod.rs` (above the test module):

```diff
--- a/src/commands/aliases/mod.rs
+++ b/src/commands/aliases/mod.rs
@@ -6,13 +6,59 @@
 
 use std::path::Path;
 
-use crate::commands::Output;
 use crate::commands::resolve::shown;
+use crate::commands::{Output, alias, unalias};
 use crate::context::Context;
 use crate::domain::alias::AliasStore;
 use crate::domain::alias_format::format_line;
 use crate::domain::implicit::{IMPLICIT_ALIASES, destination};
 use crate::error::{CliError, NvmExitCode};
+
+#[derive(Default)]
+struct Words {
+    name: Option<String>,
+    target: Option<String>,
+}
+
+/// `nvm alias [--no-colors] [name [target]]`: the first two words are the
+/// name and the target, more are ignored.
+fn parse_words(args: &[String]) -> Result<Words, CliError> {
+    let mut words = Words::default();
+    for arg in args {
+        match arg.as_str() {
+            "--" | "--no-colors" => {}
+            option if option.starts_with("--") => {
+                let message = format!("Unsupported option \"{option}\".");
+                return Err(CliError::Unsupported(message));
+            }
+            word if words.name.is_none() => words.name = Some(word.to_owned()),
+            word if words.target.is_none() => words.target = Some(word.to_owned()),
+            _ => {}
+        }
+    }
+    Ok(words)
+}
+
+/// `nvm alias`: with no words it lists, with a name it lists the aliases
+/// starting with it, with a name and a target it creates the alias, and an
+/// explicitly empty target deletes it.
+///
+/// # Errors
+/// - [`CliError::Unsupported`] for an unknown `--option`.
+/// - Whatever the listing, creation or deletion fails with.
+pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
+    let Words { name, target } = parse_words(args)?;
+    match (name, target) {
+        (Some(name), Some(target)) if target.is_empty() => unalias::run(context, &[name]),
+        (Some(name), _) if name.contains('#') => {
+            let message = "Aliases with a comment delimiter (#) are not supported.";
+            Err(CliError::InvalidArgument(message.to_owned()))
+        }
+        (Some(name), Some(target)) => alias::run(context, &name, &target),
+        (Some(name), None) => list(context, Some(&name)),
+        (None, _) => list(context, None),
+    }
+}
 
 /// # Errors
 /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
```

Apply to `src/commands/ls/mod.rs` (above the test module):

```diff
--- a/src/commands/ls/mod.rs
+++ b/src/commands/ls/mod.rs
@@ -5,6 +5,7 @@
 //! exists, and it keeps both io.js and Node versions that share a number.
 
 use crate::commands::Output;
+use crate::commands::aliases;
 use crate::commands::current;
 use crate::commands::resolve::{Resolved, resolve_installed, system_node, system_version};
 use crate::context::Context;
@@ -24,6 +25,58 @@
     entries: Vec<Entry>,
     /// Nothing matched: the status is 3, like `nvm.sh`.
     missing: bool,
+}
+
+/// The command line of `nvm ls`, as `nvm.sh` reads it: the first non-empty
+/// word is the pattern, `--no-colors` is accepted (output is always plain).
+#[derive(Debug, Default, PartialEq, Eq)]
+pub struct Options {
+    pub pattern: Option<String>,
+    pub no_alias: bool,
+}
+
+/// # Errors
+/// - [`CliError::Unsupported`] for an unknown `--option`, and for
+///   `--no-alias` together with a pattern.
+pub fn parse_options(args: &[String]) -> Result<Options, CliError> {
+    let mut options = Options::default();
+    for arg in args {
+        match arg.as_str() {
+            "--" | "--no-colors" => {}
+            "--no-alias" => options.no_alias = true,
+            option if option.starts_with("--") => {
+                let message = format!("Unsupported option \"{option}\".");
+                return Err(CliError::Unsupported(message));
+            }
+            word if options.pattern.is_none() && !word.is_empty() => {
+                options.pattern = Some(word.to_owned());
+            }
+            _ => {}
+        }
+    }
+    if options.pattern.is_some() && options.no_alias {
+        let message = "`--no-alias` is not supported when a pattern is provided.";
+        return Err(CliError::Unsupported(message.to_owned()));
+    }
+    Ok(options)
+}
+
+/// `nvm ls`: the versions, then (without a pattern or `--no-alias`) the
+/// aliases, with the exit status of the versions part.
+///
+/// # Errors
+/// As [`parse_options`], and [`CliError::NvmDirUnresolved`] when `$NVM_DIR`
+/// cannot be found.
+pub fn run_command(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
+    let options = parse_options(args)?;
+    let mut output = run(context, options.pattern.as_deref())?;
+    if options.pattern.is_none() && !options.no_alias {
+        let listed = aliases::list(context, None)?;
+        if !listed.stdout.is_empty() {
+            output.stdout = format!("{}\n{}", output.stdout, listed.stdout);
+        }
+    }
+    Ok(output)
 }
 
 /// # Errors
```

Apply to `src/error.rs` (above the test module):

```diff
--- a/src/error.rs
+++ b/src/error.rs
@@ -15,6 +15,9 @@
     AliasLoop = 8,
     /// `nvm alias lts/<name>` for an alias that does not exist.
     NoSuchAlias = 2,
+    /// An option `nvm.sh` does not support, or one used in a combination it
+    /// does not support.
+    UnsupportedOption = 55,
     /// A usage error, or a requested system version that does not exist.
     NotFound = 127,
 }
@@ -69,6 +72,9 @@
     /// A rejected argument, with the message to print.
     #[error("{0}")]
     InvalidArgument(String),
+    /// An unsupported option, with the message to print.
+    #[error("{0}")]
+    Unsupported(String),
     /// A failed file-system change, naming the path it was made on.
     #[error("{}: {source}", path.display())]
     Io {
@@ -95,6 +101,7 @@
             | Self::InvalidArgument(_)
             | Self::Io { .. } => NvmExitCode::Failure,
             Self::Usage(_) | Self::SystemNodeNotFound => NvmExitCode::NotFound,
+            Self::Unsupported(_) => NvmExitCode::UnsupportedOption,
             Self::Floor(_) => NvmExitCode::BelowVersionFloor,
             Self::Alias(_) => NvmExitCode::AliasLoop,
         }
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 213 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(cli): read ls and alias options like nvm.sh and list aliases after versions"
```

---

### Task 8: End-to-end tests against the `nvm.sh` outputs, and the gate

**Files:**

- Create: `tests/listing_cli.rs`

**Interfaces:**

- Consumes: the `nvm` and `nvmrc` binaries (`CARGO_BIN_EXE_nvm`,
  `CARGO_BIN_EXE_nvmrc`).
- Produces: acceptance tests whose expected text is the output the real
  `nvm.sh` printed for the same `$NVM_DIR`: the full `ls`, `ls` options and
  patterns, exit 55, the alias listings, an empty `$NVM_DIR`, old 0.x release
  lines with comments, blank lines and loops, a real executable system `node`
  (Unix only), a failed alias write that names its path, and both binaries.
- [ ] **Step 1: Write the end-to-end tests**

Create `tests/listing_cli.rs`:

```rust
//! End-to-end: `ls` and `alias` listings of the real binaries, compared with
//! what the real `nvm.sh` printed for the same `$NVM_DIR` (captured with its
//! stdout piped, so without colors).

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
    let path = OsStr::new("/nonexistent");
    run(env!("CARGO_BIN_EXE_nvm"), nvm_dir, path, args)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn install(nvm_dir: &Path, directory: &str) {
    let bin = nvm_dir.join(directory).join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("node"), "").unwrap();
}

fn alias(nvm_dir: &Path, name: &str, target: &str) {
    let file = nvm_dir.join("alias").join(name);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(file, format!("{target}\n")).unwrap();
}

/// The fixture the golden outputs below were captured from.
fn golden_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for version in ["v20.1.0", "v20.10.0", "v18.9.0"] {
        install(dir.path(), &format!("versions/node/{version}"));
    }
    for version in ["v3.0.0", "v2.5.0"] {
        install(dir.path(), &format!("versions/io.js/{version}"));
    }
    alias(dir.path(), "default", "node");
    alias(dir.path(), "work", "v18");
    alias(dir.path(), "lts/*", "lts/iron");
    alias(dir.path(), "lts/iron", "v20.10.0");
    alias(dir.path(), "lts/gallium", "v16.20.2");
    dir
}

const GOLDEN_VERSIONS: &str = "    iojs-v2.5.0 *
    iojs-v3.0.0 *
        v18.9.0 *
        v20.1.0 *
       v20.10.0 *
";

const GOLDEN_ALIASES: &str = "default -> node (-> v20.10.0 *)
work -> v18 (-> v18.9.0 *)
iojs -> iojs-v3.0 (-> iojs-v3.0.0 *) (default)
node -> stable (-> v20.10.0 *) (default)
stable -> 20.10 (-> v20.10.0 *) (default)
unstable -> N/A (default)
lts/* -> lts/iron (-> v20.10.0 *)
lts/gallium -> v16.20.2 (-> N/A)
lts/iron -> v20.10.0 *
";

#[test]
fn ls_matches_nvm_sh() {
    let dir = golden_dir();
    let output = nvm(dir.path(), &["ls"]);
    assert_eq!(
        stdout(&output),
        format!("{GOLDEN_VERSIONS}{GOLDEN_ALIASES}")
    );
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&nvm(dir.path(), &["list"])), stdout(&output));
}

#[test]
fn ls_options_and_patterns_match_nvm_sh() {
    let dir = golden_dir();
    let no_alias = nvm(dir.path(), &["ls", "--no-colors", "--no-alias"]);
    assert_eq!(stdout(&no_alias), GOLDEN_VERSIONS);
    let twenty = nvm(dir.path(), &["ls", "20"]);
    assert_eq!(stdout(&twenty), "        v20.1.0 *\n       v20.10.0 *\n");
    let iojs = nvm(dir.path(), &["ls", "3"]);
    assert_eq!(stdout(&iojs), "    iojs-v3.0.0 *\n");
    let alias = nvm(dir.path(), &["ls", "work"]);
    assert_eq!(stdout(&alias), "        v18.9.0 *\n");
    let missing = nvm(dir.path(), &["ls", "99"]);
    assert_eq!(stdout(&missing), "            N/A\n");
    assert_eq!(missing.status.code(), Some(3));
}

#[test]
fn unsupported_ls_options_exit_55() {
    let dir = golden_dir();
    let bogus = nvm(dir.path(), &["ls", "--bogus"]);
    assert_eq!(bogus.status.code(), Some(55));
    assert_eq!(stderr(&bogus), "Unsupported option \"--bogus\".\n");
    let both = nvm(dir.path(), &["ls", "20", "--no-alias"]);
    assert_eq!(both.status.code(), Some(55));
    assert_eq!(
        stderr(&both),
        "`--no-alias` is not supported when a pattern is provided.\n"
    );
}

#[test]
fn alias_listings_match_nvm_sh() {
    let dir = golden_dir();
    assert_eq!(stdout(&nvm(dir.path(), &["alias"])), GOLDEN_ALIASES);
    let work = nvm(dir.path(), &["alias", "wor"]);
    assert_eq!(stdout(&work), "work -> v18 (-> v18.9.0 *)\n");
    let iojs = nvm(dir.path(), &["alias", "iojs"]);
    assert_eq!(
        stdout(&iojs),
        "iojs -> iojs-v3.0 (-> iojs-v3.0.0 *) (default)\n"
    );
    assert_eq!(
        stdout(&nvm(dir.path(), &["alias", "lts/iron"])),
        "v20.10.0\n"
    );
    let nothing = nvm(dir.path(), &["alias", "nope"]);
    assert!(nothing.stdout.is_empty() && nothing.status.success());
    let missing = nvm(dir.path(), &["alias", "lts/nope"]);
    assert_eq!(missing.status.code(), Some(2));
    assert_eq!(stderr(&missing), "Alias does not exist.\n");
    assert_eq!(
        nvm(dir.path(), &["alias", "--bogus"]).status.code(),
        Some(55)
    );
}

#[test]
fn an_empty_nvm_dir_matches_nvm_sh() {
    let dir = tempfile::tempdir().unwrap();
    let output = nvm(dir.path(), &["ls"]);
    assert_eq!(
        stdout(&output),
        "            N/A *
iojs -> N/A (default)
node -> stable (-> N/A) (default)
unstable -> N/A (default)
"
    );
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn old_release_lines_match_nvm_sh() {
    let dir = tempfile::tempdir().unwrap();
    for version in ["v0.10.48", "v0.11.16", "v0.12.18", "v4.2.0"] {
        install(dir.path(), &format!("versions/node/{version}"));
    }
    fs::create_dir_all(dir.path().join("alias")).unwrap();
    fs::write(dir.path().join("alias/commented"), "v4 # four\n").unwrap();
    fs::write(dir.path().join("alias/blankfirst"), "\n\nv0.10\n").unwrap();
    fs::write(dir.path().join("alias/a"), "b\n").unwrap();
    fs::write(dir.path().join("alias/b"), "a\n").unwrap();
    let output = nvm(dir.path(), &["alias"]);
    assert_eq!(
        stdout(&output),
        "a -> b (-> ∞)
b -> a (-> ∞)
blankfirst -> v0.10 (-> v0.10.48 *)
commented -> v4 (-> v4.2.0 *)
iojs -> N/A (default)
node -> stable (-> v4.2.0 *) (default)
stable -> 4.2 (-> v4.2.0 *) (default)
unstable -> 0.11 (-> v0.11.16 *) (default)
"
    );
    assert_eq!(
        stdout(&nvm(dir.path(), &["version", "unstable"])),
        "v0.11.16\n"
    );
}

#[cfg(unix)]
#[test]
fn a_real_system_node_is_listed_with_the_version_it_prints() {
    use std::os::unix::fs::PermissionsExt;

    let dir = golden_dir();
    let system = tempfile::tempdir().unwrap();
    let node = system.path().join("node");
    fs::write(&node, "#!/bin/sh\necho v22.1.0\n").unwrap();
    fs::set_permissions(&node, fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths([system.path()]).unwrap();
    let bin = env!("CARGO_BIN_EXE_nvm");

    let listed = run(bin, dir.path(), &path, &["ls", "--no-alias"]);
    let expected = format!("{GOLDEN_VERSIONS}->       system * (-> v22.1.0)\n");
    assert_eq!(stdout(&listed), expected);
    let only = run(bin, dir.path(), &path, &["ls", "system"]);
    assert_eq!(stdout(&only), "->       system * (-> v22.1.0)\n");
    let sys_alias = run(bin, dir.path(), &path, &["alias", "default", "system"]);
    assert_eq!(stdout(&sys_alias), "default -> system *\n");
}

#[test]
fn a_failed_alias_write_names_the_path() {
    let dir = golden_dir();
    let output = nvm(dir.path(), &["alias", "lts", "20"]);
    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    let expected_path = dir.path().join("alias/lts");
    assert!(
        message.starts_with(&expected_path.display().to_string()),
        "{message}"
    );
}

#[test]
fn both_binaries_list_the_same() {
    let dir = golden_dir();
    let path = OsStr::new("/nonexistent");
    let from_nvm = run(env!("CARGO_BIN_EXE_nvm"), dir.path(), path, &["ls"]);
    let from_nvmrc = run(env!("CARGO_BIN_EXE_nvmrc"), dir.path(), path, &["ls"]);
    assert_eq!(from_nvm.stdout, from_nvmrc.stdout);
}
```

- [ ] **Step 2: Run them**

Run: `cargo test --test listing_cli`
Expected: PASS, 9 tests (8 on non-Unix targets). They pass at once because the
behaviour already exists; they are the acceptance net for Tasks 1 to 7. If one
fails, fix the code, not the test, unless the test contradicts `nvm.sh`.

- [ ] **Step 3: Run the full quality gate**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
rustup run 1.88 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings, 213 unit tests plus 6 + 9 +
1 end-to-end tests passing, no advisories, and the crate still builds on MSRV
1.88.

Also check that no file is over 300 lines:

```bash
find src tests -name '*.rs' | xargs wc -l | sort -n | tail -3
```

- [ ] **Step 4: Commit**

```bash
git add tests
git commit -S -m "test(cli): compare ls and alias listings with nvm.sh output"
```

---

## Self-review against the spec

- **Spec coverage:** section 3 layering is kept (`domain/` for rows, alias
  lines and implicit aliases; `ports/` gains `Process`; `commands/` gains `ls`
  and `aliases`); section 4.2 `AliasTarget::System` is now `Resolved::System` /
  `Shown::System`; section 4.3 resolution handles `node`, `stable`,
  `unstable`, `iojs` and `system`; section 6 exit codes gain 2 and 55; section
  11 step 3 is complete with `ls` and the alias listing.
- **Deliberate deviations from `nvm.sh`, each to be pinned by the Plan 9
  compatibility contract:**
  - No colors, `set-colors` or terminal detection (Plan 7); `--no-colors` is
    accepted and does nothing.
  - `ls` prints no phantom blank row when only a system node exists, lists
    both io.js and Node versions that share a number (`nvm.sh` keeps one), and
    prints one `N/A` row for `ls unstable` (`nvm.sh` prints two).
  - Lines are sorted by byte order; `nvm.sh` uses the locale's `sort`.
  - `ls` does not read `.nvmrc` content passed as a pattern, the legacy
    `$NVM_DIR/vX.Y.Z` layout, or a system io.js.
  - `current` still reads the version from the `node` path; only the system
    node is run (`Process`).
- **Type consistency:** names match across tasks (`Shown`, `Resolved`,
  `Implicit`, `Options`, `Context::new`, `Process`, `Output::status`,
  `format_line`, `format_row`).
