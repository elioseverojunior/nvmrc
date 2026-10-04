# nvmrc Plan 8: Shell Conflict Detection and Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the move from `nvm.sh` to nvmrc safe: `nvm doctor` finds, in the
user's shell files, every place that still loads `nvm.sh` (the loader lines of
`install.sh`, Homebrew's, lazy loaders and stubs), `nvm migrate` rewrites those
files (dry run, backup, atomic write, syntax check, undo), and a shell that
already has `nvm.sh` loaded gets one warning when it evaluates `nvm init`.

**Architecture:** Same layering as Plans 1 to 7. `ports/` is now a directory and
gains `FileSystem::{canonicalize, replace_file}` (an atomic write that goes
through symlinks and keeps the mode), `Clock` and `Prompt`; `domain/conflict`
holds the rules (hand-written line matchers, no regex crate), `domain/migration`
the pure planning (edits, diff, backup names, which program checks the syntax);
`commands/conflict` walks the files, `commands/doctor` and `commands/migrate`
are the two commands, and the init snippets carry the runtime check, calling the
hidden `nvm __conflict`.

**Tech Stack:** Rust 2024 edition (MSRV 1.85), the dependencies of Plan 5. No
new crate: the spec's `grep-regex`/`grep-searcher` are not used (see the
deviations).

**Spec:** `docs/superpowers/specs/2026-10-02-nvmrc-design.md` (section 7, which
this plan corrects where the research proved it unworkable). This plan starts
from the final state of `2026-10-03-nvmrc-plan-7-colors.md` (the `main` branch
at `ed1876f`).

**Reference.** The oracle is the research digest of the real `install.sh`,
Homebrew's nvm, oh-my-zsh, zsh-nvm and a real dotfiles tree, with every shell
claim run in bash 3.2 and 5.3, zsh, dash and ksh. fish is not installed on the
development Mac: what concerns fish comes from its documentation and runs only
in the Docker image. At the end of the plan `nvm doctor` and `nvm migrate
--dry-run` were run, read-only, against a real zsh setup (a stow-linked
`.zshrc` with lazy stubs and the oh-my-zsh plugin) and every finding was
checked against the lines of the files.

**Revised roadmap:** Plan 9 compatibility contract, CI and README.

**Not in this plan, on purpose:** editing lazy-loader files, runtime hooks that
catch a late `source nvm.sh` (`PROMPT_COMMAND`, `precmd`, `trap DEBUG`),
the `ignore` directory walker, PowerShell and nushell.

## Global Constraints

- Rust edition 2024, `rust-version = "1.85.0"`; no API newer than 1.85. Run
  cargo through the rustup proxies. `.cargo/config.toml` sets `-D warnings`. No
  new dependency and no change to `Cargo.toml`.
- Files under 300 lines (tests included), functions under 30 lines, cyclomatic
  complexity under 10, meaningful names without abbreviations. A module with
  tests is `foo/mod.rs` plus `foo/tests.rs` (or `*_tests.rs` beside it); a file
  that would pass 300 lines is split.
- Errors use `thiserror` in the library. Exit codes: `doctor` 0 (no conflict, or
  only hints and notes) or 1 (at least one conflict); `migrate` 0 (done, nothing
  to do, or a dry run) or 1 (declined, no terminal and no `--yes`, a file
  refused, a loader left after the edit, no backup for `--undo`); unknown shell
  127, unsupported option 55; the hidden `__conflict` prints on stderr and is 0
  (127 for a bad reason).
- Detection is read-only and never fails a command. `$NVM_DIR` and `nvm.sh`
  (Homebrew's symlink included) are never touched. The scanner ignores comments,
  `# [nvmrc-migrated]` lines and everything between `# >>> nvmrc init >>>` and
  `# <<< nvmrc init <<<`.
- The load line is `eval "$(nvmrc init <shell>)"` (`nvmrc init fish | source`):
  it never calls the `nvm` function, which may be nvm.sh's and would swallow
  `init` silently.
- A file is replaced only through `FileSystem::replace_file`: temp file in the
  target's directory, mode kept, `fsync`, the candidate checked, then `rename`;
  a symlink at the path survives. Backups are regular files next to the LINK.
- Commits: Conventional Commits, GPG-signed with `git commit -S`, and no AI
  attribution or `Co-Authored-By` trailer in the message.
- Tests never touch the real home or dotfiles (temp directories and fakes);
  real-shell tests skip a shell that is not installed. Do not keep test scripts
  or test-output directories in the repository.

Each task below lists its steps in TDD order. Where a file already exists, the
code is shown as a unified diff against the previous task's final state; where
a file is new, the full code is shown. Apply diffs by hand (hunk headers are
informative, line numbers may drift), then run `cargo fmt`. A new module with
tests is created first as its `mod.rs` holding only the `#[cfg(test)] mod
tests;` declaration and its `tests.rs`; the implementation is then inserted
ABOVE that declaration, which must stay in the file.

---

### Task 1: The ports the migration needs

**Files:**

- Create: `src/adapters/no_clock.rs`, `src/adapters/no_prompt.rs`,
  `src/adapters/std_clock.rs`, `src/adapters/std_fs/atomic.rs`,
  `src/adapters/std_fs/atomic_tests.rs`, `src/adapters/std_fs/mod.rs`,
  `src/adapters/std_fs/tests.rs`, `src/adapters/std_prompt.rs`,
  `src/domain/timestamp.rs`, `src/fakes/clock.rs`,
  `src/fakes/file_system/links.rs`, `src/fakes/file_system/links_tests.rs`,
  `src/fakes/prompt.rs`, `src/ports/archive.rs`, `src/ports/channel.rs`,
  `src/ports/env.rs`, `src/ports/fs.rs`, `src/ports/http.rs`,
  `src/ports/process.rs`, `src/ports/system.rs`, `src/ports/terminal.rs`
- Modify: `src/adapters/mod.rs`, `src/cli/mod.rs`, `src/context/mod.rs`,
  `src/context/tests.rs`, `src/domain/mod.rs`, `src/fakes/file_system/mod.rs`,
  `src/fakes/mod.rs`, `src/ports/mod.rs`
- Delete: `src/adapters/std_fs.rs`

**Interfaces:**

- Produces: `src/ports/mod.rs` becomes a directory (`fs`, `process`, `http`,
  `archive`, `env`, `channel`, `system`, `terminal`), re-exporting everything
  so no import changes; `FileSystem::canonicalize(path) -> io::Result<PathBuf>`;
  `FileSystem::replace_file(path, contents, verify: &dyn Fn(&Path) ->
  io::Result<()>) -> io::Result<()>`; `Clock::unix_seconds()` (`StdClock`,
  `NoClock`, `FakeClock::at`); `Prompt::confirm(question) -> io::Result<bool>`
  (`StdPrompt`, `NoPrompt`, `FakePrompt::{answering, unavailable, asked}`);
  `Context::{with_clock, clock, with_prompt, prompt}`; `domain::timestamp::
  compact_utc(unix_seconds) -> String` (`yyyymmddThhmmssZ`);
  `FakeFileSystem::replaced()`; `adapters/std_fs.rs` becomes a directory.
- Behaviour: `replace_file` resolves symlinks (the link at `path` survives),
  refuses a dangling link with `NotFound`, creates a temp file
  `.nvmrc-tmp-<pid>- <counter>` in the target's directory with `create_new`,
  applies the target's permissions to the open handle (a new file gets 0o644;
  the umask cannot change it), writes, `sync_all`, calls `verify(temp)` (an
  `Err` removes the temp and is returned, the target untouched) and renames; a
  path that is not a link and does not exist is created. `StdPrompt` writes the
  question to stderr exactly as given and reads one line when stdin is a
  terminal (`y`/`yes`, any case, is true); otherwise it returns `Unsupported`
  without reading. The fake file system also follows links in `read_to_string`
  and `is_file` (relative targets, chains, parent links, loops: limit 40).
- [ ] **Step 1: Declare the new modules**

Apply to `src/adapters/mod.rs`:

```diff
--- a/src/adapters/mod.rs
+++ b/src/adapters/mod.rs
@@ -1,18 +1,22 @@
 pub mod fd_channel;
 pub mod fs_alias_store;
 pub mod no_archive;
+pub mod no_clock;
 pub mod no_cpu;
 pub mod no_digest;
 pub mod no_http;
 pub mod no_process;
+pub mod no_prompt;
 pub mod no_sleeper;
 pub mod no_terminal;
 pub mod retrying_http;
 pub mod sha256_digest;
+pub mod std_clock;
 pub mod std_cpu;
 pub mod std_env;
 pub mod std_fs;
 pub mod std_process;
+pub mod std_prompt;
 pub mod std_sleeper;
 mod std_spawn;
 pub mod std_terminal;
```

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -20,6 +20,7 @@
 pub mod remote;
 pub mod remote_format;
 pub mod source_build;
+pub mod timestamp;
 pub mod valid_version;
 pub mod version;
 pub mod version_prefix;
```

- [ ] **Step 2: Write the failing tests**

Create `src/adapters/no_clock.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_always_the_epoch() {
        assert_eq!(NoClock.unix_seconds(), 0);
    }
}
```

Create `src/adapters/no_prompt.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_never_ask() {
        let error = NoPrompt.confirm("Apply?").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}
```

Create `src/adapters/std_clock.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_system_time_in_seconds() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let now = u64::try_from(StdClock.unix_seconds()).unwrap();
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(before <= now && now <= after);
    }
}
```

Delete `src/adapters/std_fs.rs` (`git rm src/adapters/std_fs.rs`).

Create `src/adapters/std_fs/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/adapters/std_fs/tests.rs`:

```rust
use super::*;

#[test]
fn remove_dir_all_removes_a_tree_a_file_and_tolerates_a_missing_path() {
    let root = tempfile::tempdir().unwrap();
    let tree = root.path().join("tree");
    fs::create_dir_all(tree.join("a/b")).unwrap();
    fs::write(tree.join("a/b/file"), "x").unwrap();
    let file = root.path().join("file");
    fs::write(&file, "x").unwrap();
    for path in [&tree, &file, &root.path().join("missing")] {
        StdFileSystem.remove_dir_all(path).unwrap();
        assert!(!path.exists());
    }
}

#[cfg(unix)]
#[test]
fn remove_dir_all_removes_a_symlink_and_leaves_its_target() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("target");
    fs::create_dir(&target).unwrap();
    let link = root.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    StdFileSystem.remove_dir_all(&link).unwrap();
    assert!(!link.exists() && target.exists());
}

#[cfg(unix)]
#[test]
fn same_directory_follows_symlinks_and_tells_directories_apart() {
    let root = tempfile::tempdir().unwrap();
    let (real, other) = (root.path().join("real"), root.path().join("other"));
    fs::create_dir(&real).unwrap();
    fs::create_dir(&other).unwrap();
    let link = root.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(StdFileSystem.same_directory(&link, &real));
    assert!(!StdFileSystem.same_directory(&other, &real));
    assert!(!StdFileSystem.same_directory(&root.path().join("gone"), &real));
}

#[test]
fn file_info_tells_size_kind_and_executability() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("tool");
    fs::write(&file, "abc").unwrap();
    let info = StdFileSystem.file_info(&file).unwrap();
    assert_eq!((info.is_dir, info.len), (false, 3));
    assert!(info.modified.is_some());
    assert!(StdFileSystem.file_info(root.path()).unwrap().is_dir);
    assert!(
        StdFileSystem
            .file_info(&root.path().join("missing"))
            .is_err()
    );
}

#[cfg(unix)]
#[test]
fn file_info_sees_the_execute_bit() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("tool");
    fs::write(&file, "x").unwrap();
    assert!(!StdFileSystem.file_info(&file).unwrap().executable);
    fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(StdFileSystem.file_info(&file).unwrap().executable);
}

#[test]
fn create_dir_fails_when_the_directory_exists_and_rename_moves_a_tree() {
    let root = tempfile::tempdir().unwrap();
    let first = root.path().join("lock");
    StdFileSystem.create_dir(&first).unwrap();
    let again = StdFileSystem.create_dir(&first).unwrap_err();
    assert_eq!(again.kind(), io::ErrorKind::AlreadyExists);
    fs::write(first.join("f"), "x").unwrap();
    let moved = root.path().join("moved");
    StdFileSystem.rename(&first, &moved).unwrap();
    assert!(moved.join("f").is_file() && !first.exists());
}

#[test]
fn write_bytes_stores_what_is_not_text() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("blob");
    StdFileSystem.write_bytes(&file, &[0, 255, 1]).unwrap();
    assert_eq!(fs::read(&file).unwrap(), [0, 255, 1]);
}

#[cfg(unix)]
#[test]
fn symlink_is_created_read_back_and_refuses_an_existing_path() {
    let root = tempfile::tempdir().unwrap();
    let link = root.path().join("current");
    StdFileSystem.symlink(Path::new("/v/1"), &link).unwrap();
    assert_eq!(StdFileSystem.read_link(&link).unwrap(), Path::new("/v/1"));
    let again = StdFileSystem.symlink(Path::new("/v/2"), &link).unwrap_err();
    assert_eq!(again.kind(), io::ErrorKind::AlreadyExists);
    StdFileSystem.remove_file(&link).unwrap();
    let gone = StdFileSystem.read_link(&link).unwrap_err();
    assert_eq!(gone.kind(), io::ErrorKind::NotFound);
}

#[cfg(unix)]
#[test]
fn canonicalize_resolves_a_chain_of_links_and_refuses_a_dangling_one() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("dotfiles/.zshrc");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, "x").unwrap();
    let (first, second) = (root.path().join("first"), root.path().join("second"));
    std::os::unix::fs::symlink("dotfiles/.zshrc", &second).unwrap();
    std::os::unix::fs::symlink(&second, &first).unwrap();
    let expected = fs::canonicalize(&target).unwrap();
    assert_eq!(StdFileSystem.canonicalize(&first).unwrap(), expected);
    let dangling = root.path().join("dangling");
    std::os::unix::fs::symlink("gone", &dangling).unwrap();
    for missing in [&dangling, &root.path().join("missing")] {
        let error = StdFileSystem.canonicalize(missing).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
}
```

Create `src/adapters/std_prompt.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};

    use super::*;

    fn answer(line: &str) -> (bool, String) {
        let mut output = Vec::new();
        let confirmed = ask("Apply? ", true, &mut Cursor::new(line), &mut output).unwrap();
        (confirmed, String::from_utf8(output).unwrap())
    }

    #[test]
    fn y_and_yes_in_any_case_confirm_and_the_question_is_shown() {
        for line in ["y\n", "Y\n", "yes\n", "YeS\n", "  yes  \n", "y"] {
            assert_eq!(answer(line), (true, "Apply? ".to_owned()), "{line:?}");
        }
    }

    #[test]
    fn anything_else_declines() {
        for line in ["n\n", "no\n", "\n", "", "yep\n", "y es\n"] {
            assert!(!answer(line).0, "{line:?}");
        }
    }

    struct Untouchable;

    impl Read for Untouchable {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            panic!("standard input must not be read")
        }
    }

    impl BufRead for Untouchable {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            panic!("standard input must not be read")
        }

        fn consume(&mut self, _amount: usize) {}
    }

    #[test]
    fn without_a_terminal_it_is_unsupported_and_neither_reads_nor_asks() {
        let mut output = Vec::new();
        let error = ask("Apply? ", false, &mut Untouchable, &mut output).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert!(output.is_empty());
    }

    #[test]
    fn the_real_prompt_is_unsupported_when_stdin_is_not_a_terminal() {
        // Never block the suite: only checked when `cargo test` runs with
        // standard input redirected (CI, pipes), where nothing can answer.
        if io::stdin().is_terminal() {
            return;
        }
        let error = StdPrompt.confirm("Apply? ").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}
```

Apply to `src/context/tests.rs`:

```diff
--- a/src/context/tests.rs
+++ b/src/context/tests.rs
@@ -1,7 +1,8 @@
 use super::*;
 use crate::domain::platform::Platform;
 use crate::fakes::{
-    FakeArchive, FakeCpu, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeSleeper,
+    FakeArchive, FakeClock, FakeCpu, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakePrompt,
+    FakeSleeper,
 };
 
 fn nvm_dir_for(env: &FakeEnv) -> Result<PathBuf, CliError> {
@@ -135,6 +136,29 @@
 }
 
 #[test]
+fn a_context_is_at_the_epoch_until_it_is_given_a_clock() {
+    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
+    assert_eq!(Context::new(&fs, &env).clock().unix_seconds(), 0);
+    let clock = FakeClock::at(1_791_110_040);
+    let context = Context::new(&fs, &env).with_clock(&clock);
+    assert_eq!(context.clock().unix_seconds(), 1_791_110_040);
+}
+
+#[test]
+fn a_context_cannot_ask_until_it_is_given_a_prompt() {
+    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
+    let unanswered = Context::new(&fs, &env).prompt().confirm("Apply?");
+    assert_eq!(
+        unanswered.unwrap_err().kind(),
+        std::io::ErrorKind::Unsupported
+    );
+    let prompt = FakePrompt::answering(true);
+    let context = Context::new(&fs, &env).with_prompt(&prompt);
+    assert!(context.prompt().confirm("Apply?").unwrap());
+    assert_eq!(prompt.asked(), ["Apply?"]);
+}
+
+#[test]
 fn a_context_is_linux_x64_until_told_otherwise() {
     let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
     let context = Context::new(&fs, &env);
```

Create `src/domain/timestamp.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_seconds_as_a_compact_utc_timestamp() {
        let cases = [
            (0, "19700101T000000Z"),
            (1_709_210_096, "20240229T123456Z"),
            (951_782_400, "20000229T000000Z"),
            (1_791_072_000, "20261004T000000Z"),
            (1_791_110_040, "20261004T103400Z"),
            (1_791_158_399, "20261004T235959Z"),
            (1_791_158_400, "20261005T000000Z"),
            (1_767_225_599, "20251231T235959Z"),
            (-1, "19691231T235959Z"),
            (-86_400, "19691231T000000Z"),
            (-2_208_988_800, "19000101T000000Z"),
        ];
        for (seconds, expected) in cases {
            assert_eq!(compact_utc(seconds), expected, "{seconds}");
        }
    }
}
```

Create `src/fakes/clock.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_the_time_it_was_set_to() {
        assert_eq!(FakeClock::at(1_791_110_040).unix_seconds(), 1_791_110_040);
    }
}
```

Create `src/fakes/prompt.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_as_told_and_remembers_the_questions() {
        let yes = FakePrompt::answering(true);
        assert!(yes.confirm("Apply?").unwrap());
        assert!(!FakePrompt::answering(false).confirm("Apply?").unwrap());
        assert_eq!(yes.asked(), ["Apply?"]);
    }

    #[test]
    fn unavailable_is_unsupported_and_still_records_the_question() {
        let nobody = FakePrompt::unavailable();
        let error = nobody.confirm("Apply?").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert_eq!(nobody.asked(), ["Apply?"]);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find trait `Clock`", "no
method named `replace_file` found" and "cannot find function `compact_utc`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/adapters/no_clock.rs`:

```rust
use crate::ports::Clock;

/// The `Clock` of a `Context` that was not given one: always the Unix epoch.
pub struct NoClock;

impl Clock for NoClock {
    fn unix_seconds(&self) -> i64 {
        0
    }
}

```

Insert above the `#[cfg(test)]` line of `src/adapters/no_prompt.rs`:

```rust
use std::io;

use crate::ports::Prompt;

/// The `Prompt` of a `Context` that was not given one: nobody can answer.
pub struct NoPrompt;

impl Prompt for NoPrompt {
    fn confirm(&self, _question: &str) -> io::Result<bool> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

```

Insert above the `#[cfg(test)]` line of `src/adapters/std_clock.rs`:

```rust
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ports::Clock;

/// The real [`Clock`]: the system time.
pub struct StdClock;

impl Clock for StdClock {
    fn unix_seconds(&self) -> i64 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(after) => i64::try_from(after.as_secs()).unwrap_or(i64::MAX),
            Err(before) => i64::try_from(before.duration().as_secs()).map_or(i64::MIN, |s| -s),
        }
    }
}

```

Create `src/adapters/std_fs/atomic.rs`:

```rust
//! Replacing a file atomically, through a symbolic link.

use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// How many names are tried before giving up on a free temporary file.
const NAME_ATTEMPTS: u32 = 100;

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

pub(super) fn replace_file(
    path: &Path,
    contents: &str,
    verify: &dyn Fn(&Path) -> io::Result<()>,
) -> io::Result<()> {
    let target = resolve_target(path)?;
    let permissions = permissions_of(&target)?;
    let (file, temporary) = create_temporary(&target)?;
    let result = fill(file, contents, permissions)
        .and_then(|()| verify(&temporary))
        .and_then(|()| fs::rename(&temporary, &target));
    if result.is_err() {
        // The rename did not happen, so the temporary file is still there.
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// The file a write to `path` must replace: the end of a symbolic link (the
/// link itself is kept), or `path`, which may not exist yet.
fn resolve_target(path: &Path) -> io::Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::canonicalize(path).map_err(|error| dangling(path, &error))
        }
        Ok(_) => Ok(path.to_path_buf()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(error),
    }
}

fn dangling(link: &Path, error: &io::Error) -> io::Error {
    let message = format!("{}: dangling symbolic link ({error})", link.display());
    io::Error::new(error.kind(), message)
}

/// The permissions of `target`, or those of a new file when it is missing.
fn permissions_of(target: &Path) -> io::Result<Option<Permissions>> {
    match fs::metadata(target) {
        Ok(metadata) => Ok(Some(metadata.permissions())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(new_file_permissions()),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn new_file_permissions() -> Option<Permissions> {
    use std::os::unix::fs::PermissionsExt;
    Some(Permissions::from_mode(0o644))
}

#[cfg(not(unix))]
fn new_file_permissions() -> Option<Permissions> {
    None
}

/// A new, empty file next to `target`, with a name no one else uses.
fn create_temporary(target: &Path) -> io::Result<(File, PathBuf)> {
    let directory = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut last_error = io::Error::from(io::ErrorKind::AlreadyExists);
    for _ in 0..NAME_ATTEMPTS {
        let counter = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
        let name = format!(".nvmrc-tmp-{}-{counter}", std::process::id());
        let temporary = directory.join(name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((file, temporary)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => last_error = error,
            Err(error) => return Err(error),
        }
    }
    Err(last_error)
}

/// Sets the permissions, writes `contents` and syncs them to the disk.
fn fill(mut file: File, contents: &str, permissions: Option<Permissions>) -> io::Result<()> {
    if let Some(permissions) = permissions {
        file.set_permissions(permissions)?;
    }
    file.write_all(contents.as_bytes())?;
    file.sync_all()
}
```

Create `src/adapters/std_fs/atomic_tests.rs`:

```rust
use std::sync::Mutex;

use super::*;

fn names_in(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn accept(_temporary: &Path) -> io::Result<()> {
    Ok(())
}

#[test]
fn replace_file_replaces_a_plain_file_and_verifies_the_new_contents_first() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(".bashrc");
    fs::write(&file, "old").unwrap();
    let seen = Mutex::new(Vec::new());
    let verify = |temporary: &Path| {
        let contents = fs::read_to_string(temporary)?;
        seen.lock()
            .unwrap()
            .push((temporary.to_path_buf(), contents));
        Ok(())
    };
    StdFileSystem.replace_file(&file, "new", &verify).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "new");
    let seen = seen.into_inner().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0.parent(), Some(root.path()));
    assert!(
        seen[0]
            .0
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".nvmrc-tmp-")
    );
    assert_eq!(seen[0].1, "new");
    assert_eq!(names_in(root.path()), [".bashrc"]);
}

#[cfg(unix)]
#[test]
fn replace_file_creates_a_missing_file_with_mode_644() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(".kshrc");
    StdFileSystem.replace_file(&file, "x", &accept).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "x");
    let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o644);
}

/// A stow-style `home/.zshrc -> ../dotfiles/zsh/.zshrc`, the target in mode
/// 0o640: returns the home and dotfiles directories.
#[cfg(unix)]
fn stowed_zshrc(root: &Path) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let (home, dotfiles) = (root.join("home"), root.join("dotfiles/zsh"));
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&dotfiles).unwrap();
    let target = dotfiles.join(".zshrc");
    fs::write(&target, "old").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    std::os::unix::fs::symlink("../dotfiles/zsh/.zshrc", home.join(".zshrc")).unwrap();
    (home, dotfiles)
}

#[cfg(unix)]
#[test]
fn replace_file_writes_through_a_relative_symlink_keeping_the_link_and_the_mode() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let (home, dotfiles) = stowed_zshrc(root.path());
    let (link, target) = (home.join(".zshrc"), dotfiles.join(".zshrc"));
    let target_directory = fs::canonicalize(&dotfiles).unwrap();
    let verify = |temporary: &Path| {
        assert_eq!(temporary.parent(), Some(target_directory.as_path()));
        Ok(())
    };
    StdFileSystem.replace_file(&link, "new", &verify).unwrap();
    // `read_link` fails on anything but a symbolic link: the link survived.
    let kept = fs::read_link(&link).unwrap();
    assert_eq!(kept, Path::new("../dotfiles/zsh/.zshrc"));
    assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o640);
    assert_eq!(names_in(&dotfiles), [".zshrc"]);
    assert_eq!(names_in(&home), [".zshrc"]);
}

#[test]
fn replace_file_leaves_the_target_untouched_when_verify_fails() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(".profile");
    fs::write(&file, "old").unwrap();
    let refuse = |_temporary: &Path| Err(io::Error::other("syntax error"));
    let error = StdFileSystem
        .replace_file(&file, "new", &refuse)
        .unwrap_err();
    assert_eq!(error.to_string(), "syntax error");
    assert_eq!(fs::read_to_string(&file).unwrap(), "old");
    assert_eq!(names_in(root.path()), [".profile"]);
}

#[cfg(unix)]
#[test]
fn replace_file_refuses_a_dangling_symlink() {
    let root = tempfile::tempdir().unwrap();
    let link = root.path().join(".zshrc");
    std::os::unix::fs::symlink("missing/.zshrc", &link).unwrap();
    let error = StdFileSystem
        .replace_file(&link, "new", &accept)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(names_in(root.path()), [".zshrc"]);
}

#[test]
fn replace_file_gives_concurrent_writers_unique_temporary_files() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join(".bashrc");
    fs::write(&file, "old").unwrap();
    let temporaries = Mutex::new(Vec::new());
    let record = |temporary: &Path| {
        temporaries.lock().unwrap().push(temporary.to_path_buf());
        Ok(())
    };
    std::thread::scope(|scope| {
        for writer in 0..8 {
            let (file, record) = (&file, &record);
            scope.spawn(move || {
                let contents = format!("writer {writer}");
                StdFileSystem.replace_file(file, &contents, record).unwrap();
            });
        }
    });
    let mut temporaries = temporaries.into_inner().unwrap();
    temporaries.sort();
    temporaries.dedup();
    assert_eq!(temporaries.len(), 8);
    assert!(fs::read_to_string(&file).unwrap().starts_with("writer "));
    assert_eq!(names_in(root.path()), [".bashrc"]);
}
```

Insert above the `#[cfg(test)]` line of `src/adapters/std_fs/mod.rs`:

```rust
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::ports::{DirEntry, FileInfo, FileSystem};

mod atomic;

pub struct StdFileSystem;

impl FileSystem for StdFileSystem {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>> {
        fs::read_dir(path)?
            .map(|entry| {
                let entry = entry?;
                Ok(DirEntry {
                    name: entry.file_name().to_string_lossy().into_owned(),
                    is_dir: entry.path().is_dir(),
                })
            })
            .collect()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
            Ok(_) => fs::remove_file(path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()> {
        fs::write(path, contents)
    }

    fn write_bytes(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        fs::write(path, contents)
    }

    fn file_info(&self, path: &Path) -> io::Result<FileInfo> {
        let metadata = fs::metadata(path)?;
        Ok(FileInfo {
            is_dir: metadata.is_dir(),
            len: metadata.len(),
            executable: is_executable(&metadata),
            modified: metadata.modified().ok(),
        })
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        fs::create_dir(path)
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    #[cfg(unix)]
    fn symlink(&self, target: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(not(unix))]
    fn symlink(&self, _target: &Path, _link: &Path) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    #[cfg(unix)]
    fn read_link(&self, link: &Path) -> io::Result<PathBuf> {
        fs::read_link(link)
    }

    #[cfg(not(unix))]
    fn read_link(&self, _link: &Path) -> io::Result<PathBuf> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    #[cfg(unix)]
    fn same_directory(&self, a: &Path, b: &Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        match (fs::metadata(a), fs::metadata(b)) {
            (Ok(a), Ok(b)) => a.is_dir() && a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }

    #[cfg(not(unix))]
    fn same_directory(&self, a: &Path, b: &Path) -> bool {
        match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(a), Ok(b)) => a == b && a.is_dir(),
            _ => false,
        }
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        fs::canonicalize(path)
    }

    fn replace_file(
        &self,
        path: &Path,
        contents: &str,
        verify: &dyn Fn(&Path) -> io::Result<()>,
    ) -> io::Result<()> {
        atomic::replace_file(path, contents, verify)
    }
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod atomic_tests;
```

Insert above the `#[cfg(test)]` line of `src/adapters/std_prompt.rs`:

```rust
use std::io::{self, BufRead, IsTerminal, Write};

use crate::ports::Prompt;

/// The real [`Prompt`]: the question goes to standard error and one line of
/// standard input is the answer, when standard input is a terminal.
pub struct StdPrompt;

impl Prompt for StdPrompt {
    fn confirm(&self, question: &str) -> io::Result<bool> {
        let stdin = io::stdin();
        let interactive = stdin.is_terminal();
        ask(question, interactive, &mut stdin.lock(), &mut io::stderr())
    }
}

/// Asks `question` on `output` and reads the answer from `input`, only when
/// `interactive`; otherwise reads nothing.
fn ask(
    question: &str,
    interactive: bool,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> io::Result<bool> {
    if !interactive {
        return Err(io::Error::from(io::ErrorKind::Unsupported));
    }
    output.write_all(question.as_bytes())?;
    output.flush()?;
    let mut answer = String::new();
    input.read_line(&mut answer)?;
    Ok(is_yes(&answer))
}

/// `y` or `yes` in any case, surrounding blanks ignored.
fn is_yes(answer: &str) -> bool {
    let answer = answer.trim();
    answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes")
}

```

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -15,10 +15,12 @@
 use crate::adapters::fd_channel::FdChannel;
 use crate::adapters::retrying_http::RetryingHttp;
 use crate::adapters::sha256_digest::Sha256Digest;
+use crate::adapters::std_clock::StdClock;
 use crate::adapters::std_cpu::StdCpu;
 use crate::adapters::std_env::StdEnv;
 use crate::adapters::std_fs::StdFileSystem;
 use crate::adapters::std_process::StdProcess;
+use crate::adapters::std_prompt::StdPrompt;
 use crate::adapters::std_sleeper::StdSleeper;
 use crate::adapters::std_terminal::StdTerminal;
 use crate::adapters::tar_archive::TarArchive;
@@ -155,6 +157,8 @@
         .with_sleeper(&StdSleeper)
         .with_cpu(&StdCpu)
         .with_terminal(&StdTerminal)
+        .with_clock(&StdClock)
+        .with_prompt(&StdPrompt)
         .with_platform(platform);
     match &channel {
         Some(channel) => body(&context.with_script_channel(channel)),
```

Apply to `src/context/mod.rs` (above the test module):

```diff
--- a/src/context/mod.rs
+++ b/src/context/mod.rs
@@ -4,10 +4,12 @@
 
 use crate::adapters::fs_alias_store::FsAliasStore;
 use crate::adapters::no_archive::NoArchive;
+use crate::adapters::no_clock::NoClock;
 use crate::adapters::no_cpu::NoCpu;
 use crate::adapters::no_digest::NoDigest;
 use crate::adapters::no_http::NoHttp;
 use crate::adapters::no_process::NoProcess;
+use crate::adapters::no_prompt::NoPrompt;
 use crate::adapters::no_sleeper::NoSleeper;
 use crate::adapters::no_terminal::NoTerminal;
 use crate::domain::alias::AliasStore;
@@ -15,7 +17,8 @@
 use crate::domain::version::Version;
 use crate::error::CliError;
 use crate::ports::{
-    Archive, Cpu, Digest, Env, FileSystem, Http, Process, ScriptChannel, Sleeper, Terminal,
+    Archive, Clock, Cpu, Digest, Env, FileSystem, Http, Process, Prompt, ScriptChannel, Sleeper,
+    Terminal,
 };
 
 pub struct Context<'a> {
@@ -28,6 +31,8 @@
     sleeper: &'a dyn Sleeper,
     cpu: &'a dyn Cpu,
     terminal: &'a dyn Terminal,
+    clock: &'a dyn Clock,
+    prompt: &'a dyn Prompt,
     platform: Option<Platform>,
     script_channel: Option<&'a dyn ScriptChannel>,
 }
@@ -47,6 +52,8 @@
             sleeper: &NoSleeper,
             cpu: &NoCpu,
             terminal: &NoTerminal,
+            clock: &NoClock,
+            prompt: &NoPrompt,
             platform: Some(Platform {
                 os: Os::Linux,
                 arch: "x64".to_owned(),
@@ -104,6 +111,30 @@
     #[must_use]
     pub fn terminal(&self) -> &dyn Terminal {
         self.terminal
+    }
+
+    /// The time; a context starts out at the Unix epoch.
+    #[must_use]
+    pub fn with_clock(mut self, clock: &'a dyn Clock) -> Self {
+        self.clock = clock;
+        self
+    }
+
+    #[must_use]
+    pub fn clock(&self) -> &dyn Clock {
+        self.clock
+    }
+
+    /// Who answers questions; a context starts out with nobody to ask.
+    #[must_use]
+    pub fn with_prompt(mut self, prompt: &'a dyn Prompt) -> Self {
+        self.prompt = prompt;
+        self
+    }
+
+    #[must_use]
+    pub fn prompt(&self) -> &dyn Prompt {
+        self.prompt
     }
 
     /// The machine the binaries are for; `None` when it has no official ones.
```

Insert above the `#[cfg(test)]` line of `src/domain/timestamp.rs`:

```rust
//! Compact UTC timestamps, as backup file names carry them.

/// `unix_seconds` as `yyyymmddThhmmssZ` in UTC (`20261004T103400Z`).
#[must_use]
pub fn compact_utc(unix_seconds: i64) -> String {
    const SECONDS_PER_DAY: i64 = 86_400;
    let days = unix_seconds.div_euclid(SECONDS_PER_DAY);
    let second_of_day = unix_seconds.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        second_of_day / 3600,
        second_of_day % 3600 / 60,
        second_of_day % 60
    )
}

/// The proleptic Gregorian date `days` after 1970-01-01, by Howard
/// Hinnant's `civil_from_days` (eras of 400 years starting on March 1st).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

```

Insert above the `#[cfg(test)]` line of `src/fakes/clock.rs`:

```rust
use crate::ports::Clock;

/// A clock stopped at one moment.
pub struct FakeClock(i64);

impl FakeClock {
    #[must_use]
    pub fn at(unix_seconds: i64) -> Self {
        Self(unix_seconds)
    }
}

impl Clock for FakeClock {
    fn unix_seconds(&self) -> i64 {
        self.0
    }
}

```

Create `src/fakes/file_system/links.rs`:

```rust
//! Symbolic links in the fake: resolving them and writing through them.

use std::io;
use std::path::{Component, Path, PathBuf};

use super::FakeFileSystem;
use crate::ports::FileSystem;

/// How many links one path may go through, as the kernel's `ELOOP` limit.
const LINK_LIMIT: usize = 40;

/// `path` with `.` and `..` taken out, without looking at links.
fn normalize(path: &Path) -> PathBuf {
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normal.pop();
            }
            other => normal.push(other),
        }
    }
    normal
}

fn not_found(path: &Path, why: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{}: {why}", path.display()),
    )
}

impl FakeFileSystem {
    /// `path` with every symbolic link it goes through replaced by its target
    /// (a relative target is relative to the link's directory); `None` for a
    /// loop.
    pub(super) fn resolve(&self, path: &Path) -> Option<PathBuf> {
        let mut current = normalize(path);
        for _ in 0..LINK_LIMIT {
            match self.follow_one_link(&current) {
                Some(next) => current = next,
                None => return Some(current),
            }
        }
        None
    }

    fn follow_one_link(&self, path: &Path) -> Option<PathBuf> {
        let links = self.links.borrow();
        links.iter().find_map(|(link, target)| {
            let rest = path.strip_prefix(link).ok()?;
            let directory = link.parent().unwrap_or_else(|| Path::new("/"));
            Some(normalize(&directory.join(target).join(rest)))
        })
    }

    pub(super) fn canonical(&self, path: &Path) -> io::Result<PathBuf> {
        let resolved = self
            .resolve(path)
            .ok_or_else(|| not_found(path, "link loop"))?;
        match self.file_info(&resolved) {
            Ok(_) => Ok(resolved),
            Err(_) => Err(not_found(path, "no such file")),
        }
    }

    /// [`FileSystem::replace_file`] on the maps: the same observable contract
    /// as the real one, plus a record of the paths replaced.
    pub(super) fn replace_through_links(
        &self,
        path: &Path,
        contents: &str,
        verify: &dyn Fn(&Path) -> io::Result<()>,
    ) -> io::Result<()> {
        let target = self.replace_target(path)?;
        let temporary = self.temporary_beside(&target);
        self.write_file(&temporary, contents)?;
        let result = verify(&temporary).and_then(|()| self.rename(&temporary, &target));
        match result {
            Ok(()) => self.replaced.borrow_mut().push(path.to_path_buf()),
            Err(_) => {
                self.files.borrow_mut().remove(&temporary);
            }
        }
        result
    }

    /// The end of the link `path`, or the file `path` names (maybe missing).
    fn replace_target(&self, path: &Path) -> io::Result<PathBuf> {
        if self.read_link(path).is_ok() {
            return self
                .canonical(path)
                .map_err(|_| not_found(path, "dangling symbolic link"));
        }
        self.resolve(path)
            .ok_or_else(|| not_found(path, "link loop"))
    }

    fn temporary_beside(&self, target: &Path) -> PathBuf {
        let counter = self.temporaries.get();
        self.temporaries.set(counter + 1);
        let name = format!(".nvmrc-tmp-{}-{counter}", std::process::id());
        target.parent().unwrap_or_else(|| Path::new("/")).join(name)
    }
}
```

Create `src/fakes/file_system/links_tests.rs`:

```rust
use std::cell::RefCell;

use super::*;

fn path(text: &str) -> PathBuf {
    PathBuf::from(text)
}

fn stowed() -> FakeFileSystem {
    let fs = FakeFileSystem::default()
        .with_file("/dotfiles/zsh/.zshrc", "old")
        .with_file("/real/sub/f", "x")
        .with_dir("/home");
    fs.symlink(
        Path::new("../dotfiles/zsh/.zshrc"),
        Path::new("/home/.zshrc"),
    )
    .unwrap();
    fs
}

#[test]
fn canonicalize_resolves_relative_absolute_chained_and_directory_links() {
    let fs = stowed();
    fs.symlink(Path::new("/real"), Path::new("/dir-link"))
        .unwrap();
    fs.symlink(Path::new("/home/.zshrc"), Path::new("/chain"))
        .unwrap();
    let canonical = |text: &str| fs.canonicalize(Path::new(text)).unwrap();
    assert_eq!(canonical("/home/.zshrc"), path("/dotfiles/zsh/.zshrc"));
    assert_eq!(canonical("/chain"), path("/dotfiles/zsh/.zshrc"));
    assert_eq!(canonical("/dir-link/sub/f"), path("/real/sub/f"));
    assert_eq!(canonical("/real/sub/f"), path("/real/sub/f"));
    assert_eq!(canonical("/real/sub"), path("/real/sub"));
}

#[test]
fn canonicalize_is_not_found_for_a_missing_path_or_a_dangling_link() {
    let fs = stowed();
    fs.symlink(Path::new("gone"), Path::new("/home/dangling"))
        .unwrap();
    fs.symlink(Path::new("/loop-b"), Path::new("/loop-a"))
        .unwrap();
    fs.symlink(Path::new("/loop-a"), Path::new("/loop-b"))
        .unwrap();
    for missing in ["/home/dangling", "/missing", "/loop-a"] {
        let error = fs.canonicalize(Path::new(missing)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound, "{missing}");
    }
}

#[test]
fn reading_through_a_link_reads_its_target() {
    let fs = stowed();
    assert_eq!(fs.read_to_string(Path::new("/home/.zshrc")).unwrap(), "old");
    assert!(fs.is_file(Path::new("/home/.zshrc")));
}

#[test]
fn replace_file_writes_through_the_link_and_keeps_it() {
    let fs = stowed();
    let seen = RefCell::new(Vec::new());
    let verify = |temporary: &Path| {
        seen.borrow_mut()
            .push((temporary.to_path_buf(), fs.read_to_string(temporary)?));
        Ok(())
    };
    fs.replace_file(Path::new("/home/.zshrc"), "new", &verify)
        .unwrap();
    let link = fs.read_link(Path::new("/home/.zshrc")).unwrap();
    assert_eq!(link, path("../dotfiles/zsh/.zshrc"));
    assert_eq!(
        fs.read_to_string(Path::new("/dotfiles/zsh/.zshrc"))
            .unwrap(),
        "new"
    );
    let seen = seen.into_inner();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0.parent(), Some(Path::new("/dotfiles/zsh")));
    assert_eq!(seen[0].1, "new");
    let listed = fs.read_dir(Path::new("/dotfiles/zsh")).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(fs.replaced(), [path("/home/.zshrc")]);
}

#[test]
fn replace_file_keeps_the_target_and_drops_the_temporary_when_verify_fails() {
    let fs = stowed();
    let refuse = |_temporary: &Path| Err(io::Error::other("syntax error"));
    let error = fs
        .replace_file(Path::new("/home/.zshrc"), "new", &refuse)
        .unwrap_err();
    assert_eq!(error.to_string(), "syntax error");
    assert_eq!(
        fs.read_to_string(Path::new("/dotfiles/zsh/.zshrc"))
            .unwrap(),
        "old"
    );
    assert_eq!(fs.read_dir(Path::new("/dotfiles/zsh")).unwrap().len(), 1);
    assert!(fs.replaced().is_empty());
}

#[test]
fn replace_file_refuses_a_dangling_link_and_creates_a_plain_missing_file() {
    let fs = stowed();
    fs.symlink(Path::new("gone"), Path::new("/home/dangling"))
        .unwrap();
    let accept = |_temporary: &Path| Ok(());
    let error = fs
        .replace_file(Path::new("/home/dangling"), "x", &accept)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    assert!(!fs.is_file(Path::new("/home/gone")));
    fs.replace_file(Path::new("/home/.kshrc"), "y", &accept)
        .unwrap();
    assert_eq!(fs.read_to_string(Path::new("/home/.kshrc")).unwrap(), "y");
    // The fake lists files and directories, not links: no temporary is left.
    let listed = fs.read_dir(Path::new("/home")).unwrap();
    let names: Vec<&str> = listed.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, [".kshrc"]);
}
```

Apply to `src/fakes/file_system/mod.rs` (above the test module):

```diff
--- a/src/fakes/file_system/mod.rs
+++ b/src/fakes/file_system/mod.rs
@@ -1,4 +1,4 @@
-use std::cell::RefCell;
+use std::cell::{Cell, RefCell};
 use std::collections::{BTreeMap, BTreeSet};
 use std::io;
 use std::path::{Path, PathBuf};
@@ -13,6 +13,8 @@
     executables: RefCell<BTreeSet<PathBuf>>,
     modified: RefCell<BTreeMap<PathBuf, SystemTime>>,
     links: RefCell<BTreeMap<PathBuf, PathBuf>>,
+    replaced: RefCell<Vec<PathBuf>>,
+    temporaries: Cell<u64>,
 }
 
 impl FakeFileSystem {
@@ -41,6 +43,13 @@
     pub fn with_modified(mut self, path: &str, time: SystemTime) -> Self {
         self.modified.get_mut().insert(PathBuf::from(path), time);
         self
+    }
+
+    /// The paths given to [`FileSystem::replace_file`] that it replaced, in
+    /// order.
+    #[must_use]
+    pub fn replaced(&self) -> Vec<PathBuf> {
+        self.replaced.borrow().clone()
     }
 
     /// An explicit, possibly empty, directory. Parents of files exist already.
@@ -66,7 +75,8 @@
 
 impl FileSystem for FakeFileSystem {
     fn read_to_string(&self, path: &Path) -> io::Result<String> {
-        let bytes = self.files.borrow().get(path).cloned();
+        let path = self.resolve(path).unwrap_or_else(|| path.to_path_buf());
+        let bytes = self.files.borrow().get(&path).cloned();
         let bytes = bytes.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
         String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
     }
@@ -100,7 +110,8 @@
     }
 
     fn is_file(&self, path: &Path) -> bool {
-        self.files.borrow().contains_key(path)
+        let path = self.resolve(path).unwrap_or_else(|| path.to_path_buf());
+        self.files.borrow().contains_key(&path)
     }
 
     fn remove_file(&self, path: &Path) -> io::Result<()> {
@@ -199,19 +210,27 @@
     }
 
     fn same_directory(&self, a: &Path, b: &Path) -> bool {
-        let (a, b) = (self.resolve(a), self.resolve(b));
-        a == b && self.file_info(&a).is_ok_and(|info| info.is_dir)
-    }
-}
-
-impl FakeFileSystem {
-    /// `path` with a symlink it starts with replaced by its target.
-    fn resolve(&self, path: &Path) -> PathBuf {
-        let links = self.links.borrow();
-        let found = links
-            .iter()
-            .find_map(|(link, target)| Some(target.join(path.strip_prefix(link).ok()?)));
-        found.unwrap_or_else(|| path.to_path_buf())
-    }
-}
-
+        match (self.resolve(a), self.resolve(b)) {
+            (Some(a), Some(b)) => a == b && self.file_info(&a).is_ok_and(|info| info.is_dir),
+            _ => false,
+        }
+    }
+
+    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
+        self.canonical(path)
+    }
+
+    fn replace_file(
+        &self,
+        path: &Path,
+        contents: &str,
+        verify: &dyn Fn(&Path) -> io::Result<()>,
+    ) -> io::Result<()> {
+        self.replace_through_links(path, contents, verify)
+    }
+}
+
+mod links;
+
+#[cfg(test)]
+mod links_tests;
```

Apply to `src/fakes/mod.rs` (above the test module):

```diff
--- a/src/fakes/mod.rs
+++ b/src/fakes/mod.rs
@@ -2,22 +2,26 @@
 
 mod archive;
 mod channel;
+mod clock;
 mod cpu;
 mod digest;
 mod env;
 mod file_system;
 mod http;
 mod process;
+mod prompt;
 mod sleeper;
 mod terminal;
 
 pub use archive::FakeArchive;
 pub use channel::FakeScriptChannel;
+pub use clock::FakeClock;
 pub use cpu::FakeCpu;
 pub use digest::FakeDigest;
 pub use env::FakeEnv;
 pub use file_system::FakeFileSystem;
 pub use http::FakeHttp;
 pub use process::FakeProcess;
+pub use prompt::FakePrompt;
 pub use sleeper::FakeSleeper;
 pub use terminal::FakeTerminal;
```

Insert above the `#[cfg(test)]` line of `src/fakes/prompt.rs`:

```rust
use std::cell::RefCell;
use std::io;

use crate::ports::Prompt;

/// A person who always gives the same answer, or nobody at all.
pub struct FakePrompt {
    answer: Option<bool>,
    asked: RefCell<Vec<String>>,
}

impl FakePrompt {
    #[must_use]
    pub fn answering(answer: bool) -> Self {
        Self {
            answer: Some(answer),
            asked: RefCell::default(),
        }
    }

    /// Standard input is not a terminal: every question is `Unsupported`.
    #[must_use]
    pub fn unavailable() -> Self {
        Self {
            answer: None,
            asked: RefCell::default(),
        }
    }

    /// The questions asked so far, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<String> {
        self.asked.borrow().clone()
    }
}

impl Prompt for FakePrompt {
    fn confirm(&self, question: &str) -> io::Result<bool> {
        self.asked.borrow_mut().push(question.to_owned());
        self.answer
            .ok_or_else(|| io::Error::from(io::ErrorKind::Unsupported))
    }
}

```

Create `src/ports/archive.rs`:

```rust
use std::io;
use std::path::Path;

pub trait Digest {
    /// The SHA-256 of the file at `path`, in lowercase hex.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn sha256_file(&self, path: &Path) -> io::Result<String>;
}

pub trait Archive {
    /// Unpacks the `.tar.gz` or `.tar.xz` at `archive` into the existing
    /// directory `destination`, keeping every path inside it.
    ///
    /// # Errors
    /// Propagates the underlying I/O error, and fails on a file that is not a
    /// gzip- or xz-compressed tar.
    fn extract(&self, archive: &Path, destination: &Path) -> io::Result<()>;
}
```

Create `src/ports/channel.rs`:

```rust
use std::io;

/// Where the shell code of a command goes in the `nvm` function.
pub trait ScriptChannel {
    /// Hands `code` over to the calling shell.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn send(&self, code: &str) -> io::Result<()>;
}
```

Create `src/ports/env.rs`:

```rust
use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

pub trait Env {
    /// The variable as text; `None` when unset or not valid UTF-8.
    fn var(&self, key: &str) -> Option<String>;

    /// The variable as an OS string, so non-UTF-8 paths survive.
    fn var_os(&self, key: &str) -> Option<OsString>;

    /// Every variable, as text. A variable whose name or value is not valid
    /// UTF-8 is skipped.
    fn vars(&self) -> Vec<(String, String)>;

    /// The directory of the process, as the system resolves it.
    ///
    /// # Errors
    /// Fails when it cannot be read (removed, or not searchable).
    fn current_dir(&self) -> io::Result<PathBuf>;
}
```

Create `src/ports/fs.rs`:

```rust
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// A direct child of a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// What a path is, following symlinks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileInfo {
    pub is_dir: bool,
    pub len: u64,
    /// Any execute permission bit is set (always true off Unix).
    pub executable: bool,
    pub modified: Option<SystemTime>,
}

pub trait FileSystem {
    /// # Errors
    /// Propagates the underlying I/O error.
    fn read_to_string(&self, path: &Path) -> io::Result<String>;

    /// The direct children of `path`, in unspecified order.
    ///
    /// # Errors
    /// Propagates the underlying I/O error (for example when `path` is missing
    /// or is not a directory).
    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>>;

    fn is_file(&self, path: &Path) -> bool;

    /// # Errors
    /// Propagates the underlying I/O error (for example when `path` is missing).
    fn remove_file(&self, path: &Path) -> io::Result<()>;

    /// Removes `path` and everything under it, like `rm -rf`: a path that is
    /// missing is fine.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;

    /// Creates or replaces the file at `path`.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()>;

    /// Like [`Self::write_file`], for contents that are not text.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn write_bytes(&self, path: &Path, contents: &[u8]) -> io::Result<()>;

    /// # Errors
    /// Propagates the underlying I/O error (for example when `path` is missing).
    fn file_info(&self, path: &Path) -> io::Result<FileInfo>;

    /// Moves a file or a directory (with everything in it) to `to`, which must
    /// not exist.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// Creates one directory, failing with `AlreadyExists` when it is there:
    /// the atomic step a lock is made of.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn create_dir(&self, path: &Path) -> io::Result<()>;

    /// Creates `path` and any missing parents; fine if it already exists.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;

    /// Creates a symbolic link at `link` pointing to `target`, failing with
    /// `AlreadyExists` when `link` is there. `ErrorKind::Unsupported` where
    /// the platform has no symlinks.
    ///
    /// # Errors
    /// Propagates the underlying I/O error.
    fn symlink(&self, target: &Path, link: &Path) -> io::Result<()>;

    /// Where the symbolic link `link` points.
    ///
    /// # Errors
    /// Propagates the underlying I/O error (for example when `link` is
    /// missing or is not a symbolic link).
    fn read_link(&self, link: &Path) -> io::Result<PathBuf>;

    /// Whether `a` and `b` are one existing directory, symlinks followed
    /// (the same device and inode on Unix).
    fn same_directory(&self, a: &Path, b: &Path) -> bool;

    /// The absolute path `path` names, every symbolic link resolved.
    ///
    /// # Errors
    /// `NotFound` when `path`, or the end of a dangling link, is missing;
    /// otherwise propagates the underlying I/O error.
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;

    /// Atomically replaces the file `path` resolves to with `contents`: a
    /// symbolic link at `path` is followed and SURVIVES. The contents go to a
    /// unique temporary file in the target's directory, with the target's
    /// permissions (a new file gets `0o644` on Unix), synced, then
    /// `verify(temporary file)` runs, and only then the temporary file is
    /// renamed onto the target. Any failure removes the temporary file and
    /// leaves the target untouched.
    ///
    /// # Errors
    /// `NotFound` for a dangling link; the error of `verify`; otherwise
    /// propagates the underlying I/O error.
    fn replace_file(
        &self,
        path: &Path,
        contents: &str,
        verify: &dyn Fn(&Path) -> io::Result<()>,
    ) -> io::Result<()>;
}
```

Create `src/ports/http.rs`:

```rust
use thiserror::Error;

/// Why a download failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum HttpError {
    #[error("{url}: HTTP {code}")]
    Status { url: String, code: u16 },
    #[error("{url}: {message}")]
    Transport { url: String, message: String },
    /// The server answered, but the body is not usable (too large, or not
    /// text). Asking again would not change that.
    #[error("{url}: {message}")]
    Body { url: String, message: String },
}

pub trait Http {
    /// Fetches `url` and returns the body as it is (an archive, say).
    ///
    /// # Errors
    /// Fails on a non-success status, on a network error, or when the body is
    /// over the size limit.
    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError>;

    /// Fetches `url` and returns the body as text.
    ///
    /// # Errors
    /// Fails on a non-success status, on a network error, or when the body is
    /// not text.
    fn get_text(&self, url: &str) -> Result<String, HttpError>;
}
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -1,299 +1,19 @@
 //! Traits through which the domain and commands reach the outside world.
 
-use std::ffi::OsString;
-use std::io;
-use std::path::{Path, PathBuf};
-use std::time::{Duration, SystemTime};
+mod archive;
+mod channel;
+mod env;
+mod fs;
+mod http;
+mod process;
+mod system;
+mod terminal;
 
-use thiserror::Error;
-
-/// A direct child of a directory.
-#[derive(Debug, Clone, PartialEq, Eq)]
-pub struct DirEntry {
-    pub name: String,
-    pub is_dir: bool,
-}
-
-/// What a path is, following symlinks.
-#[derive(Debug, Clone, Copy, PartialEq, Eq)]
-pub struct FileInfo {
-    pub is_dir: bool,
-    pub len: u64,
-    /// Any execute permission bit is set (always true off Unix).
-    pub executable: bool,
-    pub modified: Option<SystemTime>,
-}
-
-pub trait FileSystem {
-    /// # Errors
-    /// Propagates the underlying I/O error.
-    fn read_to_string(&self, path: &Path) -> io::Result<String>;
-
-    /// The direct children of `path`, in unspecified order.
-    ///
-    /// # Errors
-    /// Propagates the underlying I/O error (for example when `path` is missing
-    /// or is not a directory).
-    fn read_dir(&self, path: &Path) -> io::Result<Vec<DirEntry>>;
-
-    fn is_file(&self, path: &Path) -> bool;
-
-    /// # Errors
-    /// Propagates the underlying I/O error (for example when `path` is missing).
-    fn remove_file(&self, path: &Path) -> io::Result<()>;
-
-    /// Removes `path` and everything under it, like `rm -rf`: a path that is
-    /// missing is fine.
-    ///
-    /// # Errors
-    /// Propagates the underlying I/O error.
-    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
-
-    /// Creates or replaces the file at `path`.
-    ///
-    /// # Errors
-    /// Propagates the underlying I/O error.
-    fn write_file(&self, path: &Path, contents: &str) -> io::Result<()>;
-
-    /// Like [`Self::write_file`], for contents that are not text.
-    ///
-    /// # Errors
-    /// Propagates the underlying I/O error.
-    fn write_bytes(&self, path: &Path, contents: &[u8]) -> io::Result<()>;
-
-    /// # Errors
-    /// Propagates the underlying I/O error (for example when `path` is missing).
-    fn file_info(&self, path: &Path) -> io::Result<FileInfo>;
-
-    /// Moves a file or a directory (with everything in it) to `to`, which must
-    /// not exist.
-    ///
-    /// # Errors
-    /// Propagates the underlying I/O error.
-    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
-
-    /// Creates one directory, failing with `AlreadyExists` when it is there:
-    /// the atomic step a lock is made of.
-    ///
-    /// # Errors
-    /// Propagates the underlying I/O error.
-    fn create_dir(&self, path: &Path) -> io::Result<()>;
-
-    /// Creates `path` and any missing parents; fine if it already exists.
-    ///
-    /// # Errors
-    /// Propagates the underlying I/O error.
-    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
-
-    /// Creates a symbolic link at `link` pointing to `target`, failing with
-    /// `AlreadyExists` when `link` is there. `ErrorKind::Unsupported` where
-    /// the platform has no symlinks.
-    ///
-    /// # Errors
-    /// Propagates the underlying I/O error.
-    fn symlink(&self, target: &Path, link: &Path) -> io::Result<()>;
-
-    /// Where the symbolic link `link` points.
-    ///
-    /// # Errors
-    /// Propagates the underlying I/O error (for example when `link` is
-    /// missing or is not a symbolic link).
-    fn read_link(&self, link: &Path) -> io::Result<PathBuf>;
-
-    /// Whether `a` and `b` are one existing directory, symlinks followed
-    /// (the same device and inode on Unix).
-    fn same_directory(&self, a: &Path, b: &Path) -> bool;
-}
-
-/// What a finished child process left behind.
-#[derive(Debug, Clone, PartialEq, Eq)]
-pub struct ProcessOutput {
-    pub success: bool,
-    pub stdout: String,
-}
-
-/// A program to run to completion, with no time limit: `npm install`,
-/// `./configure`, `make`.
-#[derive(Debug, Clone, Default, PartialEq, Eq)]
-pub struct Invocation {
-    pub program: PathBuf,
-    pub args: Vec<String>,
-    /// Where to run it; the current directory when `None`.
-    pub dir: Option<PathBuf>,
-    /// Variables added to the environment it inherits.
-    pub env: Vec<(String, String)>,
-    /// Variables taken out of the environment it inherits.
-    pub env_remove: Vec<String>,
-    /// A directory put in front of `PATH`, so a `node` that `npm` starts is
-    /// the one next to it.
-    pub path_prefix: Option<PathBuf>,
-}
-
-impl Invocation {
-    #[must_use]
-    pub fn new(program: impl Into<PathBuf>) -> Self {
-        Self {
-            program: program.into(),
-            ..Self::default()
-        }
-    }
-
-    #[must_use]
-    pub fn args(mut self, args: &[&str]) -> Self {
-        self.args.extend(args.iter().map(|arg| (*arg).to_owned()));
-        self
-    }
-
-    #[must_use]
-    pub fn dir(mut self, dir: impl Into<PathBuf>) -> Self {
-        self.dir = Some(dir.into());
-        self
-    }
-
-    #[must_use]
-    pub fn env(mut self, name: &str, value: &str) -> Self {
-        self.env.push((name.to_owned(), value.to_owned()));
-        self
-    }
-
-    #[must_use]
-    pub fn env_remove(mut self, name: &str) -> Self {
-        self.env_remove.push(name.to_owned());
-        self
-    }
-
-    #[must_use]
-    pub fn path_prefix(mut self, directory: impl Into<PathBuf>) -> Self {
-        self.path_prefix = Some(directory.into());
-        self
-    }
-}
-
-/// What a program that ran to the end printed.
-#[derive(Debug, Clone, Default, PartialEq, Eq)]
-pub struct Completed {
-    pub success: bool,
-    /// The exit status, when the program ended by itself.
-    pub code: Option<i32>,
-    pub stdout: String,
-    pub stderr: String,
-}
-
-pub trait Process {
-    /// Runs `invocation` until it ends and returns everything it printed.
-    ///
-    /// # Errors
-    /// Fails when the program cannot be started.
-    fn execute(&self, invocation: &Invocation) -> io::Result<Completed>;
-
-    /// Runs `program` with `args` and waits for it.
-    ///
-    /// # Errors
-    /// Fails when the program cannot be started.
-    fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput>;
-
-    /// Runs `invocation` with stdin, stdout and stderr inherited from this
-    /// process, waits for it, and returns its exit status; a child killed by
-    /// a signal gives `128 + signal` (Unix).
-    ///
-    /// `invocation.env_remove` is taken out of the inherited environment,
-    /// `invocation.env` is added to it, and an entry named
-    /// `PATH` replaces the `PATH` the child would inherit. `path_prefix` is
-    /// then put in front of whichever `PATH` the child ends up with. A
-    /// program named without a `/` is looked up on that final `PATH`. No
-    /// program gets `NVMRC_SCRIPT_FD`, whatever `invocation` says.
-    ///
-    /// # Errors
-    /// Fails when the program cannot be started (`NotFound` when it does not
-    /// exist).
-    fn spawn(&self, invocation: &Invocation) -> io::Result<i32>;
-}
-
-/// Why a download failed.
-#[derive(Debug, Clone, PartialEq, Eq, Error)]
-pub enum HttpError {
-    #[error("{url}: HTTP {code}")]
-    Status { url: String, code: u16 },
-    #[error("{url}: {message}")]
-    Transport { url: String, message: String },
-    /// The server answered, but the body is not usable (too large, or not
-    /// text). Asking again would not change that.
-    #[error("{url}: {message}")]
-    Body { url: String, message: String },
-}
-
-pub trait Http {
-    /// Fetches `url` and returns the body as it is (an archive, say).
-    ///
-    /// # Errors
-    /// Fails on a non-success status, on a network error, or when the body is
-    /// over the size limit.
-    fn get_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError>;
-
-    /// Fetches `url` and returns the body as text.
-    ///
-    /// # Errors
-    /// Fails on a non-success status, on a network error, or when the body is
-    /// not text.
-    fn get_text(&self, url: &str) -> Result<String, HttpError>;
-}
-
-pub trait Digest {
-    /// The SHA-256 of the file at `path`, in lowercase hex.
-    ///
-    /// # Errors
-    /// Propagates the underlying I/O error.
-    fn sha256_file(&self, path: &Path) -> io::Result<String>;
-}
-
-pub trait Archive {
-    /// Unpacks the `.tar.gz` or `.tar.xz` at `archive` into the existing
-    /// directory `destination`, keeping every path inside it.
-    ///
-    /// # Errors
-    /// Propagates the underlying I/O error, and fails on a file that is not a
-    /// gzip- or xz-compressed tar.
-    fn extract(&self, archive: &Path, destination: &Path) -> io::Result<()>;
-}
-
-pub trait Terminal {
-    /// Whether standard output is a terminal (`[ -t 1 ]`).
-    fn stdout_is_terminal(&self) -> bool;
-}
-
-pub trait Cpu {
-    /// How many processors this machine has for a build to use, when known.
-    fn cores(&self) -> Option<usize>;
-}
-
-pub trait Sleeper {
-    /// Waits for `duration` (between retries).
-    fn sleep(&self, duration: Duration);
-}
-
-pub trait Env {
-    /// The variable as text; `None` when unset or not valid UTF-8.
-    fn var(&self, key: &str) -> Option<String>;
-
-    /// The variable as an OS string, so non-UTF-8 paths survive.
-    fn var_os(&self, key: &str) -> Option<OsString>;
-
-    /// Every variable, as text. A variable whose name or value is not valid
-    /// UTF-8 is skipped.
-    fn vars(&self) -> Vec<(String, String)>;
-
-    /// The directory of the process, as the system resolves it.
-    ///
-    /// # Errors
-    /// Fails when it cannot be read (removed, or not searchable).
-    fn current_dir(&self) -> io::Result<PathBuf>;
-}
-
-/// Where the shell code of a command goes in the `nvm` function.
-pub trait ScriptChannel {
-    /// Hands `code` over to the calling shell.
-    ///
-    /// # Errors
-    /// Propagates the underlying I/O error.
-    fn send(&self, code: &str) -> io::Result<()>;
-}
+pub use archive::{Archive, Digest};
+pub use channel::ScriptChannel;
+pub use env::Env;
+pub use fs::{DirEntry, FileInfo, FileSystem};
+pub use http::{Http, HttpError};
+pub use process::{Completed, Invocation, Process, ProcessOutput};
+pub use system::{Clock, Cpu, Sleeper};
+pub use terminal::{Prompt, Terminal};
```

Create `src/ports/process.rs`:

```rust
use std::io;
use std::path::{Path, PathBuf};

/// What a finished child process left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub success: bool,
    pub stdout: String,
}

/// A program to run to completion, with no time limit: `npm install`,
/// `./configure`, `make`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Invocation {
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Where to run it; the current directory when `None`.
    pub dir: Option<PathBuf>,
    /// Variables added to the environment it inherits.
    pub env: Vec<(String, String)>,
    /// Variables taken out of the environment it inherits.
    pub env_remove: Vec<String>,
    /// A directory put in front of `PATH`, so a `node` that `npm` starts is
    /// the one next to it.
    pub path_prefix: Option<PathBuf>,
}

impl Invocation {
    #[must_use]
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn args(mut self, args: &[&str]) -> Self {
        self.args.extend(args.iter().map(|arg| (*arg).to_owned()));
        self
    }

    #[must_use]
    pub fn dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.dir = Some(dir.into());
        self
    }

    #[must_use]
    pub fn env(mut self, name: &str, value: &str) -> Self {
        self.env.push((name.to_owned(), value.to_owned()));
        self
    }

    #[must_use]
    pub fn env_remove(mut self, name: &str) -> Self {
        self.env_remove.push(name.to_owned());
        self
    }

    #[must_use]
    pub fn path_prefix(mut self, directory: impl Into<PathBuf>) -> Self {
        self.path_prefix = Some(directory.into());
        self
    }
}

/// What a program that ran to the end printed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Completed {
    pub success: bool,
    /// The exit status, when the program ended by itself.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub trait Process {
    /// Runs `invocation` until it ends and returns everything it printed.
    ///
    /// # Errors
    /// Fails when the program cannot be started.
    fn execute(&self, invocation: &Invocation) -> io::Result<Completed>;

    /// Runs `program` with `args` and waits for it.
    ///
    /// # Errors
    /// Fails when the program cannot be started.
    fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput>;

    /// Runs `invocation` with stdin, stdout and stderr inherited from this
    /// process, waits for it, and returns its exit status; a child killed by
    /// a signal gives `128 + signal` (Unix).
    ///
    /// `invocation.env_remove` is taken out of the inherited environment,
    /// `invocation.env` is added to it, and an entry named
    /// `PATH` replaces the `PATH` the child would inherit. `path_prefix` is
    /// then put in front of whichever `PATH` the child ends up with. A
    /// program named without a `/` is looked up on that final `PATH`. No
    /// program gets `NVMRC_SCRIPT_FD`, whatever `invocation` says.
    ///
    /// # Errors
    /// Fails when the program cannot be started (`NotFound` when it does not
    /// exist).
    fn spawn(&self, invocation: &Invocation) -> io::Result<i32>;
}
```

Create `src/ports/system.rs`:

```rust
use std::time::Duration;

pub trait Cpu {
    /// How many processors this machine has for a build to use, when known.
    fn cores(&self) -> Option<usize>;
}

pub trait Sleeper {
    /// Waits for `duration` (between retries).
    fn sleep(&self, duration: Duration);
}

pub trait Clock {
    /// The current time, in seconds since the Unix epoch (UTC); negative
    /// before 1970.
    fn unix_seconds(&self) -> i64;
}
```

Create `src/ports/terminal.rs`:

```rust
use std::io;

pub trait Terminal {
    /// Whether standard output is a terminal (`[ -t 1 ]`).
    fn stdout_is_terminal(&self) -> bool;
}

/// Asks the person at the terminal.
pub trait Prompt {
    /// Shows `question` and waits for a yes or no; only `y` or `yes` (any
    /// case) count as yes.
    ///
    /// # Errors
    /// `Unsupported` when nobody can answer (standard input is not a
    /// terminal); otherwise propagates the underlying I/O error.
    fn confirm(&self, question: &str) -> io::Result<bool>;
}
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1168 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(ports): atomic replace through symlinks, a clock and a prompt"
```

---

### Task 2: The conflict rules

**Files:**

- Create: `src/domain/conflict/expand.rs`,
  `src/domain/conflict/expand_tests.rs`, `src/domain/conflict/kind.rs`,
  `src/domain/conflict/kind_tests.rs`, `src/domain/conflict/lexer.rs`,
  `src/domain/conflict/lexer_tests.rs`, `src/domain/conflict/mod.rs`,
  `src/domain/conflict/nesting.rs`, `src/domain/conflict/nesting_tests.rs`,
  `src/domain/conflict/plugins.rs`, `src/domain/conflict/rules.rs`,
  `src/domain/conflict/rules_tests.rs`, `src/domain/conflict/source_path.rs`,
  `src/domain/conflict/source_path_tests.rs`, `src/domain/conflict/tests.rs`
- Modify: `src/domain/mod.rs`, `src/shell/init/mod.rs`

**Interfaces:**

- Produces: `domain::conflict::{Severity, Kind, Hit, scan_text, source_target,
  Expansion, expand_path, BEGIN_MARKER, END_MARKER, MIGRATED_PREFIX}`. `Kind` is
  `Loader`, `Completion`, `LazyLoader`, `LazyStub`, `Unset`, `OmzPlugin`,
  `ZshNvm`, `Bass`, `HelperCall`, `NvmDirExport`, `NotFollowed`, with
  `severity()`, `is_conflict()`, `is_auto_migratable()` (only `Loader` and
  `Completion`) and `label()`; `Hit { line (1-based), kind, text }`;
  `scan_text(&str) -> Vec<Hit>`. The markers moved here from `shell::init`,
  which re-exports them.
- Behaviour (research digest sections 1 to 3 and 7, every example is a test):
  Loader is a `source`/`.`/`\.` of a path ending in `nvm.sh` (the install.sh
  forms old and new, absolute and `~` paths, Homebrew's
  `${HOMEBREW_PREFIX}/opt/nvm/nvm.sh` and `$(brew --prefix nvm)/nvm.sh`, the `[
  -s ... ] && \. ...` one-liners) and Completion the nvm `bash_completion` line;
  `dnvm.sh`, `nvm.sh.bak`, comments, `# [nvmrc-migrated]` lines and the nvmrc
  block are not hits. A Loader or Completion nested under an opener (`{`,
  `then`, `do`, indented deeper than the opener; one-liners are top level) or in
  a file with a `LazyStub` or `Unset` hit is a `LazyLoader`: manual. `LazyStub`
  is a definition of nvm, node, npm, npx, yarn, pnpm, pnpx or corepack in both
  `name() {` and `function name {` forms; `OmzPlugin` is the word `nvm` inside a
  (multi-line) `plugins=( ... )` or a `zstyle ':omz:plugins:nvm'`; a `#` starts
  a comment only at the start of a word outside quotes. `source_target` extracts
  the path token; `expand_path` expands `~`, `$HOME`, `${ZDOTDIR:-$HOME}` and
  any `$VAR`/`${VAR}` from the environment and calls anything else (`$(...)`,
  backticks, `${0:A:h}`, other `${..:..}`, globs) unresolvable. One `Hit` per
  matching kind of a line, in a fixed order.
- [ ] **Step 1: Declare the new modules**

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -3,6 +3,7 @@
 pub mod checksum;
 pub mod colors;
 pub mod compression;
+pub mod conflict;
 pub mod current;
 #[cfg(test)]
 pub(crate) mod fixtures;
```

- [ ] **Step 2: Write the failing tests**

Create `src/domain/conflict/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;

pub use expand::{Expansion, expand_path};
pub use kind::{Kind, Severity};
pub use source_path::source_target;

use lexer::{indentation, strip_comment, tokens};
use nesting::BlockTracker;
use plugins::PluginsBlock;
use rules::line_kinds;

/// The first line of nvmrc's init block (the snippet of `nvm init`).
pub const BEGIN_MARKER: &str = "# >>> nvmrc init >>>";
/// The last line of nvmrc's init block.
pub const END_MARKER: &str = "# <<< nvmrc init <<<";
/// What `migrate` puts before a line it disables.
pub const MIGRATED_PREFIX: &str = "# [nvmrc-migrated]";

/// One rule matching one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// The line number, from 1.
    pub line: usize,
    pub kind: Kind,
    /// The whole line, as written (without its line ending).
    pub text: String,
}

/// Every hit of the startup file `text` (see the module documentation).
#[must_use]
pub fn scan_text(text: &str) -> Vec<Hit> {
    let mut scanner = Scanner::default();
    for (index, line) in text.lines().enumerate() {
        scanner.observe(index + 1, line);
    }
    scanner.finish()
}

/// The state carried from line to line.
#[derive(Debug, Default)]
struct Scanner {
    hits: Vec<Hit>,
    inside_init_block: bool,
    plugins: PluginsBlock,
    blocks: BlockTracker,
}

impl Scanner {
    fn observe(&mut self, number: usize, line: &str) {
        if self.is_skipped(line) {
            return;
        }
        let code = strip_comment(line);
        let words = tokens(code);
        let nested = self.blocks.observe(&words, indentation(line));
        let mut kinds = line_kinds(code);
        if self.plugins.observe(&words) && !kinds.contains(&Kind::OmzPlugin) {
            kinds.push(Kind::OmzPlugin);
        }
        for kind in kinds {
            let kind = if nested { lazy(kind) } else { kind };
            self.hits.push(Hit {
                line: number,
                kind,
                text: line.to_string(),
            });
        }
    }

    /// Whether `line` is in (or delimits) the init block, or was migrated.
    fn is_skipped(&mut self, line: &str) -> bool {
        let trimmed = line.trim();
        if self.inside_init_block {
            self.inside_init_block = trimmed != END_MARKER;
            return true;
        }
        self.inside_init_block = trimmed == BEGIN_MARKER;
        self.inside_init_block || trimmed.starts_with(MIGRATED_PREFIX)
    }

    /// The hits, every loader lazy when the file defines stubs or unsets.
    fn finish(self) -> Vec<Hit> {
        let lazy_file = self
            .hits
            .iter()
            .any(|hit| matches!(hit.kind, Kind::LazyStub | Kind::Unset));
        if !lazy_file {
            return self.hits;
        }
        self.hits
            .into_iter()
            .map(|hit| Hit {
                kind: lazy(hit.kind),
                ..hit
            })
            .collect()
    }
}

/// `kind`, or [`Kind::LazyLoader`] for a loader or completion.
fn lazy(kind: Kind) -> Kind {
    if kind.is_auto_migratable() {
        Kind::LazyLoader
    } else {
        kind
    }
}
```

Create `src/domain/conflict/tests.rs`:

```rust
use super::Kind::{
    self, Completion, LazyLoader, LazyStub, Loader, NvmDirExport, OmzPlugin, Unset, ZshNvm,
};
use super::{BEGIN_MARKER, END_MARKER, Hit, scan_text};

/// `(line, kind)` of every hit.
fn found(text: &str) -> Vec<(usize, Kind)> {
    scan_text(text)
        .into_iter()
        .map(|hit| (hit.line, hit.kind))
        .collect()
}

/// `scripts/lazy-functions.zsh` of the user (digest 3.4), at its real line
/// numbers.
fn lazy_functions_zsh() -> String {
    let header = "# header\n".repeat(12);
    format!(
        "{header}nvm() {{\n\
  unfunction nvm node npm npx yarn pnpm 2>/dev/null\n\
  export NVM_DIR=\"${{HOME}}/.nvm\"\n\
  local nvm_prefix=\"${{HOMEBREW_PREFIX:-/opt/homebrew}}/opt/nvm\"\n\
  [ -s \"${{nvm_prefix}}/nvm.sh\" ] && \\. \"${{nvm_prefix}}/nvm.sh\"\n\
  [ -s \"${{nvm_prefix}}/etc/bash_completion.d/nvm\" ] && \\. \"${{nvm_prefix}}/etc/bash_completion.d/nvm\"\n\
  nvm \"$@\"\n\
}}\n\
\n\
\n\
# node() {{ nvm >/dev/null 2>&1; command node \"$@\" }} # TODO: Disabled to install a real node\n\
npm() {{ nvm >/dev/null 2>&1; command npm \"$@\" }}\n\
npx() {{ nvm >/dev/null 2>&1; command npx \"$@\" }}\n\
yarn() {{ nvm >/dev/null 2>&1; command yarn \"$@\" }}\n\
pnpm() {{ nvm >/dev/null 2>&1; command pnpm \"$@\" }}\n"
    )
}

#[test]
fn the_users_lazy_functions_file_is_all_manual() {
    assert_eq!(
        found(&lazy_functions_zsh()),
        [
            (13, LazyStub),
            (14, Unset),
            (15, NvmDirExport),
            (17, LazyLoader),
            (18, LazyLoader),
            (24, LazyStub),
            (25, LazyStub),
            (26, LazyStub),
            (27, LazyStub),
        ]
    );
}

#[test]
fn hits_keep_the_line_number_and_the_original_text() {
    let line = "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm";
    let text = format!(
        "\nexport NVM_DIR=\"$HOME/.nvm\"\n{line}\n[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n"
    );
    let hits = scan_text(&text);
    assert_eq!(
        hits.iter()
            .map(|hit| (hit.line, hit.kind))
            .collect::<Vec<_>>(),
        [(2, NvmDirExport), (3, Loader), (4, Completion)]
    );
    assert_eq!(
        hits[1],
        Hit {
            line: 3,
            kind: Loader,
            text: line.to_string()
        }
    );
}

#[test]
fn crlf_line_endings_are_not_part_of_the_text() {
    let hits = scan_text("source ~/.nvm/nvm.sh\r\n");
    assert_eq!(hits[0].text, "source ~/.nvm/nvm.sh");
}

#[test]
fn homebrew_caveats_pasted_indented_stay_auto_migratable() {
    let text = "  export NVM_DIR=\"$HOME/.nvm\"\n\
  [ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ] && \\. \"/opt/homebrew/opt/nvm/nvm.sh\"  # This loads nvm\n\
  [ -s \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\"  # This loads nvm bash_completion\n";
    assert_eq!(
        found(text),
        [(1, NvmDirExport), (2, Loader), (3, Completion)]
    );
}

#[test]
fn a_loader_inside_a_function_or_an_if_body_is_a_lazy_loader() {
    let text = "nvm_load() {\n  [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"\n}\n\
if [ -s \"$NVM_DIR/nvm.sh\" ]; then\n  . \"$NVM_DIR/nvm.sh\"\nfi\n\
if true; then . \"$NVM_DIR/nvm.sh\"\nfi\n\
[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"\n";
    assert_eq!(
        found(text),
        [
            (2, LazyLoader),
            (5, LazyLoader),
            (7, LazyLoader),
            (9, Completion)
        ]
    );
}

#[test]
fn a_one_line_if_is_top_level() {
    let text = "if [ -s \"$NVM_DIR/nvm.sh\" ]; then . \"$NVM_DIR/nvm.sh\"; fi\n";
    assert_eq!(found(text), [(1, Loader)]);
}

#[test]
fn a_stub_or_an_unset_anywhere_in_the_file_makes_every_loader_lazy() {
    let stub_after = "source ~/.nvm/nvm.sh\nnpm() { nvm; command npm \"$@\" }\n";
    assert_eq!(found(stub_after), [(1, LazyLoader), (2, LazyStub)]);
    let unset_before = "unset -f nvm\n. ~/.nvm/nvm.sh\n. ~/.nvm/bash_completion\n";
    assert_eq!(
        found(unset_before),
        [(1, Unset), (2, LazyLoader), (3, LazyLoader)]
    );
}

#[test]
fn blog_patterns_a_b_and_c_are_lazy_loaders() {
    let pattern_a = "nvm() { unset -f nvm; . \"$NVM_DIR/nvm.sh\"; nvm \"$@\"; }\n";
    assert_eq!(
        found(pattern_a),
        [(1, LazyStub), (1, Unset), (1, LazyLoader)]
    );
    let pattern_b = "lazynvm() {\n  unset -f nvm node npm npx\n  export NVM_DIR=~/.nvm\n\
  [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"\n}\nnvm() { lazynvm; nvm $@; }\n\
node() { lazynvm; node $@; }\nnpm() { lazynvm; npm $@; }\nnpx() { lazynvm; npx $@; }\n";
    assert_eq!(
        found(pattern_b),
        [
            (2, Unset),
            (3, NvmDirExport),
            (4, LazyLoader),
            (6, LazyStub),
            (7, LazyStub),
            (8, LazyStub),
            (9, LazyStub),
        ]
    );
    let pattern_c = "lazy_load_nvm_cmds=(nvm node npm npx yarn)\n\
for cmd in \"${lazy_load_nvm_cmds[@]}\"; do\n\
  eval \"${cmd}() { unset -f ${lazy_load_nvm_cmds[*]}; source \\$NVM_DIR/nvm.sh; ${cmd} \\\"\\$@\\\"; }\"\n\
done\n";
    assert_eq!(found(pattern_c), [(3, Unset), (3, LazyLoader)]);
}

#[test]
fn blog_patterns_d_and_e() {
    let pattern_d = "alias nvm='unalias nvm node npm && . \"$NVM_DIR\"/nvm.sh && nvm'\n";
    assert_eq!(found(pattern_d), [(1, Loader)]);
    let pattern_e =
        "export PATH=\"$NVM_DIR/versions/node/$(cat $NVM_DIR/alias/default)/bin:$PATH\"\n";
    assert_eq!(found(pattern_e), []);
}

#[test]
fn the_word_nvm_in_a_multi_line_plugins_block() {
    let text =
        "plugins=(\n    git\n    node\n    nvm\n#    nvm\n    'nvm'\n    pnvm\n)\necho nvm\nnvm\n";
    assert_eq!(found(text), [(4, OmzPlugin), (6, OmzPlugin)]);
}

#[test]
fn one_line_plugins_blocks() {
    assert_eq!(found("plugins=(git nvm)\n"), [(1, OmzPlugin)]);
    assert_eq!(found("plugins+=(nvm)\n"), [(1, OmzPlugin)]);
    assert_eq!(found("plugins=(git zsh-nvm)\n"), [(1, ZshNvm)]);
    assert_eq!(found("plugins=(git) ; echo nvm\n"), []);
    assert_eq!(found("my_plugins=(nvm)\n"), []);
    assert_eq!(
        found("zstyle ':omz:plugins:nvm' lazy yes\n"),
        [(1, OmzPlugin)]
    );
}

#[test]
fn comments_migrated_lines_and_the_init_block_are_skipped() {
    let text = format!(
        "# [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"\n\
# [nvmrc-migrated] [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"\n\
foo # . ~/.nvm/nvm.sh\n\
echo dnvm.sh\n\
. ~/.dnx/dnvm/dnvm.sh\n\
{BEGIN_MARKER}\n\
source ~/.nvm/nvm.sh\n\
nvm() {{ command nvm \"$@\"; }}\n\
{END_MARKER}\n\
source ~/.nvm/nvm.sh\n"
    );
    assert_eq!(found(&text), [(10, Loader)]);
}

#[test]
fn an_empty_text_has_no_hits() {
    assert_eq!(found(""), []);
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find module `conflict`
in `domain`", "cannot find function `scan_text`" and "cannot find enum
`Kind`".

- [ ] **Step 4: Write the implementation**

Create `src/domain/conflict/expand.rs`:

```rust
//! Expanding a sourced path without a shell (digest section 6), in order:
//!
//! 1. a leading `~` or `~/` becomes `$HOME`;
//! 2. `$HOME` and `${HOME}`;
//! 3. `${ZDOTDIR:-$HOME}` and `${ZDOTDIR:-${HOME}}`: `ZDOTDIR` when it is set
//!    and not empty, else `HOME`;
//! 4. any other `$VAR` or `${VAR}` from the environment.
//!
//! Anything else is unresolvable, never guessed: a `$(...)` or `` `...` ``, a
//! `${...}` of another form (`${0:A:h}`, `${VAR:-x}`, `${#VAR}`), a special
//! parameter (`$0`, `$@`), an unset variable, `~user`, or a glob character
//! (`*`, `?`, `[`) in the result. A relative path stays relative (the caller
//! resolves it). The values from the environment are not expanded again.

/// The outcome of [`expand_path`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expansion {
    /// The path, every expansion done.
    Resolved(String),
    /// Why the path cannot be followed, such as `unset $FOO`.
    Unresolvable(String),
}

/// Lookup of an environment variable.
type Environment<'a> = &'a dyn Fn(&str) -> Option<String>;

/// `token` (a [`source_target`](super::source_target)) with `~` and the
/// variables of `environment` expanded (see the module documentation).
#[must_use]
pub fn expand_path(token: &str, environment: &dyn Fn(&str) -> Option<String>) -> Expansion {
    match expand(token, environment) {
        Ok(path) if path.contains(['*', '?', '[']) => {
            Expansion::Unresolvable(format!("glob {path}"))
        }
        Ok(path) => Expansion::Resolved(path),
        Err(reason) => Expansion::Unresolvable(reason),
    }
}

fn expand(token: &str, environment: Environment<'_>) -> Result<String, String> {
    let (mut path, mut rest) = expand_tilde(token, environment)?;
    while let Some(position) = rest.find(['$', '`']) {
        path.push_str(&rest[..position]);
        let expression = &rest[position..];
        if expression.starts_with('`') {
            return Err(format!("command substitution {expression}"));
        }
        let (value, length) = expand_dollar(expression, environment)?;
        path.push_str(&value);
        rest = &expression[length..];
    }
    path.push_str(rest);
    Ok(path)
}

/// The expanded leading `~` (or nothing) and the rest of `token`.
fn expand_tilde<'a>(
    token: &'a str,
    environment: Environment<'_>,
) -> Result<(String, &'a str), String> {
    let Some(after) = token.strip_prefix('~') else {
        return Ok((String::new(), token));
    };
    if after.is_empty() || after.starts_with('/') {
        Ok((lookup("HOME", environment)?, after))
    } else {
        Err(format!("unsupported {token}"))
    }
}

/// The value of the `$` expression at the start of `expression`, and its
/// length.
fn expand_dollar(
    expression: &str,
    environment: Environment<'_>,
) -> Result<(String, usize), String> {
    let after = &expression[1..];
    if after.starts_with('(') {
        let length = closing(expression, '(', ')').unwrap_or(expression.len());
        return Err(format!("command substitution {}", &expression[..length]));
    }
    if after.starts_with('{') {
        return expand_braced(expression, environment);
    }
    let name_length = identifier_length(after);
    if name_length == 0 {
        return Err(format!("unsupported {expression}"));
    }
    let value = lookup(&after[..name_length], environment)?;
    Ok((value, 1 + name_length))
}

/// A `${...}` at the start of `expression`.
fn expand_braced(
    expression: &str,
    environment: Environment<'_>,
) -> Result<(String, usize), String> {
    let length =
        closing(expression, '{', '}').ok_or_else(|| format!("unsupported {expression}"))?;
    let inner = &expression[2..length - 1];
    let value = match inner {
        "ZDOTDIR:-$HOME" | "ZDOTDIR:-${HOME}" => zdotdir_or_home(environment)?,
        name if identifier_length(name) == name.len() && !name.is_empty() => {
            lookup(name, environment)?
        }
        _ => return Err(format!("unsupported {}", &expression[..length])),
    };
    Ok((value, length))
}

fn zdotdir_or_home(environment: Environment<'_>) -> Result<String, String> {
    match environment("ZDOTDIR").filter(|value| !value.is_empty()) {
        Some(zdotdir) => Ok(zdotdir),
        None => lookup("HOME", environment),
    }
}

fn lookup(name: &str, environment: Environment<'_>) -> Result<String, String> {
    environment(name).ok_or_else(|| format!("unset ${name}"))
}

/// The length of the shell identifier at the start of `text` (0 when none).
fn identifier_length(text: &str) -> usize {
    let starts_identifier = text
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_');
    if !starts_identifier {
        return 0;
    }
    text.find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .unwrap_or(text.len())
}

/// The length of `$` plus the `open ... close` group after it, nesting
/// included; `None` when it never closes.
fn closing(expression: &str, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    for (index, character) in expression.char_indices().skip(1) {
        if character == open {
            depth += 1;
        } else if character == close {
            depth -= 1;
            if depth == 0 {
                return Some(index + 1);
            }
        }
    }
    None
}
```

Create `src/domain/conflict/expand_tests.rs`:

```rust
use super::expand::{Expansion, expand_path};

fn environment(name: &str) -> Option<String> {
    let value = match name {
        "HOME" => "/home/me",
        "ZSH" => "/home/me/.oh-my-zsh",
        "ZSH_CONFIG_SCRIPTS" => "/home/me/dotfiles/zsh/scripts",
        "NVM_DIR" => "/home/me/.nvm",
        "EMPTY" => "",
        _ => return None,
    };
    Some(value.to_string())
}

fn with_zdotdir(name: &str) -> Option<String> {
    match name {
        "ZDOTDIR" => Some("/home/me/.config/zsh".to_string()),
        _ => environment(name),
    }
}

fn without_home(name: &str) -> Option<String> {
    (name != "HOME").then(|| environment(name)).flatten()
}

fn resolved(path: &str) -> Expansion {
    Expansion::Resolved(path.to_string())
}

fn unresolvable(reason: &str) -> Expansion {
    Expansion::Unresolvable(reason.to_string())
}

#[test]
fn resolves_home_tilde_zdotdir_and_environment_variables() {
    let rows = [
        ("~", "/home/me"),
        ("~/.nvm/nvm.sh", "/home/me/.nvm/nvm.sh"),
        ("$HOME/.bun/_bun", "/home/me/.bun/_bun"),
        ("${HOME}/.bun/_bun", "/home/me/.bun/_bun"),
        ("${ZDOTDIR:-$HOME}/.zshenv", "/home/me/.zshenv"),
        ("${ZDOTDIR:-${HOME}}/.zshenv", "/home/me/.zshenv"),
        ("$ZSH/oh-my-zsh.sh", "/home/me/.oh-my-zsh/oh-my-zsh.sh"),
        (
            "${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh",
            "/home/me/dotfiles/zsh/scripts/lazy-functions.zsh",
        ),
        ("$NVM_DIR/nvm.sh", "/home/me/.nvm/nvm.sh"),
        ("/etc/profile", "/etc/profile"),
        ("relative/file.sh", "relative/file.sh"),
        ("$EMPTY/x", "/x"),
        ("a$HOME", "a/home/me"),
    ];
    for (token, expected) in rows {
        assert_eq!(
            expand_path(token, &environment),
            resolved(expected),
            "{token:?}"
        );
    }
}

#[test]
fn zdotdir_wins_over_home_when_set() {
    assert_eq!(
        expand_path("${ZDOTDIR:-$HOME}/.zshrc", &with_zdotdir),
        resolved("/home/me/.config/zsh/.zshrc")
    );
    assert_eq!(
        expand_path("${ZDOTDIR:-${HOME}}/.zshrc", &with_zdotdir),
        resolved("/home/me/.config/zsh/.zshrc")
    );
}

#[test]
fn anything_left_unexpanded_is_unresolvable_with_a_reason() {
    let rows = [
        (
            "$(brew --prefix nvm)/nvm.sh",
            "command substitution $(brew --prefix nvm)",
        ),
        (
            "`brew --prefix nvm`/nvm.sh",
            "command substitution `brew --prefix nvm`/nvm.sh",
        ),
        ("${0:A:h}/x.zsh", "unsupported ${0:A:h}"),
        ("$f", "unset $f"),
        ("$FOO/x", "unset $FOO"),
        ("${FOO}/x", "unset $FOO"),
        (
            "${NVM_DIR:-$HOME/.nvm}/nvm.sh",
            "unsupported ${NVM_DIR:-$HOME/.nvm}",
        ),
        ("${#HOME}", "unsupported ${#HOME}"),
        ("${list[@]}", "unsupported ${list[@]}"),
        ("$1/x", "unsupported $1/x"),
        ("$@", "unsupported $@"),
        ("x$", "unsupported $"),
        ("${HOME", "unsupported ${HOME"),
        ("~user/x", "unsupported ~user/x"),
        ("$HOME/conf.d/*.zsh", "glob /home/me/conf.d/*.zsh"),
        ("$HOME/file?.sh", "glob /home/me/file?.sh"),
        ("$HOME/[ab].sh", "glob /home/me/[ab].sh"),
    ];
    for (token, reason) in rows {
        assert_eq!(
            expand_path(token, &environment),
            unresolvable(reason),
            "{token:?}"
        );
    }
}

#[test]
fn home_must_be_set_to_expand_it() {
    assert_eq!(
        expand_path("~/x", &without_home),
        unresolvable("unset $HOME")
    );
    assert_eq!(
        expand_path("${ZDOTDIR:-$HOME}/.zshrc", &without_home),
        unresolvable("unset $HOME")
    );
}
```

Create `src/domain/conflict/kind.rs`:

```rust
//! What a rule found, how serious it is, and whether `migrate` may edit it.

use std::fmt;

/// How a finding weighs in `nvm doctor`'s verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Worth knowing, never a conflict (an `NVM_DIR` export, a `source` whose
    /// path cannot be followed).
    Info,
    /// Breaks once nvm.sh is gone (a call of an nvm.sh helper), not a conflict.
    Hint,
    /// nvm.sh (or something loading it) competes with nvmrc's `nvm`.
    Conflict,
}

/// The kind of a line the rules recognised (digest section 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A top-level `.`, `\.` or `source` of `nvm.sh`.
    Loader,
    /// A top-level source of nvm's `bash_completion`.
    Completion,
    /// A loader or completion inside a block, or in a file that defines lazy
    /// stubs: never edited automatically.
    LazyLoader,
    /// A function named like a node command (`nvm() {`, `function npm {`).
    LazyStub,
    /// `unset -f nvm`, `unfunction nvm`: a self-removing stub.
    Unset,
    /// `nvm` in oh-my-zsh's `plugins=( ... )`, or its `zstyle`.
    OmzPlugin,
    /// lukechilds/zsh-nvm.
    ZshNvm,
    /// fish's `bass source .../nvm.sh`.
    Bass,
    /// A call of an nvm.sh internal (`nvm_find_nvmrc`, `nvm_ls`, ...).
    HelperCall,
    /// `export NVM_DIR=...`: kept, nvmrc reads it.
    NvmDirExport,
    /// A `source` whose path holds something that cannot be expanded.
    NotFollowed,
}

impl Kind {
    /// How serious the finding is.
    #[must_use]
    pub const fn severity(self) -> Severity {
        match self {
            Self::NvmDirExport | Self::NotFollowed => Severity::Info,
            Self::HelperCall => Severity::Hint,
            _ => Severity::Conflict,
        }
    }

    /// Whether the finding counts as a conflict (`nvm doctor` exits 1).
    #[must_use]
    pub const fn is_conflict(self) -> bool {
        matches!(self.severity(), Severity::Conflict)
    }

    /// Whether `migrate` comments the line out on its own.
    #[must_use]
    pub const fn is_auto_migratable(self) -> bool {
        matches!(self, Self::Loader | Self::Completion)
    }

    /// The name shown in reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Loader => "nvm.sh loader",
            Self::Completion => "nvm bash_completion",
            Self::LazyLoader => "lazy loader",
            Self::LazyStub => "lazy stub",
            Self::Unset => "nvm unset",
            Self::OmzPlugin => "oh-my-zsh nvm plugin",
            Self::ZshNvm => "zsh-nvm",
            Self::Bass => "bass nvm.sh",
            Self::HelperCall => "nvm.sh helper call",
            Self::NvmDirExport => "NVM_DIR export",
            Self::NotFollowed => "source not followed",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}
```

Create `src/domain/conflict/kind_tests.rs`:

```rust
use super::kind::{Kind, Severity};

const ALL: [Kind; 11] = [
    Kind::Loader,
    Kind::Completion,
    Kind::LazyLoader,
    Kind::LazyStub,
    Kind::Unset,
    Kind::OmzPlugin,
    Kind::ZshNvm,
    Kind::Bass,
    Kind::HelperCall,
    Kind::NvmDirExport,
    Kind::NotFollowed,
];

#[test]
fn severity_is_info_for_exports_and_unfollowed_sources_hint_for_helpers() {
    for kind in ALL {
        let expected = match kind {
            Kind::NvmDirExport | Kind::NotFollowed => Severity::Info,
            Kind::HelperCall => Severity::Hint,
            _ => Severity::Conflict,
        };
        assert_eq!(kind.severity(), expected, "{kind:?}");
        assert_eq!(
            kind.is_conflict(),
            expected == Severity::Conflict,
            "{kind:?}"
        );
    }
}

#[test]
fn only_loaders_and_completions_are_auto_migratable() {
    let migratable: Vec<Kind> = ALL
        .into_iter()
        .filter(|kind| kind.is_auto_migratable())
        .collect();
    assert_eq!(migratable, [Kind::Loader, Kind::Completion]);
}

#[test]
fn every_kind_has_its_own_label_and_displays_it() {
    let labels: Vec<&str> = ALL.into_iter().map(Kind::label).collect();
    for (index, label) in labels.iter().enumerate() {
        assert!(!label.is_empty());
        assert!(!labels[..index].contains(label), "duplicate label {label}");
        assert_eq!(ALL[index].to_string(), *label);
    }
}

#[test]
fn labels_name_what_was_found() {
    assert_eq!(Kind::Loader.label(), "nvm.sh loader");
    assert_eq!(Kind::Completion.label(), "nvm bash_completion");
    assert_eq!(Kind::LazyLoader.label(), "lazy loader");
    assert_eq!(Kind::OmzPlugin.label(), "oh-my-zsh nvm plugin");
    assert_eq!(Kind::NvmDirExport.label(), "NVM_DIR export");
}
```

Create `src/domain/conflict/lexer.rs`:

```rust
//! A one-line shell lexer, just enough for the rules: where a comment starts,
//! the words and operators of a line, its indentation and word boundaries.
//!
//! Quotes are tracked within one line only (a string spanning lines, like
//! oh-my-zsh's `eval "..."`, is read line by line as unquoted text).

/// A word (its quotes removed) or one of the operators `;&|()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Token {
    /// `quoted` is true when any part of it was quoted or escaped, so it can
    /// never be a reserved word like `{` or `then`.
    Word {
        value: String,
        quoted: bool,
    },
    Operator(char),
}

const OPERATORS: [char; 5] = [';', '&', '|', '(', ')'];

/// Where the scan stands inside quotes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quote {
    None,
    Single,
    Double,
}

/// `line` without its comment: a `#` that starts a word outside quotes and
/// everything after it (`${#x}`, `$#` and `a#b` are not comments).
pub(super) fn strip_comment(line: &str) -> &str {
    let mut quote = Quote::None;
    let mut escaped = false;
    let mut previous = None;
    for (index, character) in line.char_indices() {
        let starts_word = previous
            .is_none_or(|before: char| before.is_whitespace() || OPERATORS.contains(&before));
        if character == '#' && quote == Quote::None && !escaped && starts_word {
            return &line[..index];
        }
        (quote, escaped) = step(quote, escaped, character);
        previous = Some(character);
    }
    line
}

/// The quote state after `character`; `escaped` is whether it is escaped.
fn step(quote: Quote, escaped: bool, character: char) -> (Quote, bool) {
    if escaped {
        return (quote, false);
    }
    match (quote, character) {
        (Quote::None, '\'') => (Quote::Single, false),
        (Quote::Single, '\'') | (Quote::Double, '"') => (Quote::None, false),
        (Quote::None, '"') => (Quote::Double, false),
        (Quote::None | Quote::Double, '\\') => (quote, true),
        _ => (quote, false),
    }
}

/// The words and operators of `code` (a line without its comment).
pub(super) fn tokens(code: &str) -> Vec<Token> {
    let mut lexer = Lexer::default();
    let mut characters = code.chars();
    while let Some(character) = characters.next() {
        lexer.feed(character, &mut characters);
    }
    lexer.end_word();
    lexer.tokens
}

#[derive(Debug, Default)]
struct Lexer {
    tokens: Vec<Token>,
    word: Option<(String, bool)>,
}

impl Lexer {
    fn feed(&mut self, character: char, rest: &mut std::str::Chars<'_>) {
        match character {
            blank if blank.is_whitespace() => self.end_word(),
            operator if OPERATORS.contains(&operator) => {
                self.end_word();
                self.tokens.push(Token::Operator(operator));
            }
            '\'' => self.quoted_until(rest, '\''),
            '"' => self.quoted_until(rest, '"'),
            '\\' => {
                if let Some(next) = rest.next() {
                    self.push(next, true);
                }
            }
            other => self.push(other, false),
        }
    }

    /// Reads a quoted part up to `close`; inside double quotes `\` escapes
    /// `"`, `\`, `$` and `` ` ``.
    fn quoted_until(&mut self, rest: &mut std::str::Chars<'_>, close: char) {
        self.word.get_or_insert_with(Default::default).1 = true;
        while let Some(character) = rest.next() {
            if character == close {
                return;
            }
            let escapable = |next: &char| matches!(next, '"' | '\\' | '$' | '`');
            let escaped = rest.clone().next().filter(escapable);
            match escaped {
                Some(next) if close == '"' && character == '\\' => {
                    rest.next();
                    self.push(next, true);
                }
                _ => self.push(character, true),
            }
        }
    }

    fn push(&mut self, character: char, quoted: bool) {
        let word = self.word.get_or_insert_with(Default::default);
        word.0.push(character);
        word.1 |= quoted;
    }

    fn end_word(&mut self) {
        if let Some((value, quoted)) = self.word.take() {
            self.tokens.push(Token::Word { value, quoted });
        }
    }
}

/// The number of blanks before the first other character of `line`.
pub(super) fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|character| character.is_whitespace())
        .count()
}

/// A letter, a digit or `_`, as a regex `\w`.
pub(super) fn is_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

/// Whether `text` holds `word` with a word boundary on both sides (regex
/// `\bword\b`, for a `word` that starts and ends with a word character).
pub(super) fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(start, _)| {
        let after = text[start + word.len()..].chars().next();
        boundary_before(text, start) && !after.is_some_and(is_word_character)
    })
}

/// Whether `text` holds `prefix` with a word boundary before it (regex
/// `\bprefix`).
pub(super) fn has_word_start(text: &str, prefix: &str) -> bool {
    text.match_indices(prefix)
        .any(|(start, _)| boundary_before(text, start))
}

/// Whether the character before byte `start` of `text` is not a word character.
pub(super) fn boundary_before(text: &str, start: usize) -> bool {
    !text[..start]
        .chars()
        .next_back()
        .is_some_and(is_word_character)
}
```

Create `src/domain/conflict/lexer_tests.rs`:

```rust
use super::lexer::{Token, has_word, has_word_start, indentation, strip_comment, tokens};

fn word(value: &str) -> Token {
    Token::Word {
        value: value.to_string(),
        quoted: false,
    }
}

fn quoted(value: &str) -> Token {
    Token::Word {
        value: value.to_string(),
        quoted: true,
    }
}

#[test]
fn strip_comment_cuts_a_hash_that_starts_a_word_outside_quotes() {
    let rows = [
        ("# [ -s \"$NVM_DIR/nvm.sh\" ]", ""),
        ("   # indented comment", "   "),
        ("foo # . ~/.nvm/nvm.sh", "foo "),
        ("a;# c", "a;"),
        ("echo \"a # b\"", "echo \"a # b\""),
        ("echo 'a # b'", "echo 'a # b'"),
        ("echo ${#array[@]} $#", "echo ${#array[@]} $#"),
        ("echo a#b", "echo a#b"),
        ("echo \\# not a comment", "echo \\# not a comment"),
        ("echo \"it\\\"s\" # c", "echo \"it\\\"s\" "),
        ("plain line", "plain line"),
    ];
    for (line, code) in rows {
        assert_eq!(strip_comment(line), code, "{line:?}");
    }
}

#[test]
fn tokens_split_words_and_operators_and_unquote() {
    assert_eq!(
        tokens("nvm() {"),
        [
            word("nvm"),
            Token::Operator('('),
            Token::Operator(')'),
            word("{")
        ]
    );
    assert_eq!(
        tokens("if [ -s \"$x\" ]; then"),
        [
            word("if"),
            word("["),
            word("-s"),
            quoted("$x"),
            word("]"),
            Token::Operator(';'),
            word("then"),
        ]
    );
    assert_eq!(tokens("  'nvm' \"node\""), [quoted("nvm"), quoted("node")]);
    assert_eq!(
        tokens("a&&b|c"),
        [
            word("a"),
            Token::Operator('&'),
            Token::Operator('&'),
            word("b"),
            Token::Operator('|'),
            word("c"),
        ]
    );
    assert_eq!(tokens("\\{ x"), [quoted("{"), word("x")]);
    assert_eq!(
        tokens("echo \"a \\\" b\""),
        [word("echo"), quoted("a \" b")]
    );
    assert_eq!(tokens(""), []);
}

#[test]
fn indentation_counts_leading_blanks() {
    assert_eq!(indentation("x"), 0);
    assert_eq!(indentation("  x"), 2);
    assert_eq!(indentation("\tx"), 1);
    assert_eq!(indentation(""), 0);
}

#[test]
fn has_word_needs_a_boundary_on_both_sides() {
    let rows = [
        ("unset -f nvm", "nvm", true),
        ("unset -f nvm node", "nvm", true),
        ("unset -f pnvm", "nvm", false),
        ("unset -f nvmx", "nvm", false),
        ("nvm_ls", "nvm", false),
        ("$NVM_DIR/nvm.sh", "nvm", true),
        ("antigen bundle zsh-nvm", "zsh-nvm", true),
        ("foo-zsh-nvm-bar", "zsh-nvm", true),
        ("zsh-nvmx", "zsh-nvm", false),
        ("nvm_ls_remote", "nvm_ls", false),
        ("", "nvm", false),
    ];
    for (text, needle, expected) in rows {
        assert_eq!(has_word(text, needle), expected, "{text:?} {needle:?}");
    }
}

#[test]
fn has_word_start_needs_a_boundary_before_only() {
    assert!(has_word_start(
        "export NVM_LAZY_LOAD=true",
        "NVM_LAZY_LOAD="
    ));
    assert!(!has_word_start("MY_NVM_LAZY_LOAD=1", "NVM_LAZY_LOAD="));
}
```

Insert above the `#[cfg(test)]` line of `src/domain/conflict/mod.rs`:

```rust
//! The lines of shell startup files that load nvm.sh, or would fight nvmrc's
//! `nvm` (plan 8; the rule table of the research digest, section 7, written
//! by hand: the crate has no regex engine).
//!
//! [`scan_text`] reads one file. It skips comments (a `#` that starts a word
//! outside quotes, so `foo # . nvm.sh` is no hit), the lines `migrate` already
//! commented out (`# [nvmrc-migrated] ...`) and nvmrc's own init block
//! ([`BEGIN_MARKER`] to [`END_MARKER`], both included). Every other line gets
//! one [`Hit`] per [`Kind`] it matches, in the rules' order.
//!
//! A loader or completion is reported as [`Kind::LazyLoader`] (manual fix)
//! instead when it is nested (see `nesting.rs`: indented under an unclosed
//! `{`, `then` or `do`, or on a line that opens or closes a block), or when
//! the file also defines a lazy stub or unsets `nvm`. [`source_target`] and
//! [`expand_path`] let the caller follow the files a line sources.

mod expand;
mod kind;
mod lexer;
mod nesting;
mod plugins;
mod rules;
mod source_path;

#[cfg(test)]
mod expand_tests;
#[cfg(test)]
mod kind_tests;
#[cfg(test)]
mod lexer_tests;
#[cfg(test)]
mod nesting_tests;
#[cfg(test)]
mod rules_tests;
#[cfg(test)]
mod source_path_tests;
```

Create `src/domain/conflict/nesting.rs`:

```rust
//! The nesting heuristic: is a line part of a block, so that commenting it out
//! could empty a body or unbalance the file?
//!
//! The openers are the unquoted words `{`, `then` and `do`; the closers are
//! `}`, `fi` and `done`; `elif` closes the `then` before it (its own `then`
//! reopens). Every opener remembers the indentation of its line. A line is
//! nested when
//!
//! - its indentation is deeper than the innermost open block's line (so an
//!   indented line with no opener above, such as Homebrew's caveats pasted
//!   with their two spaces, is top level, and so is a body written at the
//!   opener's own indentation), or
//! - the line itself opens a block it does not close, or closes one it did
//!   not open (`if x; then . nvm.sh` with the `fi` below).
//!
//! A block opened and closed on the same line (`if [ -s f ]; then . f; fi`,
//! `npm() { ...; }`) leaves the line top level: commenting the whole line out
//! keeps the file valid. `case` is not tracked.

use super::lexer::Token;

/// The indentations of the lines of the blocks still open, innermost last.
#[derive(Debug, Default)]
pub(super) struct BlockTracker {
    open: Vec<usize>,
}

/// What a structural word does to the nesting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    Open,
    Close,
}

impl BlockTracker {
    /// Reads the next line (its `tokens` and `indentation`); true when it is
    /// nested (see the module documentation).
    pub(super) fn observe(&mut self, tokens: &[Token], indentation: usize) -> bool {
        let inside = self.open.last().is_some_and(|opener| indentation > *opener);
        let mut depth: isize = 0;
        let mut lowest: isize = 0;
        for effect in tokens.iter().filter_map(effect) {
            match effect {
                Effect::Open => {
                    self.open.push(indentation);
                    depth += 1;
                }
                Effect::Close => {
                    self.open.pop();
                    depth -= 1;
                    lowest = lowest.min(depth);
                }
            }
        }
        inside || depth > 0 || lowest < 0
    }
}

/// The effect of `token` on the nesting, when it is a structural word.
fn effect(token: &Token) -> Option<Effect> {
    let Token::Word {
        value,
        quoted: false,
    } = token
    else {
        return None;
    };
    match value.as_str() {
        "{" | "then" | "do" => Some(Effect::Open),
        // `elif` closes the `then` before it; its own `then` reopens.
        "}" | "fi" | "done" | "elif" => Some(Effect::Close),
        _ => None,
    }
}
```

Create `src/domain/conflict/nesting_tests.rs`:

```rust
use super::lexer::{indentation, strip_comment, tokens};
use super::nesting::BlockTracker;

/// Whether each line of `text` sits inside a block or opens or closes one
/// (commenting it out could break the file), as the scanner asks.
fn nested_flags(text: &str) -> Vec<bool> {
    let mut tracker = BlockTracker::default();
    text.lines()
        .map(|line| tracker.observe(&tokens(strip_comment(line)), indentation(line)))
        .collect()
}

#[test]
fn install_sh_one_liners_are_top_level() {
    let text = "\nexport NVM_DIR=\"$HOME/.nvm\"\n\
[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
[[ -r $NVM_DIR/bash_completion ]] && \\. $NVM_DIR/bash_completion\n";
    assert_eq!(nested_flags(text), [false, false, false, false]);
}

#[test]
fn indentation_alone_does_not_nest() {
    let text =
        "  [ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ] && \\. \"/opt/homebrew/opt/nvm/nvm.sh\"\n";
    assert_eq!(nested_flags(text), [false]);
}

#[test]
fn a_function_and_its_body_are_nested_and_the_line_after_it_is_not() {
    let text = "nvm_load() {\n  . \"$NVM_DIR/nvm.sh\"\n}\n. \"$NVM_DIR/nvm.sh\"\n";
    assert_eq!(nested_flags(text), [true, true, true, false]);
}

#[test]
fn an_if_with_a_body_is_nested_through_else_and_elif() {
    let text = "if [ -s \"$NVM_DIR/nvm.sh\" ]; then\n  . a\nelif true; then\n  . b\nelse\n  . c\nfi\n. d\n";
    assert_eq!(
        nested_flags(text),
        [true, true, true, true, false, true, true, false]
    );
}

#[test]
fn a_one_line_if_or_function_is_top_level() {
    let text = "if [ -s x ]; then . \"$NVM_DIR/nvm.sh\"; fi\nnpm() { nvm; command npm \"$@\" }\n";
    assert_eq!(nested_flags(text), [false, false]);
}

#[test]
fn a_line_that_opens_or_closes_a_block_is_nested() {
    let text = "if true; then . \"$NVM_DIR/nvm.sh\"\nfi\n";
    assert_eq!(nested_flags(text), [true, true]);
}

#[test]
fn loops_and_brace_groups_nest() {
    let text =
        "for cmd in a b; do\n  eval \"x\"\ndone\n{\n  . y\n}\nwhile false\ndo\n  . z\ndone\n";
    assert_eq!(
        nested_flags(text),
        [true, true, true, true, true, true, false, true, true, true]
    );
}

#[test]
fn a_body_at_the_openers_indentation_is_not_nested() {
    let text = "if true; then\n. \"$NVM_DIR/nvm.sh\"\nfi\n";
    assert_eq!(nested_flags(text), [true, false, true]);
}

#[test]
fn nested_blocks_close_one_at_a_time() {
    let text = "f() {\n  if x; then\n    . a\n  fi\n  . b\n}\n. c\n";
    assert_eq!(
        nested_flags(text),
        [true, true, true, true, true, true, false]
    );
}

#[test]
fn braces_in_quotes_comments_and_expansions_are_not_blocks() {
    let text = "echo \"{\" '{' ${HOME} # {\n  . a\necho \\{\n  . b\n";
    assert_eq!(nested_flags(text), [false, false, false, false]);
}

#[test]
fn a_stray_closer_is_ignored() {
    let text = "}\n  . a\n";
    assert_eq!(nested_flags(text), [true, false]);
}
```

Create `src/domain/conflict/plugins.rs`:

```rust
//! oh-my-zsh's `plugins=( ... )` array, which may span many lines: the word
//! `nvm` inside it loads omz's nvm plugin (digest 3.1).

use super::lexer::Token;

/// Whether the scan is inside a `plugins=(` (or `plugins+=(`) array.
#[derive(Debug, Default)]
pub(super) struct PluginsBlock {
    open: bool,
}

impl PluginsBlock {
    /// Reads the next line's `tokens`; true when one of its array elements is
    /// exactly `nvm` (quoted or not; `zsh-nvm` and `pnvm` are not).
    pub(super) fn observe(&mut self, tokens: &[Token]) -> bool {
        let mut found = false;
        let mut previous = None;
        for token in tokens {
            if self.open {
                match token {
                    Token::Operator(')') => self.open = false,
                    Token::Word { value, .. } => found |= value == "nvm",
                    Token::Operator(_) => {}
                }
            } else {
                self.open = opens_array(previous, token);
            }
            previous = Some(token);
        }
        found
    }
}

/// `plugins=` (or `plugins+=`) followed by `(`.
fn opens_array(previous: Option<&Token>, token: &Token) -> bool {
    let assigns = matches!(
        previous,
        Some(Token::Word { value, quoted: false }) if value == "plugins=" || value == "plugins+="
    );
    assigns && *token == Token::Operator('(')
}
```

Create `src/domain/conflict/rules.rs`:

```rust
//! The per-line rules of digest section 7, hand-written (no regex engine),
//! one function per row of the table. They read a line without its comment
//! and know nothing of the lines around it (see the scanner in `mod.rs`).
//!
//! Deviations from the regexes, all stricter or documented:
//!
//! - Loader and Completion read the parsed source path ([`source_targets`]),
//!   so `"$NVM_DIR"/nvm.sh` is a loader and `nvm.sh.bak` is not; a bare
//!   `. nvm.sh` is a loader.
//! - A `bass source .../nvm.sh` line is Bass only, not also a Loader.
//! - `unset` and `unfunction` must start a word; the zstyle quote is optional.

use super::kind::Kind;
use super::lexer::{boundary_before, has_word, has_word_start, is_word_character};
use super::source_path::source_targets;

type Rule = fn(&str) -> bool;

/// Every rule, in the order the kinds of one line are reported.
const RULES: [(Kind, Rule); 9] = [
    (Kind::LazyStub, defines_stub),
    (Kind::Unset, unsets_nvm),
    (Kind::NvmDirExport, exports_nvm_dir),
    (Kind::Bass, bass_sources_nvm_sh),
    (Kind::Loader, sources_nvm_sh),
    (Kind::Completion, sources_completion),
    (Kind::OmzPlugin, configures_omz_plugin),
    (Kind::ZshNvm, mentions_zsh_nvm),
    (Kind::HelperCall, calls_helper),
];

/// The commands a lazy stub is named after.
const STUB_NAMES: [&str; 8] = [
    "nvm", "node", "npm", "npx", "yarn", "pnpm", "pnpx", "corepack",
];

/// The nvm.sh internals whose callers break once nvm.sh is gone.
const HELPERS: [&str; 7] = [
    "nvm_find_nvmrc",
    "nvm_find_up",
    "nvm_ls",
    "nvm_version",
    "nvm_has",
    "nvm_echo",
    "nvm_rc_version",
];

/// The kinds `code` (a line without its comment) matches, in table order.
pub(super) fn line_kinds(code: &str) -> Vec<Kind> {
    RULES
        .iter()
        .filter(|(_, rule)| rule(code))
        .map(|(kind, _)| *kind)
        .collect()
}

/// `name() {`, `name () {`, `function name {`, `function name() {`.
fn defines_stub(code: &str) -> bool {
    let line = code.trim_start();
    let (rest, keyword) = match after_word(line, "function") {
        Some(after) => (after, true),
        None => (line, false),
    };
    STUB_NAMES
        .iter()
        .find_map(|name| rest.strip_prefix(name))
        .and_then(|after| skip_parentheses(after.trim_start(), keyword))
        .is_some_and(|body| body.trim_start().starts_with('{'))
}

/// `text` after `()` (blanks allowed inside); the parentheses are optional
/// after the `function` keyword.
fn skip_parentheses(text: &str, keyword: bool) -> Option<&str> {
    match text.strip_prefix('(') {
        Some(inside) => inside.trim_start().strip_prefix(')'),
        None => keyword.then_some(text),
    }
}

/// `unset -f ... nvm ...` or `unfunction ... nvm ...`.
fn unsets_nvm(code: &str) -> bool {
    let unset =
        word_starts(code, "unset").filter_map(|rest| after_blanks(rest)?.strip_prefix("-f"));
    word_starts(code, "unfunction")
        .chain(unset)
        .any(|rest| !rest.starts_with(is_word_character) && has_word(rest, "nvm"))
}

/// `NVM_DIR=...` or `export NVM_DIR=...` at the start of the line.
fn exports_nvm_dir(code: &str) -> bool {
    let line = code.trim_start();
    after_word(line, "export")
        .unwrap_or(line)
        .starts_with("NVM_DIR=")
}

/// `bass source <path holding nvm.sh>`.
fn bass_sources_nvm_sh(code: &str) -> bool {
    word_starts(code, "bass")
        .filter_map(|rest| after_word(after_blanks(rest)?, "source"))
        .any(|arguments| {
            arguments
                .split_whitespace()
                .next()
                .is_some_and(|path| path.contains("nvm.sh"))
        })
}

fn sources_nvm_sh(code: &str) -> bool {
    !bass_sources_nvm_sh(code) && source_targets(code).iter().any(|path| is_nvm_sh(path))
}

/// A path whose file is `nvm.sh` (after `/`, after `}`, or alone).
fn is_nvm_sh(target: &str) -> bool {
    let path = target.trim_matches(['"', '\'']);
    path.strip_suffix("nvm.sh")
        .is_some_and(|head| head.is_empty() || head.ends_with(['/', '}']))
}

fn sources_completion(code: &str) -> bool {
    const ENDINGS: [&str; 4] = [
        "/bash_completion.d/nvm",
        "nvm/bash_completion",
        "NVM_DIR/bash_completion",
        "NVM_DIR}/bash_completion",
    ];
    source_targets(code).iter().any(|target| {
        let path = target.trim_matches(['"', '\'']);
        ENDINGS.iter().any(|ending| path.ends_with(ending))
    })
}

/// `zstyle ':omz:plugins:nvm' ...`.
fn configures_omz_plugin(code: &str) -> bool {
    word_starts(code, "zstyle")
        .filter_map(after_blanks)
        .any(|rest| {
            rest.strip_prefix(['\'', '"'])
                .unwrap_or(rest)
                .starts_with(":omz:plugins:nvm")
        })
}

/// `zsh-nvm` as a word (`lukechilds/zsh-nvm` included) or `NVM_LAZY_LOAD=`.
fn mentions_zsh_nvm(code: &str) -> bool {
    has_word(code, "zsh-nvm") || has_word_start(code, "NVM_LAZY_LOAD=")
}

fn calls_helper(code: &str) -> bool {
    HELPERS.iter().any(|helper| has_word(code, helper))
}

/// What follows each occurrence of `word` that starts a word in `code`.
fn word_starts<'a>(code: &'a str, word: &'a str) -> impl Iterator<Item = &'a str> {
    code.match_indices(word)
        .filter(|(start, _)| boundary_before(code, *start))
        .map(move |(start, _)| &code[start + word.len()..])
}

/// `text` after `word` and at least one blank, when it starts with them.
fn after_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    after_blanks(text.strip_prefix(word)?)
}

/// `text` without its leading blanks, when it has at least one.
fn after_blanks(text: &str) -> Option<&str> {
    text.starts_with(char::is_whitespace)
        .then(|| text.trim_start())
}
```

Create `src/domain/conflict/rules_tests.rs`:

```rust
//! The per-line rules of digest section 7, line by line, without the scanner's
//! state (plugins blocks, nesting, the file-level lazy rule).

use super::kind::Kind::{
    self, Bass, Completion, HelperCall, LazyStub, Loader, NvmDirExport, OmzPlugin, Unset, ZshNvm,
};
use super::lexer::strip_comment;
use super::rules::line_kinds;

fn assert_rows(rows: &[(&str, &[Kind])]) {
    for (line, expected) in rows {
        assert_eq!(line_kinds(strip_comment(line)), *expected, "{line:?}");
    }
}

/// `exp/corpus.sh`, line by line (digest section 7: 13 true positives, the
/// bass line now its own kind, and the rejects of lines 19 to 22).
#[test]
fn the_corpus_of_the_digest() {
    assert_rows(&[
        ("export NVM_DIR=\"$HOME/.nvm\"", &[NvmDirExport]),
        (
            "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm",
            &[Loader],
        ),
        (
            "[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion",
            &[Completion],
        ),
        (
            "[ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"  # This loads nvm",
            &[Loader],
        ),
        (
            "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\" --no-use # This loads nvm, without auto-using the default version",
            &[Loader],
        ),
        (
            "export NVM_DIR=\"$([ -z \"${XDG_CONFIG_HOME-}\" ] && printf %s \"${HOME}/.nvm\" || printf %s \"${XDG_CONFIG_HOME}/nvm\")\"",
            &[NvmDirExport],
        ),
        (
            "[[ -r $NVM_DIR/bash_completion ]] && \\. $NVM_DIR/bash_completion",
            &[Completion],
        ),
        (
            "  [ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ] && \\. \"/opt/homebrew/opt/nvm/nvm.sh\"  # This loads nvm",
            &[Loader],
        ),
        (
            "  [ -s \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\"  # This loads nvm bash_completion",
            &[Completion],
        ),
        ("source $(brew --prefix nvm)/nvm.sh", &[Loader]),
        (". \"$(brew --prefix nvm)/nvm.sh\"", &[Loader]),
        (
            "[ -s \"$HOMEBREW_PREFIX/opt/nvm/nvm.sh\" ] && \\. \"$HOMEBREW_PREFIX/opt/nvm/nvm.sh\"",
            &[Loader],
        ),
        (
            "  [ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"",
            &[Loader],
        ),
        (
            "  [ -s \"${nvm_prefix}/etc/bash_completion.d/nvm\" ] && \\. \"${nvm_prefix}/etc/bash_completion.d/nvm\"",
            &[Completion],
        ),
        ("source ~/.nvm/nvm.sh", &[Loader]),
        (
            "nvm() { unset -f nvm; . \"$NVM_DIR/nvm.sh\"; nvm \"$@\"; }",
            &[LazyStub, Unset, Loader],
        ),
        (
            "      [[ -f \\\"\\$NVM_DIR/nvm.sh\\\" ]] && source \\\"\\$NVM_DIR/nvm.sh\\\"",
            &[Loader],
        ),
        (
            "  bass source ~/.nvm/nvm.sh --no-use ';' nvm $argv",
            &[Bass],
        ),
        (
            "# [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"",
            &[],
        ),
        (
            "# [nvmrc-migrated] [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"",
            &[],
        ),
        ("echo dnvm.sh", &[]),
        (". ~/.dnx/dnvm/dnvm.sh", &[]),
        ("source \"$HOME/.nvm/nvm.sh\"", &[Loader]),
    ]);
}

/// install.sh's older forms (1.3), the README (1.4) and Homebrew (2).
#[test]
fn installer_readme_and_homebrew_forms() {
    assert_rows(&[
        ("export NVM_DIR=\"/home/u/.nvm\"", &[NvmDirExport]),
        (
            "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\" # This loads nvm",
            &[Loader],
        ),
        (
            "export NVM_DIR=\"$HOME/.nvm\" && ( git clone https://github.com/nvm-sh/nvm.git \"$NVM_DIR\" ) && \\. \"$NVM_DIR/nvm.sh\"",
            &[NvmDirExport, Loader],
        ),
        ("\\. \"/usr/local/opt/nvm/nvm.sh\"", &[Loader]),
        (
            "\\. \"/home/linuxbrew/.linuxbrew/opt/nvm/nvm.sh\"",
            &[Loader],
        ),
        ("source ${HOMEBREW_PREFIX}/opt/nvm/nvm.sh", &[Loader]),
        (
            "[ -s \"$(brew --prefix)/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"$(brew --prefix)/opt/nvm/etc/bash_completion.d/nvm\"",
            &[Completion],
        ),
        ("source ~/.nvm/bash_completion", &[Completion]),
        ("source ${NVM_DIR}/bash_completion", &[Completion]),
        (
            "    nvm_path=\"$(nvm_find_up .nvmrc | command tr -d '\\n')\"",
            &[HelperCall],
        ),
        ("  local nvmrc_path=\"$(nvm_find_nvmrc)\"", &[HelperCall]),
        ("alias cd='cdnvm'", &[]),
        ("add-zsh-hook chpwd load-nvmrc", &[]),
        ("ZSH_VERSION= source \"$_nvm_completion\"", &[]),
    ]);
}

/// oh-my-zsh (3.1), zsh-nvm (3.2) and the blog patterns A to E (3.3).
#[test]
fn lazy_loaders_and_other_managers() {
    assert_rows(&[
        ("which nvm &>/dev/null && return", &[]),
        ("zstyle ':omz:plugins:nvm' lazy yes", &[OmzPlugin]),
        ("zstyle \":omz:plugins:nvm\" autoload yes", &[OmzPlugin]),
        ("    function $nvm_lazy_cmd {", &[]),
        ("          unfunction \\$func", &[]),
        ("antigen bundle lukechilds/zsh-nvm", &[ZshNvm]),
        ("zplug \"lukechilds/zsh-nvm\"", &[ZshNvm]),
        ("zinit light lukechilds/zsh-nvm", &[ZshNvm]),
        ("export NVM_LAZY_LOAD=true", &[ZshNvm]),
        ("    eval \"$cmd(){", &[]),
        ("      unset -f $cmds > /dev/null 2>&1", &[]),
        ("lazynvm() {", &[]),
        ("  unset -f nvm node npm npx", &[Unset]),
        ("  export NVM_DIR=~/.nvm", &[NvmDirExport]),
        (
            "  [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"",
            &[Loader],
        ),
        ("nvm() { lazynvm; nvm $@; }", &[LazyStub]),
        ("node() { lazynvm; node $@; }", &[LazyStub]),
        ("lazy_load_nvm_cmds=(nvm node npm npx yarn)", &[]),
        ("for cmd in \"${lazy_load_nvm_cmds[@]}\"; do", &[]),
        (
            "  eval \"${cmd}() { unset -f ${lazy_load_nvm_cmds[*]}; source \\$NVM_DIR/nvm.sh; ${cmd} \\\"\\$@\\\"; }\"",
            &[Unset, Loader],
        ),
        (
            "alias nvm='unalias nvm node npm && . \"$NVM_DIR\"/nvm.sh && nvm'",
            &[Loader],
        ),
        (
            "export PATH=\"$NVM_DIR/versions/node/$(cat $NVM_DIR/alias/default)/bin:$PATH\"",
            &[],
        ),
    ]);
}

/// The user's own dotfiles (3.4).
#[test]
fn the_users_dotfiles() {
    assert_rows(&[
        ("export NVM_DIR=\"${HOME}/.nvm\"", &[NvmDirExport]),
        (
            "# Node (nvm itself is lazy-loaded via scripts/lazy-functions.zsh)",
            &[],
        ),
        (
            "[[ -r \"${ZDOTDIR:-$HOME}/.zshenv\" ]] && source \"${ZDOTDIR:-$HOME}/.zshenv\"",
            &[],
        ),
        ("  . \"$HOME/.swiftly/env.sh\"", &[]),
        ("plugins=(", &[]),
        ("    nvm", &[]),
        ("source $ZSH/oh-my-zsh.sh", &[]),
        ("source ${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh", &[]),
        ("[[ ! -d \"${NVM_DIR}\" ]] && mkdir \"${NVM_DIR}\"", &[]),
        ("nvm() {", &[LazyStub]),
        (
            "  unfunction nvm node npm npx yarn pnpm 2>/dev/null",
            &[Unset],
        ),
        ("  export NVM_DIR=\"${HOME}/.nvm\"", &[NvmDirExport]),
        (
            "  local nvm_prefix=\"${HOMEBREW_PREFIX:-/opt/homebrew}/opt/nvm\"",
            &[],
        ),
        ("  nvm \"$@\"", &[]),
        (
            "# node() { nvm >/dev/null 2>&1; command node \"$@\" } # TODO: Disabled to install a real node",
            &[],
        ),
        (
            "npm() { nvm >/dev/null 2>&1; command npm \"$@\" }",
            &[LazyStub],
        ),
        (
            "npx() { nvm >/dev/null 2>&1; command npx \"$@\" }",
            &[LazyStub],
        ),
        (
            "yarn() { nvm >/dev/null 2>&1; command yarn \"$@\" }",
            &[LazyStub],
        ),
        (
            "pnpm() { nvm >/dev/null 2>&1; command pnpm \"$@\" }",
            &[LazyStub],
        ),
    ]);
}

#[test]
fn stub_unset_and_helper_forms_and_their_rejects() {
    assert_rows(&[
        ("function nvm {", &[LazyStub]),
        ("function nvm() {", &[LazyStub]),
        ("function yarn{", &[LazyStub]),
        ("node () {", &[LazyStub]),
        ("corepack(){", &[LazyStub]),
        ("  pnpx ( ) {", &[LazyStub]),
        ("pnvm() {", &[]),
        ("nvmx() {", &[]),
        ("nvm()", &[]),
        ("unfunction nvm", &[Unset]),
        ("unset -f node nvm", &[Unset]),
        ("unset nvm", &[]),
        ("unset -f pnvm", &[]),
        ("myunset -f nvm", &[]),
        ("if nvm_has node; then", &[HelperCall]),
        ("nvm_ls_remote", &[]),
        (
            "echo $(nvm_version current) $(nvm_rc_version) $(nvm_echo x) $(nvm_ls)",
            &[HelperCall],
        ),
        ("source ~/.nvm/nvm.sh.bak", &[]),
        ("./nvm.sh", &[]),
        ("defer_source ~/.nvm/nvm.sh", &[]),
        ("cd ~/.nvm && . nvm.sh", &[Loader]),
        ("(source ~/.nvm/nvm.sh)", &[Loader]),
        ("true;source ~/.nvm/nvm.sh", &[Loader]),
        ("NVM_DIR=/opt/nvm", &[NvmDirExport]),
        ("MY_NVM_DIR=/opt/nvm", &[]),
    ]);
}
```

Create `src/domain/conflict/source_path.rs`:

```rust
//! The path a line sources (digest section 6): the token after the command
//! word `source`, `.` or `\.`, which starts the line or follows a blank or one
//! of `;&|{(`, and is followed by a blank.
//!
//! The token is a `"..."`, `'...'` or unquoted run up to a blank or one of
//! `;&|)`, its parts concatenated with the quotes removed (`"$NVM_DIR"/nvm.sh`
//! is `$NVM_DIR/nvm.sh`); `$(...)`, `${...}` and `` `...` `` are kept whole,
//! blanks included. A command word inside a quoted string counts too (an
//! `eval "... source \$NVM_DIR/nvm.sh ..."`), as the digest's rule table does.

use std::iter::Peekable;
use std::str::Chars;

use super::lexer::strip_comment;

/// The command words that read a file into the shell.
const COMMANDS: [&str; 3] = ["source", "\\.", "."];
/// What may stand right before a command word.
const COMMAND_PREFIXES: [char; 5] = [';', '&', '|', '{', '('];
/// What ends an unquoted token besides a blank.
const TOKEN_ENDS: [char; 4] = [';', '&', '|', ')'];

type Characters<'a> = Peekable<Chars<'a>>;

/// The path the first `source`, `.` or `\.` of `line` reads, quotes removed;
/// `None` when the line sources nothing outside its comment.
#[must_use]
pub fn source_target(line: &str) -> Option<String> {
    source_targets(strip_comment(line)).into_iter().next()
}

/// Every path `code` (a line without its comment) sources, in order.
pub(super) fn source_targets(code: &str) -> Vec<String> {
    code.char_indices()
        .filter(|(start, _)| starts_command(code, *start))
        .filter_map(|(start, _)| command_end(code, start))
        .filter_map(|end| read_token(&code[end..]))
        .collect()
}

/// Whether a command word may start at byte `start` of `code`.
fn starts_command(code: &str, start: usize) -> bool {
    code[..start]
        .chars()
        .next_back()
        .is_none_or(|before| before.is_whitespace() || COMMAND_PREFIXES.contains(&before))
}

/// The end of the command word starting at `start`, when one does and a
/// blank follows it.
fn command_end(code: &str, start: usize) -> Option<usize> {
    let rest = &code[start..];
    COMMANDS.iter().find_map(|command| {
        let after = rest.strip_prefix(command)?;
        after
            .starts_with(char::is_whitespace)
            .then_some(start + command.len())
    })
}

/// The token at the start of `text` (after its blanks), quotes removed.
fn read_token(text: &str) -> Option<String> {
    let mut characters = text.trim_start().chars().peekable();
    let mut token = String::new();
    let mut consumed = false;
    while let Some(character) = characters.next_if(|next| !ends_token(*next)) {
        consumed = true;
        match character {
            '\'' => copy_until(&mut characters, &mut token, '\''),
            '"' => read_double_quoted(&mut characters, &mut token),
            '\\' => token.extend(characters.next()),
            '$' if matches!(characters.peek(), Some('(' | '{')) => {
                copy_expansion(&mut characters, &mut token);
            }
            '`' => {
                token.push('`');
                copy_until(&mut characters, &mut token, '`');
                token.push('`');
            }
            other => token.push(other),
        }
    }
    consumed.then_some(token)
}

fn ends_token(character: char) -> bool {
    character.is_whitespace() || TOKEN_ENDS.contains(&character)
}

/// Copies up to `close`, which is consumed but not copied.
fn copy_until(characters: &mut Characters<'_>, token: &mut String, close: char) {
    token.extend(
        characters
            .by_ref()
            .take_while(|character| *character != close),
    );
}

/// The rest of a `"..."`: `\` escapes `"`, `\`, `$` and `` ` ``.
fn read_double_quoted(characters: &mut Characters<'_>, token: &mut String) {
    while let Some(character) = characters.next() {
        match character {
            '"' => return,
            '\\' => match characters.next_if(|next| matches!(next, '"' | '\\' | '$' | '`')) {
                Some(escaped) => token.push(escaped),
                None => token.push('\\'),
            },
            '$' if matches!(characters.peek(), Some('(' | '{')) => {
                copy_expansion(characters, token);
            }
            other => token.push(other),
        }
    }
}

/// Copies a `$(...)` or `${...}` whole (the `$` already read), nesting
/// included.
fn copy_expansion(characters: &mut Characters<'_>, token: &mut String) {
    token.push('$');
    let Some(open) = characters.next() else {
        return;
    };
    let close = if open == '(' { ')' } else { '}' };
    token.push(open);
    let mut depth = 1;
    for character in characters.by_ref() {
        token.push(character);
        if character == open {
            depth += 1;
        } else if character == close {
            depth -= 1;
            if depth == 0 {
                return;
            }
        }
    }
}
```

Create `src/domain/conflict/source_path_tests.rs`:

```rust
use super::source_path::{source_target, source_targets};

/// Digest section 6: the real syntaxes, and the token after `source`, `.` or
/// `\.` with its quotes removed.
#[test]
fn source_target_reads_the_path_token() {
    let rows = [
        ("source $ZSH/oh-my-zsh.sh", "$ZSH/oh-my-zsh.sh"),
        (
            "source ${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh",
            "${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh",
        ),
        (
            "[[ -r \"${ZDOTDIR:-$HOME}/.zshenv\" ]] && source \"${ZDOTDIR:-$HOME}/.zshenv\"",
            "${ZDOTDIR:-$HOME}/.zshenv",
        ),
        ("  . \"$HOME/.swiftly/env.sh\"", "$HOME/.swiftly/env.sh"),
        (
            "[ -s \"$HOME/.bun/_bun\" ] && source \"$HOME/.bun/_bun\"",
            "$HOME/.bun/_bun",
        ),
        (
            "[ -f \"${HOME}/.openclaw/completions/openclaw.zsh\" ] && source \"${HOME}/.openclaw/completions/openclaw.zsh\"",
            "${HOME}/.openclaw/completions/openclaw.zsh",
        ),
        (
            "[[ -r $NVM_DIR/bash_completion ]] && \\. $NVM_DIR/bash_completion",
            "$NVM_DIR/bash_completion",
        ),
        ("source ~/.nvm/nvm.sh", "~/.nvm/nvm.sh"),
        (
            "source ${0:A:h}/zsh-syntax-highlighting.zsh",
            "${0:A:h}/zsh-syntax-highlighting.zsh",
        ),
        (
            "for f in \"${_deferred_sources[@]}\"; do source \"$f\"; done",
            "$f",
        ),
        (
            "source $(brew --prefix nvm)/nvm.sh",
            "$(brew --prefix nvm)/nvm.sh",
        ),
        (
            ". \"$(brew --prefix nvm)/nvm.sh\"",
            "$(brew --prefix nvm)/nvm.sh",
        ),
        ("[ -f p ] && . p", "p"),
        ("[[ -r p ]] && source p", "p"),
        ("source 'a b'/c", "a b/c"),
        ("source x;echo y", "x"),
        ("(source x)", "x"),
        ("source x&", "x"),
        ("source x|cat", "x"),
        ("\tsource x", "x"),
        ("source \"$NVM_DIR\"/nvm.sh", "$NVM_DIR/nvm.sh"),
        ("source a\\ b", "a b"),
        (
            "source `brew --prefix nvm`/nvm.sh",
            "`brew --prefix nvm`/nvm.sh",
        ),
        ("source x # . y", "x"),
    ];
    for (line, expected) in rows {
        assert_eq!(source_target(line).as_deref(), Some(expected), "{line:?}");
    }
}

#[test]
fn source_target_rejects_comments_and_non_source_lines() {
    let rows = [
        "# source ~/.nvm/nvm.sh",
        "  # . ~/.nvm/nvm.sh",
        "echo hi # source x",
        "defer_source file.zsh",
        "defer_eval 'eval \"$(direnv hook zsh)\"'",
        "./script.sh",
        "../script.sh",
        "source",
        "source   ",
        ". ;",
        "echo dnvm.sh",
        "resource x",
        "",
    ];
    for line in rows {
        assert_eq!(source_target(line), None, "{line:?}");
    }
}

#[test]
fn source_targets_lists_every_sourced_path_of_a_line() {
    assert_eq!(
        source_targets(". a; source \"b c\" && \\. d"),
        ["a", "b c", "d"]
    );
    assert_eq!(
        source_targets(
            "      [[ -f \\\"\\$NVM_DIR/nvm.sh\\\" ]] && source \\\"\\$NVM_DIR/nvm.sh\\\""
        ),
        ["\"$NVM_DIR/nvm.sh\""]
    );
    assert!(source_targets("echo nothing").is_empty());
}
```

Apply to `src/shell/init/mod.rs` (above the test module):

```diff
--- a/src/shell/init/mod.rs
+++ b/src/shell/init/mod.rs
@@ -30,10 +30,9 @@
 
 use crate::error::CliError;
 
-/// The first line of the snippet.
-pub const BEGIN_MARKER: &str = "# >>> nvmrc init >>>";
-/// The last line of the snippet.
-pub const END_MARKER: &str = "# <<< nvmrc init <<<";
+/// The first and last lines of the snippet (defined with the conflict rules,
+/// which skip the block).
+pub use crate::domain::conflict::{BEGIN_MARKER, END_MARKER};
 /// How `nvm init` is called.
 pub const USAGE: &str = "Usage: nvm init <shell> [--no-use] [--install]";
 
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1213 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(conflict): the rules that find nvm.sh loaders, completions and lazy loaders"
```

---

### Task 3: The scanner

**Files:**

- Create: `src/commands/conflict/fixtures.rs`, `src/commands/conflict/mod.rs`,
  `src/commands/conflict/report_tests.rs`, `src/commands/conflict/roots.rs`,
  `src/commands/conflict/roots_tests.rs`, `src/commands/conflict/walk.rs`,
  `src/commands/conflict/walk_tests.rs`
- Modify: `src/commands/mod.rs`, `src/domain/conflict/kind.rs`,
  `src/domain/conflict/kind_tests.rs`, `src/shell/init/mod.rs`

**Interfaces:**

- Produces: `commands::conflict::{roots, scan, MAX_DEPTH, FileFindings, Report,
  Note, NoteReason}` with `Report::{conflicts, has_conflicts, auto_migratable}`;
  `shell::init::SHELLS` becomes public; `Kind::OmzPlugin` is a hint
  (`Severity::Hint`: reported with the manual fix, not counted as a conflict:
  the plugin returns at once while the nvmrc binary is on `PATH`).
- Behaviour (digest sections 5 and 6): `roots(context, shell)` lists the files
  that exist, deduplicated: bash `.bashrc .bash_profile .bash_login .profile`;
  zsh `.zshenv` then (with a `ZDOTDIR`) `$ZDOTDIR/{.zshenv,.zprofile,.zshrc,
  .zlogin}`; sh and dash `.profile` and `$ENV`; ksh `.profile` and `$ENV`
  (default `~/.kshrc`); fish `conf.d/*.fish` sorted, then `config.fish`; no
  shell means every shell. `scan` is breadth first: the roots are depth 0, the
  files they `source` depth 1 and 2 (a root another root sources stays depth 0),
  deduplicated by canonical path, the first name wins; unresolvable paths,
  relative paths (resolved from the sourcing file's directory), missing and
  unreadable files and files deeper than two levels are `Note`s, never errors.
  `auto_migratable()` is only a `Loader` or `Completion` in a depth 0 file.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -3,6 +3,7 @@
 pub mod auto;
 pub mod cache;
 pub mod color_policy;
+pub mod conflict;
 pub mod current;
 pub mod deactivate;
 pub mod exec;
```

- [ ] **Step 2: Write the failing tests**

Create `src/commands/conflict/fixtures.rs`:

```rust
//! Startup files for the scanner's tests.

use std::path::{Path, PathBuf};

use super::{Report, scan};
use crate::context::Context;
use crate::domain::conflict::Kind;
use crate::fakes::{FakeEnv, FakeFileSystem};
use crate::ports::FileSystem;

pub const HOME: &str = "/Users/u";

/// The user's zsh setup of digest section 3.4: stow links in `$HOME` to
/// `dotfiles/zsh`, scripts sourced through `$ZSH_CONFIG_SCRIPTS`.
pub fn users_dotfiles() -> FakeFileSystem {
    let fs = users_scripts(users_profiles());
    for name in [".zshenv", ".zprofile", ".zshrc"] {
        link(
            &fs,
            &format!("dotfiles/zsh/{name}"),
            &format!("{HOME}/{name}"),
        );
    }
    fs
}

/// The files of `dotfiles/zsh` that stow links into `$HOME`.
fn users_profiles() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file(
            "/Users/u/dotfiles/zsh/.zshenv",
            "export DOTFILES_HOME=\"${HOME}/dotfiles\"\n\
export ZSH_CONFIG_SCRIPTS=\"${DOTFILES_HOME}/zsh/scripts\"\n\
export NVM_DIR=\"${HOME}/.nvm\"\n",
        )
        .with_file(
            "/Users/u/dotfiles/zsh/.zprofile",
            &lines(&[
                "[[ -r \"${ZDOTDIR:-$HOME}/.zshenv\" ]] && source \"${ZDOTDIR:-$HOME}/.zshenv\"",
                "if [ -f \"$HOME/.swiftly/env.sh\" ]; then",
                "  . \"$HOME/.swiftly/env.sh\"",
                "fi",
            ]),
        )
        .with_file(
            "/Users/u/dotfiles/zsh/.zshrc",
            "plugins=(\n    git\n    nvm\n)\n\
source $ZSH/oh-my-zsh.sh\n\
source ${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh\n\
source ${ZSH_CONFIG_SCRIPTS}/environment.zsh\n",
        )
}

/// The files `.zprofile` and `.zshrc` source.
fn users_scripts(fs: FakeFileSystem) -> FakeFileSystem {
    fs.with_file("/Users/u/.swiftly/env.sh", "export SWIFTLY_HOME=x\n")
        .with_file(
            "/Users/u/.oh-my-zsh/oh-my-zsh.sh",
            "for plugin ($plugins); do\n  source \"$ZSH/plugins/$plugin/$plugin.plugin.zsh\"\ndone\n",
        )
        .with_file(
            "/Users/u/dotfiles/zsh/scripts/lazy-functions.zsh",
            &lines(&[
                "nvm() {",
                "  unfunction nvm node npm npx yarn pnpm 2>/dev/null",
                "  [ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"",
                "  nvm \"$@\"",
                "}",
                "npm() { nvm >/dev/null 2>&1; command npm \"$@\" }",
            ]),
        )
        .with_file(
            "/Users/u/dotfiles/zsh/scripts/environment.zsh",
            "export EDITOR=vim\n",
        )
}

/// The environment the user's `.zshenv` exports.
pub fn users_env() -> FakeEnv {
    FakeEnv::default()
        .with_var("HOME", HOME)
        .with_var("ZSH", "/Users/u/.oh-my-zsh")
        .with_var("ZSH_CONFIG_SCRIPTS", "/Users/u/dotfiles/zsh/scripts")
}

/// `text` as a file: every line newline-terminated (indentation kept).
fn lines(text: &[&str]) -> String {
    text.iter().map(|line| format!("{line}\n")).collect()
}

pub fn home_env() -> FakeEnv {
    FakeEnv::default().with_var("HOME", HOME)
}

pub fn link(fs: &FakeFileSystem, target: &str, link: &str) {
    let made = fs.symlink(Path::new(target), Path::new(link));
    assert!(made.is_ok(), "symlink {link}");
}

pub fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

pub fn scan_with(fs: &FakeFileSystem, env: &FakeEnv, roots: &[&str]) -> Report {
    scan(&Context::new(fs, env), &paths(roots))
}

/// `(path, depth)` of every scanned file.
pub fn scanned(report: &Report) -> Vec<(String, usize)> {
    report
        .files
        .iter()
        .map(|file| (file.path.display().to_string(), file.depth))
        .collect()
}

/// `(line, kind)` of the hits of the file named `path`.
pub fn hits_of(report: &Report, path: &str) -> Vec<(usize, Kind)> {
    let file = report
        .files
        .iter()
        .find(|file| file.path == Path::new(path));
    let file = file.unwrap_or_else(|| panic!("{path} was not scanned"));
    file.hits.iter().map(|hit| (hit.line, hit.kind)).collect()
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function `scan`",
"cannot find function `roots`" and "cannot find struct `Report`".

- [ ] **Step 4: Write the implementation**

Create `src/commands/conflict/mod.rs`:

```rust
//! Finding the lines of the startup files that load nvm.sh or fight nvmrc's
//! `nvm` (plan 8, digest sections 5 and 6): [`roots`] names the startup files
//! of a shell, [`scan`] reads them with the rules of
//! [`crate::domain::conflict`] and follows the files they source, two levels
//! deep, into a [`Report`] that `nvm doctor` prints and `nvm migrate` edits.

mod roots;
mod walk;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod report_tests;
#[cfg(test)]
mod roots_tests;
#[cfg(test)]
mod walk_tests;

use std::path::PathBuf;

use crate::domain::conflict::Hit;

pub use roots::roots;
pub use walk::{MAX_DEPTH, scan};

/// One scanned file and what the rules found in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFindings {
    /// The path as named: the root, or the expanded path of the `source`
    /// line that reached it (a symbolic link stays a link).
    pub path: PathBuf,
    /// The file [`Self::path`] resolves to, links followed.
    pub canonical: PathBuf,
    /// 0 for a root, 1 for a file a root sources, 2 for the next level.
    pub depth: usize,
    /// Every hit, in line order (none for a clean file).
    pub hits: Vec<Hit>,
}

/// Why a [`Note`] was written: all of them are Info, never a conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteReason {
    /// The sourced path cannot be expanded without a shell; the text says
    /// why (`unset $FOO`, `command substitution $(brew --prefix nvm)`).
    Unresolvable(String),
    /// The sourced path is relative: it was resolved from the directory of
    /// the sourcing file (the shell resolves it from `$PWD`) to this path.
    Relative(PathBuf),
    /// The sourced path, expanded, names no file.
    Missing(PathBuf),
    /// The file exists but cannot be read as text; the I/O error.
    Unreadable(String),
    /// The sourced file is deeper than [`MAX_DEPTH`]: not scanned.
    TooDeep(PathBuf),
}

/// Something the scan could not do, or did on a guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// The file the note is about (the sourcing file for a `source` line).
    pub path: PathBuf,
    /// The `source` line's number, from 1; `None` for the whole file.
    pub line: Option<usize>,
    /// The `source` line as written; empty for the whole file.
    pub text: String,
    pub reason: NoteReason,
}

/// Everything [`scan`] found: the files in scan order, then the notes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub files: Vec<FileFindings>,
    pub notes: Vec<Note>,
}

impl Report {
    /// Every hit that is a conflict (not Info, not Hint), with its file.
    pub fn conflicts(&self) -> impl Iterator<Item = (&FileFindings, &Hit)> {
        self.hits().filter(|(_, hit)| hit.kind.is_conflict())
    }

    /// Whether `nvm doctor` should fail: any hit is a conflict.
    #[must_use]
    pub fn has_conflicts(&self) -> bool {
        self.conflicts().next().is_some()
    }

    /// The hits `migrate` comments out on its own: top-level loaders and
    /// completions of a root file (depth 0). One in a sourced file is still
    /// a conflict, fixed by hand.
    pub fn auto_migratable(&self) -> impl Iterator<Item = (&FileFindings, &Hit)> {
        self.hits()
            .filter(|(file, hit)| file.depth == 0 && hit.kind.is_auto_migratable())
    }

    fn hits(&self) -> impl Iterator<Item = (&FileFindings, &Hit)> {
        self.files
            .iter()
            .flat_map(|file| file.hits.iter().map(move |hit| (file, hit)))
    }
}
```

Create `src/commands/conflict/report_tests.rs`:

```rust
use std::path::PathBuf;

use super::{FileFindings, Report};
use crate::domain::conflict::Hit;
use crate::domain::conflict::Kind::{
    self, Completion, HelperCall, LazyStub, Loader, NotFollowed, NvmDirExport, OmzPlugin,
};

fn file(path: &str, depth: usize, kinds: &[Kind]) -> FileFindings {
    let hits = kinds
        .iter()
        .enumerate()
        .map(|(index, kind)| Hit {
            line: index + 1,
            kind: *kind,
            text: format!("line {}", index + 1),
        })
        .collect();
    FileFindings {
        path: PathBuf::from(path),
        canonical: PathBuf::from(path),
        depth,
        hits,
    }
}

fn report(files: Vec<FileFindings>) -> Report {
    Report {
        files,
        notes: Vec::new(),
    }
}

fn kinds<'a>(found: impl Iterator<Item = (&'a FileFindings, &'a Hit)>) -> Vec<(String, Kind)> {
    found
        .map(|(file, hit)| (file.path.display().to_string(), hit.kind))
        .collect()
}

#[test]
fn info_and_hints_are_not_conflicts() {
    let quiet = report(vec![file(
        "/h/.zshrc",
        0,
        &[NvmDirExport, HelperCall, OmzPlugin, NotFollowed],
    )]);
    assert!(!quiet.has_conflicts());
    assert_eq!(quiet.conflicts().count(), 0);
    assert!(!Report::default().has_conflicts());
}

#[test]
fn conflicts_are_every_conflicting_hit_of_every_file_in_order() {
    let found = report(vec![
        file("/h/.zshrc", 0, &[NvmDirExport, Loader]),
        file("/h/lazy.zsh", 1, &[LazyStub, HelperCall]),
    ]);
    assert!(found.has_conflicts());
    let expected = [
        ("/h/.zshrc".to_owned(), Loader),
        ("/h/lazy.zsh".to_owned(), LazyStub),
    ];
    assert_eq!(kinds(found.conflicts()), expected);
}

#[test]
fn only_loaders_and_completions_of_roots_are_auto_migratable() {
    let found = report(vec![
        file(
            "/h/.bashrc",
            0,
            &[Loader, NvmDirExport, Completion, LazyStub],
        ),
        file("/h/extra.sh", 1, &[Loader]),
    ]);
    let expected = [
        ("/h/.bashrc".to_owned(), Loader),
        ("/h/.bashrc".to_owned(), Completion),
    ];
    assert_eq!(kinds(found.auto_migratable()), expected);
}
```

Create `src/commands/conflict/roots.rs`:

```rust
//! The startup files each shell reads (digest section 5).
//!
//! `sh` and `dash` share one list: as a profile reader they are the same
//! POSIX shell (`~/.profile` for a login shell, `$ENV` for an interactive
//! one); the separate names only matter to `nvm init`.

use std::path::{Path, PathBuf};

use crate::context::Context;
use crate::domain::conflict::{Expansion, expand_path};
use crate::shell::init::{SHELLS, Shell};

const BASH_FILES: [&str; 4] = [".bashrc", ".bash_profile", ".bash_login", ".profile"];
const ZSH_FILES: [&str; 4] = [".zshenv", ".zprofile", ".zshrc", ".zlogin"];

/// The startup files of `shell` that exist (links followed), in the order the
/// shell reads them; `None` for those of every shell, each file once:
///
/// - bash: `~/.bashrc`, `~/.bash_profile`, `~/.bash_login`, `~/.profile`;
/// - zsh: `.zshenv`, `.zprofile`, `.zshrc`, `.zlogin` in `$ZDOTDIR` (else
///   `$HOME`), after `~/.zshenv` when `ZDOTDIR` is elsewhere (that file is
///   read first, and is where `ZDOTDIR` is usually set);
/// - sh and dash: `~/.profile` and `$ENV`;
/// - ksh: `~/.profile` and `$ENV`, which defaults to `~/.kshrc`;
/// - fish: `conf.d/*.fish` by name, then `config.fish`, in
///   `${XDG_CONFIG_HOME:-~/.config}/fish`.
///
/// `$ENV` is expanded like a sourced path; one that cannot be is skipped.
#[must_use]
pub fn roots(context: &Context<'_>, shell: Option<Shell>) -> Vec<PathBuf> {
    let shells = shell.map_or(SHELLS.to_vec(), |shell| vec![shell]);
    let mut found: Vec<PathBuf> = Vec::new();
    for candidate in shells
        .into_iter()
        .flat_map(|shell| candidates(context, shell))
    {
        if !found.contains(&candidate) && context.fs.is_file(&candidate) {
            found.push(candidate);
        }
    }
    found
}

/// Every file `shell` may read, present or not.
fn candidates(context: &Context<'_>, shell: Shell) -> Vec<PathBuf> {
    let home = directory_variable(context, "HOME");
    match shell {
        Shell::Bash => in_directory(home.as_deref(), &BASH_FILES),
        Shell::Zsh => zsh_files(context, home.as_deref()),
        Shell::Sh | Shell::Dash => profile_and_env(context, home.as_deref(), None),
        Shell::Ksh => profile_and_env(context, home.as_deref(), Some(".kshrc")),
        Shell::Fish => fish_files(context, home.as_deref()),
    }
}

/// A variable naming a directory; `None` when unset or empty.
fn directory_variable(context: &Context<'_>, name: &str) -> Option<PathBuf> {
    let value = context.env.var_os(name).filter(|value| !value.is_empty());
    value.map(PathBuf::from)
}

fn in_directory(directory: Option<&Path>, names: &[&str]) -> Vec<PathBuf> {
    directory.map_or_else(Vec::new, |directory| {
        names.iter().map(|name| directory.join(name)).collect()
    })
}

fn zsh_files(context: &Context<'_>, home: Option<&Path>) -> Vec<PathBuf> {
    let zdotdir = directory_variable(context, "ZDOTDIR");
    let mut files = Vec::new();
    if let (Some(zdotdir), Some(home)) = (zdotdir.as_deref(), home) {
        if zdotdir != home {
            files.push(home.join(".zshenv"));
        }
    }
    files.extend(in_directory(zdotdir.as_deref().or(home), &ZSH_FILES));
    files
}

/// `~/.profile`, then `$ENV` (or `~/<default_env>` when `ENV` is unset).
fn profile_and_env(
    context: &Context<'_>,
    home: Option<&Path>,
    default_env: Option<&str>,
) -> Vec<PathBuf> {
    let mut files = in_directory(home, &[".profile"]);
    let env_file = match context.env.var("ENV").filter(|value| !value.is_empty()) {
        Some(value) => absolute_expansion(context, &value),
        None => default_env.and_then(|name| home.map(|home| home.join(name))),
    };
    files.extend(env_file);
    files
}

/// `value` expanded as a sourced path, when that gives an absolute path.
fn absolute_expansion(context: &Context<'_>, value: &str) -> Option<PathBuf> {
    let lookup = |name: &str| context.env.var(name);
    match expand_path(value, &lookup) {
        Expansion::Resolved(path) => Some(PathBuf::from(path)).filter(|path| path.is_absolute()),
        Expansion::Unresolvable(_) => None,
    }
}

fn fish_files(context: &Context<'_>, home: Option<&Path>) -> Vec<PathBuf> {
    let config_home = directory_variable(context, "XDG_CONFIG_HOME")
        .or_else(|| home.map(|home| home.join(".config")));
    let Some(fish) = config_home.map(|config| config.join("fish")) else {
        return Vec::new();
    };
    let conf_d = fish.join("conf.d");
    let mut names: Vec<String> = context
        .fs
        .read_dir(&conf_d)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| !entry.is_dir && entry.name.ends_with(".fish"))
        .map(|entry| entry.name)
        .collect();
    names.sort();
    let mut files: Vec<PathBuf> = names.iter().map(|name| conf_d.join(name)).collect();
    files.push(fish.join("config.fish"));
    files
}
```

Create `src/commands/conflict/roots_tests.rs`:

```rust
use super::fixtures::{home_env, link, paths};
use super::roots;
use crate::context::Context;
use crate::fakes::{FakeEnv, FakeFileSystem};
use crate::shell::init::Shell;

fn with_files(names: &[&str]) -> FakeFileSystem {
    names
        .iter()
        .fold(FakeFileSystem::default(), |fs, name| fs.with_file(name, ""))
}

fn roots_of(fs: &FakeFileSystem, env: &FakeEnv, shell: Option<Shell>) -> Vec<std::path::PathBuf> {
    roots(&Context::new(fs, env), shell)
}

const BASH: [&str; 4] = [
    "/Users/u/.bashrc",
    "/Users/u/.bash_profile",
    "/Users/u/.bash_login",
    "/Users/u/.profile",
];

#[test]
fn bash_reads_bashrc_then_the_login_files_in_order() {
    let fs = with_files(&BASH);
    assert_eq!(roots_of(&fs, &home_env(), Some(Shell::Bash)), paths(&BASH));
}

#[test]
fn only_the_files_that_exist_are_roots() {
    let fs = with_files(&["/Users/u/.bash_profile", "/Users/u/.zshrc"]);
    let found = roots_of(&fs, &home_env(), Some(Shell::Bash));
    assert_eq!(found, paths(&["/Users/u/.bash_profile"]));
}

const ZSH: [&str; 4] = [
    "/Users/u/.zshenv",
    "/Users/u/.zprofile",
    "/Users/u/.zshrc",
    "/Users/u/.zlogin",
];

#[test]
fn zsh_reads_its_four_files_from_home_without_zdotdir() {
    let fs = with_files(&ZSH);
    assert_eq!(roots_of(&fs, &home_env(), Some(Shell::Zsh)), paths(&ZSH));
    let empty = home_env().with_var("ZDOTDIR", "");
    assert_eq!(roots_of(&fs, &empty, Some(Shell::Zsh)), paths(&ZSH));
}

#[test]
fn zsh_reads_the_zdotdir_files_after_the_home_zshenv_that_sets_it() {
    let fs = with_files(&ZSH)
        .with_file("/z/.zshenv", "")
        .with_file("/z/.zshrc", "");
    let env = home_env().with_var("ZDOTDIR", "/z");
    let found = roots_of(&fs, &env, Some(Shell::Zsh));
    assert_eq!(
        found,
        paths(&["/Users/u/.zshenv", "/z/.zshenv", "/z/.zshrc"])
    );
}

#[test]
fn sh_and_dash_read_profile_and_the_file_named_by_env() {
    let fs = with_files(&["/Users/u/.profile", "/Users/u/.shinit"]);
    let env = home_env().with_var("ENV", "$HOME/.shinit");
    for shell in [Shell::Sh, Shell::Dash] {
        let found = roots_of(&fs, &env, Some(shell));
        assert_eq!(found, paths(&["/Users/u/.profile", "/Users/u/.shinit"]));
        let without_env = roots_of(&fs, &home_env(), Some(shell));
        assert_eq!(without_env, paths(&["/Users/u/.profile"]));
    }
}

#[test]
fn ksh_reads_profile_and_env_which_defaults_to_kshrc() {
    let fs = with_files(&["/Users/u/.profile", "/Users/u/.kshrc", "/etc/kshenv"]);
    let found = roots_of(&fs, &home_env(), Some(Shell::Ksh));
    assert_eq!(found, paths(&["/Users/u/.profile", "/Users/u/.kshrc"]));
    let env = home_env().with_var("ENV", "/etc/kshenv");
    let found = roots_of(&fs, &env, Some(Shell::Ksh));
    assert_eq!(found, paths(&["/Users/u/.profile", "/etc/kshenv"]));
}

#[test]
fn an_env_that_cannot_be_expanded_is_ignored() {
    let fs = with_files(&["/Users/u/.profile"]);
    let env = home_env().with_var("ENV", "$(echo /x)");
    let found = roots_of(&fs, &env, Some(Shell::Sh));
    assert_eq!(found, paths(&["/Users/u/.profile"]));
}

#[test]
fn fish_reads_conf_d_sorted_then_config_fish() {
    let fs = with_files(&[
        "/Users/u/.config/fish/config.fish",
        "/Users/u/.config/fish/conf.d/b.fish",
        "/Users/u/.config/fish/conf.d/a.fish",
        "/Users/u/.config/fish/conf.d/notes.txt",
        "/Users/u/.config/fish/conf.d/dir.fish/inner.fish",
    ]);
    let found = roots_of(&fs, &home_env(), Some(Shell::Fish));
    let expected = [
        "/Users/u/.config/fish/conf.d/a.fish",
        "/Users/u/.config/fish/conf.d/b.fish",
        "/Users/u/.config/fish/config.fish",
    ];
    assert_eq!(found, paths(&expected));
}

#[test]
fn fish_follows_xdg_config_home() {
    let fs = with_files(&["/x/fish/config.fish", "/x/fish/conf.d/n.fish"]);
    let env = home_env().with_var("XDG_CONFIG_HOME", "/x");
    let found = roots_of(&fs, &env, Some(Shell::Fish));
    assert_eq!(
        found,
        paths(&["/x/fish/conf.d/n.fish", "/x/fish/config.fish"])
    );
}

#[test]
fn every_shell_without_a_name_once_each_in_shell_order() {
    let fs = with_files(&[
        "/Users/u/.profile",
        "/Users/u/.zshrc",
        "/Users/u/.bashrc",
        "/Users/u/.kshrc",
        "/Users/u/.config/fish/config.fish",
    ]);
    let found = roots_of(&fs, &home_env(), None);
    let expected = [
        "/Users/u/.bashrc",
        "/Users/u/.profile",
        "/Users/u/.zshrc",
        "/Users/u/.kshrc",
        "/Users/u/.config/fish/config.fish",
    ];
    assert_eq!(found, paths(&expected));
}

#[test]
fn a_linked_root_counts_and_a_dangling_link_does_not() {
    let fs = with_files(&["/Users/u/dotfiles/zsh/.zshrc"]);
    link(&fs, "dotfiles/zsh/.zshrc", "/Users/u/.zshrc");
    link(&fs, "dotfiles/zsh/.zprofile", "/Users/u/.zprofile");
    let found = roots_of(&fs, &home_env(), Some(Shell::Zsh));
    assert_eq!(found, paths(&["/Users/u/.zshrc"]));
}

#[test]
fn without_home_only_absolute_variables_name_roots() {
    let fs = with_files(&["/etc/shinit", "/z/.zshrc", "/.profile"]);
    let env = FakeEnv::default().with_var("ENV", "/etc/shinit");
    assert_eq!(
        roots_of(&fs, &env, Some(Shell::Sh)),
        paths(&["/etc/shinit"])
    );
    let env = FakeEnv::default().with_var("ZDOTDIR", "/z");
    assert_eq!(roots_of(&fs, &env, Some(Shell::Zsh)), paths(&["/z/.zshrc"]));
}
```

Create `src/commands/conflict/walk.rs`:

```rust
//! Reading the roots and the files they source, breadth first (digest
//! section 6): every root at depth 0 before any file they source, so a root
//! that another root sources stays a root (auto-migratable).
//!
//! A `source`, `.` or `\.` line is followed when its path expands (`~`,
//! `$HOME`, `${ZDOTDIR:-$HOME}`, any variable of the process environment);
//! a relative path is resolved from the directory of the sourcing file as
//! named (the shell would use `$PWD`, which is `$HOME` for a login shell) and
//! noted. Files are told apart by their canonical path, so a cycle or a file
//! sourced twice (or through a link) is read once, under its first name.
//! Comments are not followed; nvmrc's own init block sources nothing.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use super::{FileFindings, Note, NoteReason, Report};
use crate::context::Context;
use crate::domain::conflict::{Expansion, expand_path, scan_text, source_target};

/// How deep sourced files are read: roots are 0, the files they source 1.
pub const MAX_DEPTH: usize = 2;

/// Every root (see [`super::roots`]) and the files they source, down to
/// [`MAX_DEPTH`]: what the rules found, in scan order, and what could not be
/// followed. Never fails: a file that cannot be read becomes a [`Note`].
#[must_use]
pub fn scan(context: &Context<'_>, roots: &[PathBuf]) -> Report {
    let mut walker = Walker {
        context,
        seen: HashSet::new(),
        queue: VecDeque::new(),
        report: Report::default(),
    };
    for root in roots {
        let canonical = context
            .fs
            .canonicalize(root)
            .unwrap_or_else(|_| root.clone());
        walker.enqueue(root.clone(), canonical, 0);
    }
    while let Some(pending) = walker.queue.pop_front() {
        walker.visit(pending);
    }
    walker.report
}

/// A file waiting to be read.
struct Pending {
    path: PathBuf,
    canonical: PathBuf,
    depth: usize,
}

struct Walker<'c, 'a> {
    context: &'c Context<'a>,
    /// The canonical paths already queued.
    seen: HashSet<PathBuf>,
    queue: VecDeque<Pending>,
    report: Report,
}

impl Walker<'_, '_> {
    fn enqueue(&mut self, path: PathBuf, canonical: PathBuf, depth: usize) {
        if self.seen.insert(canonical.clone()) {
            self.queue.push_back(Pending {
                path,
                canonical,
                depth,
            });
        }
    }

    fn visit(&mut self, file: Pending) {
        let text = match self.context.fs.read_to_string(&file.path) {
            Ok(text) => text,
            Err(error) => {
                let reason = NoteReason::Unreadable(error.to_string());
                return self.note(&file.path, None, "", reason);
            }
        };
        for (index, line) in text.lines().enumerate() {
            if let Some(token) = source_target(line) {
                self.follow(&file, index + 1, line, &token);
            }
        }
        self.report.files.push(FileFindings {
            hits: scan_text(&text),
            path: file.path,
            canonical: file.canonical,
            depth: file.depth,
        });
    }

    /// Queues (or notes) the file that line `number` of `from` sources.
    fn follow(&mut self, from: &Pending, number: usize, line: &str, token: &str) {
        let Some(path) = self.resolve(from, number, line, token) else {
            return;
        };
        if !self.context.fs.is_file(&path) {
            return self.note(&from.path, Some(number), line, NoteReason::Missing(path));
        }
        let canonical = self.context.fs.canonicalize(&path);
        let canonical = canonical.unwrap_or_else(|_| path.clone());
        if from.depth < MAX_DEPTH {
            self.enqueue(path, canonical, from.depth + 1);
        } else if self.seen.insert(canonical) {
            self.note(&from.path, Some(number), line, NoteReason::TooDeep(path));
        }
    }

    /// The path `token` names, expanded, relative paths made absolute (and
    /// noted); `None` (noted) when it cannot be expanded.
    fn resolve(
        &mut self,
        from: &Pending,
        number: usize,
        line: &str,
        token: &str,
    ) -> Option<PathBuf> {
        let environment = self.context.env;
        let lookup = |name: &str| environment.var(name);
        let path = match expand_path(token, &lookup) {
            Expansion::Resolved(path) => PathBuf::from(path),
            Expansion::Unresolvable(why) => {
                self.note(
                    &from.path,
                    Some(number),
                    line,
                    NoteReason::Unresolvable(why),
                );
                return None;
            }
        };
        if path.is_absolute() {
            return Some(path);
        }
        let directory = from.path.parent().unwrap_or_else(|| Path::new("/"));
        let resolved = directory.join(path);
        let reason = NoteReason::Relative(resolved.clone());
        self.note(&from.path, Some(number), line, reason);
        Some(resolved)
    }

    fn note(&mut self, path: &Path, line: Option<usize>, text: &str, reason: NoteReason) {
        self.report.notes.push(Note {
            path: path.to_path_buf(),
            line,
            text: text.to_owned(),
            reason,
        });
    }
}
```

Create `src/commands/conflict/walk_tests.rs`:

```rust
use std::path::PathBuf;

use super::fixtures::{
    HOME, hits_of, home_env, link, scan_with, scanned, users_dotfiles, users_env,
};
use super::{Note, NoteReason, roots, scan};
use crate::context::Context;
use crate::domain::conflict::Kind::{LazyLoader, LazyStub, Loader, NvmDirExport, OmzPlugin, Unset};
use crate::fakes::FakeFileSystem;
use crate::ports::FileSystem;
use crate::shell::init::Shell;

fn note(path: &str, line: usize, text: &str, reason: NoteReason) -> Note {
    Note {
        path: PathBuf::from(path),
        line: Some(line),
        text: text.to_owned(),
        reason,
    }
}

fn owned(pairs: &[(&str, usize)]) -> Vec<(String, usize)> {
    pairs
        .iter()
        .map(|(path, depth)| ((*path).to_owned(), *depth))
        .collect()
}

#[test]
fn the_users_dotfiles_are_scanned_roots_first_then_what_they_source() {
    let (fs, env) = (users_dotfiles(), users_env());
    let context = Context::new(&fs, &env);
    let report = scan(&context, &roots(&context, Some(Shell::Zsh)));
    let expected = [
        ("/Users/u/.zshenv", 0),
        ("/Users/u/.zprofile", 0),
        ("/Users/u/.zshrc", 0),
        ("/Users/u/.swiftly/env.sh", 1),
        ("/Users/u/.oh-my-zsh/oh-my-zsh.sh", 1),
        ("/Users/u/dotfiles/zsh/scripts/lazy-functions.zsh", 1),
        ("/Users/u/dotfiles/zsh/scripts/environment.zsh", 1),
    ];
    assert_eq!(scanned(&report), owned(&expected));
    let zshrc = &report.files[2];
    assert_eq!(
        zshrc.canonical,
        PathBuf::from("/Users/u/dotfiles/zsh/.zshrc")
    );
}

#[test]
fn the_users_conflict_is_the_lazy_loader_at_depth_one() {
    let (fs, env) = (users_dotfiles(), users_env());
    let report = scan_with(
        &fs,
        &env,
        &["/Users/u/.zshenv", "/Users/u/.zprofile", "/Users/u/.zshrc"],
    );
    assert_eq!(hits_of(&report, "/Users/u/.zshenv"), [(3, NvmDirExport)]);
    assert_eq!(hits_of(&report, "/Users/u/.zshrc"), [(3, OmzPlugin)]);
    let lazy = "/Users/u/dotfiles/zsh/scripts/lazy-functions.zsh";
    let expected = [(1, LazyStub), (2, Unset), (3, LazyLoader), (6, LazyStub)];
    assert_eq!(hits_of(&report, lazy), expected);
    assert!(report.has_conflicts());
    assert_eq!(report.auto_migratable().count(), 0);
    let omz_line = "  source \"$ZSH/plugins/$plugin/$plugin.plugin.zsh\"";
    let stub_line = "  [ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"";
    let unset = |name: &str| NoteReason::Unresolvable(format!("unset ${name}"));
    let omz = "/Users/u/.oh-my-zsh/oh-my-zsh.sh";
    assert_eq!(
        report.notes,
        [
            note(omz, 2, omz_line, unset("plugin")),
            note(lazy, 3, stub_line, unset("nvm_prefix")),
        ]
    );
}

fn chain() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/Users/u/.bashrc", "source ~/a.sh\n")
        .with_file("/Users/u/a.sh", "source ~/b.sh\n")
        .with_file("/Users/u/b.sh", "source ~/c.sh\n")
        .with_file("/Users/u/c.sh", ". \"$HOME/.nvm/nvm.sh\"\n")
}

#[test]
fn depth_three_is_not_scanned_but_noted() {
    let report = scan_with(&chain(), &home_env(), &["/Users/u/.bashrc"]);
    let expected = [
        ("/Users/u/.bashrc", 0),
        ("/Users/u/a.sh", 1),
        ("/Users/u/b.sh", 2),
    ];
    assert_eq!(scanned(&report), owned(&expected));
    let too_deep = NoteReason::TooDeep(PathBuf::from("/Users/u/c.sh"));
    assert_eq!(
        report.notes,
        [note("/Users/u/b.sh", 1, "source ~/c.sh", too_deep)]
    );
    assert!(!report.has_conflicts());
}

#[test]
fn a_cycle_scans_each_file_once() {
    let fs = FakeFileSystem::default()
        .with_file("/Users/u/.bashrc", "source ~/a.sh\nsource ~/.bashrc\n")
        .with_file("/Users/u/a.sh", "source ~/b.sh\n")
        .with_file("/Users/u/b.sh", "source ~/a.sh\n. ~/.bashrc\n");
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc"]);
    let expected = [
        ("/Users/u/.bashrc", 0),
        ("/Users/u/a.sh", 1),
        ("/Users/u/b.sh", 2),
    ];
    assert_eq!(scanned(&report), owned(&expected));
    assert!(report.notes.is_empty());
}

#[test]
fn a_file_sourced_twice_or_through_a_link_is_kept_under_its_first_name() {
    let fs = FakeFileSystem::default()
        .with_file(
            "/Users/u/.bashrc",
            "source ~/a.sh\n. ~/alias.sh\nsource $HOME/a.sh\n",
        )
        .with_file("/Users/u/a.sh", "");
    link(&fs, "a.sh", "/Users/u/alias.sh");
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc"]);
    let expected = [("/Users/u/.bashrc", 0), ("/Users/u/a.sh", 1)];
    assert_eq!(scanned(&report), owned(&expected));
}

#[test]
fn a_root_sourced_by_an_earlier_root_stays_a_root() {
    let fs = FakeFileSystem::default()
        .with_file("/Users/u/.bash_profile", "source ~/.profile\n")
        .with_file("/Users/u/.profile", ". /opt/nvm/nvm.sh\n");
    let roots = ["/Users/u/.bash_profile", "/Users/u/.profile"];
    let report = scan_with(&fs, &home_env(), &roots);
    let expected = [("/Users/u/.bash_profile", 0), ("/Users/u/.profile", 0)];
    assert_eq!(scanned(&report), owned(&expected));
    assert_eq!(report.auto_migratable().count(), 1);
}

#[test]
fn a_command_substitution_is_noted_and_the_loader_still_found() {
    let line = "source $(brew --prefix nvm)/nvm.sh";
    let fs = FakeFileSystem::default().with_file("/Users/u/.bashrc", &format!("{line}\n"));
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc"]);
    assert_eq!(hits_of(&report, "/Users/u/.bashrc"), [(1, Loader)]);
    let why = NoteReason::Unresolvable("command substitution $(brew --prefix nvm)".to_owned());
    assert_eq!(report.notes, [note("/Users/u/.bashrc", 1, line, why)]);
    assert!(report.has_conflicts());
    assert_eq!(report.auto_migratable().count(), 1);
}

#[test]
fn a_relative_path_is_resolved_from_the_sourcing_file_and_noted() {
    let fs = FakeFileSystem::default()
        .with_file("/Users/u/.bashrc", "source lib/extra.sh\n")
        .with_file("/Users/u/lib/extra.sh", ". /opt/nvm/nvm.sh\n");
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc"]);
    let expected = [("/Users/u/.bashrc", 0), ("/Users/u/lib/extra.sh", 1)];
    assert_eq!(scanned(&report), owned(&expected));
    let relative = NoteReason::Relative(PathBuf::from("/Users/u/lib/extra.sh"));
    let missing = NoteReason::Missing(PathBuf::from("/opt/nvm/nvm.sh"));
    let extra = ". /opt/nvm/nvm.sh";
    assert_eq!(
        report.notes,
        [
            note("/Users/u/.bashrc", 1, "source lib/extra.sh", relative),
            note("/Users/u/lib/extra.sh", 1, extra, missing),
        ]
    );
    assert_eq!(hits_of(&report, "/Users/u/lib/extra.sh"), [(1, Loader)]);
    assert!(report.has_conflicts());
    assert_eq!(report.auto_migratable().count(), 0, "not in a root");
}

#[test]
fn a_missing_file_is_noted() {
    let text = "[ -f \"$HOME/.missing.sh\" ] && . \"$HOME/.missing.sh\"";
    let fs = FakeFileSystem::default().with_file("/Users/u/.bashrc", &format!("{text}\n"));
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc"]);
    let missing = NoteReason::Missing(PathBuf::from(format!("{HOME}/.missing.sh")));
    assert_eq!(report.notes, [note("/Users/u/.bashrc", 1, text, missing)]);
    assert_eq!(report.files.len(), 1);
}

fn unreadable_note(report_notes: &[Note], path: &str) -> bool {
    report_notes.iter().any(|note| {
        note.path == std::path::Path::new(path)
            && note.line.is_none()
            && note.text.is_empty()
            && matches!(note.reason, NoteReason::Unreadable(_))
    })
}

#[test]
fn an_unreadable_root_or_sourced_file_is_noted_not_scanned() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.profile", "source ~/bin.sh\n");
    let written = fs.write_bytes(std::path::Path::new("/Users/u/bin.sh"), b"\xff\xfe");
    assert!(written.is_ok());
    let written = fs.write_bytes(std::path::Path::new("/Users/u/.bashrc"), b"\xff");
    assert!(written.is_ok());
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc", "/Users/u/.profile"]);
    assert_eq!(scanned(&report), owned(&[("/Users/u/.profile", 0)]));
    assert_eq!(report.notes.len(), 2);
    assert!(unreadable_note(&report.notes, "/Users/u/.bashrc"));
    assert!(unreadable_note(&report.notes, "/Users/u/bin.sh"));
}

#[test]
fn a_clean_setup_has_no_hits_and_comments_are_not_followed() {
    let fs = FakeFileSystem::default()
        .with_file(
            "/Users/u/.bashrc",
            "export PATH=\"$HOME/bin:$PATH\"\nsource ~/.aliases\n# source ~/.secret.sh\n",
        )
        .with_file("/Users/u/.aliases", "alias ll='ls -l'\n")
        .with_file("/Users/u/.secret.sh", ". /opt/nvm/nvm.sh\n");
    let report = scan_with(&fs, &home_env(), &["/Users/u/.bashrc"]);
    let expected = [("/Users/u/.bashrc", 0), ("/Users/u/.aliases", 1)];
    assert_eq!(scanned(&report), owned(&expected));
    assert!(report.files.iter().all(|file| file.hits.is_empty()));
    assert!(report.notes.is_empty());
    assert!(!report.has_conflicts());
}

#[test]
fn no_roots_give_an_empty_report() {
    let report = scan_with(&FakeFileSystem::default(), &home_env(), &[]);
    assert!(report.files.is_empty() && report.notes.is_empty());
}
```

Apply to `src/domain/conflict/kind.rs` (above the test module):

```diff
--- a/src/domain/conflict/kind.rs
+++ b/src/domain/conflict/kind.rs
@@ -8,7 +8,9 @@
     /// Worth knowing, never a conflict (an `NVM_DIR` export, a `source` whose
     /// path cannot be followed).
     Info,
-    /// Breaks once nvm.sh is gone (a call of an nvm.sh helper), not a conflict.
+    /// Not a conflict, but worth a manual fix: a call of an nvm.sh helper
+    /// (breaks once nvm.sh is gone), or oh-my-zsh's `nvm` plugin (it returns
+    /// at once while nvmrc's `nvm` binary is on `PATH`, digest finding 0.5).
     Hint,
     /// nvm.sh (or something loading it) competes with nvmrc's `nvm`.
     Conflict,
@@ -28,7 +30,8 @@
     LazyStub,
     /// `unset -f nvm`, `unfunction nvm`: a self-removing stub.
     Unset,
-    /// `nvm` in oh-my-zsh's `plugins=( ... )`, or its `zstyle`.
+    /// `nvm` in oh-my-zsh's `plugins=( ... )`, or its `zstyle`: a hint, since
+    /// the plugin does nothing while nvmrc's binary is on `PATH`.
     OmzPlugin,
     /// lukechilds/zsh-nvm.
     ZshNvm,
@@ -48,7 +51,7 @@
     pub const fn severity(self) -> Severity {
         match self {
             Self::NvmDirExport | Self::NotFollowed => Severity::Info,
-            Self::HelperCall => Severity::Hint,
+            Self::HelperCall | Self::OmzPlugin => Severity::Hint,
             _ => Severity::Conflict,
         }
     }
```

Apply to `src/domain/conflict/kind_tests.rs` (above the test module):

```diff
--- a/src/domain/conflict/kind_tests.rs
+++ b/src/domain/conflict/kind_tests.rs
@@ -15,11 +15,11 @@
 ];
 
 #[test]
-fn severity_is_info_for_exports_and_unfollowed_sources_hint_for_helpers() {
+fn severity_is_info_for_exports_and_unfollowed_sources_hint_for_helpers_and_omz() {
     for kind in ALL {
         let expected = match kind {
             Kind::NvmDirExport | Kind::NotFollowed => Severity::Info,
-            Kind::HelperCall => Severity::Hint,
+            Kind::HelperCall | Kind::OmzPlugin => Severity::Hint,
             _ => Severity::Conflict,
         };
         assert_eq!(kind.severity(), expected, "{kind:?}");
@@ -58,3 +58,10 @@
     assert_eq!(Kind::OmzPlugin.label(), "oh-my-zsh nvm plugin");
     assert_eq!(Kind::NvmDirExport.label(), "NVM_DIR export");
 }
+
+#[test]
+fn the_omz_plugin_is_a_hint_because_the_binary_on_path_neutralises_it() {
+    assert_eq!(Kind::OmzPlugin.severity(), Severity::Hint);
+    assert!(!Kind::OmzPlugin.is_conflict());
+    assert!(!Kind::OmzPlugin.is_auto_migratable());
+}
```

Apply to `src/shell/init/mod.rs` (above the test module):

```diff
--- a/src/shell/init/mod.rs
+++ b/src/shell/init/mod.rs
@@ -47,7 +47,8 @@
     Fish,
 }
 
-const SHELLS: [Shell; 6] = [
+/// Every shell, in the order `nvm init` lists them.
+pub const SHELLS: [Shell; 6] = [
     Shell::Bash,
     Shell::Zsh,
     Shell::Sh,
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1241 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(conflict): scan the profile files and the files they source"
```

---

### Task 4: `nvm doctor`

**Files:**

- Create: `src/commands/doctor/fix.rs`, `src/commands/doctor/fix_tests.rs`,
  `src/commands/doctor/mod.rs`, `src/commands/doctor/render.rs`,
  `src/commands/doctor/render_tests.rs`, `src/commands/doctor/tests.rs`
- Modify: `src/cli/commands.rs`, `src/commands/conflict/mod.rs`,
  `src/commands/conflict/walk.rs`, `src/commands/mod.rs`

**Interfaces:**

- Produces: `commands::doctor::{run, USAGE, fix_text}`; the `doctor` subcommand;
  `impl Display for NoteReason`. The scanner skips a sourced file named `nvm.sh`
  or `bash_completion` (the loader line is still reported; scanning the real
  `nvm.sh` produced hundreds of false hints).
- Behaviour: `nvm doctor [--shell <name>]` is read-only; with no shell it scans
  the roots of every shell that has files. Output on stdout: `nvm doctor:
  scanned <N> file(s)`, then a block per file with hits (`<path>` and `->
  <canonical>` when it is a link; `<line>: <kind label>  <the line>` and `fix:
  <text>`; a `patch:` line for manual loaders), an `Info:` section (`NVM_DIR`
  exports, notes) and `Result: <K> conflict(s)[, <M> can be fixed with `nvm
  migrate`]` or `Result: no conflicts` (exit 1 or 0); no profile file at all is
  `nvm doctor: no shell profile files found` (0). The fix text is by kind: a
  top-level Loader or Completion is fixed by `nvm migrate`; a by-hand loader
  says to replace the line with `eval "$(nvmrc init <shell>)"` and a by-hand
  completion line to delete it (nvmrc ships no bash_completion); stubs and
  `unfunction nvm` lines say what they hide; the omz plugin, zsh-nvm and `bass`
  lines say what to remove.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -6,6 +6,7 @@
 pub mod conflict;
 pub mod current;
 pub mod deactivate;
+pub mod doctor;
 pub mod exec;
 pub mod init;
 pub mod install;
```

- [ ] **Step 2: Write the failing tests**

Create `src/commands/doctor/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;

use crate::commands::Output;
use crate::commands::conflict::{roots, scan};
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};
use crate::shell::init::Shell;

pub use fix::fix_text;

/// How `nvm doctor` is called.
pub const USAGE: &str = "Usage: nvm doctor [--shell <name>]";

const NO_FILES: &str = "nvm doctor: no shell profile files found";

/// Scans the startup files of the shell named by `--shell` (of every shell
/// with files when none is named) and prints what it found.
///
/// # Errors
/// [`CliError::Usage`] for an unknown shell, a missing shell name or a
/// positional word, and [`CliError::Unsupported`] for any other option.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let shell = parse_shell(args)?;
    let files = roots(context, shell);
    if files.is_empty() {
        return Ok(Output::stdout(NO_FILES));
    }
    let report = scan(context, &files);
    let status = if report.has_conflicts() {
        NvmExitCode::Failure
    } else {
        NvmExitCode::Success
    };
    Ok(Output::stdout(render::render(&report)).with_status(status))
}

/// The shell of `--shell <name>` or `--shell=<name>`; the last one wins.
fn parse_shell(args: &[String]) -> Result<Option<Shell>, CliError> {
    let mut shell = None;
    let mut arguments = args.iter();
    while let Some(argument) = arguments.next() {
        let name = if argument == "--shell" {
            let name = arguments
                .next()
                .ok_or_else(|| CliError::Usage(format!("--shell needs a name.\n{USAGE}")))?;
            Some(name.as_str())
        } else {
            argument.strip_prefix("--shell=")
        };
        match name {
            Some(name) => shell = Some(name.parse()?),
            None => return Err(rejected(argument)),
        }
    }
    Ok(shell)
}

fn rejected(argument: &str) -> CliError {
    if argument.starts_with('-') {
        CliError::Unsupported(format!("Unsupported option \"{argument}\"."))
    } else {
        CliError::Usage(format!("Unexpected argument \"{argument}\".\n{USAGE}"))
    }
}
```

Create `src/commands/doctor/tests.rs`:

```rust
use super::{USAGE, run};
use crate::commands::Output;
use crate::commands::conflict::fixtures::{HOME, home_env, link, users_dotfiles, users_env};
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};
use crate::fakes::{FakeEnv, FakeFileSystem};

fn doctor(fs: &FakeFileSystem, env: &FakeEnv, args: &[&str]) -> Result<Output, CliError> {
    let args: Vec<String> = args.iter().map(|&argument| argument.to_owned()).collect();
    run(&Context::new(fs, env), &args)
}

fn report(fs: &FakeFileSystem, env: &FakeEnv, args: &[&str]) -> Output {
    doctor(fs, env, args).unwrap()
}

const INSTALL_SH: &str = "export NVM_DIR=\"$HOME/.nvm\"\n\
[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n";

#[test]
fn no_profile_file_is_not_a_failure() {
    let output = report(&FakeFileSystem::default(), &home_env(), &[]);
    assert_eq!(output.stdout, "nvm doctor: no shell profile files found");
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn a_clean_setup_exits_zero() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.bashrc", "export PATH=$PATH:/x\n");
    let output = report(&fs, &home_env(), &[]);
    assert_eq!(
        output.stdout,
        "nvm doctor: scanned 1 file(s)\n\nResult: no conflicts"
    );
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn the_install_script_lines_are_a_loader_and_a_completion_migrate_fixes() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.bashrc", INSTALL_SH);
    let env = home_env().with_var("NVM_DIR", "/Users/u/.nvm");
    let output = report(&fs, &env, &[]);
    let fix = "      fix: run `nvm migrate` (comments the line out and adds the nvmrc init line)";
    let expected = [
        "nvm doctor: scanned 1 file(s)",
        "",
        "/Users/u/.bashrc",
        "  2: nvm.sh loader  [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm",
        fix,
        "  3: nvm bash_completion  [ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion",
        fix,
        "",
        "Info:",
        "  /Users/u/.bashrc:1: NVM_DIR export: kept by `nvm migrate`",
        "  /Users/u/.bashrc:2: file not found: /Users/u/.nvm/nvm.sh",
        "  /Users/u/.bashrc:3: file not found: /Users/u/.nvm/bash_completion",
        "",
        "Result: 2 conflict(s), 2 can be fixed with `nvm migrate`",
    ];
    assert_eq!(output.stdout, expected.join("\n"));
    assert_eq!(output.status, NvmExitCode::Failure);
}

#[test]
fn the_users_tree_shows_the_stubs_the_plugin_hint_and_the_links() {
    let output = report(&users_dotfiles(), &users_env(), &["--shell", "zsh"]);
    let stub = "      fix: the stub redefines `nvm` after the init line: delete it, or move the \
init line after it";
    let expected = [
        "nvm doctor: scanned 7 file(s)",
        "",
        "/Users/u/.zshrc -> /Users/u/dotfiles/zsh/.zshrc",
        "  3: oh-my-zsh nvm plugin  nvm",
        "      fix: remove `nvm` from `plugins=(...)`; it does nothing while the nvmrc binary is \
on PATH but depends on it",
        "",
        "/Users/u/dotfiles/zsh/scripts/lazy-functions.zsh",
        "  1: lazy stub  nvm() {",
        stub,
        "  2: nvm unset  unfunction nvm node npm npx yarn pnpm 2>/dev/null",
        stub,
        "  3: lazy loader  [ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"",
        "      fix: remove or replace this loader by hand: nvmrc has its own function (see \
`eval \"$(nvmrc init <shell>)\"`)",
        "      patch: replace the line with: eval \"$(nvmrc init zsh)\"",
        "  6: lazy stub  npm() { nvm >/dev/null 2>&1; command npm \"$@\" }",
        stub,
        "",
        "Info:",
        "  /Users/u/.zshenv:3: NVM_DIR export: kept by `nvm migrate`",
        "  /Users/u/.oh-my-zsh/oh-my-zsh.sh:2: not followed: unset $plugin",
        "  /Users/u/dotfiles/zsh/scripts/lazy-functions.zsh:3: not followed: unset $nvm_prefix",
        "",
        "Result: 4 conflict(s)",
    ];
    assert_eq!(output.stdout, expected.join("\n"));
    assert_eq!(output.status, NvmExitCode::Failure);
}

#[test]
fn a_symlinked_zshrc_shows_its_canonical_path() {
    let fs = FakeFileSystem::default().with_file("/Users/u/dots/zshrc", INSTALL_SH);
    link(&fs, "dots/zshrc", &format!("{HOME}/.zshrc"));
    let text = report(&fs, &home_env(), &[]).stdout;
    assert!(
        text.contains("\n/Users/u/.zshrc -> /Users/u/dots/zshrc\n"),
        "{text}"
    );
}

#[test]
fn shell_limits_the_roots_in_both_spellings() {
    let fs = FakeFileSystem::default()
        .with_file("/Users/u/.bashrc", INSTALL_SH)
        .with_file("/Users/u/.zshrc", "export A=1\n");
    for args in [["--shell", "zsh"], ["--shell=zsh", "--shell=zsh"]] {
        let output = report(&fs, &home_env(), &args);
        assert_eq!(output.status, NvmExitCode::Success, "{args:?}");
        assert!(output.stdout.starts_with("nvm doctor: scanned 1 file(s)"));
    }
    assert_eq!(report(&fs, &home_env(), &[]).status, NvmExitCode::Failure);
}

#[test]
fn a_shell_without_files_says_so() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.bashrc", INSTALL_SH);
    let output = report(&fs, &home_env(), &["--shell", "fish"]);
    assert_eq!(output.stdout, "nvm doctor: no shell profile files found");
}

#[test]
fn an_unknown_shell_lists_the_shells() {
    let fs = FakeFileSystem::default();
    let Err(CliError::Usage(message)) = doctor(&fs, &home_env(), &["--shell", "tcsh"]) else {
        panic!("expected a usage error");
    };
    assert!(
        message.contains("bash, zsh, sh, dash, ksh, fish"),
        "{message}"
    );
    assert_eq!(CliError::Usage(message).exit_code(), NvmExitCode::NotFound);
}

#[test]
fn a_missing_shell_name_and_positional_words_are_usage_errors() {
    let fs = FakeFileSystem::default();
    for args in [&["--shell"][..], &["zsh"][..]] {
        let result = doctor(&fs, &home_env(), args);
        assert!(matches!(&result, Err(CliError::Usage(text)) if text.contains(USAGE)));
    }
}

#[test]
fn another_option_is_unsupported() {
    let fs = FakeFileSystem::default();
    let result = doctor(&fs, &home_env(), &["--fix"]);
    assert!(
        matches!(&result, Err(CliError::Unsupported(text)) if text == "Unsupported option \"--fix\".")
    );
}

#[test]
fn a_missing_sourced_file_is_info_not_an_error() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.bashrc", ". ~/gone.sh\n");
    let output = report(&fs, &home_env(), &[]);
    assert!(
        output
            .stdout
            .contains("Info:\n  /Users/u/.bashrc:1: file not found: /Users/u/gone.sh")
    );
    assert_eq!(output.status, NvmExitCode::Success);
}
#[test]
fn nvm_sh_and_its_completion_are_reported_as_loaders_and_never_scanned() {
    let fs = FakeFileSystem::default()
        .with_file("/Users/u/.bashrc", INSTALL_SH)
        .with_file("/Users/u/.nvm/nvm.sh", "nvm() {\n  nvm_echo hi\n}\n")
        .with_file("/Users/u/.nvm/bash_completion", "nvm_echo x\n");
    let env = home_env().with_var("NVM_DIR", "/Users/u/.nvm");
    let output = report(&fs, &env, &[]);
    assert!(output.stdout.starts_with("nvm doctor: scanned 1 file(s)"));
    assert!(!output.stdout.contains("lazy stub"));
    assert!(!output.stdout.contains("helper call"));
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find module `doctor`",
"no variant named `Doctor` found" and "cannot find function `fix_text`".

- [ ] **Step 4: Write the implementation**

Apply to `src/cli/commands.rs` (above the test module):

```diff
--- a/src/cli/commands.rs
+++ b/src/cli/commands.rs
@@ -126,6 +126,13 @@
         #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
         args: Vec<String>,
     },
+    /// Report the shell startup files that still load nvm.sh or define an
+    /// `nvm` that competes with nvmrc (`--shell <name>` limits the scan to
+    /// one shell). Exits 1 when it finds a conflict; never writes a file.
+    Doctor {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
     /// What the `nvm` function runs at shell start: `use`, `install` or
     /// `none` (nvm.sh's `nvm_auto`).
     #[command(name = "__auto", hide = true)]
@@ -159,6 +166,7 @@
         Command::Exec { args } => commands::exec::run(context, args),
         Command::Run { args } => commands::run::run(context, args),
         Command::Init { args } => commands::init::run(args),
+        Command::Doctor { args } => commands::doctor::run(context, args),
         Command::Auto { args } => commands::auto::run(context, args),
     }
 }
```

Apply to `src/commands/conflict/mod.rs` (above the test module):

```diff
--- a/src/commands/conflict/mod.rs
+++ b/src/commands/conflict/mod.rs
@@ -8,7 +8,7 @@
 mod walk;
 
 #[cfg(test)]
-mod fixtures;
+pub(crate) mod fixtures;
 #[cfg(test)]
 mod report_tests;
 #[cfg(test)]
@@ -16,6 +16,7 @@
 #[cfg(test)]
 mod walk_tests;
 
+use std::fmt;
 use std::path::PathBuf;
 
 use crate::domain::conflict::Hit;
@@ -52,6 +53,24 @@
     Unreadable(String),
     /// The sourced file is deeper than [`MAX_DEPTH`]: not scanned.
     TooDeep(PathBuf),
+}
+
+impl fmt::Display for NoteReason {
+    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
+        match self {
+            Self::Unresolvable(why) => write!(formatter, "not followed: {why}"),
+            Self::Relative(path) => write!(
+                formatter,
+                "relative path resolved from the file's directory: {}",
+                path.display()
+            ),
+            Self::Missing(path) => write!(formatter, "file not found: {}", path.display()),
+            Self::Unreadable(error) => write!(formatter, "not read: {error}"),
+            Self::TooDeep(_) => {
+                write!(formatter, "not scanned: deeper than {MAX_DEPTH} levels")
+            }
+        }
+    }
 }
 
 /// Something the scan could not do, or did on a guess.
```

Apply to `src/commands/conflict/walk.rs` (above the test module):

```diff
--- a/src/commands/conflict/walk.rs
+++ b/src/commands/conflict/walk.rs
@@ -99,6 +99,9 @@
         if !self.context.fs.is_file(&path) {
             return self.note(&from.path, Some(number), line, NoteReason::Missing(path));
         }
+        if is_nvm_loader_target(&path) {
+            return;
+        }
         let canonical = self.context.fs.canonicalize(&path);
         let canonical = canonical.unwrap_or_else(|_| path.clone());
         if from.depth < MAX_DEPTH {
@@ -150,3 +153,12 @@
         });
     }
 }
+
+/// nvm.sh and its completion are what a loader line names, not startup files:
+/// reading them would report nvm's own functions and helpers as findings.
+fn is_nvm_loader_target(path: &Path) -> bool {
+    matches!(
+        path.file_name().and_then(|name| name.to_str()),
+        Some("nvm.sh" | "bash_completion")
+    )
+}
```

Create `src/commands/doctor/fix.rs`:

```rust
//! What to do about each kind of finding: one table, the single place where
//! the wording of the advice lives.

use std::path::Path;

use crate::domain::conflict::Kind;

const AUTO: &str = "run `nvm migrate` (comments the line out and adds the nvmrc init line)";
const MANUAL_LOADER: &str = "remove or replace this loader by hand: nvmrc has its own function \
(see `eval \"$(nvmrc init <shell>)\"`)";
const STUB: &str = "the stub redefines `nvm` after the init line: delete it, or move the init \
line after it";
const OMZ: &str = "remove `nvm` from `plugins=(...)`; it does nothing while the nvmrc binary \
is on PATH but depends on it";
const ZSH_NVM: &str = "remove zsh-nvm";
const BASS: &str = "remove the bass line";
const HELPER: &str = "this hook calls an nvm.sh helper that does not exist under nvmrc: use \
`nvm` subcommands";
const KEPT: &str = "kept by `nvm migrate`";

/// The advice for a finding of `kind` in a file `depth` levels from a root.
/// A loader or completion that `migrate` cannot comment out (inside a
/// sourced file) is fixed by hand, as is a lazy loader.
#[must_use]
pub fn fix_text(kind: Kind, depth: usize) -> &'static str {
    match kind {
        Kind::Loader | Kind::Completion if depth == 0 => AUTO,
        Kind::Loader | Kind::Completion | Kind::LazyLoader => MANUAL_LOADER,
        Kind::LazyStub | Kind::Unset => STUB,
        Kind::OmzPlugin => OMZ,
        Kind::ZshNvm => ZSH_NVM,
        Kind::Bass => BASS,
        Kind::HelperCall => HELPER,
        Kind::NvmDirExport => KEPT,
        Kind::NotFollowed => "",
    }
}

/// The one-line patch suggested for a loader fixed by hand, `None` for the
/// kinds that have none.
#[must_use]
pub fn suggested_patch(kind: Kind, depth: usize, path: &Path) -> Option<String> {
    let by_hand = matches!(kind, Kind::LazyLoader) || (depth > 0 && kind == Kind::Loader);
    by_hand.then(|| {
        let shell = shell_of(path);
        let line = if shell == "fish" {
            "nvmrc init fish | source".to_owned()
        } else {
            format!("eval \"$(nvmrc init {shell})\"")
        };
        format!("replace the line with: {line}")
    })
}

/// The shell a file is written for, from its name: `zsh` in the name,
/// `.fish`, `ksh` in the name, bash otherwise.
fn shell_of(path: &Path) -> &'static str {
    let name = path.to_string_lossy();
    if name.ends_with(".fish") {
        "fish"
    } else if name.contains("zsh") {
        "zsh"
    } else if name.contains("ksh") {
        "ksh"
    } else {
        "bash"
    }
}
```

Create `src/commands/doctor/fix_tests.rs`:

```rust
use std::path::Path;

use super::fix::suggested_patch;
use super::fix_text;
use crate::domain::conflict::Kind;

#[test]
fn a_top_level_loader_and_completion_are_fixed_by_migrate() {
    let advice = "run `nvm migrate` (comments the line out and adds the nvmrc init line)";
    assert_eq!(fix_text(Kind::Loader, 0), advice);
    assert_eq!(fix_text(Kind::Completion, 0), advice);
}

#[test]
fn a_loader_in_a_sourced_file_or_a_lazy_loader_is_fixed_by_hand() {
    let advice = "remove or replace this loader by hand: nvmrc has its own function \
(see `eval \"$(nvmrc init <shell>)\"`)";
    for (kind, depth) in [
        (Kind::Loader, 1),
        (Kind::Completion, 2),
        (Kind::LazyLoader, 0),
        (Kind::LazyLoader, 1),
    ] {
        assert_eq!(fix_text(kind, depth), advice, "{kind:?} at {depth}");
    }
}

#[test]
fn the_other_kinds_have_their_own_advice() {
    let stub = "the stub redefines `nvm` after the init line: delete it, or move the init \
line after it";
    assert_eq!(fix_text(Kind::LazyStub, 1), stub);
    assert_eq!(fix_text(Kind::Unset, 1), stub);
    assert!(fix_text(Kind::OmzPlugin, 0).starts_with("remove `nvm` from `plugins=(...)`; it does"));
    assert_eq!(fix_text(Kind::ZshNvm, 0), "remove zsh-nvm");
    assert_eq!(fix_text(Kind::Bass, 0), "remove the bass line");
    assert!(fix_text(Kind::HelperCall, 0).contains("use `nvm` subcommands"));
    assert_eq!(fix_text(Kind::NvmDirExport, 0), "kept by `nvm migrate`");
}

#[test]
fn only_loaders_fixed_by_hand_get_a_patch_in_the_shell_of_the_file() {
    let patch = |kind, depth, path| suggested_patch(kind, depth, Path::new(path));
    assert_eq!(patch(Kind::Loader, 0, "/h/.bashrc"), None);
    assert_eq!(patch(Kind::LazyStub, 1, "/h/lazy.zsh"), None);
    let by_hand = |shell: &str| {
        Some(format!(
            "replace the line with: eval \"$(nvmrc init {shell})\""
        ))
    };
    assert_eq!(patch(Kind::LazyLoader, 1, "/h/lazy.zsh"), by_hand("zsh"));
    assert_eq!(patch(Kind::Loader, 1, "/h/.kshrc"), by_hand("ksh"));
    assert_eq!(patch(Kind::Loader, 1, "/h/nvm.sh.d/x.sh"), by_hand("bash"));
    assert_eq!(
        patch(Kind::LazyLoader, 1, "/h/nvm.fish"),
        Some("replace the line with: nvmrc init fish | source".to_owned())
    );
}
```

Insert above the `#[cfg(test)]` line of `src/commands/doctor/mod.rs`:

```rust
//! `nvm doctor [--shell <name>]`: reports the startup files that still load
//! nvm.sh or fight nvmrc's `nvm` (plan 8). Read-only: it scans and prints,
//! and never writes a file. Exit status 1 when a conflict exists.

mod fix;
mod render;

#[cfg(test)]
mod fix_tests;
#[cfg(test)]
mod render_tests;
```

Create `src/commands/doctor/render.rs`:

```rust
//! The text of `nvm doctor`: one block per file, the Info section, the verdict.

use std::fmt::Write as _;

use super::fix::{fix_text, suggested_patch};
use crate::commands::conflict::{FileFindings, Report};
use crate::domain::conflict::{Hit, Kind, Severity};

/// Kinds that are told in the Info section, not in a file's block.
fn is_info(kind: Kind) -> bool {
    kind.severity() == Severity::Info
}

/// The whole report as text, without a trailing newline.
pub fn render(report: &Report) -> String {
    let mut sections = vec![format!(
        "nvm doctor: scanned {} file(s)",
        report.files.len()
    )];
    sections.extend(report.files.iter().filter_map(file_block));
    sections.extend(info_section(report));
    sections.push(verdict(report));
    sections.join("\n\n")
}

fn file_block(file: &FileFindings) -> Option<String> {
    let hits: Vec<&Hit> = file.hits.iter().filter(|hit| !is_info(hit.kind)).collect();
    if hits.is_empty() {
        return None;
    }
    let mut block = file.path.display().to_string();
    if file.canonical != file.path {
        let _ = write!(block, " -> {}", file.canonical.display());
    }
    for hit in hits {
        block.push('\n');
        block.push_str(&hit_lines(file, hit));
    }
    Some(block)
}

fn hit_lines(file: &FileFindings, hit: &Hit) -> String {
    let mut lines = format!(
        "  {}: {}  {}\n      fix: {}",
        hit.line,
        hit.kind,
        hit.text.trim(),
        fix_text(hit.kind, file.depth)
    );
    if let Some(patch) = suggested_patch(hit.kind, file.depth, &file.path) {
        let _ = write!(lines, "\n      patch: {patch}");
    }
    lines
}

fn info_section(report: &Report) -> Option<String> {
    let kept = report.files.iter().flat_map(|file| {
        file.hits
            .iter()
            .filter(|hit| hit.kind == Kind::NvmDirExport)
            .map(move |hit| {
                let text = format!("{}: {}", hit.kind, fix_text(hit.kind, file.depth));
                format!("  {}:{}: {text}", file.path.display(), hit.line)
            })
    });
    let notes = report.notes.iter().map(|note| match note.line {
        Some(line) => format!("  {}:{line}: {}", note.path.display(), note.reason),
        None => format!("  {}: {}", note.path.display(), note.reason),
    });
    let lines: Vec<String> = kept.chain(notes).collect();
    (!lines.is_empty()).then(|| format!("Info:\n{}", lines.join("\n")))
}

fn verdict(report: &Report) -> String {
    let conflicts = report.conflicts().count();
    if conflicts == 0 {
        return "Result: no conflicts".to_owned();
    }
    let fixable = report.auto_migratable().count();
    if fixable == 0 {
        format!("Result: {conflicts} conflict(s)")
    } else {
        format!("Result: {conflicts} conflict(s), {fixable} can be fixed with `nvm migrate`")
    }
}
```

Create `src/commands/doctor/render_tests.rs`:

```rust
use std::path::PathBuf;

use super::render::render;
use crate::commands::conflict::{FileFindings, Note, NoteReason, Report};
use crate::domain::conflict::Hit;
use crate::domain::conflict::Kind;

fn file(path: &str, canonical: &str, depth: usize, hits: &[(usize, Kind, &str)]) -> FileFindings {
    FileFindings {
        path: PathBuf::from(path),
        canonical: PathBuf::from(canonical),
        depth,
        hits: hits
            .iter()
            .map(|(line, kind, text)| Hit {
                line: *line,
                kind: *kind,
                text: (*text).to_owned(),
            })
            .collect(),
    }
}

#[test]
fn a_clean_scan_says_no_conflicts_and_has_no_info() {
    let report = Report {
        files: vec![file("/h/.bashrc", "/h/.bashrc", 0, &[])],
        notes: Vec::new(),
    };
    assert_eq!(
        render(&report),
        "nvm doctor: scanned 1 file(s)\n\nResult: no conflicts"
    );
}

#[test]
fn a_hint_alone_is_shown_but_is_not_a_conflict() {
    let report = Report {
        files: vec![file(
            "/h/.zshrc",
            "/h/.zshrc",
            0,
            &[(3, Kind::OmzPlugin, "    nvm")],
        )],
        notes: Vec::new(),
    };
    let text = render(&report);
    assert!(text.contains("  3: oh-my-zsh nvm plugin  nvm\n      fix: remove `nvm` from"));
    assert!(text.ends_with("Result: no conflicts"));
}

#[test]
fn a_manual_loader_gets_a_patch_line_and_no_migrate_count() {
    let report = Report {
        files: vec![file(
            "/h/lazy.zsh",
            "/h/lazy.zsh",
            1,
            &[(3, Kind::LazyLoader, "  . \"$P/nvm.sh\"")],
        )],
        notes: Vec::new(),
    };
    let text = render(&report);
    assert!(text.contains("      patch: replace the line with: eval \"$(nvmrc init zsh)\""));
    assert!(text.ends_with("Result: 1 conflict(s)"));
}

#[test]
fn info_lists_the_kept_exports_then_the_notes() {
    let report = Report {
        files: vec![file(
            "/h/.zshenv",
            "/h/.zshenv",
            0,
            &[
                (1, Kind::NvmDirExport, "export NVM_DIR=x"),
                (2, Kind::NotFollowed, "source $X"),
            ],
        )],
        notes: vec![
            Note {
                path: PathBuf::from("/h/.zshenv"),
                line: Some(2),
                text: "source $X".to_owned(),
                reason: NoteReason::Unresolvable("unset $X".to_owned()),
            },
            Note {
                path: PathBuf::from("/h/gone"),
                line: None,
                text: String::new(),
                reason: NoteReason::Unreadable("denied".to_owned()),
            },
        ],
    };
    assert_eq!(
        render(&report),
        "nvm doctor: scanned 1 file(s)\n\n\
Info:\n  /h/.zshenv:1: NVM_DIR export: kept by `nvm migrate`\n  \
/h/.zshenv:2: not followed: unset $X\n  /h/gone: not read: denied\n\n\
Result: no conflicts"
    );
}

#[test]
fn note_reasons_have_their_text() {
    let path = PathBuf::from("/h/x");
    assert_eq!(
        NoteReason::Relative(path.clone()).to_string(),
        "relative path resolved from the file's directory: /h/x"
    );
    assert_eq!(
        NoteReason::Missing(path.clone()).to_string(),
        "file not found: /h/x"
    );
    assert_eq!(
        NoteReason::TooDeep(path).to_string(),
        "not scanned: deeper than 2 levels"
    );
}
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1262 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(doctor): report the shell files that still load nvm.sh"
```

---

### Task 5: Planning a migration

**Files:**

- Create: `src/domain/migration/backup.rs`,
  `src/domain/migration/backup_tests.rs`, `src/domain/migration/diff.rs`,
  `src/domain/migration/diff_tests.rs`, `src/domain/migration/mod.rs`,
  `src/domain/migration/tests.rs`
- Modify: `src/domain/mod.rs`, `src/shell/init/mod.rs`,
  `src/shell/init/tests.rs`

**Interfaces:**

- Produces: `domain::migration::{load_line, init_block, Migration, BlockChange,
  migrate_text, unified_diff, backup_name, latest_backup, syntax_check}`;
  `Shell::load_line` is public and now returns `eval "$(nvmrc init <shell>)"`
  (fish `nvmrc init fish | source`).
- Behaviour: `migrate_text(text, shell)` turns every top-level Loader and
  Completion line into `# [nvmrc-migrated] <original line, indentation kept>`
  (never deleted: install.sh's `grep /nvm.sh` guard then does not re-add them;
  `NVM_DIR` exports stay) and inserts the init block before the FIRST migrated
  line, or replaces an existing block (same markers; the first complete pair
  only), or leaves a file with neither unchanged; it is idempotent (the second
  run is `Unchanged` and the re-scanned text has no auto-migratable hit), keeps
  the file's line endings (CRLF included) and its final-newline state, and never
  touches a line inside a block or already migrated. `unified_diff` is a
  GNU-style diff with 3 lines of context (a small LCS, one whole-file hunk above
  20000 lines, a memory guard for the middle). `backup_name` is
  `<file>.nvmrc-backup- <yyyymmddThhmmssZ>` and `latest_backup` picks the
  greatest stamp of that exact pattern. `syntax_check(name)` names the program
  that checks a candidate: zsh files (`.zprofile`, `.zlogin` included) `zsh -n`,
  bash `bash -n`, `.fish` `fish --no-execute`, ksh `ksh -n`, anything else `sh
  -n`; the caller appends the temp path.
- [ ] **Step 1: Declare the new modules**

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -12,6 +12,7 @@
 pub mod implicit;
 pub mod index;
 pub mod listing;
+pub mod migration;
 pub mod mirror;
 pub mod npm;
 pub mod nvmrc;
```

- [ ] **Step 2: Write the failing tests**

Create `src/domain/migration/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;

pub use backup::{backup_name, latest_backup};
pub use diff::unified_diff;

use crate::domain::conflict::{BEGIN_MARKER, END_MARKER, MIGRATED_PREFIX, scan_text};
use crate::shell::init::Shell;

/// The planned edit of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    /// The whole new content (equal to the input when nothing changes).
    pub text: String,
    /// The lines of the ORIGINAL file turned into comments, from 1.
    pub migrated_lines: Vec<usize>,
    /// What happened to the init block.
    pub block: BlockChange,
}

/// What [`migrate_text`] did with the init block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockChange {
    /// Added before line `before_line` of the original file (from 1).
    Inserted { before_line: usize },
    /// An existing block (same markers) was rewritten with the current one.
    Replaced,
    /// No block was added, or the existing one is already current.
    Unchanged,
}

/// The line of the init block that loads nvmrc ([`Shell::load_line`]): it
/// runs the `nvmrc` binary, never the `nvm` function (digest finding 0.1).
#[must_use]
pub fn load_line(shell: Shell) -> String {
    shell.load_line()
}

/// The init block `migrate` writes: the begin marker, [`load_line`] and the
/// end marker, each ending in `\n`.
#[must_use]
pub fn init_block(shell: Shell) -> String {
    format!("{BEGIN_MARKER}\n{}\n{END_MARKER}\n", load_line(shell))
}

/// The migration of the startup file `text` for `shell` (see the module
/// documentation). The original line endings are kept: a file without final
/// newline stays without, and the block's lines take the file's line ending
/// (`\r\n` when its first line ends so). A file with nothing to migrate and no
/// block is returned as is ([`BlockChange::Unchanged`]); a second run over
/// the result changes nothing.
#[must_use]
pub fn migrate_text(text: &str, shell: Shell) -> Migration {
    let plan = Plan::new(text, shell);
    let new_text = plan.render();
    if new_text == text {
        return Migration {
            text: new_text,
            migrated_lines: Vec::new(),
            block: BlockChange::Unchanged,
        };
    }
    Migration {
        text: new_text,
        block: plan.block_change(),
        migrated_lines: plan.targets,
    }
}

/// What [`migrate_text`] found in the file and will write.
struct Plan<'a> {
    /// The lines of the file as `(content, line ending)`.
    lines: Vec<(&'a str, &'a str)>,
    /// The lines to comment out, from 1.
    targets: Vec<usize>,
    /// The 0-based lines of the block already in the file.
    existing: Option<(usize, usize)>,
    /// The line (from 1) the new block goes before, when none exists.
    anchor: Option<usize>,
    /// The current block's lines, without endings.
    block: [String; 3],
    /// The line ending the block's lines take.
    ending: &'a str,
}

impl<'a> Plan<'a> {
    fn new(text: &'a str, shell: Shell) -> Self {
        let lines = split_lines(text);
        let targets = migratable_lines(text);
        let existing = existing_block(&lines);
        Self {
            anchor: existing
                .is_none()
                .then(|| targets.first().copied())
                .flatten(),
            ending: file_ending(&lines),
            block: [
                BEGIN_MARKER.to_owned(),
                load_line(shell),
                END_MARKER.to_owned(),
            ],
            lines,
            targets,
            existing,
        }
    }

    /// The new text of the file.
    fn render(&self) -> String {
        let mut text = String::new();
        for (index, line) in self.lines.iter().enumerate() {
            if self.anchor == Some(index + 1) {
                self.push_block(&mut text, self.ending);
            }
            match self.existing {
                Some((begin, end)) if index == begin => {
                    self.push_block(&mut text, self.lines[end].1)
                }
                Some((begin, end)) if (begin..=end).contains(&index) => {}
                _ if self.targets.contains(&(index + 1)) => push_migrated(&mut text, *line),
                _ => push_line(&mut text, line.0, line.1),
            }
        }
        text
    }

    /// The block, its last line ending in `last_ending`.
    fn push_block(&self, text: &mut String, last_ending: &str) {
        push_line(text, &self.block[0], self.ending);
        push_line(text, &self.block[1], self.ending);
        push_line(text, &self.block[2], last_ending);
    }

    /// What happens to the block (the text does change).
    fn block_change(&self) -> BlockChange {
        match (self.anchor, self.existing) {
            (Some(before_line), _) => BlockChange::Inserted { before_line },
            (None, Some((begin, end)))
                if self.lines[begin..=end]
                    .iter()
                    .map(|line| line.0)
                    .ne(self.block.iter().map(String::as_str)) =>
            {
                BlockChange::Replaced
            }
            _ => BlockChange::Unchanged,
        }
    }
}

/// The line numbers (from 1) of the auto-migratable hits, each once.
fn migratable_lines(text: &str) -> Vec<usize> {
    let mut lines: Vec<usize> = scan_text(text)
        .into_iter()
        .filter(|hit| hit.kind.is_auto_migratable())
        .map(|hit| hit.line)
        .collect();
    lines.dedup();
    lines
}

/// Each line of `text` as `(content, line ending)`; the ending is `\r\n`,
/// `\n`, or empty for a last line without newline.
fn split_lines(text: &str) -> Vec<(&str, &str)> {
    text.split_inclusive('\n')
        .map(|segment| {
            let content = segment
                .strip_suffix("\r\n")
                .or_else(|| segment.strip_suffix('\n'))
                .unwrap_or(segment);
            (content, &segment[content.len()..])
        })
        .collect()
}

/// The line ending of the file: the first one, `\n` when there is none.
fn file_ending<'a>(lines: &[(&str, &'a str)]) -> &'a str {
    lines
        .iter()
        .map(|line| line.1)
        .find(|ending| !ending.is_empty())
        .unwrap_or("\n")
}

/// The 0-based lines of the first complete init block (markers included).
fn existing_block(lines: &[(&str, &str)]) -> Option<(usize, usize)> {
    let begin = lines
        .iter()
        .position(|line| line.0.trim() == BEGIN_MARKER)?;
    let length = lines[begin..]
        .iter()
        .position(|line| line.0.trim() == END_MARKER)?;
    Some((begin, begin + length))
}

/// `line` commented out by `migrate`, indentation kept after the prefix.
fn push_migrated(text: &mut String, line: (&str, &str)) {
    text.push_str(MIGRATED_PREFIX);
    text.push(' ');
    push_line(text, line.0, line.1);
}

fn push_line(text: &mut String, content: &str, ending: &str) {
    text.push_str(content);
    text.push_str(ending);
}

/// The program (and its arguments) that syntax-checks a candidate for the
/// startup file `path`, chosen by its file name: `zsh` in the name (or
/// `.zprofile`, `.zlogin`, `.zlogout`) `zsh -n`; `bash` `bash -n`; `.fish`
/// `fish --no-execute`; `ksh` (`.kshrc`) `ksh -n`; any other name (`.profile`,
/// an `$ENV` file) `sh -n`. `None` when `path` names no file. The caller
/// appends the path of the temp file to the arguments.
#[must_use]
pub fn syntax_check(path: &str) -> Option<(&'static str, Vec<&'static str>)> {
    const ZSH_FILES: [&str; 3] = [".zprofile", ".zlogin", ".zlogout"];
    let name = path.rsplit('/').next().unwrap_or_default();
    let (program, flag) = if name.is_empty() {
        return None;
    } else if name.contains("zsh") || ZSH_FILES.contains(&name) {
        ("zsh", "-n")
    } else if name.contains("bash") {
        ("bash", "-n")
    } else if name.ends_with(".fish") {
        ("fish", "--no-execute")
    } else if name.contains("ksh") {
        ("ksh", "-n")
    } else {
        ("sh", "-n")
    };
    Some((program, vec![flag]))
}
```

Create `src/domain/migration/tests.rs`:

```rust
use super::{BlockChange, Migration, init_block, load_line, migrate_text, syntax_check};
use crate::domain::conflict::scan_text;
use crate::shell::init::{SHELLS, Shell};

const LOADER: &str = "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm";
const COMPLETION: &str = "[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion";
const BREW_LOADER: &str = "[ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ] && \\. \"/opt/homebrew/opt/nvm/nvm.sh\"  # This loads nvm";
const BREW_COMPLETION: &str = "[ -s \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\"  # This loads nvm bash_completion";
const ZSH_BLOCK: &str = "# >>> nvmrc init >>>\neval \"$(nvmrc init zsh)\"\n# <<< nvmrc init <<<\n";
const BASH_BLOCK: &str =
    "# >>> nvmrc init >>>\neval \"$(nvmrc init bash)\"\n# <<< nvmrc init <<<\n";

/// install.sh's appended bytes (digest 1.2) after a PATH line.
fn install_sh_bashrc() -> String {
    format!(
        "export PATH=\"$HOME/bin:$PATH\"\n\nexport NVM_DIR=\"$HOME/.nvm\"\n{LOADER}\n{COMPLETION}\n"
    )
}

/// Every fixture the properties run over, with its shell.
fn fixtures() -> Vec<(String, Shell)> {
    vec![
        (install_sh_bashrc(), Shell::Bash),
        (
            format!("export NVM_DIR=\"$HOME/.nvm\"\n{BREW_LOADER}\n{BREW_COMPLETION}\n"),
            Shell::Zsh,
        ),
        (format!("{LOADER}\nalias ll='ls -l'\n"), Shell::Bash),
        (format!("alias ll='ls -l'\n{LOADER}"), Shell::Bash),
        (users_zshrc(), Shell::Zsh),
        (users_style_loader(), Shell::Zsh),
        (install_sh_bashrc().replace('\n', "\r\n"), Shell::Bash),
        (format!("a\n{ZSH_BLOCK}b\n{LOADER}\n"), Shell::Zsh),
        (
            format!("if true; then\n  {LOADER}\nfi\nnvm() {{ :; }}\n"),
            Shell::Bash,
        ),
        (format!("  {LOADER}\n"), Shell::Bash),
        (
            format!("# [nvmrc-migrated] {LOADER}\n{ZSH_BLOCK}"),
            Shell::Zsh,
        ),
        ("source ~/.nvm/nvm.sh\n".to_owned(), Shell::Fish),
        (String::new(), Shell::Ksh),
    ]
}

/// The nvm-relevant lines of the user's `.zshrc` (digest 3.4): nothing to
/// migrate automatically.
fn users_zshrc() -> String {
    "plugins=(\n    node\n    nvm\n)\nsource $ZSH/oh-my-zsh.sh\nsource ${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh\n\
[[ ! -d \"${NVM_DIR}\" ]] && mkdir \"${NVM_DIR}\"\n"
        .to_owned()
}

/// The user's loader lines (digest 3.4, `lazy-functions.zsh:17-18`) written
/// at the top level.
fn users_style_loader() -> String {
    "export NVM_DIR=\"${HOME}/.nvm\"\n\
[ -s \"${HOMEBREW_PREFIX}/opt/nvm/nvm.sh\" ] && \\. \"${HOMEBREW_PREFIX}/opt/nvm/nvm.sh\"\n\
[ -s \"${HOMEBREW_PREFIX}/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"${HOMEBREW_PREFIX}/opt/nvm/etc/bash_completion.d/nvm\"\n"
        .to_owned()
}

fn migrated(line: &str) -> String {
    format!("# [nvmrc-migrated] {line}")
}

#[test]
fn the_load_line_runs_the_nvmrc_binary_never_the_nvm_function() {
    let expected = [
        (Shell::Bash, "eval \"$(nvmrc init bash)\""),
        (Shell::Zsh, "eval \"$(nvmrc init zsh)\""),
        (Shell::Sh, "eval \"$(nvmrc init sh)\""),
        (Shell::Dash, "eval \"$(nvmrc init dash)\""),
        (Shell::Ksh, "eval \"$(nvmrc init ksh)\""),
        (Shell::Fish, "nvmrc init fish | source"),
    ];
    for (shell, line) in expected {
        assert_eq!(load_line(shell), line);
        assert_eq!(
            init_block(shell),
            format!("# >>> nvmrc init >>>\n{line}\n# <<< nvmrc init <<<\n")
        );
    }
}

#[test]
fn the_install_sh_pair_becomes_comments_under_one_block() {
    let result = migrate_text(&install_sh_bashrc(), Shell::Bash);
    let expected = format!(
        "export PATH=\"$HOME/bin:$PATH\"\n\nexport NVM_DIR=\"$HOME/.nvm\"\n{BASH_BLOCK}{}\n{}\n",
        migrated(LOADER),
        migrated(COMPLETION)
    );
    assert_eq!(
        result,
        Migration {
            text: expected,
            migrated_lines: vec![4, 5],
            block: BlockChange::Inserted { before_line: 4 },
        }
    );
}

#[test]
fn homebrew_lines_are_migrated_and_the_nvm_dir_export_stays() {
    let text = format!("export NVM_DIR=\"$HOME/.nvm\"\n{BREW_LOADER}\n{BREW_COMPLETION}\n");
    let result = migrate_text(&text, Shell::Zsh);
    let expected = format!(
        "export NVM_DIR=\"$HOME/.nvm\"\n{ZSH_BLOCK}{}\n{}\n",
        migrated(BREW_LOADER),
        migrated(BREW_COMPLETION)
    );
    assert_eq!(result.text, expected);
    assert_eq!(result.migrated_lines, [2, 3]);
}

#[test]
fn the_block_goes_before_a_loader_on_the_first_line() {
    let result = migrate_text(&format!("{LOADER}\nalias ll='ls -l'\n"), Shell::Bash);
    assert_eq!(
        result.text,
        format!("{BASH_BLOCK}{}\nalias ll='ls -l'\n", migrated(LOADER))
    );
    assert_eq!(result.block, BlockChange::Inserted { before_line: 1 });
}

#[test]
fn a_last_line_without_newline_stays_without() {
    let result = migrate_text(&format!("alias ll='ls -l'\n{LOADER}"), Shell::Bash);
    assert_eq!(
        result.text,
        format!("alias ll='ls -l'\n{BASH_BLOCK}{}", migrated(LOADER))
    );
    assert_eq!(result.migrated_lines, [2]);
}

#[test]
fn the_users_top_level_style_is_migrated() {
    let result = migrate_text(&users_style_loader(), Shell::Zsh);
    assert_eq!(result.migrated_lines, [2, 3]);
    assert!(
        result
            .text
            .starts_with(&format!("export NVM_DIR=\"${{HOME}}/.nvm\"\n{ZSH_BLOCK}"))
    );
}

#[test]
fn a_file_without_loaders_is_unchanged() {
    for text in [
        users_zshrc(),
        String::new(),
        "export NVM_DIR=\"$HOME/.nvm\"\n".to_owned(),
    ] {
        let result = migrate_text(&text, Shell::Zsh);
        assert_eq!(
            result,
            Migration {
                text: text.clone(),
                migrated_lines: Vec::new(),
                block: BlockChange::Unchanged,
            }
        );
    }
}

#[test]
fn manual_kinds_are_never_touched() {
    let lazy = format!("if true; then\n  {LOADER}\nfi\nnvm() {{ :; }}\nsource ~/.nvm/nvm.sh\n");
    let result = migrate_text(&lazy, Shell::Bash);
    assert_eq!(result.text, lazy);
    assert_eq!(result.block, BlockChange::Unchanged);
}

#[test]
fn an_existing_block_is_replaced_and_no_second_block_is_added() {
    let pasted = "# >>> nvmrc init >>>\nnvm() { old; }\n# <<< nvmrc init <<<\n";
    let text = format!("a\n{pasted}b\n{LOADER}\n");
    let result = migrate_text(&text, Shell::Zsh);
    assert_eq!(
        result.text,
        format!("a\n{ZSH_BLOCK}b\n{}\n", migrated(LOADER))
    );
    assert_eq!(result.migrated_lines, [6]);
    assert_eq!(result.block, BlockChange::Replaced);
}

#[test]
fn a_current_block_with_nothing_to_migrate_is_unchanged() {
    let text = format!("a\n{ZSH_BLOCK}b");
    let result = migrate_text(&text, Shell::Zsh);
    assert_eq!(result.text, text);
    assert_eq!(result.block, BlockChange::Unchanged);
}

#[test]
fn a_block_at_the_end_without_newline_keeps_that_state() {
    let text = "a\n# >>> nvmrc init >>>\nold\n# <<< nvmrc init <<<";
    let result = migrate_text(text, Shell::Fish);
    assert_eq!(
        result.text,
        "a\n# >>> nvmrc init >>>\nnvmrc init fish | source\n# <<< nvmrc init <<<"
    );
}

#[test]
fn indentation_is_kept_after_the_prefix() {
    let result = migrate_text(&format!("  {LOADER}\n"), Shell::Bash);
    assert_eq!(
        result.text,
        format!("{BASH_BLOCK}# [nvmrc-migrated]   {LOADER}\n")
    );
}

#[test]
fn crlf_files_stay_crlf() {
    let text = install_sh_bashrc().replace('\n', "\r\n");
    let result = migrate_text(&text, Shell::Bash);
    let expected = format!(
        "export PATH=\"$HOME/bin:$PATH\"\r\n\r\nexport NVM_DIR=\"$HOME/.nvm\"\r\n{}{}\r\n{}\r\n",
        BASH_BLOCK.replace('\n', "\r\n"),
        migrated(LOADER),
        migrated(COMPLETION)
    );
    assert_eq!(result.text, expected);
}

#[test]
fn migrated_lines_and_lines_inside_a_block_are_left_alone() {
    let block_with_loader = format!("# >>> nvmrc init >>>\n{LOADER}\n# <<< nvmrc init <<<\n");
    let already = format!("{}\n", migrated(LOADER));
    for text in [already.clone(), format!("{already}{ZSH_BLOCK}")] {
        assert_eq!(migrate_text(&text, Shell::Zsh).text, text);
    }
    let result = migrate_text(&block_with_loader, Shell::Zsh);
    assert_eq!(result.text, ZSH_BLOCK);
    assert!(result.migrated_lines.is_empty());
}

#[test]
fn migrating_twice_changes_nothing_the_second_time() {
    for shell in SHELLS {
        for (text, _) in fixtures() {
            let first = migrate_text(&text, shell);
            let second = migrate_text(&first.text, shell);
            assert_eq!(second.text, first.text, "{text:?}");
            assert_eq!(second.block, BlockChange::Unchanged, "{text:?}");
            assert!(second.migrated_lines.is_empty(), "{text:?}");
        }
    }
}

#[test]
fn the_migrated_text_has_no_auto_migratable_hit_left() {
    for (text, shell) in fixtures() {
        let result = migrate_text(&text, shell);
        let left: Vec<_> = scan_text(&result.text)
            .into_iter()
            .filter(|hit| hit.kind.is_auto_migratable())
            .collect();
        assert!(left.is_empty(), "{text:?} -> {left:?}");
    }
}

#[test]
fn the_syntax_checker_follows_the_file_name() {
    let cases = [
        ("/home/u/.zshrc", Some(("zsh", vec!["-n"]))),
        ("/home/u/dotfiles/zsh/.zprofile", Some(("zsh", vec!["-n"]))),
        ("/home/u/.bashrc", Some(("bash", vec!["-n"]))),
        ("/home/u/.bash_profile", Some(("bash", vec!["-n"]))),
        (
            "/home/u/.config/fish/config.fish",
            Some(("fish", vec!["--no-execute"])),
        ),
        (
            "/home/u/.config/fish/conf.d/nvm.fish",
            Some(("fish", vec!["--no-execute"])),
        ),
        ("/home/u/.kshrc", Some(("ksh", vec!["-n"]))),
        ("/home/u/.mkshrc", Some(("ksh", vec!["-n"]))),
        ("/home/u/.profile", Some(("sh", vec!["-n"]))),
        ("/home/u/zsh/.profile", Some(("sh", vec!["-n"]))),
        ("/home/u/.shinit", Some(("sh", vec!["-n"]))),
        ("", None),
        ("/home/u/", None),
    ];
    for (path, expected) in cases {
        assert_eq!(syntax_check(path), expected, "{path}");
    }
}
```

Apply to `src/shell/init/tests.rs`:

```diff
--- a/src/shell/init/tests.rs
+++ b/src/shell/init/tests.rs
@@ -70,10 +70,11 @@
     for (name, shell) in ALL {
         let text = snippet(shell, &InitOptions::default());
         assert!(text.is_ascii());
-        assert!(text.contains(&format!("nvm init {name}")), "{text}");
+        assert!(text.contains(&format!("`{}`", shell.load_line())), "{text}");
+        assert!(text.contains(&format!("nvmrc init {name}")), "{text}");
     }
     let fish = snippet(Shell::Fish, &InitOptions::default());
-    assert!(fish.contains("`nvm init fish | source`"), "{fish}");
+    assert!(fish.contains("`nvmrc init fish | source`"), "{fish}");
 }
 
 #[test]
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find module `migration`
in `domain`", "cannot find function `migrate_text`" and "no function or
associated item named `load_line` found".

- [ ] **Step 4: Write the implementation**

Create `src/domain/migration/backup.rs`:

```rust
//! The names of the copies `migrate` keeps before it edits a file.

use crate::domain::timestamp::compact_utc;

/// What separates the file name from the timestamp.
const BACKUP_INFIX: &str = ".nvmrc-backup-";

/// The backup of `file_name` taken at `unix_seconds`:
/// `<file_name>.nvmrc-backup-<yyyymmddThhmmssZ>` (UTC).
#[must_use]
pub fn backup_name(file_name: &str, unix_seconds: i64) -> String {
    format!("{file_name}{BACKUP_INFIX}{}", compact_utc(unix_seconds))
}

/// The newest backup of `file_name` among the file names `candidates`: the
/// greatest timestamp among the names of exactly the [`backup_name`]
/// pattern (other files and malformed timestamps are ignored). The names
/// share their prefix, so the greatest name holds the greatest timestamp.
#[must_use]
pub fn latest_backup<'a>(file_name: &str, candidates: &'a [String]) -> Option<&'a str> {
    candidates
        .iter()
        .map(String::as_str)
        .filter(|candidate| {
            candidate
                .strip_prefix(file_name)
                .and_then(|rest| rest.strip_prefix(BACKUP_INFIX))
                .is_some_and(is_compact_utc)
        })
        .max()
}

/// Whether `stamp` has the shape of [`compact_utc`]: `yyyymmddThhmmssZ`.
fn is_compact_utc(stamp: &str) -> bool {
    let bytes = stamp.as_bytes();
    bytes.len() == 16
        && bytes[..8].iter().all(u8::is_ascii_digit)
        && bytes[8] == b'T'
        && bytes[9..15].iter().all(u8::is_ascii_digit)
        && bytes[15] == b'Z'
}
```

Create `src/domain/migration/backup_tests.rs`:

```rust
use super::{backup_name, latest_backup};

#[test]
fn the_backup_name_carries_a_compact_utc_stamp() {
    assert_eq!(
        backup_name(".zshrc", 1_791_110_040),
        ".zshrc.nvmrc-backup-20261004T103400Z"
    );
    assert_eq!(
        backup_name(".bashrc", 0),
        ".bashrc.nvmrc-backup-19700101T000000Z"
    );
}

#[test]
fn the_latest_backup_is_the_greatest_stamp_of_that_file() {
    let candidates: Vec<String> = [
        ".zshrc",
        ".zshrc.nvmrc-backup-20261004T103400Z",
        ".zshrc.nvmrc-backup-20261005T000000Z",
        ".zshrc.nvmrc-backup-20250101T000000Z",
        ".zshrc.nvmrc-backup-20991231T235959",
        ".zshrc.nvmrc-backup-2099123IT235959Z",
        ".zshrc.nvmrc-backup-20991231T235959Z.old",
        ".zshrc.nvmrc-backup-",
        ".zshrc2.nvmrc-backup-20991231T235959Z",
        "x.zshrc.nvmrc-backup-20991231T235959Z",
        ".bashrc.nvmrc-backup-20991231T235959Z",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(
        latest_backup(".zshrc", &candidates),
        Some(".zshrc.nvmrc-backup-20261005T000000Z")
    );
    assert_eq!(
        latest_backup(".bashrc", &candidates),
        Some(".bashrc.nvmrc-backup-20991231T235959Z")
    );
    assert_eq!(latest_backup(".profile", &candidates), None);
}

#[test]
fn equal_stamps_pick_the_lexicographically_greater_name() {
    let name = ".zshrc.nvmrc-backup-20261004T103400Z".to_owned();
    let candidates = vec![name.clone(), name.clone()];
    assert_eq!(latest_backup(".zshrc", &candidates), Some(name.as_str()));
    assert_eq!(latest_backup(".zshrc", &[]), None);
}
```

Create `src/domain/migration/diff.rs`:

```rust
//! A unified diff (`diff -u` format) of two texts, line by line.
//!
//! The common head and tail are matched first; what lies between them is
//! aligned with a longest-common-subsequence table. Two guards keep the cost
//! bounded: a file of more than [`MAX_LINES`] lines is shown as one
//! whole-file hunk (every old line removed, every new line added), and a
//! middle whose table would exceed [`MAX_CELLS`] cells is shown as removed
//! then added (still a correct diff, just not a minimal one).

use std::ops::Range;

/// The lines of context around each change.
const CONTEXT: usize = 3;
/// Above this many lines (either side), one whole-file hunk.
const MAX_LINES: usize = 20_000;
/// The largest LCS table built (4 bytes a cell: 16 MB).
const MAX_CELLS: usize = 4_000_000;

/// What a step of the edit script does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Change {
    Same,
    Removed,
    Added,
}

/// One step, with the 0-based positions in both texts before it.
#[derive(Debug, Clone, Copy)]
struct Step {
    change: Change,
    old_index: usize,
    new_index: usize,
}

/// The edit script being built, with the current positions.
#[derive(Debug, Default)]
struct Script {
    steps: Vec<Step>,
    old_index: usize,
    new_index: usize,
}

impl Script {
    fn push(&mut self, change: Change, times: usize) {
        for _ in 0..times {
            self.steps.push(Step {
                change,
                old_index: self.old_index,
                new_index: self.new_index,
            });
            self.old_index += usize::from(change != Change::Added);
            self.new_index += usize::from(change != Change::Removed);
        }
    }
}

/// The unified diff from `old` to `new`, both shown as `path` (`--- a/<path>`,
/// `+++ b/<path>`), with 3 lines of context and the hunks merged when their
/// contexts touch; a line without final newline is followed by
/// `\ No newline at end of file`. Empty when the texts are equal. A range of
/// one line is written without its count (`@@ -1 +1,2 @@`), an empty range
/// as the line before it with count 0 (`-0,0`), as GNU diff does.
#[must_use]
pub fn unified_diff(path: &str, old: &str, new: &str) -> String {
    if old == new {
        return String::new();
    }
    let old_lines: Vec<&str> = old.split_inclusive('\n').collect();
    let new_lines: Vec<&str> = new.split_inclusive('\n').collect();
    let mut script = Script::default();
    if old_lines.len() > MAX_LINES || new_lines.len() > MAX_LINES {
        script.push(Change::Removed, old_lines.len());
        script.push(Change::Added, new_lines.len());
    } else {
        edit_script(&old_lines, &new_lines, &mut script);
    }
    let mut diff = format!("--- a/{path}\n+++ b/{path}\n");
    for range in hunks(&script.steps) {
        render_hunk(&mut diff, &script.steps[range], &old_lines, &new_lines);
    }
    diff
}

/// The steps from `old` to `new`: common head, aligned middle, common tail.
fn edit_script(old: &[&str], new: &[&str], script: &mut Script) {
    let head = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let tail = old[head..]
        .iter()
        .rev()
        .zip(new[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    script.push(Change::Same, head);
    let old_middle = &old[head..old.len() - tail];
    let new_middle = &new[head..new.len() - tail];
    if old_middle.len().saturating_mul(new_middle.len()) > MAX_CELLS {
        script.push(Change::Removed, old_middle.len());
        script.push(Change::Added, new_middle.len());
    } else {
        align(old_middle, new_middle, script);
    }
    script.push(Change::Same, tail);
}

/// The steps from `old` to `new` along a longest common subsequence,
/// removals before additions.
fn align(old: &[&str], new: &[&str], script: &mut Script) {
    let width = new.len() + 1;
    let mut table = vec![0_u32; (old.len() + 1) * width];
    for row in (0..old.len()).rev() {
        for column in (0..new.len()).rev() {
            table[row * width + column] = if old[row] == new[column] {
                table[(row + 1) * width + column + 1] + 1
            } else {
                table[(row + 1) * width + column].max(table[row * width + column + 1])
            };
        }
    }
    let (mut row, mut column) = (0, 0);
    while row < old.len() && column < new.len() {
        let change = if old[row] == new[column] {
            Change::Same
        } else if table[(row + 1) * width + column] >= table[row * width + column + 1] {
            Change::Removed
        } else {
            Change::Added
        };
        script.push(change, 1);
        row += usize::from(change != Change::Added);
        column += usize::from(change != Change::Removed);
    }
    script.push(Change::Removed, old.len() - row);
    script.push(Change::Added, new.len() - column);
}

/// The step ranges of the hunks: each run of changes with its context, runs
/// at most `2 * CONTEXT` unchanged lines apart sharing one hunk.
fn hunks(steps: &[Step]) -> Vec<Range<usize>> {
    let mut runs: Vec<Range<usize>> = Vec::new();
    let changes = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| step.change != Change::Same);
    for (index, _) in changes {
        match runs.last_mut() {
            Some(run) if index - run.end <= 2 * CONTEXT => run.end = index + 1,
            _ => runs.push(index..index + 1),
        }
    }
    runs.into_iter()
        .map(|run| run.start.saturating_sub(CONTEXT)..(run.end + CONTEXT).min(steps.len()))
        .collect()
}

/// Appends the hunk of `steps` (header and lines) to `diff`.
fn render_hunk(diff: &mut String, steps: &[Step], old: &[&str], new: &[&str]) {
    let Some(first) = steps.first() else {
        return;
    };
    let count = |kept: Change| steps.iter().filter(|step| step.change != kept).count();
    diff.push_str(&format!(
        "@@ -{} +{} @@\n",
        hunk_range(first.old_index, count(Change::Added)),
        hunk_range(first.new_index, count(Change::Removed))
    ));
    for step in steps {
        let (marker, line) = match step.change {
            Change::Same => (' ', old[step.old_index]),
            Change::Removed => ('-', old[step.old_index]),
            Change::Added => ('+', new[step.new_index]),
        };
        diff.push(marker);
        diff.push_str(line);
        if !line.ends_with('\n') {
            diff.push_str("\n\\ No newline at end of file\n");
        }
    }
}

/// `start,count` of a hunk header (`start` 0-based, shown 1-based).
fn hunk_range(start: usize, count: usize) -> String {
    match count {
        0 => format!("{start},0"),
        1 => format!("{}", start + 1),
        _ => format!("{},{count}", start + 1),
    }
}
```

Create `src/domain/migration/diff_tests.rs`:

```rust
use super::unified_diff;

/// `count` numbered lines, `1\n2\n...`.
fn numbered(count: usize) -> String {
    (1..=count).map(|number| format!("{number}\n")).collect()
}

#[test]
fn equal_texts_have_no_diff() {
    assert_eq!(unified_diff(".zshrc", "a\nb\n", "a\nb\n"), "");
    assert_eq!(unified_diff(".zshrc", "", ""), "");
}

#[test]
fn an_insertion_shows_three_lines_of_context() {
    let diff = unified_diff(
        ".bashrc",
        "a\nb\nc\nd\ne\nf\ng\n",
        "a\nb\nc\nd\nX\ne\nf\ng\n",
    );
    assert_eq!(
        diff,
        "--- a/.bashrc\n+++ b/.bashrc\n@@ -2,6 +2,7 @@\n b\n c\n d\n+X\n e\n f\n g\n"
    );
}

#[test]
fn a_replaced_line_is_a_deletion_then_an_insertion() {
    let diff = unified_diff("p", "1\n2\n3\n", "1\nTWO\n3\n");
    assert_eq!(
        diff,
        "--- a/p\n+++ b/p\n@@ -1,3 +1,3 @@\n 1\n-2\n+TWO\n 3\n"
    );
}

#[test]
fn distant_changes_make_two_hunks() {
    let old = numbered(20);
    let new = old.replace("\n2\n", "\nB\n").replace("\n18\n", "\nR\n");
    assert_eq!(
        unified_diff("p", &old, &new),
        "--- a/p\n+++ b/p\n\
@@ -1,5 +1,5 @@\n 1\n-2\n+B\n 3\n 4\n 5\n\
@@ -15,6 +15,6 @@\n 15\n 16\n 17\n-18\n+R\n 19\n 20\n"
    );
}

#[test]
fn changes_whose_contexts_touch_share_one_hunk() {
    let old = numbered(20);
    let new = old.replace("\n2\n", "\nB\n").replace("\n9\n", "\nN\n");
    let diff = unified_diff("p", &old, &new);
    assert!(
        diff.starts_with("--- a/p\n+++ b/p\n@@ -1,12 +1,12 @@\n"),
        "{diff}"
    );
    assert_eq!(diff.matches("@@ -").count(), 1);
}

#[test]
fn a_missing_final_newline_is_marked() {
    assert_eq!(
        unified_diff("p", "a\nb", "a\nc"),
        "--- a/p\n+++ b/p\n@@ -1,2 +1,2 @@\n a\n-b\n\\ No newline at end of file\n+c\n\\ No newline at end of file\n"
    );
    assert_eq!(
        unified_diff("p", "a", "a\n"),
        "--- a/p\n+++ b/p\n@@ -1 +1 @@\n-a\n\\ No newline at end of file\n+a\n"
    );
}

#[test]
fn an_empty_side_starts_at_line_zero() {
    assert_eq!(
        unified_diff("p", "", "a\n"),
        "--- a/p\n+++ b/p\n@@ -0,0 +1 @@\n+a\n"
    );
    assert_eq!(
        unified_diff("p", "a\nb\n", ""),
        "--- a/p\n+++ b/p\n@@ -1,2 +0,0 @@\n-a\n-b\n"
    );
}

#[test]
fn huge_files_fall_back_to_one_whole_file_hunk() {
    let old = numbered(20_001);
    let new = old.replace("\n20001\n", "\nlast\n");
    let diff = unified_diff("p", &old, &new);
    assert!(
        diff.starts_with("--- a/p\n+++ b/p\n@@ -1,20001 +1,20001 @@\n-1\n"),
        "{}",
        &diff[..80]
    );
    assert_eq!(
        diff.lines().filter(|line| line.starts_with('-')).count(),
        20_002
    );
    assert_eq!(
        diff.lines().filter(|line| line.starts_with('+')).count(),
        20_002
    );
}

#[test]
fn a_large_rewrite_stays_cheap_and_correct() {
    let old: String = (0..3000).map(|number| format!("old {number}\n")).collect();
    let new: String = (0..3000).map(|number| format!("new {number}\n")).collect();
    let diff = unified_diff("p", &old, &new);
    assert!(diff.starts_with("--- a/p\n+++ b/p\n@@ -1,3000 +1,3000 @@\n-old 0\n"));
    assert_eq!(diff.lines().count(), 3 + 6000);
}
```

Insert above the `#[cfg(test)]` line of `src/domain/migration/mod.rs`:

```rust
//! Planning `nvm migrate` (plan 8, digest section 5), pure: the new text of
//! a startup file whose top-level nvm.sh loader and completion lines become
//! comments under nvmrc's init block, the diff shown before writing, the
//! backup names, and the program that syntax-checks the candidate.
//!
//! [`migrate_text`] never deletes a line: each [`Kind::Loader`] or
//! [`Kind::Completion`] line [`scan_text`] reports becomes
//! `# [nvmrc-migrated] <line>` (install.sh's duplicate guard, a `grep` for
//! `/nvm.sh`, still matches it and so never appends the loader again, digest
//! finding 0.8). `export NVM_DIR=...` lines stay (finding 0.7), and so do the
//! lines that need a manual fix (lazy loaders, stubs, plugins). The init
//! block goes before the first migrated line (keeping the order relative to
//! the `PATH` setup), or replaces the block already in the file.
//!
//! [`Kind::Loader`]: crate::domain::conflict::Kind::Loader
//! [`Kind::Completion`]: crate::domain::conflict::Kind::Completion

mod backup;
mod diff;

#[cfg(test)]
mod backup_tests;
#[cfg(test)]
mod diff_tests;
```

Apply to `src/shell/init/mod.rs` (above the test module):

```diff
--- a/src/shell/init/mod.rs
+++ b/src/shell/init/mod.rs
@@ -71,11 +71,17 @@
         }
     }
 
-    /// The line of the startup file that loads the snippet.
-    fn load_line(self) -> String {
+    /// The line of the startup file that loads the snippet. It runs the
+    /// `nvmrc` binary by name, never `nvm`: when nvm.sh (or a lazy `nvm()`
+    /// stub) is already loaded, `eval "$(nvm init zsh)"` would run nvm.sh's
+    /// function, which does not know `init` and prints nothing, so nvm.sh
+    /// silently stays in charge (digest finding 0.1). No function or alias
+    /// is ever named `nvmrc`.
+    #[must_use]
+    pub fn load_line(self) -> String {
         match self {
-            Self::Fish => "nvm init fish | source".to_owned(),
-            _ => format!("eval \"$(nvm init {})\"", self.name()),
+            Self::Fish => "nvmrc init fish | source".to_owned(),
+            _ => format!("eval \"$(nvmrc init {})\"", self.name()),
         }
     }
 }
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1291 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(migration): plan the edits that move a profile from nvm.sh to nvmrc init"
```

---

### Task 6: `nvm migrate`

**Files:**

- Create: `src/commands/migrate/apply.rs`,
  `src/commands/migrate/apply_tests.rs`, `src/commands/migrate/args.rs`,
  `src/commands/migrate/args_tests.rs`, `src/commands/migrate/fixtures.rs`,
  `src/commands/migrate/mod.rs`, `src/commands/migrate/plan.rs`,
  `src/commands/migrate/tests.rs`, `src/commands/migrate/undo.rs`,
  `src/commands/migrate/undo_tests.rs`, `tests/migrate_cli.rs`
- Modify: `src/cli/commands.rs`, `src/commands/mod.rs`

**Interfaces:**

- Produces: `commands::migrate::{run, USAGE, shell_of}`; the `migrate`
  subcommand.
- Behaviour: `nvm migrate [--dry-run] [--yes|-y] [--undo] [--shell <name>]`. It
  scans, computes per root file `migrate_text` and its diff, prints the diffs
  and stops on `--dry-run`; otherwise it needs `--yes` or an interactive answer
  (in interactive mode the diffs are part of the question on stderr; without a
  terminal and without `--yes` the diff is printed, then `nvm migrate:
  confirmation needs a terminal: pass --yes`, exit 1; a no is `nvm migrate:
  aborted`, exit 1). Per file: write the BACKUP next to the link path
  (`write_file`, a regular file), then `replace_file` with a `verify` that runs
  the syntax check on the temp file; a failing check refuses that file (left
  byte-identical, exit 1, the others go on); a checker that is not installed
  only warns. It prints `migrated <path> (backup: <backup>)`, re-scans and fails
  if an auto-migratable hit is left, and ends with `nvm migrate: done` (plus `N
  manual finding(s) left: run `nvm doctor``) or `nothing to migrate`. The shell
  of the block is `--shell`, else the shell of a current init line already in
  the file, else from the file name (a `.profile` or `$ENV` file uses `$SHELL`
  when it is a supported name, else bash). `--undo` restores the latest backup
  of each root THROUGH the link with the same atomic procedure (backups stay),
  skips a file already equal to its backup (`nothing to restore`), and fails
  with `no backup found` when there is none. The end-to-end tests run the real
  binary on a temp `HOME`: a bash profile with the install.sh and Homebrew lines
  migrates, passes `bash -n`, has one backup, `nvm doctor` is then clean,
  `--undo` restores it byte for byte, the same through a stow-style symlink
  (link and mode 0640 kept, backup beside the link), `--dry-run` changes
  nothing, and a candidate that would not pass `bash -n` is refused.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -13,6 +13,7 @@
 pub mod install_latest_npm;
 pub mod ls;
 pub mod ls_remote;
+pub mod migrate;
 pub mod npm;
 pub mod nvm_exec;
 pub mod nvmrc_file;
```

- [ ] **Step 2: Write the failing tests**

Create `src/commands/migrate/fixtures.rs`:

```rust
//! What the tests of `nvm migrate` share.

use std::path::Path;

use super::run;
use crate::commands::Output;
use crate::context::Context;
use crate::error::CliError;
use crate::fakes::{FakeClock, FakeEnv, FakeFileSystem, FakeProcess, FakePrompt};
use crate::ports::FileSystem;

pub use crate::commands::conflict::fixtures::{HOME, home_env, link};

/// The fake clock's time: 2026-10-04 10:34:00 UTC.
pub const NOW: i64 = 1_791_110_040;
/// The suffix of a backup taken at [`NOW`].
pub const STAMP: &str = ".nvmrc-backup-20261004T103400Z";
/// The question asked before writing.
pub const QUESTION: &str = "Apply these changes? [y/N] ";

/// The lines install.sh appends to `.bashrc`.
pub const INSTALL_SH: &str = "export NVM_DIR=\"$HOME/.nvm\"\n\
[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n";

/// [`INSTALL_SH`] after `migrate` for bash.
pub const INSTALL_SH_MIGRATED: &str = "export NVM_DIR=\"$HOME/.nvm\"\n\
# >>> nvmrc init >>>\n\
eval \"$(nvmrc init bash)\"\n\
# <<< nvmrc init <<<\n\
# [nvmrc-migrated] [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
# [nvmrc-migrated] [ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n";

/// Every syntax checker present and happy.
pub fn checkers() -> FakeProcess {
    FakeProcess::default()
        .with_output("bash", "")
        .with_output("zsh", "")
        .with_output("sh", "")
        .with_output("ksh", "")
        .with_output("fish", "")
}

/// The parts of the context a test varies.
pub struct Setup<'a> {
    pub fs: &'a FakeFileSystem,
    pub env: &'a FakeEnv,
    pub process: &'a FakeProcess,
    pub prompt: &'a FakePrompt,
}

impl Setup<'_> {
    /// `nvm migrate <args>` at [`NOW`].
    pub fn migrate(&self, args: &[&str]) -> Result<Output, CliError> {
        let clock = FakeClock::at(NOW);
        let context = Context::new(self.fs, self.env)
            .with_clock(&clock)
            .with_process(self.process)
            .with_prompt(self.prompt);
        let args: Vec<String> = args.iter().map(|&argument| argument.to_owned()).collect();
        run(&context, &args)
    }

    pub fn output(&self, args: &[&str]) -> Output {
        self.migrate(args).unwrap()
    }
}

/// `nvm migrate <args>` with `fs`, `$HOME` set, every checker and nobody at
/// the terminal.
pub fn migrate(fs: &FakeFileSystem, args: &[&str]) -> Output {
    let process = checkers();
    let prompt = FakePrompt::unavailable();
    let setup = Setup {
        fs,
        env: &home_env(),
        process: &process,
        prompt: &prompt,
    };
    setup.output(args)
}

pub fn read(fs: &FakeFileSystem, path: &str) -> String {
    fs.read_to_string(Path::new(path)).unwrap()
}
```

Create `src/commands/migrate/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
#[cfg(test)]
mod undo_tests;

use std::io;
use std::path::PathBuf;

use crate::commands::Output;
use crate::commands::conflict::{Report, roots, scan};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::error::{CliError, NvmExitCode};

pub use plan::shell_of;

use args::Options;

/// How `nvm migrate` is called.
pub const USAGE: &str = "Usage: nvm migrate [--dry-run] [--yes] [--undo] [--shell <name>]";

const QUESTION: &str = "Apply these changes? [y/N] ";

/// Migrates (or with `--undo` restores) the startup files of the shell named
/// by `--shell`, of every shell when none is named.
///
/// # Errors
/// [`CliError::Usage`] for an unknown shell, a missing shell name or a
/// positional word, and [`CliError::Unsupported`] for any other option.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let options = args::parse(args)?;
    let files = roots(context, options.shell);
    if options.undo {
        return Ok(undo::run(context, &files, options));
    }
    Ok(migrate(context, &files, options))
}

/// The forward migration of `files`.
fn migrate(context: &Context<'_>, files: &[PathBuf], options: Options) -> Output {
    let mut transcript = Transcript::default();
    let report = scan(context, files);
    let (changes, unreadable) = plan::planned(context, &report, options.shell, &mut transcript);
    if changes.is_empty() {
        transcript.out("nvm migrate: nothing to migrate");
        manual_line(&report, &mut transcript);
        return transcript.finish(status_of(unreadable));
    }
    let diffs: String = changes.iter().map(plan::Change::diff).collect();
    if let Some(status) = review(context, &diffs, options, &mut transcript) {
        return transcript.finish(status);
    }
    let skipped = apply::apply_all(context, &changes, &mut transcript);
    let (report, left) = apply::verify(context, files, &skipped, &mut transcript);
    let failed = unreadable || left || !skipped.is_empty();
    if !failed {
        transcript.out("nvm migrate: done");
    }
    manual_line(&report, &mut transcript);
    transcript.finish(status_of(failed))
}

fn status_of(failed: bool) -> NvmExitCode {
    if failed {
        NvmExitCode::Failure
    } else {
        NvmExitCode::Success
    }
}

/// `nvm migrate: N manual finding(s) left: ...` when the scan has conflicts
/// that `migrate` does not fix.
fn manual_line(report: &Report, transcript: &mut Transcript) {
    let manual = report
        .conflicts()
        .filter(|(file, hit)| file.depth != 0 || !hit.kind.is_auto_migratable())
        .count();
    if manual > 0 {
        transcript.out(format!(
            "nvm migrate: {manual} manual finding(s) left: run `nvm doctor`"
        ));
    }
}

/// Shows `diffs` and decides whether to write them: `None` to go on, the
/// status to stop with otherwise. `--dry-run` stops after the diffs, `--yes`
/// goes on, else the prompt asks with the diffs in its question (so they
/// appear before it); without a terminal the diffs go to stdout and it
/// stops.
fn review(
    context: &Context<'_>,
    diffs: &str,
    options: Options,
    transcript: &mut Transcript,
) -> Option<NvmExitCode> {
    if options.dry_run || options.yes {
        transcript.out(diffs.trim_end());
        return options.dry_run.then_some(NvmExitCode::Success);
    }
    let refusal = match context.prompt().confirm(&format!("{diffs}{QUESTION}")) {
        Ok(true) => return None,
        Ok(false) => return refused(transcript, "nvm migrate: aborted"),
        Err(error) if error.kind() == io::ErrorKind::Unsupported => {
            "nvm migrate: confirmation needs a terminal: pass --yes".to_owned()
        }
        Err(error) => format!("nvm migrate: {error}"),
    };
    transcript.out(diffs.trim_end());
    refused(transcript, refusal)
}

fn refused(transcript: &mut Transcript, message: impl Into<String>) -> Option<NvmExitCode> {
    transcript.err(message);
    Some(NvmExitCode::Failure)
}
```

Create `src/commands/migrate/tests.rs`:

```rust
use std::path::Path;

use super::fixtures::{
    HOME, INSTALL_SH, INSTALL_SH_MIGRATED, QUESTION, STAMP, Setup, checkers, home_env, migrate,
    read,
};
use crate::error::NvmExitCode;
use crate::fakes::{FakeFileSystem, FakePrompt};
use crate::ports::FileSystem;

const BASHRC: &str = "/Users/u/.bashrc";

fn install_sh() -> FakeFileSystem {
    FakeFileSystem::default().with_file(BASHRC, INSTALL_SH)
}

const INSTALL_SH_DIFF: &str = "--- a//Users/u/.bashrc\n\
+++ b//Users/u/.bashrc\n\
@@ -1,3 +1,6 @@\n \
export NVM_DIR=\"$HOME/.nvm\"\n\
-[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
-[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n\
+# >>> nvmrc init >>>\n\
+eval \"$(nvmrc init bash)\"\n\
+# <<< nvmrc init <<<\n\
+# [nvmrc-migrated] [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
+# [nvmrc-migrated] [ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n";

fn backup_of(path: &str) -> String {
    format!("{path}{STAMP}")
}

#[test]
fn the_install_script_lines_are_migrated_with_a_backup() {
    let fs = install_sh();
    let output = migrate(&fs, &["--yes"]);
    let backup = backup_of(BASHRC);
    let expected = format!(
        "{}\nmigrated {BASHRC} (backup: {backup})\nnvm migrate: done",
        INSTALL_SH_DIFF.trim_end()
    );
    assert_eq!(output.stdout, expected);
    assert_eq!(output.stderr, "");
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
    assert_eq!(read(&fs, &backup), INSTALL_SH);
    assert_eq!(fs.replaced(), [Path::new(BASHRC)]);
}

#[test]
fn the_edited_file_is_checked_with_its_temporary_path() {
    let fs = install_sh();
    let temporary = format!("-n {HOME}/.nvmrc-tmp-{}-0", std::process::id());
    let process = checkers()
        .with_failure("bash")
        .with_run("bash", &temporary, true, "");
    let prompt = FakePrompt::unavailable();
    let env = home_env();
    let setup = Setup {
        fs: &fs,
        env: &env,
        process: &process,
        prompt: &prompt,
    };
    assert_eq!(setup.output(&["-y"]).status, NvmExitCode::Success);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
}

#[test]
fn dry_run_prints_the_diff_and_writes_nothing() {
    let fs = install_sh();
    let output = migrate(&fs, &["--dry-run"]);
    assert_eq!(output.stdout, INSTALL_SH_DIFF.trim_end());
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH);
    assert!(!fs.is_file(Path::new(&backup_of(BASHRC))));
    assert!(fs.replaced().is_empty());
}

fn with_answer(prompt: &FakePrompt) -> (FakeFileSystem, crate::commands::Output) {
    let fs = install_sh();
    let process = checkers();
    let env = home_env();
    let setup = Setup {
        fs: &fs,
        env: &env,
        process: &process,
        prompt,
    };
    let output = setup.output(&[]);
    (fs, output)
}

#[test]
fn a_yes_at_the_prompt_applies_and_the_question_shows_the_diff() {
    let prompt = FakePrompt::answering(true);
    let (fs, output) = with_answer(&prompt);
    assert_eq!(prompt.asked(), [format!("{INSTALL_SH_DIFF}{QUESTION}")]);
    let backup = backup_of(BASHRC);
    assert_eq!(
        output.stdout,
        format!("migrated {BASHRC} (backup: {backup})\nnvm migrate: done")
    );
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
}

#[test]
fn a_no_at_the_prompt_aborts_without_writing() {
    let prompt = FakePrompt::answering(false);
    let (fs, output) = with_answer(&prompt);
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, "nvm migrate: aborted");
    assert_eq!(output.status, NvmExitCode::Failure);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH);
    assert!(!fs.is_file(Path::new(&backup_of(BASHRC))));
}

#[test]
fn without_a_terminal_and_without_yes_it_shows_the_diff_and_fails() {
    let prompt = FakePrompt::unavailable();
    let (fs, output) = with_answer(&prompt);
    assert_eq!(output.stdout, INSTALL_SH_DIFF.trim_end());
    assert_eq!(
        output.stderr,
        "nvm migrate: confirmation needs a terminal: pass --yes"
    );
    assert_eq!(output.status, NvmExitCode::Failure);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH);
}

#[test]
fn yes_never_asks() {
    let fs = install_sh();
    let process = checkers();
    let prompt = FakePrompt::answering(false);
    let env = home_env();
    let setup = Setup {
        fs: &fs,
        env: &env,
        process: &process,
        prompt: &prompt,
    };
    assert_eq!(setup.output(&["--yes"]).status, NvmExitCode::Success);
    assert!(prompt.asked().is_empty());
}

#[test]
fn a_second_run_has_nothing_to_migrate() {
    let fs = install_sh();
    migrate(&fs, &["--yes"]);
    let output = migrate(&fs, &["--yes"]);
    assert_eq!(output.stdout, "nvm migrate: nothing to migrate");
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
}

#[test]
fn no_profile_file_is_nothing_to_migrate() {
    let output = migrate(&FakeFileSystem::default(), &["--yes"]);
    assert_eq!(output.stdout, "nvm migrate: nothing to migrate");
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn an_outdated_block_is_replaced() {
    let old = "export A=1\n# >>> nvmrc init >>>\neval \"$(nvm init bash)\"\n# <<< nvmrc init <<<\n";
    let fs = FakeFileSystem::default().with_file(BASHRC, old);
    let output = migrate(&fs, &["--yes"]);
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        read(&fs, BASHRC),
        "export A=1\n# >>> nvmrc init >>>\neval \"$(nvmrc init bash)\"\n# <<< nvmrc init <<<\n"
    );
    assert_eq!(read(&fs, &backup_of(BASHRC)), old);
}

#[test]
fn a_loader_in_a_sourced_file_is_left_for_a_manual_fix() {
    let sourcing = "source ~/.nvm-load.sh\n";
    let loader = "[ -s \"$HOME/.nvm/nvm.sh\" ] && \\. \"$HOME/.nvm/nvm.sh\"\n";
    let fs = FakeFileSystem::default()
        .with_file(BASHRC, sourcing)
        .with_file("/Users/u/.nvm-load.sh", loader);
    let output = migrate(&fs, &["--yes"]);
    assert_eq!(
        output.stdout,
        "nvm migrate: nothing to migrate\n\
nvm migrate: 1 manual finding(s) left: run `nvm doctor`"
    );
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(read(&fs, BASHRC), sourcing);
    assert_eq!(read(&fs, "/Users/u/.nvm-load.sh"), loader);
    assert!(fs.replaced().is_empty());
}

#[test]
fn a_current_block_for_another_shell_is_kept() {
    let profile = "# >>> nvmrc init >>>\neval \"$(nvmrc init sh)\"\n# <<< nvmrc init <<<\n";
    let fs = FakeFileSystem::default().with_file("/Users/u/.profile", profile);
    let output = migrate(&fs, &["--yes"]);
    assert_eq!(output.stdout, "nvm migrate: nothing to migrate");
    assert_eq!(read(&fs, "/Users/u/.profile"), profile);
}
```

Create `tests/migrate_cli.rs`:

```rust
//! End-to-end: `nvm migrate` with the real binary against a temporary
//! `$HOME` (no real dotfile is touched), the real file system and the real
//! `bash -n`.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const PROFILE: &str = "export PATH=\"$HOME/bin:$PATH\"\n\
export NVM_DIR=\"$HOME/.nvm\"\n\
[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm\n\
[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion\n\
[ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ] && \\. \"/opt/homebrew/opt/nvm/nvm.sh\"  # This loads nvm\n\
[ -s \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\"  # This loads nvm bash_completion\n\
alias ll='ls -l'\n";

/// `nvm <args>` with `HOME` set to `home`, bash as the login shell, no
/// terminal on stdin and nothing else from this environment.
fn nvm(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_nvm"))
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("SHELL", "/bin/bash")
        .env("PATH", "/usr/bin:/bin")
        .env("NVM_DIR", home.join(".nvm"))
        .current_dir(home)
        .stdin(Stdio::null())
        .output()
        .expect("run the binary")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Every entry under `directory`: file contents, or the target of a link.
fn snapshot(directory: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut entries = BTreeMap::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let metadata = fs::symlink_metadata(&path).unwrap();
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path).unwrap();
            entries.insert(path, target.into_os_string().into_encoded_bytes());
        } else if metadata.is_dir() {
            entries.extend(snapshot(&path));
        } else {
            let contents = fs::read(&path).unwrap();
            entries.insert(path, contents);
        }
    }
    entries
}

/// The backups of `name` in `directory`.
fn backups(directory: &Path, name: &str) -> Vec<PathBuf> {
    let prefix = format!("{name}.nvmrc-backup-");
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|file| file.to_string_lossy().starts_with(&prefix))
        })
        .collect()
}

fn bash_accepts(path: &Path) -> bool {
    Command::new("bash")
        .arg("-n")
        .arg(path)
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn migrate_rewrites_the_profile_and_undo_restores_it_byte_for_byte() {
    let home = TempDir::new().unwrap();
    let profile = home.path().join(".bash_profile");
    fs::write(&profile, PROFILE).unwrap();
    let migrated = nvm(home.path(), &["migrate", "--yes"]);
    assert_eq!(
        migrated.status.code(),
        Some(0),
        "{}",
        text(&migrated.stderr)
    );
    let new = fs::read_to_string(&profile).unwrap();
    assert!(new.contains("eval \"$(nvmrc init bash)\"\n"), "{new}");
    assert!(new.contains("# [nvmrc-migrated] [ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ]"));
    assert!(bash_accepts(&profile));
    let saved = backups(home.path(), ".bash_profile");
    assert_eq!(saved.len(), 1);
    assert_eq!(fs::read_to_string(&saved[0]).unwrap(), PROFILE);
    let doctor = nvm(home.path(), &["doctor"]);
    assert_eq!(doctor.status.code(), Some(0), "{}", text(&doctor.stdout));
    let undone = nvm(home.path(), &["migrate", "--undo", "--yes"]);
    assert_eq!(undone.status.code(), Some(0), "{}", text(&undone.stderr));
    assert_eq!(fs::read(&profile).unwrap(), PROFILE.as_bytes());
    assert_eq!(backups(home.path(), ".bash_profile").len(), 1);
}

#[test]
fn a_symlinked_profile_keeps_its_link_and_permissions() {
    let home = TempDir::new().unwrap();
    let repository = home.path().join("dotfiles/bash");
    fs::create_dir_all(&repository).unwrap();
    let target = repository.join(".bash_profile");
    fs::write(&target, PROFILE).unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    let link = home.path().join(".bash_profile");
    symlink("dotfiles/bash/.bash_profile", &link).unwrap();
    let migrated = nvm(home.path(), &["migrate", "--yes"]);
    assert_eq!(
        migrated.status.code(),
        Some(0),
        "{}",
        text(&migrated.stderr)
    );
    let kept = |path: &Path| {
        let link_kind = fs::symlink_metadata(path).unwrap().file_type();
        let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        (link_kind.is_symlink(), mode)
    };
    assert_eq!(kept(&link), (true, 0o640));
    assert!(
        fs::read_to_string(&target)
            .unwrap()
            .contains("nvmrc init bash")
    );
    let saved = backups(home.path(), ".bash_profile");
    assert_eq!(saved.len(), 1);
    assert!(fs::symlink_metadata(&saved[0]).unwrap().is_file());
    assert!(backups(&repository, ".bash_profile").is_empty());
    let undone = nvm(home.path(), &["migrate", "--undo", "--yes"]);
    assert_eq!(undone.status.code(), Some(0), "{}", text(&undone.stderr));
    assert_eq!(fs::read(&target).unwrap(), PROFILE.as_bytes());
    assert_eq!(kept(&link), (true, 0o640));
}

#[test]
fn dry_run_changes_nothing() {
    let home = TempDir::new().unwrap();
    fs::write(home.path().join(".bash_profile"), PROFILE).unwrap();
    let before = snapshot(home.path());
    let output = nvm(home.path(), &["migrate", "--dry-run"]);
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert!(text(&output.stdout).contains("+eval \"$(nvmrc init bash)\""));
    assert_eq!(snapshot(home.path()), before);
}

#[test]
fn an_if_whose_body_would_be_emptied_is_refused() {
    let home = TempDir::new().unwrap();
    let bashrc = home.path().join(".bashrc");
    // The block goes before the first loader, so the `if` below it is left
    // with an empty body once its line is commented out.
    let original = "[ -s \"$HOME/.nvm/nvm.sh\" ] && \\. \"$HOME/.nvm/nvm.sh\"\n\
if [ -d \"$HOME/.nvm\" ]; then\n\
[ -s \"$HOME/.nvm/bash_completion\" ] && \\. \"$HOME/.nvm/bash_completion\"\n\
fi\n";
    fs::write(&bashrc, original).unwrap();
    let output = nvm(home.path(), &["migrate", "--yes"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("would not pass `bash -n`; left unchanged"),
        "{}",
        text(&output.stderr)
    );
    assert_eq!(fs::read(&bashrc).unwrap(), original.as_bytes());
}

#[test]
fn without_yes_and_without_a_terminal_nothing_changes() {
    let home = TempDir::new().unwrap();
    fs::write(home.path().join(".bash_profile"), PROFILE).unwrap();
    let before = snapshot(home.path());
    let output = nvm(home.path(), &["migrate"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        text(&output.stderr),
        "nvm migrate: confirmation needs a terminal: pass --yes\n"
    );
    assert_eq!(snapshot(home.path()), before);
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find module `migrate`",
"no variant named `Migrate` found" and "cannot find function `shell_of`".

- [ ] **Step 4: Write the implementation**

Apply to `src/cli/commands.rs` (above the test module):

```diff
--- a/src/cli/commands.rs
+++ b/src/cli/commands.rs
@@ -133,6 +133,14 @@
         #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
         args: Vec<String>,
     },
+    /// Move the shell startup files from nvm.sh to nvmrc's init line: shows
+    /// the diffs, then (`--yes` or a confirmation) backs each file up and
+    /// rewrites it (`--dry-run` writes nothing; `--undo` restores the latest
+    /// backups; `--shell <name>` limits it to one shell).
+    Migrate {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
     /// What the `nvm` function runs at shell start: `use`, `install` or
     /// `none` (nvm.sh's `nvm_auto`).
     #[command(name = "__auto", hide = true)]
@@ -167,6 +175,7 @@
         Command::Run { args } => commands::run::run(context, args),
         Command::Init { args } => commands::init::run(args),
         Command::Doctor { args } => commands::doctor::run(context, args),
+        Command::Migrate { args } => commands::migrate::run(context, args),
         Command::Auto { args } => commands::auto::run(context, args),
     }
 }
```

Create `src/commands/migrate/apply.rs`:

```rust
//! Writing the planned edits: a backup beside each file, then an atomic
//! replace whose candidate must pass the shell's syntax check, then a scan
//! that proves no loader is left (digest section 5).

use std::cell::Cell;
use std::io;
use std::path::{Path, PathBuf};

use super::plan::Change;
use crate::commands::conflict::{Report, scan};
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::migration::{backup_name, syntax_check};
use crate::ports::Process;

/// Applies every change in order. Returns the paths that were NOT written
/// (refused or failed); each one is reported on `transcript`.
pub fn apply_all(
    context: &Context<'_>,
    changes: &[Change],
    transcript: &mut Transcript,
) -> Vec<PathBuf> {
    changes
        .iter()
        .filter(|change| !apply(context, change, transcript))
        .map(|change| change.path.clone())
        .collect()
}

/// Backs `change.path` up and writes it; whether it was written.
fn apply(context: &Context<'_>, change: &Change, transcript: &mut Transcript) -> bool {
    let path = change.path.display();
    let backup = backup_path(context, &change.path);
    if let Err(error) = context.fs.write_file(&backup, &change.old) {
        let backup = backup.display();
        transcript.err(format!(
            "nvm migrate: {path}: cannot write {backup}: {error}"
        ));
        return false;
    }
    let check = SyntaxCheck::new(context.process(), &change.path);
    let written = context
        .fs
        .replace_file(&change.path, &change.new, &|temporary| {
            check.verify(temporary)
        });
    report(change, &backup, &check, &written, transcript);
    written.is_ok()
}

/// What happened to one file, on `transcript`.
fn report(
    change: &Change,
    backup: &Path,
    check: &SyntaxCheck<'_>,
    written: &io::Result<()>,
    transcript: &mut Transcript,
) {
    let path = change.path.display();
    if let Some(command) = check.missing() {
        transcript.err(format!(
            "nvm migrate: {command} not found: the syntax of {path} was not checked"
        ));
    }
    match written {
        Ok(()) => transcript.out(format!("migrated {path} (backup: {})", backup.display())),
        Err(_) if check.rejected.get() => transcript.err(format!(
            "nvm migrate: {path}: the edited file would not pass `{}`; left unchanged",
            check.command_line()
        )),
        Err(error) => transcript.err(format!("nvm migrate: {path}: {error}")),
    }
}

/// `<file name>.nvmrc-backup-<now>` in the directory of `path` as named (a
/// symbolic link's directory, not its target's).
fn backup_path(context: &Context<'_>, path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let directory = path.parent().unwrap_or_else(|| Path::new("/"));
    directory.join(backup_name(&name, context.clock().unix_seconds()))
}

/// The syntax checker of one file, run on the candidate before it replaces
/// the file, remembering what happened.
struct SyntaxCheck<'a> {
    process: &'a dyn Process,
    command: Option<(&'static str, Vec<&'static str>)>,
    missing: Cell<bool>,
    rejected: Cell<bool>,
}

impl<'a> SyntaxCheck<'a> {
    fn new(process: &'a dyn Process, path: &Path) -> Self {
        Self {
            process,
            command: syntax_check(&path.display().to_string()),
            missing: Cell::new(false),
            rejected: Cell::new(false),
        }
    }

    /// Runs the checker on `temporary`: a failure or a non-success status is
    /// an error; a checker that is not installed is skipped.
    fn verify(&self, temporary: &Path) -> io::Result<()> {
        let Some((program, flags)) = &self.command else {
            return Ok(());
        };
        let temporary = temporary.to_string_lossy();
        let mut arguments = flags.clone();
        arguments.push(&temporary);
        match self.process.run(Path::new(program), &arguments) {
            Ok(output) if output.success => Ok(()),
            Ok(_) => {
                self.rejected.set(true);
                Err(io::Error::other("the syntax check failed"))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.missing.set(true);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// The checker's program, when it was not found.
    fn missing(&self) -> Option<&'static str> {
        self.command
            .as_ref()
            .filter(|_| self.missing.get())
            .map(|(program, _)| *program)
    }

    /// `bash -n`, as the messages show it.
    fn command_line(&self) -> String {
        self.command
            .as_ref()
            .map(|(program, flags)| format!("{program} {}", flags.join(" ")))
            .unwrap_or_default()
    }
}

/// Scans `files` again and reports every top-level loader or completion
/// left outside the `skipped` files; whether one is left. Returns the new
/// scan too.
pub fn verify(
    context: &Context<'_>,
    files: &[PathBuf],
    skipped: &[PathBuf],
    transcript: &mut Transcript,
) -> (Report, bool) {
    let report = scan(context, files);
    let mut left = false;
    for (file, hit) in report.auto_migratable() {
        if !skipped.contains(&file.path) {
            left = true;
            transcript.err(format!(
                "nvm migrate: {}:{}: {} still there after the edit",
                file.path.display(),
                hit.line,
                hit.kind.label()
            ));
        }
    }
    (report, left)
}
```

Create `src/commands/migrate/apply_tests.rs`:

```rust
use std::path::Path;

use super::fixtures::{INSTALL_SH, STAMP, Setup, checkers, home_env, link, migrate, read};
use crate::error::NvmExitCode;
use crate::fakes::{FakeFileSystem, FakeProcess, FakePrompt};
use crate::ports::FileSystem;

const ZSHRC: &str = "/Users/u/.zshrc";
const TARGET: &str = "/Users/u/dotfiles/zsh/.zshrc";
const BASHRC: &str = "/Users/u/.bashrc";

fn run_with(fs: &FakeFileSystem, process: &FakeProcess) -> crate::commands::Output {
    let prompt = FakePrompt::unavailable();
    let env = home_env();
    let setup = Setup {
        fs,
        env: &env,
        process,
        prompt: &prompt,
    };
    setup.output(&["--yes"])
}

#[test]
fn a_symlinked_zshrc_is_written_through_the_link_with_the_backup_beside_the_link() {
    let fs = FakeFileSystem::default().with_file(TARGET, INSTALL_SH);
    link(&fs, "dotfiles/zsh/.zshrc", ZSHRC);
    let output = migrate(&fs, &["--yes"]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert_eq!(
        fs.read_link(Path::new(ZSHRC)).unwrap(),
        Path::new("dotfiles/zsh/.zshrc")
    );
    let backup = format!("{ZSHRC}{STAMP}");
    assert!(
        output
            .stdout
            .contains(&format!("migrated {ZSHRC} (backup: {backup})"))
    );
    assert_eq!(read(&fs, &backup), INSTALL_SH);
    assert!(fs.read_link(Path::new(&backup)).is_err());
    assert!(read(&fs, TARGET).contains("eval \"$(nvmrc init zsh)\"\n"));
    assert!(!fs.is_file(Path::new(&format!("{TARGET}{STAMP}"))));
}

#[test]
fn a_file_that_fails_its_check_is_refused_and_the_others_are_migrated() {
    let fs = FakeFileSystem::default()
        .with_file(BASHRC, INSTALL_SH)
        .with_file(ZSHRC, INSTALL_SH);
    let process = checkers().with_failure("bash");
    let output = run_with(&fs, &process);
    assert_eq!(output.status, NvmExitCode::Failure);
    assert_eq!(
        output.stderr,
        format!("nvm migrate: {BASHRC}: the edited file would not pass `bash -n`; left unchanged")
    );
    assert_eq!(read(&fs, BASHRC), INSTALL_SH);
    assert!(read(&fs, ZSHRC).contains("nvmrc init zsh"));
    assert!(output.stdout.contains(&format!("migrated {ZSHRC} ")));
    assert!(!output.stdout.contains(&format!("migrated {BASHRC} ")));
    assert_eq!(fs.replaced(), [Path::new(ZSHRC)]);
}

#[test]
fn a_missing_checker_is_noted_and_the_file_is_still_migrated() {
    let fs = FakeFileSystem::default().with_file(BASHRC, INSTALL_SH);
    let output = run_with(&fs, &FakeProcess::default());
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        output.stderr,
        format!("nvm migrate: bash not found: the syntax of {BASHRC} was not checked")
    );
    assert!(read(&fs, BASHRC).contains("nvmrc init bash"));
}

#[test]
fn shell_overrides_the_shell_of_the_block() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.profile", INSTALL_SH);
    let output = migrate(&fs, &["--yes", "--shell=ksh"]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert!(read(&fs, "/Users/u/.profile").contains("eval \"$(nvmrc init ksh)\"\n"));
}

#[test]
fn the_profile_takes_the_login_shell_of_shell() {
    let fs = FakeFileSystem::default().with_file("/Users/u/.profile", INSTALL_SH);
    let process = checkers();
    let prompt = FakePrompt::unavailable();
    let env = home_env().with_var("SHELL", "/bin/zsh");
    let setup = Setup {
        fs: &fs,
        env: &env,
        process: &process,
        prompt: &prompt,
    };
    assert_eq!(setup.output(&["--yes"]).status, NvmExitCode::Success);
    assert!(read(&fs, "/Users/u/.profile").contains("eval \"$(nvmrc init zsh)\"\n"));
}

#[test]
fn manual_findings_left_after_a_migration_are_counted() {
    let fs = FakeFileSystem::default()
        .with_file(BASHRC, &format!("{INSTALL_SH}source ~/.nvm-load.sh\n"))
        .with_file("/Users/u/.nvm-load.sh", "source ~/.nvm/nvm.sh\n");
    let output = migrate(&fs, &["--yes"]);
    assert!(
        output.stdout.ends_with(
            "nvm migrate: done\nnvm migrate: 1 manual finding(s) left: run `nvm doctor`"
        )
    );
    assert_eq!(output.status, NvmExitCode::Success);
}
```

Create `src/commands/migrate/args.rs`:

```rust
//! The arguments of `nvm migrate`.

use super::USAGE;
use crate::error::CliError;
use crate::shell::init::Shell;

/// How `nvm migrate` was called.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// `--dry-run`: print the diffs and write nothing.
    pub dry_run: bool,
    /// `--yes` or `-y`: apply without asking.
    pub yes: bool,
    /// `--undo`: restore the latest backups.
    pub undo: bool,
    /// `--shell <name>`: the shell whose files are scanned, and whose init
    /// line goes in the block; every shell when `None`.
    pub shell: Option<Shell>,
}

/// The options in `args`; a repeated `--shell` takes the last name.
///
/// # Errors
/// [`CliError::Usage`] for an unknown shell, a missing shell name or a
/// positional word, and [`CliError::Unsupported`] for any other option.
pub fn parse(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    let mut arguments = args.iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--dry-run" => options.dry_run = true,
            "--yes" | "-y" => options.yes = true,
            "--undo" => options.undo = true,
            "--shell" => {
                let name = arguments
                    .next()
                    .ok_or_else(|| CliError::Usage(format!("--shell needs a name.\n{USAGE}")))?;
                options.shell = Some(name.parse()?);
            }
            other => match other.strip_prefix("--shell=") {
                Some(name) => options.shell = Some(name.parse()?),
                None => return Err(rejected(other)),
            },
        }
    }
    Ok(options)
}

fn rejected(argument: &str) -> CliError {
    if argument.starts_with('-') {
        CliError::Unsupported(format!("Unsupported option \"{argument}\"."))
    } else {
        CliError::Usage(format!("Unexpected argument \"{argument}\".\n{USAGE}"))
    }
}
```

Create `src/commands/migrate/args_tests.rs`:

```rust
use super::args::{Options, parse};
use super::fixtures::Setup;
use super::shell_of;
use crate::error::{CliError, NvmExitCode};
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess, FakePrompt};
use crate::shell::init::Shell;

fn parsed(args: &[&str]) -> Result<Options, CliError> {
    let args: Vec<String> = args.iter().map(|&argument| argument.to_owned()).collect();
    parse(&args)
}

#[test]
fn every_flag_is_read() {
    let options = parsed(&["--dry-run", "-y", "--undo", "--shell", "zsh"]).unwrap();
    let expected = Options {
        dry_run: true,
        yes: true,
        undo: true,
        shell: Some(Shell::Zsh),
    };
    assert_eq!(options, expected);
    assert_eq!(
        parsed(&["--yes", "--shell=fish"]).unwrap().shell,
        Some(Shell::Fish)
    );
    assert_eq!(parsed(&[]).unwrap(), Options::default());
}

#[test]
fn an_unknown_shell_lists_the_supported_ones() {
    let Err(CliError::Usage(message)) = parsed(&["--shell", "tcsh"]) else {
        panic!("expected a usage error");
    };
    assert!(
        message.contains("bash, zsh, sh, dash, ksh, fish"),
        "{message}"
    );
}

#[test]
fn a_missing_shell_name_is_a_usage_error() {
    let Err(CliError::Usage(message)) = parsed(&["--shell"]) else {
        panic!("expected a usage error");
    };
    assert!(message.starts_with("--shell needs a name."), "{message}");
}

#[test]
fn another_option_is_unsupported() {
    let error = parsed(&["--force"]).unwrap_err();
    assert_eq!(error.to_string(), "Unsupported option \"--force\".");
    assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption);
    assert_eq!(error.exit_code().code(), 55);
}

#[test]
fn a_positional_word_is_a_usage_error() {
    let Err(CliError::Usage(message)) = parsed(&["now"]) else {
        panic!("expected a usage error");
    };
    assert!(
        message.contains("Unexpected argument \"now\"."),
        "{message}"
    );
}

#[test]
fn run_rejects_bad_arguments_before_reading_anything() {
    let fs = FakeFileSystem::default();
    let setup = Setup {
        fs: &fs,
        env: &FakeEnv::default(),
        process: &FakeProcess::default(),
        prompt: &FakePrompt::unavailable(),
    };
    assert!(matches!(
        setup.migrate(&["--nope"]),
        Err(CliError::Unsupported(_))
    ));
}

#[test]
fn the_shell_of_a_file_comes_from_its_name() {
    let env = FakeEnv::default();
    let cases = [
        ("/h/.zshrc", Shell::Zsh),
        ("/h/.zprofile", Shell::Zsh),
        ("/h/.zlogin", Shell::Zsh),
        ("/h/.zshenv", Shell::Zsh),
        ("/h/.bashrc", Shell::Bash),
        ("/h/.bash_profile", Shell::Bash),
        ("/h/.bash_login", Shell::Bash),
        ("/h/.kshrc", Shell::Ksh),
        ("/h/.config/fish/config.fish", Shell::Fish),
        ("/h/.config/fish/conf.d/x.fish", Shell::Fish),
        ("/h/.profile", Shell::Bash),
    ];
    for (path, shell) in cases {
        assert_eq!(shell_of(path, &env), shell, "{path}");
    }
}

#[test]
fn the_profile_follows_a_supported_login_shell() {
    let cases = [
        ("/bin/zsh", Shell::Zsh),
        ("/bin/ksh", Shell::Ksh),
        ("/bin/dash", Shell::Dash),
        ("/usr/bin/tcsh", Shell::Bash),
    ];
    for (login, shell) in cases {
        let env = FakeEnv::default().with_var("SHELL", login);
        assert_eq!(shell_of("/h/.profile", &env), shell, "{login}");
    }
}
```

Insert above the `#[cfg(test)]` line of `src/commands/migrate/mod.rs`:

```rust
//! `nvm migrate [--dry-run] [--yes] [--undo] [--shell <name>]`: moves the
//! shell startup files from nvm.sh to nvmrc's init line (plan 8, digest
//! section 5). The top-level nvm.sh loader and completion lines of each root
//! file become `# [nvmrc-migrated]` comments under the init block; the diffs
//! are shown, and only `--yes` or a confirmation writes them: a backup
//! beside each file (beside the LINK for a symbolic link), then an atomic
//! replace through the link that the shell's syntax check must pass, then a
//! scan proving no loader is left. `--undo` restores the latest backups.
//! `$NVM_DIR` and nvm.sh are never touched; paths are shown as named.

mod apply;
mod args;
mod plan;
mod undo;

#[cfg(test)]
mod apply_tests;
#[cfg(test)]
mod args_tests;
#[cfg(test)]
mod fixtures;
```

Create `src/commands/migrate/plan.rs`:

```rust
//! Which startup files `migrate` edits, and how.

use std::path::PathBuf;

use crate::commands::conflict::Report;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::migration::{migrate_text, unified_diff};
use crate::ports::Env;
use crate::shell::init::{SHELLS, Shell};

/// The planned new content of one file, named as the scan named it (a
/// symbolic link stays a link).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: PathBuf,
    pub old: String,
    pub new: String,
}

impl Change {
    /// The unified diff from the current content to the new one.
    pub fn diff(&self) -> String {
        unified_diff(&self.path.display().to_string(), &self.old, &self.new)
    }
}

/// The edits of the root files (depth 0) of `report` that change: the ones
/// with top-level loaders or completions, or with an outdated init block.
/// The block is for `shell`, else the shell of a current block already in
/// the file, else [`shell_of`] the file. A root that cannot be read is
/// reported on `transcript` and makes the second value `true`.
pub fn planned(
    context: &Context<'_>,
    report: &Report,
    shell: Option<Shell>,
    transcript: &mut Transcript,
) -> (Vec<Change>, bool) {
    let mut changes = Vec::new();
    let mut failed = false;
    for file in report.files.iter().filter(|file| file.depth == 0) {
        match context.fs.read_to_string(&file.path) {
            Ok(old) => changes.extend(change_of(context, file.path.clone(), old, shell)),
            Err(error) => {
                transcript.err(format!("nvm migrate: {}: {error}", file.path.display()));
                failed = true;
            }
        }
    }
    (changes, failed)
}

/// The edit of the file `path` whose content is `old`, when it changes.
fn change_of(
    context: &Context<'_>,
    path: PathBuf,
    old: String,
    shell: Option<Shell>,
) -> Option<Change> {
    let shell = shell
        .or_else(|| block_shell(&old))
        .unwrap_or_else(|| shell_of(&path.display().to_string(), context.env));
    let new = migrate_text(&old, shell).text;
    (new != old).then_some(Change { path, old, new })
}

/// The shell of a current init line already in `text`, so that a block the
/// user chose stays as it is.
fn block_shell(text: &str) -> Option<Shell> {
    text.lines().find_map(|line| {
        SHELLS
            .into_iter()
            .find(|shell| line.trim() == shell.load_line())
    })
}

/// The shell whose init line goes in the startup file `path`, by its name:
/// zsh's files (`zsh` in the name, `.zprofile`, `.zlogin`, `.zlogout`) zsh,
/// `.bash*` bash, `.kshrc` ksh, `*.fish` fish; any other file (`.profile`,
/// an `$ENV` file) the login shell of `$SHELL` when it is a supported one,
/// else bash.
#[must_use]
pub fn shell_of(path: &str, env: &dyn Env) -> Shell {
    const ZSH_FILES: [&str; 3] = [".zprofile", ".zlogin", ".zlogout"];
    let name = path.rsplit('/').next().unwrap_or_default();
    if name.contains("zsh") || ZSH_FILES.contains(&name) {
        Shell::Zsh
    } else if name.starts_with(".bash") {
        Shell::Bash
    } else if name == ".kshrc" {
        Shell::Ksh
    } else if name.ends_with(".fish") {
        Shell::Fish
    } else {
        login_shell(env).unwrap_or(Shell::Bash)
    }
}

/// The supported shell `$SHELL` names (its basename).
fn login_shell(env: &dyn Env) -> Option<Shell> {
    let shell = env.var("SHELL")?;
    shell.rsplit('/').next()?.parse().ok()
}
```

Create `src/commands/migrate/undo.rs`:

```rust
//! `nvm migrate --undo`: each startup file gets back the content of its
//! latest backup, written through a symbolic link with the same atomic
//! replace; the backups stay.

use std::path::{Path, PathBuf};

use super::args::Options;
use super::plan::Change;
use super::{review, status_of};
use crate::commands::Output;
use crate::commands::transcript::Transcript;
use crate::context::Context;
use crate::domain::migration::latest_backup;
use crate::error::NvmExitCode;

/// A change back to a backup.
struct Restore {
    change: Change,
    backup: PathBuf,
}

/// Restores the latest backup of each of `files`.
pub fn run(context: &Context<'_>, files: &[PathBuf], options: Options) -> Output {
    let mut transcript = Transcript::default();
    let backups: Vec<(PathBuf, PathBuf)> = files
        .iter()
        .filter_map(|path| Some((path.clone(), latest(context, path)?)))
        .collect();
    if backups.is_empty() {
        transcript.err("nvm migrate: no backup found");
        return transcript.finish(NvmExitCode::Failure);
    }
    let (restores, mut failed) = restores(context, backups, &mut transcript);
    if restores.is_empty() && !failed {
        transcript.out("nvm migrate: nothing to restore");
        return transcript.finish(NvmExitCode::Success);
    }
    let diffs: String = restores
        .iter()
        .map(|restore| restore.change.diff())
        .collect();
    if let Some(status) = review(context, &diffs, options, &mut transcript) {
        return transcript.finish(status);
    }
    for restore in &restores {
        failed |= !write(context, restore, &mut transcript);
    }
    transcript.finish(status_of(failed))
}

/// The latest backup of `path` in the directory of `path` as named.
fn latest(context: &Context<'_>, path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let directory = path.parent()?;
    let names: Vec<String> = context
        .fs
        .read_dir(directory)
        .ok()?
        .into_iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.name)
        .collect();
    latest_backup(&name, &names).map(|backup| directory.join(backup))
}

/// The restores that change something; a file or backup that cannot be read
/// is reported and makes the second value `true`.
fn restores(
    context: &Context<'_>,
    backups: Vec<(PathBuf, PathBuf)>,
    transcript: &mut Transcript,
) -> (Vec<Restore>, bool) {
    let mut found = Vec::new();
    let mut failed = false;
    for (path, backup) in backups {
        let read = |path: &Path| context.fs.read_to_string(path);
        match read(&path).and_then(|old| Ok((old, read(&backup)?))) {
            Ok((old, new)) if old == new => {}
            Ok((old, new)) => found.push(Restore {
                change: Change { path, old, new },
                backup,
            }),
            Err(error) => {
                transcript.err(format!("nvm migrate: {}: {error}", path.display()));
                failed = true;
            }
        }
    }
    (found, failed)
}

/// Writes one restore through the link; whether it was written.
fn write(context: &Context<'_>, restore: &Restore, transcript: &mut Transcript) -> bool {
    let change = &restore.change;
    let path = change.path.display();
    match context
        .fs
        .replace_file(&change.path, &change.new, &|_| Ok(()))
    {
        Ok(()) => {
            transcript.out(format!("restored {path} from {}", restore.backup.display()));
            true
        }
        Err(error) => {
            transcript.err(format!("nvm migrate: {path}: {error}"));
            false
        }
    }
}
```

Create `src/commands/migrate/undo_tests.rs`:

```rust
use std::path::Path;

use super::fixtures::{INSTALL_SH, INSTALL_SH_MIGRATED, link, migrate, read};
use crate::domain::migration::unified_diff;
use crate::error::NvmExitCode;
use crate::fakes::FakeFileSystem;
use crate::ports::FileSystem;

const BASHRC: &str = "/Users/u/.bashrc";
const OLDER: &str = "/Users/u/.bashrc.nvmrc-backup-20260101T000000Z";
const NEWER: &str = "/Users/u/.bashrc.nvmrc-backup-20261001T000000Z";

fn with_two_backups() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file(BASHRC, INSTALL_SH_MIGRATED)
        .with_file(OLDER, "older\n")
        .with_file(NEWER, INSTALL_SH)
        .with_file("/Users/u/.bashrc.nvmrc-backup-junk", "junk\n")
}

#[test]
fn undo_restores_the_latest_backup_and_keeps_the_backups() {
    let fs = with_two_backups();
    let output = migrate(&fs, &["--undo", "--yes"]);
    let diff = unified_diff(BASHRC, INSTALL_SH_MIGRATED, INSTALL_SH);
    assert_eq!(
        output.stdout,
        format!("{}\nrestored {BASHRC} from {NEWER}", diff.trim_end())
    );
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH);
    assert_eq!(read(&fs, NEWER), INSTALL_SH);
    assert_eq!(read(&fs, OLDER), "older\n");
}

#[test]
fn undo_dry_run_only_prints_the_diff() {
    let fs = with_two_backups();
    let output = migrate(&fs, &["--undo", "--dry-run"]);
    let diff = unified_diff(BASHRC, INSTALL_SH_MIGRATED, INSTALL_SH);
    assert_eq!(output.stdout, diff.trim_end());
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
    assert!(fs.replaced().is_empty());
}

#[test]
fn undo_without_a_terminal_and_without_yes_fails() {
    let fs = with_two_backups();
    let output = migrate(&fs, &["--undo"]);
    assert_eq!(output.status, NvmExitCode::Failure);
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
}

#[test]
fn undo_without_any_backup_fails() {
    let fs = FakeFileSystem::default().with_file(BASHRC, INSTALL_SH_MIGRATED);
    let output = migrate(&fs, &["--undo", "--yes"]);
    assert_eq!(output.stderr, "nvm migrate: no backup found");
    assert_eq!(output.status, NvmExitCode::Failure);
}

#[test]
fn undo_of_a_backup_equal_to_the_file_has_nothing_to_restore() {
    let fs = FakeFileSystem::default()
        .with_file(BASHRC, INSTALL_SH)
        .with_file(NEWER, INSTALL_SH);
    let output = migrate(&fs, &["--undo", "--yes"]);
    assert_eq!(output.stdout, "nvm migrate: nothing to restore");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(fs.replaced().is_empty());
}

#[test]
fn migrate_then_undo_through_a_symlink_restores_the_target() {
    let target = "/Users/u/dotfiles/.zshrc";
    let fs = FakeFileSystem::default().with_file(target, INSTALL_SH);
    link(&fs, "dotfiles/.zshrc", "/Users/u/.zshrc");
    migrate(&fs, &["--yes"]);
    let output = migrate(&fs, &["--undo", "--yes"]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert_eq!(read(&fs, target), INSTALL_SH);
    assert!(fs.read_link(Path::new("/Users/u/.zshrc")).is_ok());
}
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1323 unit tests plus the end-to-end tests
(`tests/migrate_cli.rs`, Unix only).

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(migrate): move the shell profiles from nvm.sh to nvmrc init, with backups and undo"
```

---

### Task 7: The runtime check in the init snippets

**Files:**

- Create: `src/commands/runtime_conflict.rs`,
  `src/domain/conflict/runtime.rs`, `src/domain/conflict/runtime_tests.rs`,
  `src/shell/init/conflict_check.rs`,
  `src/shell/init/conflict_check_tests.rs`, `tests/init_conflict_cli.rs`
- Modify: `docs/superpowers/specs/2026-10-02-nvmrc-design.md`,
  `src/cli/commands.rs`, `src/commands/mod.rs`, `src/domain/conflict/mod.rs`,
  `src/shell/init/mod.rs`, `tests/init_lab/mod.rs`

**Interfaces:**

- Produces: `domain::conflict::{RuntimeConflict, runtime_warning,
  RUNTIME_WARNING_ADVICE}`; `commands::runtime_conflict::{USAGE, run}`; the
  hidden subcommand `__conflict <helpers|function>`;
  `shell::init::conflict_check` (inserted by `snippet()`); the spec sections
  7.2, 7.3, 7.4 and 10 are rewritten.
- Behaviour: spec 7.2 said the binary checks `NVM_BIN` and the type of `nvm`; it
  cannot (shell functions never reach a child, nvmrc's own `use` exports
  `NVM_BIN`, and nvm.sh's `nvm` would shadow the binary). The SNIPPET checks,
  once, in an INTERACTIVE shell only (`case $- in *i*)`; fish `status
  is-interactive`), before it exports `NVMRC_SHELL` or defines its function:
  nvm.sh helpers loaded (`type nvm_has`; fish `functions -q nvm_has`) gives
  reason `helpers`; `nvm` already a function that is not ours gives `function`
  (bash `declare -F` and a body test for `NVMRC_SCRIPT_FD`, zsh
  `functions[nvm]`, ksh `typeset -f`, dash and sh `command -v` and skipped when
  `NVMRC_SHELL` is already set because dash cannot tell whose function it is,
  fish `functions -q` and a body test). Only then does it run `command \nvm
  __conflict <reason> >&2 || true`, which prints on stderr (exit 0): `nvm:
  nvm.sh is still loaded in this shell (<why>)` and two indented lines (nvmrc
  now answers `nvm` but the old loader keeps running on every shell start; run
  `nvm doctor` to see where and `nvm migrate` to move it). A second init of our
  own function, a clean shell and a non-interactive shell stay silent; the check
  never fails the init. The end-to-end tests run in every installed shell with
  `-i` and no profile (bash `--norc --noprofile`, zsh `-f`, fish `--no-config`)
  using a fake `nvm_has` and a fake `nvm` stub only.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -22,6 +22,7 @@
 pub mod remote_index;
 pub mod resolve;
 pub mod run;
+pub mod runtime_conflict;
 pub mod sanitize;
 pub mod set_colors;
 pub mod transcript;
```

- [ ] **Step 2: Write the failing tests**

Create `src/commands/runtime_conflict.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::NvmExitCode;

    fn arguments(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn each_reason_prints_its_warning_on_stderr_only_and_succeeds() {
        for reason in RuntimeConflict::ALL {
            let output = run(&arguments(&[reason.name()])).unwrap();
            assert_eq!(output.stderr, runtime_warning(reason));
            assert_eq!(output.stdout, "");
            assert!(!output.blank_stdout);
            assert_eq!(output.status, NvmExitCode::Success);
        }
    }

    #[test]
    fn a_missing_unknown_or_extra_reason_is_a_usage_error() {
        for words in [&[][..], &["nvm.sh"], &["helpers", "function"], &[""]] {
            let error = run(&arguments(words)).expect_err("usage");
            assert!(
                matches!(&error, CliError::Usage(text) if text == USAGE),
                "{words:?}"
            );
        }
    }
}
```

Apply to the test module of `src/domain/conflict/mod.rs`:

```diff
--- a/src/domain/conflict/mod.rs
+++ b/src/domain/conflict/mod.rs
@@ -3,6 +3,7 @@
 
 pub use expand::{Expansion, expand_path};
 pub use kind::{Kind, Severity};
+pub use runtime::{RUNTIME_WARNING_ADVICE, RuntimeConflict, runtime_warning};
 pub use source_path::source_target;
 
 use lexer::{indentation, strip_comment, tokens};
```

Create `tests/init_conflict_cli.rs`:

```rust
//! End-to-end: the init snippet warns, in an interactive shell, when nvm.sh
//! is still loaded (its `nvm_has` helper is defined, or `nvm` already is a
//! function that is not nvmrc's), and stays silent otherwise. Real shells
//! (each one skipped when missing), started with `-i` without a terminal;
//! nvm.sh is faked by the functions it would leave behind, never sourced.
#![cfg(unix)]

mod init_lab;

use init_lab::{Lab, Run, each_shell, in_dialect, load_line};

const ADVICE: &str = "  nvmrc now answers `nvm`, but the old loader keeps running on every \
shell start;\n  run `nvm doctor` to see where, and `nvm migrate` to move it\n";
const HELPERS: &str =
    "nvm: nvm.sh is still loaded in this shell (its helper functions are defined)\n";
const FUNCTION: &str = "nvm: nvm.sh is still loaded in this shell (nvm was already a function)\n";

/// What nvm.sh leaves behind: its helpers, and its `nvm` function.
fn fake_nvm_sh(name: &str) -> &'static str {
    in_dialect(
        name,
        "nvm_has() { :; }\nnvm() { :; }\n",
        "function nvm_has; end\nfunction nvm; end\n",
    )
}

/// A lazy loader's stub: `nvm` is a function, the helpers are absent.
fn lazy_stub(name: &str) -> &'static str {
    in_dialect(name, "nvm() { :; }\n", "function nvm; end\n")
}

/// `before`, the init (with the automatic `use`), then `nvm use 18`.
fn init_then_use(name: &str, before: &str) -> String {
    format!(
        "{before}{}\nnvm use 18 >/dev/null\necho \"BIN=$NVM_BIN\"",
        load_line(name)
    )
}

fn assert_used_18(lab: &Lab, name: &str, run: &Run) {
    let expected = format!("BIN={}\n", lab.version_bin("v18.20.4"));
    assert_eq!(
        (run.status, run.stdout.as_str()),
        (0, expected.as_str()),
        "{name}: {}",
        run.stderr
    );
}

fn assert_silent(name: &str, run: &Run) {
    assert!(!run.stderr.contains("nvm:"), "{name}: {}", run.stderr);
}

#[test]
fn nvm_sh_helpers_loaded_before_the_init_are_reported_once_on_stderr() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let run = lab.run_interactive(shell, &init_then_use(name, fake_nvm_sh(name)));
        let warning = format!("{HELPERS}{ADVICE}");
        assert!(run.stderr.contains(&warning), "{name}: {}", run.stderr);
        assert_eq!(
            run.stderr.matches("nvm:").count(),
            1,
            "{name}: {}",
            run.stderr
        );
        assert_used_18(&lab, name, &run);
    });
}

#[test]
fn a_lazy_nvm_stub_defined_before_the_init_is_reported() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let run = lab.run_interactive(shell, &init_then_use(name, lazy_stub(name)));
        let warning = format!("{FUNCTION}{ADVICE}");
        assert!(run.stderr.contains(&warning), "{name}: {}", run.stderr);
        assert_eq!(
            run.stderr.matches("nvm:").count(),
            1,
            "{name}: {}",
            run.stderr
        );
        assert_used_18(&lab, name, &run);
    });
}

#[test]
fn a_second_init_and_a_clean_shell_stay_silent() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let clean = lab.run_interactive(shell, &init_then_use(name, ""));
        assert_silent(name, &clean);
        assert_used_18(&lab, name, &clean);
        let twice = format!("{}\n", load_line(name));
        let again = lab.run_interactive(shell, &init_then_use(name, &twice));
        assert_silent(name, &again);
        assert_used_18(&lab, name, &again);
    });
}

#[test]
fn a_non_interactive_shell_never_warns() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        for before in [fake_nvm_sh(name), lazy_stub(name)] {
            let run = lab.run(shell, &init_then_use(name, before));
            assert_eq!(run.stderr, "", "{name}");
            assert_used_18(&lab, name, &run);
        }
    });
}

#[test]
fn the_warning_keeps_set_eu_and_the_status_of_the_automatic_use() {
    let lab = Lab::new().file("proj/.nvmrc", "99\n");
    each_shell(|name, shell| {
        let posix = format!(
            "set -eu\nnvm() {{ :; }}\n{} || echo \"status=$?\"\necho done",
            load_line(name)
        );
        let fish = format!(
            "function nvm; end\n{}\necho \"status=$status\"\necho done",
            load_line(name)
        );
        let run = lab.run_interactive(shell, in_dialect(name, &posix, &fish));
        assert!(run.stderr.contains(FUNCTION), "{name}: {}", run.stderr);
        assert_eq!(run.stdout, "status=3\ndone\n", "{name}: {}", run.stderr);
    });
}

#[test]
fn conflict_prints_on_stderr_only_and_rejects_an_unknown_reason() {
    let run = |reason: &str| {
        std::process::Command::new(init_lab::BINARY)
            .args(["__conflict", reason])
            .output()
            .unwrap()
    };
    let helpers = run("helpers");
    assert_eq!(helpers.status.code(), Some(0));
    assert_eq!(helpers.stdout, b"");
    assert_eq!(
        String::from_utf8_lossy(&helpers.stderr),
        format!("{HELPERS}{ADVICE}")
    );
    let unknown = run("nvm.sh");
    assert_eq!(unknown.status.code(), Some(127));
    assert_eq!(unknown.stdout, b"");
}
```

Apply to `tests/init_lab/mod.rs`:

```diff
--- a/tests/init_lab/mod.rs
+++ b/tests/init_lab/mod.rs
@@ -12,6 +12,8 @@
 use std::process::Command;
 
 pub const BINARY: &str = env!("CARGO_BIN_EXE_nvm");
+/// The same program as [`BINARY`], under the name the startup files call.
+pub const NVMRC_BINARY: &str = env!("CARGO_BIN_EXE_nvmrc");
 pub const SHELLS: [&str; 6] = ["bash", "zsh", "sh", "dash", "ksh", "fish"];
 pub const USING_18: &str = "Now using node v18.20.4 (npm v10.7.0)\n";
 
@@ -53,6 +55,7 @@
             fs::create_dir_all(root.path().join(directory)).unwrap();
         }
         symlink(BINARY, root.path().join("bin/nvm")).unwrap();
+        symlink(NVMRC_BINARY, root.path().join("bin/nvmrc")).unwrap();
         Self { root }
     }
 
@@ -90,6 +93,28 @@
 
     /// Runs `commands` in `shell`, from the project directory.
     pub fn run(&self, shell: &Path, commands: &str) -> Run {
+        self.run_in(shell, &[], commands)
+    }
+
+    /// Runs `commands` in `shell` started with `-i`, as interactive, without
+    /// a terminal: stdin is empty and the output is piped. No startup file
+    /// is read (`--norc --noprofile` for bash, `-f` for zsh, `ENV` unset for
+    /// sh, dash and ksh); bash, sh and dash say on stderr that job control
+    /// is off, ksh writes a newline there and a history file in `$HOME`.
+    pub fn run_interactive(&self, shell: &Path, commands: &str) -> Run {
+        let bash = ["--norc", "--noprofile", "-i"];
+        self.run_in(
+            shell,
+            if shell.ends_with("bash") {
+                &bash
+            } else {
+                &bash[2..]
+            },
+            commands,
+        )
+    }
+
+    fn run_in(&self, shell: &Path, options: &[&str], commands: &str) -> Run {
         let mut command = Command::new(shell);
         if shell.ends_with("zsh") {
             command.arg("-f");
@@ -98,6 +123,7 @@
             command.arg("--no-config");
         }
         let output = command
+            .args(options)
             .args(["-c", commands])
             .env_clear()
             .env("PATH", self.base_path())
@@ -139,6 +165,15 @@
     }
 }
 
+/// The line `nvm migrate` writes: the `nvmrc` binary by name, which a
+/// function named `nvm` defined earlier cannot intercept.
+pub fn load_line(name: &str) -> String {
+    match name {
+        "fish" => "nvmrc init fish | source".to_owned(),
+        _ => format!("eval \"$(nvmrc init {name})\""),
+    }
+}
+
 pub fn with_function(name: &str, commands: &str) -> String {
     format!("{}\n{commands}", init_line(name, " --no-use"))
 }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find module
`runtime_conflict`", "cannot find enum `RuntimeConflict`" and "no variant
named `Conflict` found".

- [ ] **Step 4: Write the implementation**

Apply to `docs/superpowers/specs/2026-10-02-nvmrc-design.md` (a document):

```diff
--- a/docs/superpowers/specs/2026-10-02-nvmrc-design.md
+++ b/docs/superpowers/specs/2026-10-02-nvmrc-design.md
@@ -210,10 +210,15 @@
 
 ### 7.2 Two levels of checking
 
-- **Runtime (cheap, always on):** the `init` snippet exports `NVMRC_SHELL=1`
-  and the binary checks whether `nvm.sh` looks active (`NVM_BIN`, the type of
-  `nvm`). On conflict it warns once per session on stderr with the cause and
-  the fix command. It never fails a command by itself.
+- **Runtime (cheap, interactive shells only):** the binary cannot see shell
+  functions, so the check lives in the `init` snippet. Once, before it
+  defines its own `nvm`, the snippet tests with the shell's builtins whether
+  nvm.sh's helpers are loaded (`type nvm_has`) or `nvm` already is a
+  function that is not nvmrc's (a lazy stub); only then it runs the hidden
+  `nvm __conflict <helpers|function>`, which prints the cause and the fix
+  commands on stderr. It never fails the init; a second init stays silent.
+  The `NVM_CD_FLAGS` hint is not used. A `source nvm.sh` that runs after the
+  init is not seen at runtime: only `nvm doctor` finds it.
 - **On disk (`nvm doctor`, read-only):** scans the profile files and files
   they source (depth 2) and lists each hit with file and line.
 
@@ -229,17 +234,18 @@
 Rules: idempotent via `# >>> nvmrc init >>>` / `# <<< nvmrc init <<<`
 markers; reversible (`--undo` restores the latest backup); old lines are
 commented as `# [nvmrc-migrated] ...`, never deleted; `$NVM_DIR` data is never
-touched. Lazy-loader files (source 3) are never edited automatically: the
-exact snippet and a suggested patch are shown, and the file is changed only
-with an explicit `--yes` for that file.
+touched. Lazy-loader files (source 3) are never edited: the exact lines,
+the manual fix and a suggested patch are shown. The load line written is
+`eval "$(nvmrc init <shell>)"` (fish: `nvmrc init fish | source`): the
+`nvmrc` binary by name, which an `nvm` function cannot intercept.
 
 ### 7.4 Scanning engine
 
-Detection uses the ripgrep library crates (`grep-regex`, `grep-searcher`)
-behind a `LineScanner` port. The domain owns a single `RuleSet` table; the
-adapter supplies line numbers and binary/encoding handling. Tests feed bytes
-through the in-memory `FileSystem` using `search_slice`. The `ignore` walker
-is excluded from v1 (YAGNI): only a closed list of files is scanned.
+Detection uses no ripgrep crates and no `LineScanner` port: the rules are
+line patterns hand-written in the domain (the crate has no regex engine),
+table-driven and unit-tested, and the scanner reads the files through the
+`FileSystem` port. The `ignore` walker is excluded from v1 (YAGNI): only a
+closed list of files is scanned.
 
 ## 8. Testing
 
@@ -269,9 +275,10 @@
 
 ## 10. Dependencies (v1)
 
-`clap`, `thiserror`, `anyhow`, `ureq` (blocking HTTP), `tracing`, `grep-regex`
-and `grep-searcher`. Checksums and archive extraction crates are chosen in the
-implementation plan. MSRV is 1.85 as declared in `Cargo.toml`.
+`clap`, `thiserror`, `anyhow`, `ureq` (blocking HTTP) and `tracing` (no
+ripgrep crates: the conflict rules are hand-written, see 7.4). Checksums and
+archive extraction crates are chosen in the implementation plan. MSRV is
+1.85 as declared in `Cargo.toml`.
 
 ## 11. Implementation order (v1)
 
```

Apply to `src/cli/commands.rs` (above the test module):

```diff
--- a/src/cli/commands.rs
+++ b/src/cli/commands.rs
@@ -148,6 +148,13 @@
         #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
         args: Vec<String>,
     },
+    /// The warning the `nvm` function's init prints when nvm.sh is still
+    /// loaded in an interactive shell (`helpers` or `function`).
+    #[command(name = "__conflict", hide = true)]
+    Conflict {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
 }
 
 pub(super) fn dispatch(command: &Command, context: &Context<'_>) -> Result<Output, CliError> {
@@ -177,6 +184,7 @@
         Command::Doctor { args } => commands::doctor::run(context, args),
         Command::Migrate { args } => commands::migrate::run(context, args),
         Command::Auto { args } => commands::auto::run(context, args),
+        Command::Conflict { args } => commands::runtime_conflict::run(args),
     }
 }
 
```

Insert above the `#[cfg(test)]` line of `src/commands/runtime_conflict.rs`:

```rust
//! `nvm __conflict <helpers|function>`: the warning the init snippet has the
//! binary print, on stderr, when nvm.sh is still loaded in an interactive
//! shell (see [`crate::domain::conflict::RuntimeConflict`]). Exits 0: the
//! warning never fails the shell's start.

use crate::commands::Output;
use crate::domain::conflict::{RuntimeConflict, runtime_warning};
use crate::error::CliError;

/// How `nvm __conflict` is called.
pub const USAGE: &str = "Usage: nvm __conflict <helpers|function>";

/// Prints the warning for the reason named in `args`.
///
/// # Errors
/// [`CliError::Usage`] without exactly one argument naming a reason.
pub fn run(args: &[String]) -> Result<Output, CliError> {
    match args {
        [name] => RuntimeConflict::from_name(name)
            .map(|reason| Output::default().with_stderr(runtime_warning(reason)))
            .ok_or_else(usage),
        _ => Err(usage()),
    }
}

fn usage() -> CliError {
    CliError::Usage(USAGE.to_owned())
}

```

Apply to `src/domain/conflict/mod.rs` (above the test module):

```diff
--- a/src/domain/conflict/mod.rs
+++ b/src/domain/conflict/mod.rs
@@ -20,6 +20,7 @@
 mod nesting;
 mod plugins;
 mod rules;
+mod runtime;
 mod source_path;
 
 #[cfg(test)]
@@ -33,4 +34,6 @@
 #[cfg(test)]
 mod rules_tests;
 #[cfg(test)]
+mod runtime_tests;
+#[cfg(test)]
 mod source_path_tests;
```

Create `src/domain/conflict/runtime.rs`:

```rust
//! The warning the init snippet has the binary print when, at shell start,
//! nvm.sh (or a lazy `nvm` stub) is already loaded in the shell. Only the
//! shell can see its functions, so the snippet runs the checks and passes
//! the binary the reason (`nvm __conflict <reason>`); the text lives here.

/// Why the snippet found nvm.sh still loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeConflict {
    /// nvm.sh's helper functions (`nvm_has`) are defined.
    Helpers,
    /// `nvm` was already a function that is not nvmrc's (a lazy stub).
    Function,
}

/// The lines under the first line of the warning, whatever the reason.
pub const RUNTIME_WARNING_ADVICE: &str = "  nvmrc now answers `nvm`, but the old loader keeps running on \
every shell start;\n  run `nvm doctor` to see where, and `nvm migrate` to move it";

/// The first line of the warning, before the reason in parentheses.
const HEADLINE: &str = "nvm: nvm.sh is still loaded in this shell";

impl RuntimeConflict {
    /// Every reason, in the order the snippet checks them.
    pub const ALL: [Self; 2] = [Self::Helpers, Self::Function];

    /// The argument of `nvm __conflict`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Helpers => "helpers",
            Self::Function => "function",
        }
    }

    /// What the snippet saw, in the first line of the warning.
    const fn why(self) -> &'static str {
        match self {
            Self::Helpers => "its helper functions are defined",
            Self::Function => "nvm was already a function",
        }
    }

    /// The reason named `name`, if any.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|reason| reason.name() == name)
    }
}

/// The whole warning, without a trailing newline.
#[must_use]
pub fn runtime_warning(reason: RuntimeConflict) -> String {
    format!("{HEADLINE} ({})\n{RUNTIME_WARNING_ADVICE}", reason.why())
}
```

Create `src/domain/conflict/runtime_tests.rs`:

```rust
use super::runtime::{RUNTIME_WARNING_ADVICE, RuntimeConflict, runtime_warning};

#[test]
fn each_reason_has_the_name_the_snippet_passes() {
    assert_eq!(RuntimeConflict::Helpers.name(), "helpers");
    assert_eq!(RuntimeConflict::Function.name(), "function");
    for reason in RuntimeConflict::ALL {
        assert_eq!(RuntimeConflict::from_name(reason.name()), Some(reason));
    }
}

#[test]
fn other_names_are_no_reason() {
    for name in ["", "Helpers", "nvm.sh", "function ", "helper"] {
        assert_eq!(RuntimeConflict::from_name(name), None, "{name}");
    }
}

#[test]
fn the_advice_is_the_same_for_every_reason() {
    assert_eq!(
        RUNTIME_WARNING_ADVICE,
        "  nvmrc now answers `nvm`, but the old loader keeps running on every shell start;\n  \
         run `nvm doctor` to see where, and `nvm migrate` to move it"
    );
    for reason in RuntimeConflict::ALL {
        assert!(runtime_warning(reason).ends_with(RUNTIME_WARNING_ADVICE));
    }
}

#[test]
fn the_warning_names_why_on_its_first_line() {
    assert_eq!(
        runtime_warning(RuntimeConflict::Helpers),
        format!(
            "nvm: nvm.sh is still loaded in this shell (its helper functions are defined)\n\
             {RUNTIME_WARNING_ADVICE}"
        )
    );
    assert_eq!(
        runtime_warning(RuntimeConflict::Function),
        format!(
            "nvm: nvm.sh is still loaded in this shell (nvm was already a function)\n\
             {RUNTIME_WARNING_ADVICE}"
        )
    );
}
```

Create `src/shell/init/conflict_check.rs`:

```rust
//! What the snippet runs first, in an interactive shell only: whether nvm.sh
//! is still loaded (digest 4.2). The binary cannot see shell functions, so
//! the shell checks, once, before the snippet defines its own `nvm`, and
//! only when a check fires runs `nvm __conflict <reason>` (the message lives
//! in [`crate::domain::conflict::runtime_warning`]) on stderr. The checks
//! are builtins (no fork, except `$(...)` as noted); their status and the
//! binary's are ignored (`|| true`), so a `set -eu` shell starts anyway.
//!
//! First `helpers`: nvm.sh's `nvm_has` is defined (`type` is a builtin in
//! every shell; dash returns 127 when it is missing). Then `function`: `nvm`
//! is already a function that is not nvmrc's (a lazy stub, oh-my-zsh's or
//! zsh-nvm's lazy mode). nvmrc's function is recognized by its
//! `NVMRC_SCRIPT_FD`, so a second init in the same shell stays silent. sh
//! and dash cannot print a function's body: they skip this check when
//! `NVMRC_SHELL` is already set (an init already ran, here or in a parent).

use super::Shell;
use crate::domain::conflict::RuntimeConflict;

/// zsh: `nvm` is in the table of functions, and its body is not ours.
const ZSH_FOREIGN_FUNCTION: &str =
    "(( ${+functions[nvm]} )) && [[ ${functions[nvm]} != *NVMRC_SCRIPT_FD* ]]";
/// sh and dash: before any init, `command -v` names a function by its name.
const SH_FOREIGN_FUNCTION: &str =
    "[ -z \"${NVMRC_SHELL+set}\" ] && [ \"$(command -v nvm)\" = nvm ]";

/// bash and ksh: whether `nvm` is a function, then whether its body is ours.
fn by_body(defined: &str, body: &str, warn: &str) -> String {
    format!(
        "    elif {defined} >/dev/null 2>&1; then\n      \
         case \"$({body})\" in\n        *NVMRC_SCRIPT_FD*) ;;\n        \
         *) {warn} ;;\n      esac\n"
    )
}

fn posix_warn(reason: RuntimeConflict) -> String {
    format!("command \\nvm __conflict {} >&2 || true", reason.name())
}

fn posix(shell: Shell) -> String {
    let warn = posix_warn(RuntimeConflict::Function);
    let function_check = match shell {
        Shell::Bash => by_body("declare -F nvm", "declare -f nvm", &warn),
        Shell::Ksh => by_body("typeset -f nvm", "typeset -f nvm", &warn),
        Shell::Zsh => format!("    elif {ZSH_FOREIGN_FUNCTION}; then\n      {warn}\n"),
        _ => format!("    elif {SH_FOREIGN_FUNCTION}; then\n      {warn}\n"),
    };
    format!(
        "case $- in\n  *i*)\n    if type nvm_has >/dev/null 2>&1; then\n      {}\n\
         {function_check}    fi\n    ;;\nesac\n",
        posix_warn(RuntimeConflict::Helpers)
    )
}

fn fish() -> String {
    format!(
        "if status is-interactive\n    if functions -q nvm_has\n        \
         command nvm __conflict {} >&2\n    \
         else if functions -q nvm; and not functions nvm | string match -q '*NVMRC_SCRIPT_FD*'\n        \
         command nvm __conflict {} >&2\n    end\nend\n",
        RuntimeConflict::Helpers.name(),
        RuntimeConflict::Function.name()
    )
}

/// The check for `shell`, newline-terminated.
pub(super) fn conflict_check(shell: Shell) -> String {
    match shell {
        Shell::Fish => fish(),
        _ => posix(shell),
    }
}
```

Create `src/shell/init/conflict_check_tests.rs`:

```rust
use super::*;

const POSIX: [Shell; 5] = [Shell::Bash, Shell::Zsh, Shell::Sh, Shell::Dash, Shell::Ksh];

const HELPERS_POSIX: &str = "    if type nvm_has >/dev/null 2>&1; then\n      \
                             command \\nvm __conflict helpers >&2 || true\n";
const WARN_FUNCTION_POSIX: &str = "command \\nvm __conflict function >&2 || true";

fn text(shell: Shell) -> String {
    snippet(shell, &InitOptions::default())
}

/// The position of `needle` in `haystack`, which must contain it.
fn position(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in {haystack}"))
}

#[test]
fn posix_shells_check_only_when_interactive_and_before_anything_else() {
    for shell in POSIX {
        let text = text(shell);
        let check = position(&text, "\ncase $- in\n  *i*)\n");
        assert!(
            check < position(&text, "\nexport NVMRC_SHELL=1\n"),
            "{text}"
        );
        assert!(check < position(&text, "\nunalias nvm"), "{text}");
        assert!(text.contains("\n    ;;\nesac\n"), "{text}");
    }
}

#[test]
fn posix_shells_warn_first_about_the_helpers_of_nvm_sh() {
    for shell in POSIX {
        let text = text(shell);
        assert!(text.contains(HELPERS_POSIX), "{text}");
        assert!(
            position(&text, HELPERS_POSIX) < position(&text, WARN_FUNCTION_POSIX),
            "{text}"
        );
        assert_eq!(text.matches("__conflict").count(), 2, "{text}");
    }
}

#[test]
fn bash_and_ksh_warn_about_a_function_that_is_not_theirs() {
    let cases = [
        (Shell::Bash, "declare -F nvm", "declare -f nvm"),
        (Shell::Ksh, "typeset -f nvm", "typeset -f nvm"),
    ];
    for (shell, defined, body) in cases {
        let text = text(shell);
        let expected = format!(
            "    elif {defined} >/dev/null 2>&1; then\n      \
             case \"$({body})\" in\n        *NVMRC_SCRIPT_FD*) ;;\n        \
             *) {WARN_FUNCTION_POSIX} ;;\n      esac\n"
        );
        assert!(text.contains(&expected), "{text}");
    }
}

#[test]
fn zsh_reads_its_functions_table() {
    let text = text(Shell::Zsh);
    let expected = format!(
        "    elif (( ${{+functions[nvm]}} )) && [[ ${{functions[nvm]}} != *NVMRC_SCRIPT_FD* ]]; then\n      \
         {WARN_FUNCTION_POSIX}\n"
    );
    assert!(text.contains(&expected), "{text}");
}

#[test]
fn sh_and_dash_cannot_tell_whose_function_so_skip_a_second_init() {
    for shell in [Shell::Sh, Shell::Dash] {
        let text = text(shell);
        let expected = format!(
            "    elif [ -z \"${{NVMRC_SHELL+set}}\" ] && [ \"$(command -v nvm)\" = nvm ]; then\n      \
             {WARN_FUNCTION_POSIX}\n"
        );
        assert!(text.contains(&expected), "{text}");
        assert!(!text.contains("declare"), "{text}");
        assert!(!text.contains("typeset"), "{text}");
    }
}

#[test]
fn fish_checks_only_when_interactive_and_before_anything_else() {
    let text = text(Shell::Fish);
    let expected = "\nif status is-interactive\n    if functions -q nvm_has\n        \
                    command nvm __conflict helpers >&2\n    \
                    else if functions -q nvm; and not functions nvm | string match -q '*NVMRC_SCRIPT_FD*'\n        \
                    command nvm __conflict function >&2\n    end\nend\n";
    assert!(text.contains(expected), "{text}");
    assert!(
        position(&text, expected) < position(&text, "\nset -gx NVMRC_SHELL 1\n"),
        "{text}"
    );
}

#[test]
fn the_check_does_not_depend_on_the_options() {
    let no_use = InitOptions {
        no_use: true,
        install: false,
    };
    for shell in SHELLS {
        let text = snippet(shell, &no_use);
        assert_eq!(text.matches("__conflict helpers").count(), 1, "{text}");
        assert_eq!(text.matches("__conflict function").count(), 1, "{text}");
    }
}
```

Apply to `src/shell/init/mod.rs` (above the test module):

```diff
--- a/src/shell/init/mod.rs
+++ b/src/shell/init/mod.rs
@@ -23,12 +23,14 @@
 //! "$(nvm init bash)"` (and `nvm init fish | source`) is 3 when the
 //! `.nvmrc` names a missing version.
 
+mod conflict_check;
 mod fish;
 mod posix;
 
 use std::str::FromStr;
 
 use crate::error::CliError;
+use conflict_check::conflict_check;
 
 /// The first and last lines of the snippet (defined with the conflict rules,
 /// which skip the block).
@@ -161,9 +163,12 @@
     format!(
         "{BEGIN_MARKER}\n\
 # The nvm function of nvmrc, from `{load}` in the {name} startup file.\n\
-{code}{END_MARKER}\n",
+{check}{code}{END_MARKER}\n",
+        check = conflict_check(shell),
         load = shell.load_line(),
         name = shell.name(),
     )
 }
 
+#[cfg(test)]
+mod conflict_check_tests;
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1336 unit tests plus the end-to-end tests (the fish cases skip
when `fish` is not installed).

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(init): warn when nvm.sh is already loaded in an interactive shell"
```

---

### Task 8: Limits, a real profile and the gate

**Files:**

- Modify: `src/commands/doctor/fix.rs`, `src/commands/doctor/fix_tests.rs`,
  `src/commands/doctor/render.rs`, `src/commands/doctor/render_tests.rs`,
  `src/commands/doctor/tests.rs`, `src/domain/conflict/expand_tests.rs`,
  `src/domain/conflict/lexer.rs`, `src/domain/conflict/lexer_tests.rs`,
  `src/domain/conflict/rules_tests.rs`, `src/domain/conflict/source_path.rs`,
  `src/domain/conflict/source_path_tests.rs`, `tests/migrate_cli.rs`

**Interfaces:**

- Produces: nothing new for users, and three fixes found by running `nvm doctor`
  and `nvm migrate --dry-run` on a real zsh setup: a `source` or `.` must be in
  command position (after an operator, a reserved word, assignments, or the
  start of a quote left open), because `find . -name` was read as a `source` of
  `-name` and produced false "relative path" and "file not found" lines
  (`open_quote_contents` in `lexer.rs`); a by-hand completion line is told to be
  deleted, not replaced with an init line (which would add a second one); and
  each stub says what it hides (`unfunction nvm` has its own text). Eleven Plan
  8 test functions that grew past 30 lines are split or their tables moved to
  constants.

- [ ] **Step 1: Write the failing tests**

Apply to `src/commands/doctor/tests.rs`:

```diff
--- a/src/commands/doctor/tests.rs
+++ b/src/commands/doctor/tests.rs
@@ -62,38 +62,43 @@
     assert_eq!(output.status, NvmExitCode::Failure);
 }
 
+const STUB_FIX: &str = "      fix: the stub loads nvm.sh on first use, and an `nvm` stub after \
+the init line hides nvmrc: delete it";
+
+/// The block of the user's `.zshrc`, then the one of `lazy-functions.zsh`.
+const USERS_FILE_BLOCKS: [&str; 16] = [
+    "/Users/u/.zshrc -> /Users/u/dotfiles/zsh/.zshrc",
+    "  3: oh-my-zsh nvm plugin  nvm",
+    "      fix: remove `nvm` from `plugins=(...)`; it does nothing while the nvmrc binary is \
+on PATH but depends on it",
+    "",
+    "/Users/u/dotfiles/zsh/scripts/lazy-functions.zsh",
+    "  1: lazy stub  nvm() {",
+    STUB_FIX,
+    "  2: nvm unset  unfunction nvm node npm npx yarn pnpm 2>/dev/null",
+    "      fix: it removes a lazy stub once nvm.sh is loaded: delete it with the stub",
+    "  3: lazy loader  [ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"",
+    "      fix: remove or replace this loader by hand: nvmrc has its own function (see \
+`eval \"$(nvmrc init <shell>)\"`)",
+    "      patch: replace the line with: eval \"$(nvmrc init zsh)\"",
+    "  6: lazy stub  npm() { nvm >/dev/null 2>&1; command npm \"$@\" }",
+    STUB_FIX,
+    "",
+    "Info:",
+];
+
 #[test]
 fn the_users_tree_shows_the_stubs_the_plugin_hint_and_the_links() {
     let output = report(&users_dotfiles(), &users_env(), &["--shell", "zsh"]);
-    let stub = "      fix: the stub redefines `nvm` after the init line: delete it, or move the \
-init line after it";
-    let expected = [
-        "nvm doctor: scanned 7 file(s)",
-        "",
-        "/Users/u/.zshrc -> /Users/u/dotfiles/zsh/.zshrc",
-        "  3: oh-my-zsh nvm plugin  nvm",
-        "      fix: remove `nvm` from `plugins=(...)`; it does nothing while the nvmrc binary is \
-on PATH but depends on it",
-        "",
-        "/Users/u/dotfiles/zsh/scripts/lazy-functions.zsh",
-        "  1: lazy stub  nvm() {",
-        stub,
-        "  2: nvm unset  unfunction nvm node npm npx yarn pnpm 2>/dev/null",
-        stub,
-        "  3: lazy loader  [ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"",
-        "      fix: remove or replace this loader by hand: nvmrc has its own function (see \
-`eval \"$(nvmrc init <shell>)\"`)",
-        "      patch: replace the line with: eval \"$(nvmrc init zsh)\"",
-        "  6: lazy stub  npm() { nvm >/dev/null 2>&1; command npm \"$@\" }",
-        stub,
-        "",
-        "Info:",
+    let mut expected = vec!["nvm doctor: scanned 7 file(s)", ""];
+    expected.extend(USERS_FILE_BLOCKS);
+    expected.extend([
         "  /Users/u/.zshenv:3: NVM_DIR export: kept by `nvm migrate`",
         "  /Users/u/.oh-my-zsh/oh-my-zsh.sh:2: not followed: unset $plugin",
         "  /Users/u/dotfiles/zsh/scripts/lazy-functions.zsh:3: not followed: unset $nvm_prefix",
         "",
         "Result: 4 conflict(s)",
-    ];
+    ]);
     assert_eq!(output.stdout, expected.join("\n"));
     assert_eq!(output.status, NvmExitCode::Failure);
 }
@@ -171,6 +176,7 @@
     );
     assert_eq!(output.status, NvmExitCode::Success);
 }
+
 #[test]
 fn nvm_sh_and_its_completion_are_reported_as_loaders_and_never_scanned() {
     let fs = FakeFileSystem::default()
```

Apply to `tests/migrate_cli.rs`:

```diff
--- a/tests/migrate_cli.rs
+++ b/tests/migrate_cli.rs
@@ -106,16 +106,23 @@
     assert_eq!(backups(home.path(), ".bash_profile").len(), 1);
 }
 
-#[test]
-fn a_symlinked_profile_keeps_its_link_and_permissions() {
-    let home = TempDir::new().unwrap();
-    let repository = home.path().join("dotfiles/bash");
+/// A stow-like `~/.bash_profile` linked to `dotfiles/bash/.bash_profile`
+/// (mode 0640): the repository directory, the target and the link.
+fn stowed_profile(home: &Path) -> (PathBuf, PathBuf, PathBuf) {
+    let repository = home.join("dotfiles/bash");
     fs::create_dir_all(&repository).unwrap();
     let target = repository.join(".bash_profile");
     fs::write(&target, PROFILE).unwrap();
     fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
-    let link = home.path().join(".bash_profile");
+    let link = home.join(".bash_profile");
     symlink("dotfiles/bash/.bash_profile", &link).unwrap();
+    (repository, target, link)
+}
+
+#[test]
+fn a_symlinked_profile_keeps_its_link_and_permissions() {
+    let home = TempDir::new().unwrap();
+    let (repository, target, link) = stowed_profile(home.path());
     let migrated = nvm(home.path(), &["migrate", "--yes"]);
     assert_eq!(
         migrated.status.code(),
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL by assertion: `find . -name` is read as a `source` of `-name`,
a by-hand completion line is told to be replaced with an init line, and every
stub is told that it redefines `nvm`; eleven test functions are over 30 lines.

- [ ] **Step 3: Apply the changes**

Apply to `src/commands/doctor/fix.rs` (above the test module):

```diff
--- a/src/commands/doctor/fix.rs
+++ b/src/commands/doctor/fix.rs
@@ -8,8 +8,10 @@
 const AUTO: &str = "run `nvm migrate` (comments the line out and adds the nvmrc init line)";
 const MANUAL_LOADER: &str = "remove or replace this loader by hand: nvmrc has its own function \
 (see `eval \"$(nvmrc init <shell>)\"`)";
-const STUB: &str = "the stub redefines `nvm` after the init line: delete it, or move the init \
-line after it";
+const STUB: &str = "the stub loads nvm.sh on first use, and an `nvm` stub after the init line \
+hides nvmrc: delete it";
+const UNSET: &str = "it removes a lazy stub once nvm.sh is loaded: delete it with the stub";
+const DELETE_COMPLETION: &str = "delete the line: nvm's bash_completion needs nvm.sh";
 const OMZ: &str = "remove `nvm` from `plugins=(...)`; it does nothing while the nvmrc binary \
 is on PATH but depends on it";
 const ZSH_NVM: &str = "remove zsh-nvm";
@@ -26,7 +28,8 @@
     match kind {
         Kind::Loader | Kind::Completion if depth == 0 => AUTO,
         Kind::Loader | Kind::Completion | Kind::LazyLoader => MANUAL_LOADER,
-        Kind::LazyStub | Kind::Unset => STUB,
+        Kind::LazyStub => STUB,
+        Kind::Unset => UNSET,
         Kind::OmzPlugin => OMZ,
         Kind::ZshNvm => ZSH_NVM,
         Kind::Bass => BASS,
@@ -36,11 +39,16 @@
     }
 }
 
-/// The one-line patch suggested for a loader fixed by hand, `None` for the
-/// kinds that have none.
+/// The one-line patch suggested for a loader fixed by hand (`text` is its
+/// line), `None` for the kinds that have none. A completion without nvm.sh
+/// on its line is deleted: nvmrc has no completion to put in its place.
 #[must_use]
-pub fn suggested_patch(kind: Kind, depth: usize, path: &Path) -> Option<String> {
-    let by_hand = matches!(kind, Kind::LazyLoader) || (depth > 0 && kind == Kind::Loader);
+pub fn suggested_patch(kind: Kind, depth: usize, path: &Path, text: &str) -> Option<String> {
+    let loader_or_completion = matches!(kind, Kind::Loader | Kind::Completion);
+    let by_hand = kind == Kind::LazyLoader || (depth > 0 && loader_or_completion);
+    if by_hand && !text.contains("nvm.sh") {
+        return Some(DELETE_COMPLETION.to_owned());
+    }
     by_hand.then(|| {
         let shell = shell_of(path);
         let line = if shell == "fish" {
```

Apply to `src/commands/doctor/fix_tests.rs` (above the test module):

```diff
--- a/src/commands/doctor/fix_tests.rs
+++ b/src/commands/doctor/fix_tests.rs
@@ -27,10 +27,15 @@
 
 #[test]
 fn the_other_kinds_have_their_own_advice() {
-    let stub = "the stub redefines `nvm` after the init line: delete it, or move the init \
-line after it";
-    assert_eq!(fix_text(Kind::LazyStub, 1), stub);
-    assert_eq!(fix_text(Kind::Unset, 1), stub);
+    assert_eq!(
+        fix_text(Kind::LazyStub, 1),
+        "the stub loads nvm.sh on first use, and an `nvm` stub after the init line \
+hides nvmrc: delete it"
+    );
+    assert_eq!(
+        fix_text(Kind::Unset, 1),
+        "it removes a lazy stub once nvm.sh is loaded: delete it with the stub"
+    );
     assert!(fix_text(Kind::OmzPlugin, 0).starts_with("remove `nvm` from `plugins=(...)`; it does"));
     assert_eq!(fix_text(Kind::ZshNvm, 0), "remove zsh-nvm");
     assert_eq!(fix_text(Kind::Bass, 0), "remove the bass line");
@@ -40,7 +45,8 @@
 
 #[test]
 fn only_loaders_fixed_by_hand_get_a_patch_in_the_shell_of_the_file() {
-    let patch = |kind, depth, path| suggested_patch(kind, depth, Path::new(path));
+    let loader = "[ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"";
+    let patch = |kind, depth, path| suggested_patch(kind, depth, Path::new(path), loader);
     assert_eq!(patch(Kind::Loader, 0, "/h/.bashrc"), None);
     assert_eq!(patch(Kind::LazyStub, 1, "/h/lazy.zsh"), None);
     let by_hand = |shell: &str| {
@@ -56,3 +62,20 @@
         Some("replace the line with: nvmrc init fish | source".to_owned())
     );
 }
+
+/// The user's `lazy-functions.zsh:18`: a completion has no init line to
+/// replace it with, nvmrc ships none.
+#[test]
+fn a_completion_fixed_by_hand_is_deleted() {
+    let completion = "  [ -s \"${nvm_prefix}/etc/bash_completion.d/nvm\" ] && \\. \"${nvm_prefix}/etc/bash_completion.d/nvm\"";
+    let delete = Some("delete the line: nvm's bash_completion needs nvm.sh".to_owned());
+    let path = Path::new("/h/lazy-functions.zsh");
+    assert_eq!(
+        suggested_patch(Kind::LazyLoader, 1, path, completion),
+        delete
+    );
+    assert_eq!(
+        suggested_patch(Kind::Completion, 1, path, completion),
+        delete
+    );
+}
```

Apply to `src/commands/doctor/render.rs` (above the test module):

```diff
--- a/src/commands/doctor/render.rs
+++ b/src/commands/doctor/render.rs
@@ -47,7 +47,7 @@
         hit.text.trim(),
         fix_text(hit.kind, file.depth)
     );
-    if let Some(patch) = suggested_patch(hit.kind, file.depth, &file.path) {
+    if let Some(patch) = suggested_patch(hit.kind, file.depth, &file.path, &hit.text) {
         let _ = write!(lines, "\n      patch: {patch}");
     }
     lines
```

Apply to `src/commands/doctor/render_tests.rs` (above the test module):

```diff
--- a/src/commands/doctor/render_tests.rs
+++ b/src/commands/doctor/render_tests.rs
@@ -65,9 +65,9 @@
     assert!(text.ends_with("Result: 1 conflict(s)"));
 }
 
-#[test]
-fn info_lists_the_kept_exports_then_the_notes() {
-    let report = Report {
+/// An `NVM_DIR` export, a path that cannot be followed, a file not read.
+fn exports_and_notes() -> Report {
+    Report {
         files: vec![file(
             "/h/.zshenv",
             "/h/.zshenv",
@@ -91,9 +91,13 @@
                 reason: NoteReason::Unreadable("denied".to_owned()),
             },
         ],
-    };
+    }
+}
+
+#[test]
+fn info_lists_the_kept_exports_then_the_notes() {
     assert_eq!(
-        render(&report),
+        render(&exports_and_notes()),
         "nvm doctor: scanned 1 file(s)\n\n\
 Info:\n  /h/.zshenv:1: NVM_DIR export: kept by `nvm migrate`\n  \
 /h/.zshenv:2: not followed: unset $X\n  /h/gone: not read: denied\n\n\
```

Apply to `src/domain/conflict/expand_tests.rs` (above the test module):

```diff
--- a/src/domain/conflict/expand_tests.rs
+++ b/src/domain/conflict/expand_tests.rs
@@ -72,37 +72,38 @@
     );
 }
 
+const UNRESOLVABLE_ROWS: &[(&str, &str)] = &[
+    (
+        "$(brew --prefix nvm)/nvm.sh",
+        "command substitution $(brew --prefix nvm)",
+    ),
+    (
+        "`brew --prefix nvm`/nvm.sh",
+        "command substitution `brew --prefix nvm`/nvm.sh",
+    ),
+    ("${0:A:h}/x.zsh", "unsupported ${0:A:h}"),
+    ("$f", "unset $f"),
+    ("$FOO/x", "unset $FOO"),
+    ("${FOO}/x", "unset $FOO"),
+    (
+        "${NVM_DIR:-$HOME/.nvm}/nvm.sh",
+        "unsupported ${NVM_DIR:-$HOME/.nvm}",
+    ),
+    ("${#HOME}", "unsupported ${#HOME}"),
+    ("${list[@]}", "unsupported ${list[@]}"),
+    ("$1/x", "unsupported $1/x"),
+    ("$@", "unsupported $@"),
+    ("x$", "unsupported $"),
+    ("${HOME", "unsupported ${HOME"),
+    ("~user/x", "unsupported ~user/x"),
+    ("$HOME/conf.d/*.zsh", "glob /home/me/conf.d/*.zsh"),
+    ("$HOME/file?.sh", "glob /home/me/file?.sh"),
+    ("$HOME/[ab].sh", "glob /home/me/[ab].sh"),
+];
+
 #[test]
 fn anything_left_unexpanded_is_unresolvable_with_a_reason() {
-    let rows = [
-        (
-            "$(brew --prefix nvm)/nvm.sh",
-            "command substitution $(brew --prefix nvm)",
-        ),
-        (
-            "`brew --prefix nvm`/nvm.sh",
-            "command substitution `brew --prefix nvm`/nvm.sh",
-        ),
-        ("${0:A:h}/x.zsh", "unsupported ${0:A:h}"),
-        ("$f", "unset $f"),
-        ("$FOO/x", "unset $FOO"),
-        ("${FOO}/x", "unset $FOO"),
-        (
-            "${NVM_DIR:-$HOME/.nvm}/nvm.sh",
-            "unsupported ${NVM_DIR:-$HOME/.nvm}",
-        ),
-        ("${#HOME}", "unsupported ${#HOME}"),
-        ("${list[@]}", "unsupported ${list[@]}"),
-        ("$1/x", "unsupported $1/x"),
-        ("$@", "unsupported $@"),
-        ("x$", "unsupported $"),
-        ("${HOME", "unsupported ${HOME"),
-        ("~user/x", "unsupported ~user/x"),
-        ("$HOME/conf.d/*.zsh", "glob /home/me/conf.d/*.zsh"),
-        ("$HOME/file?.sh", "glob /home/me/file?.sh"),
-        ("$HOME/[ab].sh", "glob /home/me/[ab].sh"),
-    ];
-    for (token, reason) in rows {
+    for &(token, reason) in UNRESOLVABLE_ROWS {
         assert_eq!(
             expand_path(token, &environment),
             unresolvable(reason),
```

Apply to `src/domain/conflict/lexer.rs` (above the test module):

```diff
--- a/src/domain/conflict/lexer.rs
+++ b/src/domain/conflict/lexer.rs
@@ -42,6 +42,22 @@
         previous = Some(character);
     }
     line
+}
+
+/// Where the text of a quote left open at the end of `code` starts (the byte
+/// after the opening quote); `None` when every quote is closed.
+pub(super) fn open_quote_contents(code: &str) -> Option<usize> {
+    let mut quote = Quote::None;
+    let mut escaped = false;
+    let mut contents = None;
+    for (index, character) in code.char_indices() {
+        let opening = quote == Quote::None;
+        (quote, escaped) = step(quote, escaped, character);
+        if opening && quote != Quote::None {
+            contents = Some(index + character.len_utf8());
+        }
+    }
+    contents.filter(|_| quote != Quote::None)
 }
 
 /// The quote state after `character`; `escaped` is whether it is escaped.
```

Apply to `src/domain/conflict/lexer_tests.rs` (above the test module):

```diff
--- a/src/domain/conflict/lexer_tests.rs
+++ b/src/domain/conflict/lexer_tests.rs
@@ -35,7 +35,7 @@
 }
 
 #[test]
-fn tokens_split_words_and_operators_and_unquote() {
+fn tokens_split_words_and_operators() {
     assert_eq!(
         tokens("nvm() {"),
         [
@@ -57,7 +57,10 @@
             word("then"),
         ]
     );
-    assert_eq!(tokens("  'nvm' \"node\""), [quoted("nvm"), quoted("node")]);
+}
+
+#[test]
+fn tokens_split_runs_of_operators() {
     assert_eq!(
         tokens("a&&b|c"),
         [
@@ -69,6 +72,11 @@
             word("c"),
         ]
     );
+}
+
+#[test]
+fn tokens_unquote_and_mark_quoted_words() {
+    assert_eq!(tokens("  'nvm' \"node\""), [quoted("nvm"), quoted("node")]);
     assert_eq!(tokens("\\{ x"), [quoted("{"), word("x")]);
     assert_eq!(
         tokens("echo \"a \\\" b\""),
```

Apply to `src/domain/conflict/rules_tests.rs` (above the test module):

```diff
--- a/src/domain/conflict/rules_tests.rs
+++ b/src/domain/conflict/rules_tests.rs
@@ -15,243 +15,253 @@
 
 /// `exp/corpus.sh`, line by line (digest section 7: 13 true positives, the
 /// bass line now its own kind, and the rejects of lines 19 to 22).
+const CORPUS_ROWS: &[(&str, &[Kind])] = &[
+    ("export NVM_DIR=\"$HOME/.nvm\"", &[NvmDirExport]),
+    (
+        "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm",
+        &[Loader],
+    ),
+    (
+        "[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion",
+        &[Completion],
+    ),
+    (
+        "[ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"  # This loads nvm",
+        &[Loader],
+    ),
+    (
+        "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\" --no-use # This loads nvm, without auto-using the default version",
+        &[Loader],
+    ),
+    (
+        "export NVM_DIR=\"$([ -z \"${XDG_CONFIG_HOME-}\" ] && printf %s \"${HOME}/.nvm\" || printf %s \"${XDG_CONFIG_HOME}/nvm\")\"",
+        &[NvmDirExport],
+    ),
+    (
+        "[[ -r $NVM_DIR/bash_completion ]] && \\. $NVM_DIR/bash_completion",
+        &[Completion],
+    ),
+    (
+        "  [ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ] && \\. \"/opt/homebrew/opt/nvm/nvm.sh\"  # This loads nvm",
+        &[Loader],
+    ),
+    (
+        "  [ -s \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\"  # This loads nvm bash_completion",
+        &[Completion],
+    ),
+    ("source $(brew --prefix nvm)/nvm.sh", &[Loader]),
+    (". \"$(brew --prefix nvm)/nvm.sh\"", &[Loader]),
+    (
+        "[ -s \"$HOMEBREW_PREFIX/opt/nvm/nvm.sh\" ] && \\. \"$HOMEBREW_PREFIX/opt/nvm/nvm.sh\"",
+        &[Loader],
+    ),
+    (
+        "  [ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"",
+        &[Loader],
+    ),
+    (
+        "  [ -s \"${nvm_prefix}/etc/bash_completion.d/nvm\" ] && \\. \"${nvm_prefix}/etc/bash_completion.d/nvm\"",
+        &[Completion],
+    ),
+    ("source ~/.nvm/nvm.sh", &[Loader]),
+    (
+        "nvm() { unset -f nvm; . \"$NVM_DIR/nvm.sh\"; nvm \"$@\"; }",
+        &[LazyStub, Unset, Loader],
+    ),
+    (
+        "      [[ -f \\\"\\$NVM_DIR/nvm.sh\\\" ]] && source \\\"\\$NVM_DIR/nvm.sh\\\"",
+        &[Loader],
+    ),
+    (
+        "  bass source ~/.nvm/nvm.sh --no-use ';' nvm $argv",
+        &[Bass],
+    ),
+    (
+        "# [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"",
+        &[],
+    ),
+    (
+        "# [nvmrc-migrated] [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"",
+        &[],
+    ),
+    ("echo dnvm.sh", &[]),
+    (". ~/.dnx/dnvm/dnvm.sh", &[]),
+    ("source \"$HOME/.nvm/nvm.sh\"", &[Loader]),
+];
+
 #[test]
 fn the_corpus_of_the_digest() {
-    assert_rows(&[
-        ("export NVM_DIR=\"$HOME/.nvm\"", &[NvmDirExport]),
-        (
-            "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"  # This loads nvm",
-            &[Loader],
-        ),
-        (
-            "[ -s \"$NVM_DIR/bash_completion\" ] && \\. \"$NVM_DIR/bash_completion\"  # This loads nvm bash_completion",
-            &[Completion],
-        ),
-        (
-            "[ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"  # This loads nvm",
-            &[Loader],
-        ),
-        (
-            "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\" --no-use # This loads nvm, without auto-using the default version",
-            &[Loader],
-        ),
-        (
-            "export NVM_DIR=\"$([ -z \"${XDG_CONFIG_HOME-}\" ] && printf %s \"${HOME}/.nvm\" || printf %s \"${XDG_CONFIG_HOME}/nvm\")\"",
-            &[NvmDirExport],
-        ),
-        (
-            "[[ -r $NVM_DIR/bash_completion ]] && \\. $NVM_DIR/bash_completion",
-            &[Completion],
-        ),
-        (
-            "  [ -s \"/opt/homebrew/opt/nvm/nvm.sh\" ] && \\. \"/opt/homebrew/opt/nvm/nvm.sh\"  # This loads nvm",
-            &[Loader],
-        ),
-        (
-            "  [ -s \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"/opt/homebrew/opt/nvm/etc/bash_completion.d/nvm\"  # This loads nvm bash_completion",
-            &[Completion],
-        ),
-        ("source $(brew --prefix nvm)/nvm.sh", &[Loader]),
-        (". \"$(brew --prefix nvm)/nvm.sh\"", &[Loader]),
-        (
-            "[ -s \"$HOMEBREW_PREFIX/opt/nvm/nvm.sh\" ] && \\. \"$HOMEBREW_PREFIX/opt/nvm/nvm.sh\"",
-            &[Loader],
-        ),
-        (
-            "  [ -s \"${nvm_prefix}/nvm.sh\" ] && \\. \"${nvm_prefix}/nvm.sh\"",
-            &[Loader],
-        ),
-        (
-            "  [ -s \"${nvm_prefix}/etc/bash_completion.d/nvm\" ] && \\. \"${nvm_prefix}/etc/bash_completion.d/nvm\"",
-            &[Completion],
-        ),
-        ("source ~/.nvm/nvm.sh", &[Loader]),
-        (
-            "nvm() { unset -f nvm; . \"$NVM_DIR/nvm.sh\"; nvm \"$@\"; }",
-            &[LazyStub, Unset, Loader],
-        ),
-        (
-            "      [[ -f \\\"\\$NVM_DIR/nvm.sh\\\" ]] && source \\\"\\$NVM_DIR/nvm.sh\\\"",
-            &[Loader],
-        ),
-        (
-            "  bass source ~/.nvm/nvm.sh --no-use ';' nvm $argv",
-            &[Bass],
-        ),
-        (
-            "# [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"",
-            &[],
-        ),
-        (
-            "# [nvmrc-migrated] [ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"",
-            &[],
-        ),
-        ("echo dnvm.sh", &[]),
-        (". ~/.dnx/dnvm/dnvm.sh", &[]),
-        ("source \"$HOME/.nvm/nvm.sh\"", &[Loader]),
-    ]);
+    assert_rows(CORPUS_ROWS);
 }
 
 /// install.sh's older forms (1.3), the README (1.4) and Homebrew (2).
+const INSTALLER_ROWS: &[(&str, &[Kind])] = &[
+    ("export NVM_DIR=\"/home/u/.nvm\"", &[NvmDirExport]),
+    (
+        "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\" # This loads nvm",
+        &[Loader],
+    ),
+    (
+        "export NVM_DIR=\"$HOME/.nvm\" && ( git clone https://github.com/nvm-sh/nvm.git \"$NVM_DIR\" ) && \\. \"$NVM_DIR/nvm.sh\"",
+        &[NvmDirExport, Loader],
+    ),
+    ("\\. \"/usr/local/opt/nvm/nvm.sh\"", &[Loader]),
+    (
+        "\\. \"/home/linuxbrew/.linuxbrew/opt/nvm/nvm.sh\"",
+        &[Loader],
+    ),
+    ("source ${HOMEBREW_PREFIX}/opt/nvm/nvm.sh", &[Loader]),
+    (
+        "[ -s \"$(brew --prefix)/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"$(brew --prefix)/opt/nvm/etc/bash_completion.d/nvm\"",
+        &[Completion],
+    ),
+    ("source ~/.nvm/bash_completion", &[Completion]),
+    ("source ${NVM_DIR}/bash_completion", &[Completion]),
+    (
+        "    nvm_path=\"$(nvm_find_up .nvmrc | command tr -d '\\n')\"",
+        &[HelperCall],
+    ),
+    ("  local nvmrc_path=\"$(nvm_find_nvmrc)\"", &[HelperCall]),
+    ("alias cd='cdnvm'", &[]),
+    ("add-zsh-hook chpwd load-nvmrc", &[]),
+    ("ZSH_VERSION= source \"$_nvm_completion\"", &[]),
+];
+
 #[test]
 fn installer_readme_and_homebrew_forms() {
-    assert_rows(&[
-        ("export NVM_DIR=\"/home/u/.nvm\"", &[NvmDirExport]),
-        (
-            "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\" # This loads nvm",
-            &[Loader],
-        ),
-        (
-            "export NVM_DIR=\"$HOME/.nvm\" && ( git clone https://github.com/nvm-sh/nvm.git \"$NVM_DIR\" ) && \\. \"$NVM_DIR/nvm.sh\"",
-            &[NvmDirExport, Loader],
-        ),
-        ("\\. \"/usr/local/opt/nvm/nvm.sh\"", &[Loader]),
-        (
-            "\\. \"/home/linuxbrew/.linuxbrew/opt/nvm/nvm.sh\"",
-            &[Loader],
-        ),
-        ("source ${HOMEBREW_PREFIX}/opt/nvm/nvm.sh", &[Loader]),
-        (
-            "[ -s \"$(brew --prefix)/opt/nvm/etc/bash_completion.d/nvm\" ] && \\. \"$(brew --prefix)/opt/nvm/etc/bash_completion.d/nvm\"",
-            &[Completion],
-        ),
-        ("source ~/.nvm/bash_completion", &[Completion]),
-        ("source ${NVM_DIR}/bash_completion", &[Completion]),
-        (
-            "    nvm_path=\"$(nvm_find_up .nvmrc | command tr -d '\\n')\"",
-            &[HelperCall],
-        ),
-        ("  local nvmrc_path=\"$(nvm_find_nvmrc)\"", &[HelperCall]),
-        ("alias cd='cdnvm'", &[]),
-        ("add-zsh-hook chpwd load-nvmrc", &[]),
-        ("ZSH_VERSION= source \"$_nvm_completion\"", &[]),
-    ]);
+    assert_rows(INSTALLER_ROWS);
 }
 
 /// oh-my-zsh (3.1), zsh-nvm (3.2) and the blog patterns A to E (3.3).
+const LAZY_LOADER_ROWS: &[(&str, &[Kind])] = &[
+    ("which nvm &>/dev/null && return", &[]),
+    ("zstyle ':omz:plugins:nvm' lazy yes", &[OmzPlugin]),
+    ("zstyle \":omz:plugins:nvm\" autoload yes", &[OmzPlugin]),
+    ("    function $nvm_lazy_cmd {", &[]),
+    ("          unfunction \\$func", &[]),
+    ("antigen bundle lukechilds/zsh-nvm", &[ZshNvm]),
+    ("zplug \"lukechilds/zsh-nvm\"", &[ZshNvm]),
+    ("zinit light lukechilds/zsh-nvm", &[ZshNvm]),
+    ("export NVM_LAZY_LOAD=true", &[ZshNvm]),
+    ("    eval \"$cmd(){", &[]),
+    ("      unset -f $cmds > /dev/null 2>&1", &[]),
+    ("lazynvm() {", &[]),
+    ("  unset -f nvm node npm npx", &[Unset]),
+    ("  export NVM_DIR=~/.nvm", &[NvmDirExport]),
+    (
+        "  [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"",
+        &[Loader],
+    ),
+    ("nvm() { lazynvm; nvm $@; }", &[LazyStub]),
+    ("node() { lazynvm; node $@; }", &[LazyStub]),
+    ("lazy_load_nvm_cmds=(nvm node npm npx yarn)", &[]),
+    ("for cmd in \"${lazy_load_nvm_cmds[@]}\"; do", &[]),
+    (
+        "  eval \"${cmd}() { unset -f ${lazy_load_nvm_cmds[*]}; source \\$NVM_DIR/nvm.sh; ${cmd} \\\"\\$@\\\"; }\"",
+        &[Unset, Loader],
+    ),
+    (
+        "alias nvm='unalias nvm node npm && . \"$NVM_DIR\"/nvm.sh && nvm'",
+        &[Loader],
+    ),
+    (
+        "export PATH=\"$NVM_DIR/versions/node/$(cat $NVM_DIR/alias/default)/bin:$PATH\"",
+        &[],
+    ),
+];
+
 #[test]
 fn lazy_loaders_and_other_managers() {
-    assert_rows(&[
-        ("which nvm &>/dev/null && return", &[]),
-        ("zstyle ':omz:plugins:nvm' lazy yes", &[OmzPlugin]),
-        ("zstyle \":omz:plugins:nvm\" autoload yes", &[OmzPlugin]),
-        ("    function $nvm_lazy_cmd {", &[]),
-        ("          unfunction \\$func", &[]),
-        ("antigen bundle lukechilds/zsh-nvm", &[ZshNvm]),
-        ("zplug \"lukechilds/zsh-nvm\"", &[ZshNvm]),
-        ("zinit light lukechilds/zsh-nvm", &[ZshNvm]),
-        ("export NVM_LAZY_LOAD=true", &[ZshNvm]),
-        ("    eval \"$cmd(){", &[]),
-        ("      unset -f $cmds > /dev/null 2>&1", &[]),
-        ("lazynvm() {", &[]),
-        ("  unset -f nvm node npm npx", &[Unset]),
-        ("  export NVM_DIR=~/.nvm", &[NvmDirExport]),
-        (
-            "  [ -s \"$NVM_DIR/nvm.sh\" ] && . \"$NVM_DIR/nvm.sh\"",
-            &[Loader],
-        ),
-        ("nvm() { lazynvm; nvm $@; }", &[LazyStub]),
-        ("node() { lazynvm; node $@; }", &[LazyStub]),
-        ("lazy_load_nvm_cmds=(nvm node npm npx yarn)", &[]),
-        ("for cmd in \"${lazy_load_nvm_cmds[@]}\"; do", &[]),
-        (
-            "  eval \"${cmd}() { unset -f ${lazy_load_nvm_cmds[*]}; source \\$NVM_DIR/nvm.sh; ${cmd} \\\"\\$@\\\"; }\"",
-            &[Unset, Loader],
-        ),
-        (
-            "alias nvm='unalias nvm node npm && . \"$NVM_DIR\"/nvm.sh && nvm'",
-            &[Loader],
-        ),
-        (
-            "export PATH=\"$NVM_DIR/versions/node/$(cat $NVM_DIR/alias/default)/bin:$PATH\"",
-            &[],
-        ),
-    ]);
+    assert_rows(LAZY_LOADER_ROWS);
 }
 
 /// The user's own dotfiles (3.4).
+const USERS_DOTFILES_ROWS: &[(&str, &[Kind])] = &[
+    ("export NVM_DIR=\"${HOME}/.nvm\"", &[NvmDirExport]),
+    (
+        "# Node (nvm itself is lazy-loaded via scripts/lazy-functions.zsh)",
+        &[],
+    ),
+    (
+        "[[ -r \"${ZDOTDIR:-$HOME}/.zshenv\" ]] && source \"${ZDOTDIR:-$HOME}/.zshenv\"",
+        &[],
+    ),
+    ("  . \"$HOME/.swiftly/env.sh\"", &[]),
+    ("plugins=(", &[]),
+    ("    nvm", &[]),
+    ("source $ZSH/oh-my-zsh.sh", &[]),
+    ("source ${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh", &[]),
+    ("[[ ! -d \"${NVM_DIR}\" ]] && mkdir \"${NVM_DIR}\"", &[]),
+    ("nvm() {", &[LazyStub]),
+    (
+        "  unfunction nvm node npm npx yarn pnpm 2>/dev/null",
+        &[Unset],
+    ),
+    ("  export NVM_DIR=\"${HOME}/.nvm\"", &[NvmDirExport]),
+    (
+        "  local nvm_prefix=\"${HOMEBREW_PREFIX:-/opt/homebrew}/opt/nvm\"",
+        &[],
+    ),
+    ("  nvm \"$@\"", &[]),
+    (
+        "# node() { nvm >/dev/null 2>&1; command node \"$@\" } # TODO: Disabled to install a real node",
+        &[],
+    ),
+    (
+        "npm() { nvm >/dev/null 2>&1; command npm \"$@\" }",
+        &[LazyStub],
+    ),
+    (
+        "npx() { nvm >/dev/null 2>&1; command npx \"$@\" }",
+        &[LazyStub],
+    ),
+    (
+        "yarn() { nvm >/dev/null 2>&1; command yarn \"$@\" }",
+        &[LazyStub],
+    ),
+    (
+        "pnpm() { nvm >/dev/null 2>&1; command pnpm \"$@\" }",
+        &[LazyStub],
+    ),
+];
+
 #[test]
 fn the_users_dotfiles() {
-    assert_rows(&[
-        ("export NVM_DIR=\"${HOME}/.nvm\"", &[NvmDirExport]),
-        (
-            "# Node (nvm itself is lazy-loaded via scripts/lazy-functions.zsh)",
-            &[],
-        ),
-        (
-            "[[ -r \"${ZDOTDIR:-$HOME}/.zshenv\" ]] && source \"${ZDOTDIR:-$HOME}/.zshenv\"",
-            &[],
-        ),
-        ("  . \"$HOME/.swiftly/env.sh\"", &[]),
-        ("plugins=(", &[]),
-        ("    nvm", &[]),
-        ("source $ZSH/oh-my-zsh.sh", &[]),
-        ("source ${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh", &[]),
-        ("[[ ! -d \"${NVM_DIR}\" ]] && mkdir \"${NVM_DIR}\"", &[]),
-        ("nvm() {", &[LazyStub]),
-        (
-            "  unfunction nvm node npm npx yarn pnpm 2>/dev/null",
-            &[Unset],
-        ),
-        ("  export NVM_DIR=\"${HOME}/.nvm\"", &[NvmDirExport]),
-        (
-            "  local nvm_prefix=\"${HOMEBREW_PREFIX:-/opt/homebrew}/opt/nvm\"",
-            &[],
-        ),
-        ("  nvm \"$@\"", &[]),
-        (
-            "# node() { nvm >/dev/null 2>&1; command node \"$@\" } # TODO: Disabled to install a real node",
-            &[],
-        ),
-        (
-            "npm() { nvm >/dev/null 2>&1; command npm \"$@\" }",
-            &[LazyStub],
-        ),
-        (
-            "npx() { nvm >/dev/null 2>&1; command npx \"$@\" }",
-            &[LazyStub],
-        ),
-        (
-            "yarn() { nvm >/dev/null 2>&1; command yarn \"$@\" }",
-            &[LazyStub],
-        ),
-        (
-            "pnpm() { nvm >/dev/null 2>&1; command pnpm \"$@\" }",
-            &[LazyStub],
-        ),
-    ]);
-}
+    assert_rows(USERS_DOTFILES_ROWS);
+}
+
+const STUB_ROWS: &[(&str, &[Kind])] = &[
+    ("function nvm {", &[LazyStub]),
+    ("function nvm() {", &[LazyStub]),
+    ("function yarn{", &[LazyStub]),
+    ("node () {", &[LazyStub]),
+    ("corepack(){", &[LazyStub]),
+    ("  pnpx ( ) {", &[LazyStub]),
+    ("pnvm() {", &[]),
+    ("nvmx() {", &[]),
+    ("nvm()", &[]),
+    ("unfunction nvm", &[Unset]),
+    ("unset -f node nvm", &[Unset]),
+    ("unset nvm", &[]),
+    ("unset -f pnvm", &[]),
+    ("myunset -f nvm", &[]),
+    ("if nvm_has node; then", &[HelperCall]),
+    ("nvm_ls_remote", &[]),
+    (
+        "echo $(nvm_version current) $(nvm_rc_version) $(nvm_echo x) $(nvm_ls)",
+        &[HelperCall],
+    ),
+    ("source ~/.nvm/nvm.sh.bak", &[]),
+    ("./nvm.sh", &[]),
+    ("defer_source ~/.nvm/nvm.sh", &[]),
+    ("cd ~/.nvm && . nvm.sh", &[Loader]),
+    ("(source ~/.nvm/nvm.sh)", &[Loader]),
+    ("true;source ~/.nvm/nvm.sh", &[Loader]),
+    ("NVM_DIR=/opt/nvm", &[NvmDirExport]),
+    ("MY_NVM_DIR=/opt/nvm", &[]),
+];
 
 #[test]
 fn stub_unset_and_helper_forms_and_their_rejects() {
-    assert_rows(&[
-        ("function nvm {", &[LazyStub]),
-        ("function nvm() {", &[LazyStub]),
-        ("function yarn{", &[LazyStub]),
-        ("node () {", &[LazyStub]),
-        ("corepack(){", &[LazyStub]),
-        ("  pnpx ( ) {", &[LazyStub]),
-        ("pnvm() {", &[]),
-        ("nvmx() {", &[]),
-        ("nvm()", &[]),
-        ("unfunction nvm", &[Unset]),
-        ("unset -f node nvm", &[Unset]),
-        ("unset nvm", &[]),
-        ("unset -f pnvm", &[]),
-        ("myunset -f nvm", &[]),
-        ("if nvm_has node; then", &[HelperCall]),
-        ("nvm_ls_remote", &[]),
-        (
-            "echo $(nvm_version current) $(nvm_rc_version) $(nvm_echo x) $(nvm_ls)",
-            &[HelperCall],
-        ),
-        ("source ~/.nvm/nvm.sh.bak", &[]),
-        ("./nvm.sh", &[]),
-        ("defer_source ~/.nvm/nvm.sh", &[]),
-        ("cd ~/.nvm && . nvm.sh", &[Loader]),
-        ("(source ~/.nvm/nvm.sh)", &[Loader]),
-        ("true;source ~/.nvm/nvm.sh", &[Loader]),
-        ("NVM_DIR=/opt/nvm", &[NvmDirExport]),
-        ("MY_NVM_DIR=/opt/nvm", &[]),
-    ]);
-}
+    assert_rows(STUB_ROWS);
+}
```

Apply to `src/domain/conflict/source_path.rs` (above the test module):

```diff
--- a/src/domain/conflict/source_path.rs
+++ b/src/domain/conflict/source_path.rs
@@ -1,6 +1,8 @@
 //! The path a line sources (digest section 6): the token after the command
 //! word `source`, `.` or `\.`, which starts the line or follows a blank or one
-//! of `;&|{(`, and is followed by a blank.
+//! of `;&|{(`, and is followed by a blank. It must stand where a command
+//! does: after an operator, a reserved word like `then`, or assignments, so
+//! the `.` of `find . -name` is an argument.
 //!
 //! The token is a `"..."`, `'...'` or unquoted run up to a blank or one of
 //! `;&|)`, its parts concatenated with the quotes removed (`"$NVM_DIR"/nvm.sh`
@@ -11,10 +13,14 @@
 use std::iter::Peekable;
 use std::str::Chars;
 
-use super::lexer::strip_comment;
+use super::lexer::{Token, is_word_character, open_quote_contents, strip_comment, tokens};
 
 /// The command words that read a file into the shell.
 const COMMANDS: [&str; 3] = ["source", "\\.", "."];
+/// The reserved words and prefixes a command may follow.
+const COMMAND_KEYWORDS: [&str; 11] = [
+    "then", "do", "else", "if", "elif", "while", "until", "{", "!", "builtin", "command",
+];
 /// What may stand right before a command word.
 const COMMAND_PREFIXES: [char; 5] = [';', '&', '|', '{', '('];
 /// What ends an unquoted token besides a blank.
@@ -40,10 +46,43 @@
 
 /// Whether a command word may start at byte `start` of `code`.
 fn starts_command(code: &str, start: usize) -> bool {
-    code[..start]
+    let before = &code[..start];
+    before
         .chars()
         .next_back()
-        .is_none_or(|before| before.is_whitespace() || COMMAND_PREFIXES.contains(&before))
+        .is_none_or(|character| character.is_whitespace() || COMMAND_PREFIXES.contains(&character))
+        && in_command_position(before)
+}
+
+/// Whether a word after `before` is a command, not an argument: only reserved
+/// words like `then`, `builtin` and assignments stand between it and the
+/// last operator. Inside a quote left open (an `eval "..."` or an alias) the
+/// command starts at the quote.
+fn in_command_position(before: &str) -> bool {
+    let command_text = open_quote_contents(before).map_or(before, |start| &before[start..]);
+    let words = tokens(command_text);
+    let simple_command = words
+        .iter()
+        .rposition(|token| matches!(token, Token::Operator(_)))
+        .map_or(0, |operator| operator + 1);
+    words[simple_command..].iter().all(precedes_a_command)
+}
+
+/// A reserved word that a command follows, or an assignment `NAME=value`.
+fn precedes_a_command(token: &Token) -> bool {
+    let Token::Word { value, quoted } = token else {
+        return false;
+    };
+    (!quoted && COMMAND_KEYWORDS.contains(&value.as_str()))
+        || value
+            .split_once('=')
+            .is_some_and(|(name, _)| is_variable_name(name))
+}
+
+fn is_variable_name(name: &str) -> bool {
+    !name.starts_with(|character: char| character.is_ascii_digit())
+        && !name.is_empty()
+        && name.chars().all(is_word_character)
 }
 
 /// The end of the command word starting at `start`, when one does and a
```

Apply to `src/domain/conflict/source_path_tests.rs` (above the test module):

```diff
--- a/src/domain/conflict/source_path_tests.rs
+++ b/src/domain/conflict/source_path_tests.rs
@@ -2,65 +2,66 @@
 
 /// Digest section 6: the real syntaxes, and the token after `source`, `.` or
 /// `\.` with its quotes removed.
+const PATH_TOKEN_ROWS: &[(&str, &str)] = &[
+    ("source $ZSH/oh-my-zsh.sh", "$ZSH/oh-my-zsh.sh"),
+    (
+        "source ${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh",
+        "${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh",
+    ),
+    (
+        "[[ -r \"${ZDOTDIR:-$HOME}/.zshenv\" ]] && source \"${ZDOTDIR:-$HOME}/.zshenv\"",
+        "${ZDOTDIR:-$HOME}/.zshenv",
+    ),
+    ("  . \"$HOME/.swiftly/env.sh\"", "$HOME/.swiftly/env.sh"),
+    (
+        "[ -s \"$HOME/.bun/_bun\" ] && source \"$HOME/.bun/_bun\"",
+        "$HOME/.bun/_bun",
+    ),
+    (
+        "[ -f \"${HOME}/.openclaw/completions/openclaw.zsh\" ] && source \"${HOME}/.openclaw/completions/openclaw.zsh\"",
+        "${HOME}/.openclaw/completions/openclaw.zsh",
+    ),
+    (
+        "[[ -r $NVM_DIR/bash_completion ]] && \\. $NVM_DIR/bash_completion",
+        "$NVM_DIR/bash_completion",
+    ),
+    ("source ~/.nvm/nvm.sh", "~/.nvm/nvm.sh"),
+    (
+        "source ${0:A:h}/zsh-syntax-highlighting.zsh",
+        "${0:A:h}/zsh-syntax-highlighting.zsh",
+    ),
+    (
+        "for f in \"${_deferred_sources[@]}\"; do source \"$f\"; done",
+        "$f",
+    ),
+    (
+        "source $(brew --prefix nvm)/nvm.sh",
+        "$(brew --prefix nvm)/nvm.sh",
+    ),
+    (
+        ". \"$(brew --prefix nvm)/nvm.sh\"",
+        "$(brew --prefix nvm)/nvm.sh",
+    ),
+    ("[ -f p ] && . p", "p"),
+    ("[[ -r p ]] && source p", "p"),
+    ("source 'a b'/c", "a b/c"),
+    ("source x;echo y", "x"),
+    ("(source x)", "x"),
+    ("source x&", "x"),
+    ("source x|cat", "x"),
+    ("\tsource x", "x"),
+    ("source \"$NVM_DIR\"/nvm.sh", "$NVM_DIR/nvm.sh"),
+    ("source a\\ b", "a b"),
+    (
+        "source `brew --prefix nvm`/nvm.sh",
+        "`brew --prefix nvm`/nvm.sh",
+    ),
+    ("source x # . y", "x"),
+];
+
 #[test]
 fn source_target_reads_the_path_token() {
-    let rows = [
-        ("source $ZSH/oh-my-zsh.sh", "$ZSH/oh-my-zsh.sh"),
-        (
-            "source ${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh",
-            "${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh",
-        ),
-        (
-            "[[ -r \"${ZDOTDIR:-$HOME}/.zshenv\" ]] && source \"${ZDOTDIR:-$HOME}/.zshenv\"",
-            "${ZDOTDIR:-$HOME}/.zshenv",
-        ),
-        ("  . \"$HOME/.swiftly/env.sh\"", "$HOME/.swiftly/env.sh"),
-        (
-            "[ -s \"$HOME/.bun/_bun\" ] && source \"$HOME/.bun/_bun\"",
-            "$HOME/.bun/_bun",
-        ),
-        (
-            "[ -f \"${HOME}/.openclaw/completions/openclaw.zsh\" ] && source \"${HOME}/.openclaw/completions/openclaw.zsh\"",
-            "${HOME}/.openclaw/completions/openclaw.zsh",
-        ),
-        (
-            "[[ -r $NVM_DIR/bash_completion ]] && \\. $NVM_DIR/bash_completion",
-            "$NVM_DIR/bash_completion",
-        ),
-        ("source ~/.nvm/nvm.sh", "~/.nvm/nvm.sh"),
-        (
-            "source ${0:A:h}/zsh-syntax-highlighting.zsh",
-            "${0:A:h}/zsh-syntax-highlighting.zsh",
-        ),
-        (
-            "for f in \"${_deferred_sources[@]}\"; do source \"$f\"; done",
-            "$f",
-        ),
-        (
-            "source $(brew --prefix nvm)/nvm.sh",
-            "$(brew --prefix nvm)/nvm.sh",
-        ),
-        (
-            ". \"$(brew --prefix nvm)/nvm.sh\"",
-            "$(brew --prefix nvm)/nvm.sh",
-        ),
-        ("[ -f p ] && . p", "p"),
-        ("[[ -r p ]] && source p", "p"),
-        ("source 'a b'/c", "a b/c"),
-        ("source x;echo y", "x"),
-        ("(source x)", "x"),
-        ("source x&", "x"),
-        ("source x|cat", "x"),
-        ("\tsource x", "x"),
-        ("source \"$NVM_DIR\"/nvm.sh", "$NVM_DIR/nvm.sh"),
-        ("source a\\ b", "a b"),
-        (
-            "source `brew --prefix nvm`/nvm.sh",
-            "`brew --prefix nvm`/nvm.sh",
-        ),
-        ("source x # . y", "x"),
-    ];
-    for (line, expected) in rows {
+    for &(line, expected) in PATH_TOKEN_ROWS {
         assert_eq!(source_target(line).as_deref(), Some(expected), "{line:?}");
     }
 }
@@ -101,3 +102,41 @@
     );
     assert!(source_targets("echo nothing").is_empty());
 }
+
+/// A command word after a keyword, assignments, an operator or a quote that
+/// starts a command, and the path it reads.
+const COMMAND_POSITION_ROWS: &[(&str, &str)] = &[
+    ("if [ -s x ]; then . x; fi", "x"),
+    ("for f in a; do source \"$f\"; done", "$f"),
+    ("if true; then :; else . y; fi", "y"),
+    ("{ . x; }", "x"),
+    (
+        "ZSH_VERSION= source \"$_nvm_completion\"",
+        "$_nvm_completion",
+    ),
+    ("A=1 B=\"c d\" . x", "x"),
+    ("builtin source x", "x"),
+    ("  bash) source x ;;", "x"),
+    ("eval \"cd a; source x\"", "x"),
+    ("alias l='cd a && . x'", "x"),
+];
+
+/// A `.` that is an argument (the user's `scripts/macos.zsh:119-122,201`) is
+/// not a command word; one after a keyword, an assignment or an operator is.
+#[test]
+fn source_target_needs_the_command_position() {
+    let arguments = [
+        "alias qfind=\"find . -name \"                 # qfind:    Quickly search for file",
+        "ff () { /usr/bin/find . -name \"$@\" ; }      # ff:       Find file under the current directory",
+        "ffs () { /usr/bin/find . -name \"$@\"'*' ; }  # ffs:      Find file whose name starts with a given string",
+        "alias cleanupDS=\"find . -type f -name '*.DS_Store' -ls -delete\"",
+        "echo \"a\" . b",
+        "cp x . ",
+    ];
+    for line in arguments {
+        assert_eq!(source_target(line), None, "{line:?}");
+    }
+    for &(line, expected) in COMMAND_POSITION_ROWS {
+        assert_eq!(source_target(line).as_deref(), Some(expected), "{line:?}");
+    }
+}
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests**

Run: `cargo test`
Expected: PASS: 1340 unit tests plus 113 end-to-end tests on macOS (114 on
Linux).

- [ ] **Step 5: Run the full quality gate**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings, every test passing, no
advisories, and the crate still builds on MSRV 1.85.

Also check that no file is over 300 lines and that no function is over 30
(temporarily add a `clippy.toml` with `too-many-lines-threshold = 30`, run
`cargo clippy --all-targets -- -W clippy::too_many_lines`, and delete it):

```bash
find src tests -name '*.rs' | xargs wc -l | sort -n | tail -3
```

- [ ] **Step 6: Run the tests on Linux**

Run:

```bash
docker buildx build -f Dockerfile -t nvmrc:p8 .
docker run --rm nvmrc:p8
docker rmi -f nvmrc:p8
```

Expected: 1340 unit and 114 end-to-end tests passing with every shell of the
image (bash, zsh, ksh, dash, fish) installed; cargo stops at the first failing
test binary, so count the `test result` lines against the number of test
files.

- [ ] **Step 7: Dry run on a real profile (optional, manual, read-only)**

Build `target/debug/nvm` and run, against your real home and with stdin from
`/dev/null`, ONLY `nvm doctor`, `nvm doctor --shell zsh` and `nvm migrate
--dry-run`. They write nothing: check that `git status` of your dotfiles is
empty and that no `*.nvmrc-backup-*` file appeared before and after. Compare
the findings with the lines of the profile: every real loader, completion,
lazy stub and `unfunction nvm` must be listed, and nothing else (no `find .
-name`, no comments, no `defer_source` helper lines).

- [ ] **Step 8: Commit**

```bash
git add src tests
git commit -S -m "refactor: keep functions under 30 lines and files under 300, and fix the rules found with a real profile"
```

---

## Deliberate deviations from the spec and from nvm.sh

Each is pinned by a test and is for the Plan 9 compatibility contract.

- **No ripgrep crates and no `LineScanner`.** The rules are hand-written line
  patterns in `domain/conflict`, table-driven; plans 6 and 7 avoided a regex
  crate too, and a closed list of small files needs no search engine.
- **The runtime check lives in the init snippet, not in the binary** (spec 7.2:
  the binary cannot see shell functions). A late `source nvm.sh`, after the init
  line, is only found by `nvm doctor`: no `PROMPT_COMMAND`, `precmd` or `DEBUG`
  hooks.
- **Lazy-loader files are never edited**: commenting out the extent of a
  function is not safe without a parser. They are reported with the lines, the
  manual fix and a suggested patch.
- **`OmzPlugin` and `HelperCall` are hints**, not counted in the exit status.
- **A `source` must be in command position**, so a command word right after an
  opening quote with nothing before it (`eval "source x"`) is not seen.
- **Relative `source` paths resolve from the sourcing file's directory** (the
  shell uses `$PWD`): an Info note. Unresolvable paths are never guessed.
- **Only loader and completion lines are migrated**, as `# [nvmrc-migrated] ...`
  comments, never deleted; `export NVM_DIR=` stays.
- **The load line calls the `nvmrc` binary**, not `nvm init`.
- **The shell of the block is kept from an existing init line.**
- **The diff is part of the prompt when interactive.**
- **Backups do not copy permissions** (a regular file with the default mode next
  to the link); the atomic replace of the profile keeps its mode and symlink.
- **`--undo` leaves the backups**, so an undo can itself be undone.
- **A candidate that fails the syntax check is refused**, but when the checker
  is not installed the file is still written, with a warning: refusing would
  block every user who lacks that shell.
- **fish handling is unverified locally**: its rules, the load line
  `nvmrc init fish | source`, the snippet check and `fish --no-execute` run only
  in the Docker image.
- nvm.sh has no `doctor`, `migrate` or `__conflict`: they are nvmrc additions;
  the installed versions in `$NVM_DIR` stay usable by both tools.

## Self-review against the spec

- **Spec coverage:** section 7.1 (the three conflict sources: profile lines,
  Homebrew, lazy loaders), 7.2 (corrected: the runtime check in the snippet and
  `nvm doctor` on disk), 7.3 (detect, plan, diff, `--yes`, backup, atomic write,
  verify, idempotent markers, reversible, never deleting lines), 7.4 (without
  the ripgrep crates), section 6 (exit codes of the two commands) and section 3
  (the `Clock` and `Prompt` ports).
- **Placeholder scan:** none.
- **Type consistency:** names match across tasks (`Kind`, `Hit`, `Report`,
  `FileFindings`, `Note`, `Migration`, `BlockChange`, `RuntimeConflict`,
  `Shell::load_line`).
