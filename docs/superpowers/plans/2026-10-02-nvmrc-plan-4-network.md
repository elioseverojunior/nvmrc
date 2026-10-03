# nvmrc Plan 4: Network, Remote Listing and Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the network half of the read-only commands: `ls-remote`
(`list-remote`), `version-remote` and `cache`, on a blocking HTTP client with
retries, validated mirrors, the `index.tab` parser, and the refresh of the
`lts/*` aliases that `nvm.sh` does as a side effect of listing.

**Architecture:** Same layering as Plans 1 to 3. `ports/` gains `Http` and
`Sleeper`; `adapters/` gains `UreqHttp` (the only code that touches the
network), `RetryingHttp` (exponential backoff, a decorator over any `Http`) and
`StdSleeper`; `domain/` gains the mirror, index, selection, resolution and
row-formatting rules as pure functions; `commands/` gains `ls_remote`,
`version_remote`, `cache` and the shared `remote_index` fetcher. `Context`
carries the `Http` port next to `Process`.

**Tech Stack:** Rust 2024 edition (MSRV 1.88), `clap`, `thiserror`, `ureq` 3
(blocking, new), `tempfile` (dev only). No async runtime, as decided in the
spec ("Why no async I/O").

**Spec:** `docs/superpowers/specs/2026-10-02-nvmrc-design.md` (sections 3, 4.1,
6, 10 and 11). This plan starts from the final state of
`2026-10-02-nvmrc-plan-3-resolution-and-listing.md` (branch
`feat/plan-3-resolution-and-listing`).

**Ground truth:** every expected output in this plan was captured by running
the real `nvm.sh` (the reference repository) against the same fixture mirrors,
served on local ports, with stdout piped so it prints without colors. The unit
tests repeat those outputs byte for byte, and the end-to-end tests of Task 10
serve a mirror from a local socket.

**Revised roadmap:** Plan 5 `install`, `uninstall`, `reinstall-packages` and
checksum verification; Plan 6 shell (`use`, `deactivate`, `exec`, `run`,
`init`, `nvm-exec`, `.nvmrc` semantics); Plan 7 colors and terminal
detection; Plan 8 `doctor` and `migrate`; Plan 9 compatibility contract and
CI.

**Not in this plan, on purpose:**

- Checksums, downloads of tarballs and `install` (Plan 5). The `Http` port
  only has `get_text`; Plan 5 adds a download-to-file method.
- Caching `index.tab` and an offline mode (spec section 4.4). `nvm.sh`
  downloads the index on every listing and caches only tarballs, so the cache
  directory is only created and emptied here (`cache dir`, `cache clear`).
- `NVM_NODEJS_ORG_MIRROR` rejected with exit 2 (spec section 8, item 5): the
  real `nvm.sh` prints the message on stderr and lists nothing (status 3), and
  this plan keeps that behaviour for parity.
- `wget`/`curl` selection and their options (`nvm_download`); `ureq` replaces
  both. `NVM_AUTH_HEADER` is honoured, sanitized like `nvm.sh` does.

## Global Constraints

- Rust edition 2024, `rust-version = "1.88.0"`; no APIs newer than 1.88. The
  development toolchain is pinned separately by `rust-toolchain.toml`.
- Run cargo through the rustup proxies (Homebrew's `rust` formula ships a
  `cargo` that ignores `rust-toolchain.toml`).
- `.cargo/config.toml` sets `-D warnings` for every workspace build, test and
  check, so a dead-code or unused-import warning in any intermediate task is a
  build error. `Cargo.toml` must not declare any `[profile.*]` table; the only
  change to it in this plan is the `ureq` dependency (Task 2).
- No async I/O, no workspace, no plugin system, and no dependency besides
  `ureq`.
- Files under 300 lines (tests included), functions under 30 lines, cyclomatic
  complexity under 10, meaningful names without abbreviations.
- Errors use `thiserror` in the library; only the binaries touch
  `std::process::ExitCode`. Exit codes: 0 success, 1 generic failure, 2 no
  such `lts/*` alias, 3 invalid or unknown version (and a listing that finds
  nothing, or that could not download a part), 7 below the version floor, 8
  alias loop, 55 unsupported option, 127 usage error or missing system node.
- Commands return an `Output { stdout, stderr, status }` or a `CliError`; the
  CLI prints stderr first, then stdout, adds a newline to each non-empty
  stream, and finishes with `Output::status`.
- `Context::new(fs, env)` builds a context that can neither run programs nor
  reach the network; `.with_process(&dyn Process)` and `.with_http(&dyn Http)`
  add those ports. Nothing else constructs a `Context` by struct literal.
- Network rules: a request times out after 30 seconds, a body is capped at
  16 MiB, a transient failure (network error, 5xx, 429) is retried with
  exponential backoff (3 attempts, 250 ms base), and any other status is final.
- Layout compatible with nvm: `$NVM_DIR/versions/node/vX.Y.Z`,
  `$NVM_DIR/versions/io.js/vX.Y.Z`, `$NVM_DIR/alias/<name>`,
  `$NVM_DIR/alias/lts/<name>` and `$NVM_DIR/.cache`.
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

### Task 1: The `Http` port, `NoHttp`, the auth header and `FakeHttp`

**Files:**

- Create: `src/adapters/no_http.rs`, `src/domain/http_header.rs`,
  `src/fakes/http.rs`
- Modify: `src/ports/mod.rs`, `src/adapters/mod.rs`, `src/domain/mod.rs`,
  `src/context.rs`, `src/fakes/mod.rs` (the `fakes` module becomes a
  directory, Step 0)

**Interfaces:**

- Produces: `ports::{Http, HttpError}` with `Http::get_text(&self, url: &str)
  -> Result<String, HttpError>`; `HttpError::Status { url, code }` and
  `HttpError::Transport { url, message }`; `adapters::no_http::NoHttp` (every
  request fails with a transport error); `domain::http_header::
  sanitize_auth_header(&str) -> String` (keeps ASCII letters and digits and
  the characters `:_.+/=~-`, like `nvm_sanitize_auth_header`); test-only
  `fakes::FakeHttp` with `with_body(url, body)`, `with_status(url, code)` and
  `requests()` (the URLs asked for, in order); `Context::with_http(&dyn Http)`
  and `Context::http()`.
- `FakeHttp` fails with a transport error (`connection refused`) for a URL it
  was not given.
- [ ] **Step 0: Split the fakes into a directory (no behaviour change)**

`src/fakes.rs` would pass 300 lines in this task. Run:

```bash
mkdir src/fakes && git mv src/fakes.rs src/fakes/file_system.rs
```

Apply to `src/fakes/file_system.rs` (it loses `FakeEnv` and `FakeProcess`,
which move to their own files):

```diff
--- a/src/fakes.rs
+++ b/src/fakes/file_system.rs
@@ -1,12 +1,9 @@
-//! In-memory implementations of the ports, for unit tests only.
-
 use std::cell::RefCell;
 use std::collections::{BTreeMap, BTreeSet};
-use std::ffi::OsString;
 use std::io;
 use std::path::{Path, PathBuf};
 
-use crate::ports::{DirEntry, Env, FileSystem, Process, ProcessOutput};
+use crate::ports::{DirEntry, FileSystem};
 
 #[derive(Default)]
 pub struct FakeFileSystem {
@@ -93,73 +90,6 @@ impl FileSystem for FakeFileSystem {
     }
 }
 
-/// Programs by path: each prints a fixed output, or fails when it has none.
-#[derive(Default)]
-pub struct FakeProcess {
-    outputs: BTreeMap<PathBuf, ProcessOutput>,
-}
-
-impl FakeProcess {
-    #[must_use]
-    pub fn with_output(mut self, program: &str, stdout: &str) -> Self {
-        let output = ProcessOutput {
-            success: true,
-            stdout: stdout.to_owned(),
-        };
-        self.outputs.insert(PathBuf::from(program), output);
-        self
-    }
-
-    #[must_use]
-    pub fn with_failure(mut self, program: &str) -> Self {
-        let output = ProcessOutput {
-            success: false,
-            stdout: String::new(),
-        };
-        self.outputs.insert(PathBuf::from(program), output);
-        self
-    }
-}
-
-impl Process for FakeProcess {
-    fn run(&self, program: &Path, _args: &[&str]) -> io::Result<ProcessOutput> {
-        self.outputs
-            .get(program)
-            .cloned()
-            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
-    }
-}
-
-#[derive(Default)]
-pub struct FakeEnv {
-    vars: BTreeMap<String, OsString>,
-}
-
-impl FakeEnv {
-    #[must_use]
-    pub fn with_var(self, key: &str, value: &str) -> Self {
-        self.with_var_os(key, OsString::from(value))
-    }
-
-    #[must_use]
-    pub fn with_var_os(mut self, key: &str, value: OsString) -> Self {
-        self.vars.insert(key.to_owned(), value);
-        self
-    }
-}
-
-impl Env for FakeEnv {
-    fn var(&self, key: &str) -> Option<String> {
-        self.vars
-            .get(key)
-            .and_then(|value| value.to_str().map(str::to_owned))
-    }
-
-    fn var_os(&self, key: &str) -> Option<OsString> {
-        self.vars.get(key).cloned()
-    }
-}
-
 #[cfg(test)]
 mod tests {
     use super::*;
@@ -170,7 +100,7 @@ mod tests {
             is_dir,
         }
     }
     
     #[test]
     fn fake_file_system_lists_direct_children_with_their_kind() {
         let fs = FakeFileSystem::default()
@@ -232,36 +162,4 @@ mod tests {
         fs.create_dir_all(Path::new("/d/e")).unwrap();
         assert_eq!(fs.read_dir(Path::new("/d/e")).unwrap(), []);
     }
-
-    #[test]
-    fn fake_process_answers_by_program_path() {
-        let process = FakeProcess::default()
-            .with_output("/usr/bin/node", "v22.1.0\n")
-            .with_failure("/usr/bin/broken");
-        let ok = process
-            .run(Path::new("/usr/bin/node"), &["--version"])
-            .unwrap();
-        assert_eq!((ok.success, ok.stdout.as_str()), (true, "v22.1.0\n"));
-        let failed = process.run(Path::new("/usr/bin/broken"), &[]).unwrap();
-        assert!(!failed.success);
-        assert!(process.run(Path::new("/missing"), &[]).is_err());
-    }
-
-    #[test]
-    fn fake_env_returns_the_variables_it_was_given() {
-        let env = FakeEnv::default().with_var("HOME", "/home/me");
-        assert_eq!(env.var("HOME"), Some("/home/me".to_owned()));
-        assert_eq!(env.var_os("HOME"), Some(OsString::from("/home/me")));
-        assert_eq!(env.var("MISSING"), None);
-    }
-
-    #[cfg(unix)]
-    #[test]
-    fn fake_env_keeps_non_utf8_values_for_var_os_only() {
-        use std::os::unix::ffi::OsStringExt;
-        let raw = OsString::from_vec(b"/n\xff".to_vec());
-        let env = FakeEnv::default().with_var_os("NVM_DIR", raw.clone());
-        assert_eq!(env.var("NVM_DIR"), None);
-        assert_eq!(env.var_os("NVM_DIR"), Some(raw));
-    }
 }
```

Create `src/fakes/mod.rs`:

```rust
//! In-memory implementations of the ports, for unit tests only.

mod env;
mod file_system;
mod process;

pub use env::FakeEnv;
pub use file_system::FakeFileSystem;
pub use process::FakeProcess;
```

Create `src/fakes/env.rs`:

```rust
use std::collections::BTreeMap;
use std::ffi::OsString;

use crate::ports::Env;

#[derive(Default)]
pub struct FakeEnv {
    vars: BTreeMap<String, OsString>,
}

impl FakeEnv {
    #[must_use]
    pub fn with_var(self, key: &str, value: &str) -> Self {
        self.with_var_os(key, OsString::from(value))
    }

    #[must_use]
    pub fn with_var_os(mut self, key: &str, value: OsString) -> Self {
        self.vars.insert(key.to_owned(), value);
        self
    }
}

impl Env for FakeEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.vars
            .get(key)
            .and_then(|value| value.to_str().map(str::to_owned))
    }

    fn var_os(&self, key: &str) -> Option<OsString> {
        self.vars.get(key).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_env_returns_the_variables_it_was_given() {
        let env = FakeEnv::default().with_var("HOME", "/home/me");
        assert_eq!(env.var("HOME"), Some("/home/me".to_owned()));
        assert_eq!(env.var_os("HOME"), Some(OsString::from("/home/me")));
        assert_eq!(env.var("MISSING"), None);
    }

    #[cfg(unix)]
    #[test]
    fn fake_env_keeps_non_utf8_values_for_var_os_only() {
        use std::os::unix::ffi::OsStringExt;
        let raw = OsString::from_vec(b"/n\xff".to_vec());
        let env = FakeEnv::default().with_var_os("NVM_DIR", raw.clone());
        assert_eq!(env.var("NVM_DIR"), None);
        assert_eq!(env.var_os("NVM_DIR"), Some(raw));
    }
}
```

Create `src/fakes/process.rs`:

```rust
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use crate::ports::{Process, ProcessOutput};

/// Programs by path: each prints a fixed output, or fails when it has none.
#[derive(Default)]
pub struct FakeProcess {
    outputs: BTreeMap<PathBuf, ProcessOutput>,
}

impl FakeProcess {
    #[must_use]
    pub fn with_output(mut self, program: &str, stdout: &str) -> Self {
        let output = ProcessOutput {
            success: true,
            stdout: stdout.to_owned(),
        };
        self.outputs.insert(PathBuf::from(program), output);
        self
    }

    #[must_use]
    pub fn with_failure(mut self, program: &str) -> Self {
        let output = ProcessOutput {
            success: false,
            stdout: String::new(),
        };
        self.outputs.insert(PathBuf::from(program), output);
        self
    }
}

impl Process for FakeProcess {
    fn run(&self, program: &Path, _args: &[&str]) -> io::Result<ProcessOutput> {
        self.outputs
            .get(program)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_process_answers_by_program_path() {
        let process = FakeProcess::default()
            .with_output("/usr/bin/node", "v22.1.0\n")
            .with_failure("/usr/bin/broken");
        let ok = process
            .run(Path::new("/usr/bin/node"), &["--version"])
            .unwrap();
        assert_eq!((ok.success, ok.stdout.as_str()), (true, "v22.1.0\n"));
        let failed = process.run(Path::new("/usr/bin/broken"), &[]).unwrap();
        assert!(!failed.success);
        assert!(process.run(Path::new("/missing"), &[]).is_err());
    }
}
```

Run: `cargo fmt && cargo test`
Expected: PASS, 231 unit tests plus the end-to-end tests, as at the end of
Plan 3.

- [ ] **Step 1: Declare the new modules**

Apply to `src/adapters/mod.rs`:

```diff
--- a/src/adapters/mod.rs
+++ b/src/adapters/mod.rs
@@ -1,4 +1,5 @@
 pub mod fs_alias_store;
+pub mod no_http;
 pub mod no_process;
 pub mod std_env;
 pub mod std_fs;
```

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -2,6 +2,7 @@
 pub mod alias_format;
 pub mod current;
 pub mod floor;
+pub mod http_header;
 pub mod implicit;
 pub mod listing;
 pub mod nvmrc;
```

- [ ] **Step 2: Write the failing tests**

Create `src/adapters/no_http.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetches_nothing() {
        let error = NoHttp.get_text("http://example.test/x").unwrap_err();
        assert_eq!(
            error.to_string(),
            "http://example.test/x: network access is not available"
        );
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
-    use crate::fakes::{FakeEnv, FakeFileSystem};
+    use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};
 
     fn nvm_dir_for(env: &FakeEnv) -> Result<PathBuf, CliError> {
         let fs = FakeFileSystem::default();
@@ -66,6 +66,21 @@
     }
 
     #[test]
+    fn a_context_cannot_reach_the_network_until_given_an_http() {
+        let fs = FakeFileSystem::default();
+        let env = FakeEnv::default();
+        assert!(
+            Context::new(&fs, &env)
+                .http()
+                .get_text("http://x/")
+                .is_err()
+        );
+        let http = FakeHttp::default().with_body("http://x/", "ok");
+        let context = Context::new(&fs, &env).with_http(&http);
+        assert_eq!(context.http().get_text("http://x/").unwrap(), "ok");
+    }
+
+    #[test]
     fn alias_dir_is_under_nvm_dir() {
         let fs = FakeFileSystem::default();
         let env = FakeEnv::default().with_var("NVM_DIR", "/n");
```

Create `src/domain/http_header.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_basic_and_bearer_tokens_intact() {
        assert_eq!(
            sanitize_auth_header("Basic dXNlcjpwYXNz+/=="),
            "Basic dXNlcjpwYXNz+/=="
        );
        assert_eq!(sanitize_auth_header("Bearer a.b-c_d~e"), "Bearer a.b-c_d~e");
    }

    #[test]
    fn drops_anything_that_could_inject_a_header() {
        assert_eq!(
            sanitize_auth_header("Bearer x\r\nX-Evil: 1"),
            "Bearer xX-Evil: 1"
        );
        assert_eq!(sanitize_auth_header("a;b'c\"d`e$f"), "abcdef");
    }
}
```

Create `src/fakes/http.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_http_answers_by_url_and_records_requests() {
        let http = FakeHttp::default()
            .with_body("http://m/index.tab", "rows")
            .with_status("http://m/gone", 404);
        assert_eq!(http.get_text("http://m/index.tab").unwrap(), "rows");
        let status = http.get_text("http://m/gone").unwrap_err();
        assert_eq!(status.to_string(), "http://m/gone: HTTP 404");
        let unknown = http.get_text("http://m/other").unwrap_err();
        assert_eq!(unknown.to_string(), "http://m/other: connection refused");
        assert_eq!(http.requests().len(), 3);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "unresolved import
`crate::ports::Http`", "cannot find function `sanitize_auth_header`" and
"cannot find struct `FakeHttp`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/adapters/no_http.rs`:

```rust
use crate::ports::{Http, HttpError};

/// The `Http` of a `Context` that was not given one: it fetches nothing.
pub struct NoHttp;

impl Http for NoHttp {
    fn get_text(&self, url: &str) -> Result<String, HttpError> {
        Err(HttpError::Transport {
            url: url.to_owned(),
            message: "network access is not available".to_owned(),
        })
    }
}

```

Apply to `src/context.rs` (above the test module):

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -3,26 +3,30 @@
 use std::path::{Path, PathBuf};
 
 use crate::adapters::fs_alias_store::FsAliasStore;
+use crate::adapters::no_http::NoHttp;
 use crate::adapters::no_process::NoProcess;
 use crate::domain::alias::AliasStore;
 use crate::domain::version::Version;
 use crate::error::CliError;
-use crate::ports::{Env, FileSystem, Process};
+use crate::ports::{Env, FileSystem, Http, Process};
 
 pub struct Context<'a> {
     pub fs: &'a dyn FileSystem,
     pub env: &'a dyn Env,
     process: &'a dyn Process,
+    http: &'a dyn Http,
 }
 
 impl<'a> Context<'a> {
-    /// A context that cannot run programs; add that with [`Self::with_process`].
+    /// A context that cannot run programs or reach the network; add those with
+    /// [`Self::with_process`] and [`Self::with_http`].
     #[must_use]
     pub fn new(fs: &'a dyn FileSystem, env: &'a dyn Env) -> Self {
         Self {
             fs,
             env,
             process: &NoProcess,
+            http: &NoHttp,
         }
     }
 
@@ -35,6 +39,17 @@
     #[must_use]
     pub fn process(&self) -> &dyn Process {
         self.process
+    }
+
+    #[must_use]
+    pub fn with_http(mut self, http: &'a dyn Http) -> Self {
+        self.http = http;
+        self
+    }
+
+    #[must_use]
+    pub fn http(&self) -> &dyn Http {
+        self.http
     }
 }
 
```

Insert above the `#[cfg(test)]` line of `src/domain/http_header.rs`:

```rust
//! The `NVM_AUTH_HEADER` value, as `nvm.sh` sanitizes it before sending it as
//! an `Authorization` header.

/// Keeps only letters, digits, space and `: _ . + / = ~ -`: the full base64
/// and base64url alphabets plus what `Basic <token>` and `Bearer <token>`
/// need. Everything else, such as a newline that could smuggle a second
/// header, is dropped.
#[must_use]
pub fn sanitize_auth_header(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || " :_.+/=~-".contains(*character))
        .collect()
}

```

Insert above the `#[cfg(test)]` line of `src/fakes/http.rs`:

```rust
use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::ports::{Http, HttpError};

/// Pages by URL: each answers with a fixed body, status or transport error, and
/// every request is recorded.
#[derive(Default)]
pub struct FakeHttp {
    responses: BTreeMap<String, Result<String, HttpError>>,
    requests: RefCell<Vec<String>>,
}

impl FakeHttp {
    #[must_use]
    pub fn with_body(mut self, url: &str, body: &str) -> Self {
        self.responses.insert(url.to_owned(), Ok(body.to_owned()));
        self
    }

    #[must_use]
    pub fn with_status(mut self, url: &str, code: u16) -> Self {
        let error = HttpError::Status {
            url: url.to_owned(),
            code,
        };
        self.responses.insert(url.to_owned(), Err(error));
        self
    }

    /// The URLs requested so far, in order.
    #[must_use]
    pub fn requests(&self) -> Vec<String> {
        self.requests.borrow().clone()
    }
}

impl Http for FakeHttp {
    fn get_text(&self, url: &str) -> Result<String, HttpError> {
        self.requests.borrow_mut().push(url.to_owned());
        self.responses.get(url).cloned().unwrap_or_else(|| {
            Err(HttpError::Transport {
                url: url.to_owned(),
                message: "connection refused".to_owned(),
            })
        })
    }
}

```

Apply to `src/fakes/mod.rs` (above the test module):

```diff
--- a/src/fakes/mod.rs
+++ b/src/fakes/mod.rs
@@ -2,8 +2,10 @@
 
 mod env;
 mod file_system;
+mod http;
 mod process;
 
 pub use env::FakeEnv;
 pub use file_system::FakeFileSystem;
+pub use http::FakeHttp;
 pub use process::FakeProcess;
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -3,6 +3,8 @@
 use std::ffi::OsString;
 use std::io;
 use std::path::Path;
+
+use thiserror::Error;
 
 /// A direct child of a directory.
 #[derive(Debug, Clone, PartialEq, Eq)]
@@ -57,6 +59,24 @@
     fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput>;
 }
 
+/// Why a download failed.
+#[derive(Debug, Clone, PartialEq, Eq, Error)]
+pub enum HttpError {
+    #[error("{url}: HTTP {code}")]
+    Status { url: String, code: u16 },
+    #[error("{url}: {message}")]
+    Transport { url: String, message: String },
+}
+
+pub trait Http {
+    /// Fetches `url` and returns the body as text.
+    ///
+    /// # Errors
+    /// Fails on a non-success status, on a network error, or when the body is
+    /// not text.
+    fn get_text(&self, url: &str) -> Result<String, HttpError>;
+}
+
 pub trait Env {
     /// The variable as text; `None` when unset or not valid UTF-8.
     fn var(&self, key: &str) -> Option<String>;
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 236 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(ports): add the Http port, NoHttp and a sanitized auth header"
```

---

### Task 2: `ureq`, retries with backoff and the sleeper

**Files:**

- Create: `src/adapters/ureq_http.rs`, `src/adapters/retrying_http.rs`,
  `src/adapters/std_sleeper.rs`, `src/fakes/sleeper.rs`
- Modify: `Cargo.toml`, `Cargo.lock`, `src/ports/mod.rs`,
  `src/adapters/mod.rs`, `src/fakes/mod.rs`

**Interfaces:**

- Consumes: `ports::{Http, HttpError}`.
- Produces: `ports::Sleeper::sleep(&self, Duration)`;
  `adapters::std_sleeper::StdSleeper`; test-only `fakes::FakeSleeper` with
  `slept() -> Vec<Duration>`; `adapters::ureq_http::UreqHttp::new(Option<String>)`
  (the optional, already sanitized `NVM_AUTH_HEADER` value is sent as
  `Authorization: <value>`, as `nvm_download` does); `adapters::retrying_http::
  RetryingHttp::new(&dyn Http, &dyn Sleeper).with_policy(attempts, base_delay)`
  (defaults: 3 attempts, 250 ms, doubling).
- Behaviour: redirects are followed; 4xx other than 429 are final; 5xx, 429 and
  transport errors are retried and, after the last attempt, the last error is
  returned. The `UreqHttp` tests talk to a throwaway `TcpListener` on
  `127.0.0.1`, so they need no network.
- [ ] **Step 1: Add the dependency**

Run: `cargo add ureq@3`

Expected: `Cargo.toml` gains the line below under `[dependencies]` and
`Cargo.lock` is updated. Commit both with this task.

```toml
ureq = "3"
```

- [ ] **Step 2: Declare the new modules**

Apply to `src/adapters/mod.rs`:

```diff
--- a/src/adapters/mod.rs
+++ b/src/adapters/mod.rs
@@ -1,6 +1,9 @@
 pub mod fs_alias_store;
 pub mod no_http;
 pub mod no_process;
+pub mod retrying_http;
 pub mod std_env;
 pub mod std_fs;
 pub mod std_process;
+pub mod std_sleeper;
+pub mod ureq_http;
```

- [ ] **Step 3: Write the failing tests**

Create `src/adapters/retrying_http.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::*;
    use crate::fakes::FakeSleeper;

    /// Answers from a script, one entry per request.
    struct Scripted(RefCell<VecDeque<Result<String, HttpError>>>);

    impl Scripted {
        fn new(script: Vec<Result<String, HttpError>>) -> Self {
            Self(RefCell::new(script.into()))
        }
    }

    impl Http for Scripted {
        fn get_text(&self, url: &str) -> Result<String, HttpError> {
            self.0.borrow_mut().pop_front().unwrap_or_else(|| {
                Err(HttpError::Transport {
                    url: url.to_owned(),
                    message: "script exhausted".to_owned(),
                })
            })
        }
    }

    fn status(code: u16) -> Result<String, HttpError> {
        Err(HttpError::Status {
            url: "u".to_owned(),
            code,
        })
    }

    fn fetch(script: Vec<Result<String, HttpError>>) -> (Result<String, HttpError>, Vec<Duration>) {
        let inner = Scripted::new(script);
        let sleeper = FakeSleeper::default();
        let result = RetryingHttp::new(&inner, &sleeper).get_text("u");
        (result, sleeper.slept())
    }

    #[test]
    fn a_first_try_that_works_never_sleeps() {
        let (result, slept) = fetch(vec![Ok("body".to_owned())]);
        assert_eq!(result.unwrap(), "body");
        assert!(slept.is_empty());
    }

    #[test]
    fn transient_failures_are_retried_with_doubling_delays() {
        let (result, slept) = fetch(vec![status(503), status(500), Ok("body".to_owned())]);
        assert_eq!(result.unwrap(), "body");
        assert_eq!(
            slept,
            [Duration::from_millis(250), Duration::from_millis(500)]
        );
    }

    #[test]
    fn network_errors_and_429_are_transient_too() {
        let transport = Err(HttpError::Transport {
            url: "u".to_owned(),
            message: "reset".to_owned(),
        });
        let (result, slept) = fetch(vec![transport, status(429), Ok("x".to_owned())]);
        assert!(result.is_ok());
        assert_eq!(slept.len(), 2);
    }

    #[test]
    fn it_gives_up_after_the_last_attempt_with_the_last_error() {
        let (result, slept) = fetch(vec![status(503), status(503), status(502)]);
        assert_eq!(
            result.unwrap_err(),
            HttpError::Status {
                url: "u".into(),
                code: 502
            }
        );
        assert_eq!(slept.len(), 2);
    }

    #[test]
    fn a_client_error_is_final_at_once() {
        let (result, slept) = fetch(vec![status(404), Ok("never".to_owned())]);
        assert_eq!(
            result.unwrap_err(),
            HttpError::Status {
                url: "u".into(),
                code: 404
            }
        );
        assert!(slept.is_empty());
    }

    #[test]
    fn the_policy_can_be_changed_and_at_least_one_attempt_is_made() {
        let inner = Scripted::new(vec![status(503)]);
        let sleeper = FakeSleeper::default();
        let http = RetryingHttp::new(&inner, &sleeper).with_policy(0, Duration::from_secs(1));
        assert!(http.get_text("u").is_err());
        assert!(sleeper.slept().is_empty());
    }
}
```

Create `src/adapters/ureq_http.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};

    use super::*;

    /// Serves each canned response to one connection, in order, and returns
    /// the raw requests it saw.
    fn serve(responses: Vec<String>) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let mut seen = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0_u8; 4096];
                let read = stream.read(&mut buffer).unwrap();
                seen.push(String::from_utf8_lossy(&buffer[..read]).into_owned());
                stream.write_all(response.as_bytes()).unwrap();
            }
            seen
        });
        (base, handle)
    }

    fn ok(body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    fn fetches_the_body_as_text() {
        let (base, server) = serve(vec![ok("hello\nworld\n")]);
        let text = UreqHttp::new(None).get_text(&format!("{base}/index.tab"));
        assert_eq!(text.unwrap(), "hello\nworld\n");
        let requests = server.join().unwrap();
        assert!(
            requests[0].starts_with("GET /index.tab HTTP/1.1"),
            "{}",
            requests[0]
        );
    }

    #[test]
    fn an_error_status_is_a_status_error() {
        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let (base, server) = serve(vec![response.to_owned()]);
        let url = format!("{base}/missing");
        let error = UreqHttp::new(None).get_text(&url).unwrap_err();
        assert_eq!(error, HttpError::Status { url, code: 404 });
        server.join().unwrap();
    }

    #[test]
    fn redirects_are_followed() {
        let location = "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let (base, server) = serve(vec![location.to_owned(), ok("moved")]);
        let text = UreqHttp::new(None).get_text(&format!("{base}/start"));
        assert_eq!(text.unwrap(), "moved");
        let requests = server.join().unwrap();
        assert!(requests[1].starts_with("GET /final "), "{}", requests[1]);
    }

    #[test]
    fn the_auth_header_is_sent_when_given() {
        let (base, server) = serve(vec![ok("secret")]);
        let http = UreqHttp::new(Some("Bearer token123".to_owned()));
        assert_eq!(http.get_text(&format!("{base}/x")).unwrap(), "secret");
        let requests = server.join().unwrap();
        assert!(
            requests[0]
                .to_lowercase()
                .contains("authorization: bearer token123"),
            "{}",
            requests[0]
        );
    }

    #[test]
    fn a_refused_connection_is_a_transport_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/x", listener.local_addr().unwrap());
        drop(listener);
        let error = UreqHttp::new(None).get_text(&url).unwrap_err();
        assert!(matches!(error, HttpError::Transport { .. }), "{error:?}");
    }
}
```

Create `src/fakes/sleeper.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_instead_of_sleeping() {
        let sleeper = FakeSleeper::default();
        sleeper.sleep(Duration::from_millis(250));
        sleeper.sleep(Duration::from_millis(500));
        assert_eq!(
            sleeper.slept(),
            [Duration::from_millis(250), Duration::from_millis(500)]
        );
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find struct `UreqHttp`",
"cannot find struct `RetryingHttp`" and "cannot find trait `Sleeper`".

- [ ] **Step 5: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/adapters/retrying_http.rs`:

```rust
//! Retries transient download failures with exponential backoff.

use std::time::Duration;

use crate::ports::{Http, HttpError, Sleeper};

/// Wraps an [`Http`]: a network error, a 5xx or a 429 is tried again after
/// `base_delay`, then twice as long, and so on, up to `attempts` tries in all.
/// Any other status (a 404, say) is final at once.
pub struct RetryingHttp<'a> {
    inner: &'a dyn Http,
    sleeper: &'a dyn Sleeper,
    attempts: u32,
    base_delay: Duration,
}

impl<'a> RetryingHttp<'a> {
    #[must_use]
    pub fn new(inner: &'a dyn Http, sleeper: &'a dyn Sleeper) -> Self {
        Self {
            inner,
            sleeper,
            attempts: 3,
            base_delay: Duration::from_millis(250),
        }
    }

    #[must_use]
    pub fn with_policy(mut self, attempts: u32, base_delay: Duration) -> Self {
        self.attempts = attempts.max(1);
        self.base_delay = base_delay;
        self
    }
}

fn is_transient(error: &HttpError) -> bool {
    match error {
        HttpError::Transport { .. } => true,
        HttpError::Status { code, .. } => *code >= 500 || *code == 429,
    }
}

impl Http for RetryingHttp<'_> {
    fn get_text(&self, url: &str) -> Result<String, HttpError> {
        let mut delay = self.base_delay;
        let mut attempt = 1;
        loop {
            match self.inner.get_text(url) {
                Err(error) if is_transient(&error) && attempt < self.attempts => {
                    self.sleeper.sleep(delay);
                    delay *= 2;
                    attempt += 1;
                }
                result => return result,
            }
        }
    }
}

```

Create `src/adapters/std_sleeper.rs`:

```rust
use std::time::Duration;

use crate::ports::Sleeper;

pub struct StdSleeper;

impl Sleeper for StdSleeper {
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
```

Insert above the `#[cfg(test)]` line of `src/adapters/ureq_http.rs`:

```rust
//! The real `Http`: a blocking client that follows redirects and speaks TLS.

use std::time::Duration;

use crate::ports::{Http, HttpError};

const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_BODY_BYTES: u64 = 16 * 1024 * 1024;

pub struct UreqHttp {
    agent: ureq::Agent,
    auth_header: Option<String>,
}

impl UreqHttp {
    /// `auth_header` is sent as `Authorization` when given; sanitize it first
    /// (see `domain::http_header`).
    #[must_use]
    pub fn new(auth_header: Option<String>) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .build();
        Self {
            agent: config.into(),
            auth_header,
        }
    }
}

fn transport(url: &str, message: impl ToString) -> HttpError {
    HttpError::Transport {
        url: url.to_owned(),
        message: message.to_string(),
    }
}

impl Http for UreqHttp {
    fn get_text(&self, url: &str) -> Result<String, HttpError> {
        let mut request = self.agent.get(url);
        if let Some(value) = &self.auth_header {
            request = request.header("Authorization", value);
        }
        match request.call() {
            Ok(mut response) => response
                .body_mut()
                .with_config()
                .limit(MAX_BODY_BYTES)
                .read_to_string()
                .map_err(|error| transport(url, error)),
            Err(ureq::Error::StatusCode(code)) => Err(HttpError::Status {
                url: url.to_owned(),
                code,
            }),
            Err(error) => Err(transport(url, error)),
        }
    }
}

```

Apply to `src/fakes/mod.rs` (above the test module):

```diff
--- a/src/fakes/mod.rs
+++ b/src/fakes/mod.rs
@@ -4,8 +4,10 @@
 mod file_system;
 mod http;
 mod process;
+mod sleeper;
 
 pub use env::FakeEnv;
 pub use file_system::FakeFileSystem;
 pub use http::FakeHttp;
 pub use process::FakeProcess;
+pub use sleeper::FakeSleeper;
```

Insert above the `#[cfg(test)]` line of `src/fakes/sleeper.rs`:

```rust
use std::cell::RefCell;
use std::time::Duration;

use crate::ports::Sleeper;

/// Records how long it was asked to sleep, and returns at once.
#[derive(Default)]
pub struct FakeSleeper {
    slept: RefCell<Vec<Duration>>,
}

impl FakeSleeper {
    #[must_use]
    pub fn slept(&self) -> Vec<Duration> {
        self.slept.borrow().clone()
    }
}

impl Sleeper for FakeSleeper {
    fn sleep(&self, duration: Duration) {
        self.slept.borrow_mut().push(duration);
    }
}

```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -3,6 +3,7 @@
 use std::ffi::OsString;
 use std::io;
 use std::path::Path;
+use std::time::Duration;
 
 use thiserror::Error;
 
@@ -77,6 +78,11 @@
     fn get_text(&self, url: &str) -> Result<String, HttpError>;
 }
 
+pub trait Sleeper {
+    /// Waits for `duration` (between retries).
+    fn sleep(&self, duration: Duration);
+}
+
 pub trait Env {
     /// The variable as text; `None` when unset or not valid UTF-8.
     fn var(&self, key: &str) -> Option<String>;
```

Then run `cargo fmt`.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 248 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(adapters): add a ureq client with retries and exponential backoff"
```

---

### Task 3: Mirror URLs

**Files:**

- Create: `src/domain/mirror.rs`
- Modify: `src/domain/mod.rs`

**Interfaces:**

- Consumes: `ports::Env`, `domain::version::Flavor`.
- Produces: `domain::mirror::{MirrorUrl, MirrorError, from_env}`;
  `MirrorUrl::parse(&str)`, `MirrorUrl::index_url()`;
  `from_env(&dyn Env, Flavor) -> Result<MirrorUrl, MirrorError>`.
- Behaviour (from `nvm_get_mirror`): `NVM_NODEJS_ORG_MIRROR` and
  `NVM_IOJS_ORG_MIRROR` (when set and not empty) must start with `http://` or
  `https://` and contain only ASCII letters, digits and `. _ ~ : / @ % [ ] -`;
  anything else is rejected with `$NVM_NODEJS_ORG_MIRROR and
  $NVM_IOJS_ORG_MIRROR may only contain a URL`. The defaults are
  `https://nodejs.org/dist` and `https://iojs.org/dist`. The index is the
  mirror followed by `/index.tab`, appended as is (a trailing slash stays, so
  `.../dist/` gives `.../dist//index.tab`, as in `nvm.sh`).
- [ ] **Step 1: Declare the new modules**

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -5,6 +5,7 @@
 pub mod http_header;
 pub mod implicit;
 pub mod listing;
+pub mod mirror;
 pub mod nvmrc;
 pub mod path_search;
 pub mod version;
```

- [ ] **Step 2: Write the failing tests**

Create `src/domain/mirror.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::FakeEnv;

    #[test]
    fn plain_http_and_https_urls_are_accepted() {
        for raw in [
            "https://nodejs.org/dist",
            "http://127.0.0.1:18081",
            "https://user@mirror.example.com:8443/node/dist/",
            "http://[::1]:8080/dist",
            "https://mirror.example.com/a_b~c%20d-e",
        ] {
            assert!(MirrorUrl::parse(raw).is_ok(), "{raw}");
        }
    }

    #[test]
    fn anything_else_is_rejected() {
        for raw in [
            "",
            "http://",
            "ftp://x/y",
            "nodejs.org/dist",
            "HTTP://x/y",
            "http://a b",
            "http://x/y?z=1",
            "http://x/y#frag",
            "http://x/`id`",
            "http://x/'q'",
            "http://x/(a)",
            "http://x/a\\b",
            "http://x/a;b",
            "http://x/$HOME",
            "http://x/ä",
        ] {
            assert_eq!(MirrorUrl::parse(raw), Err(MirrorError), "{raw:?}");
        }
    }

    #[test]
    fn the_error_message_is_the_one_of_nvm_sh() {
        assert_eq!(
            MirrorError.to_string(),
            "$NVM_NODEJS_ORG_MIRROR and $NVM_IOJS_ORG_MIRROR may only contain a URL"
        );
    }

    #[test]
    fn the_index_is_appended_as_is() {
        let mirror = MirrorUrl::parse("http://x/dist/").unwrap();
        assert_eq!(mirror.index_url(), "http://x/dist//index.tab");
        let plain = MirrorUrl::parse("http://x/dist").unwrap();
        assert_eq!(plain.index_url(), "http://x/dist/index.tab");
    }

    #[test]
    fn the_defaults_are_nodejs_org_and_iojs_org() {
        let env = FakeEnv::default();
        let node = from_env(&env, Flavor::Node).unwrap();
        assert_eq!(node.index_url(), "https://nodejs.org/dist/index.tab");
        let iojs = from_env(&env, Flavor::IoJs).unwrap();
        assert_eq!(iojs.index_url(), "https://iojs.org/dist/index.tab");
    }

    #[test]
    fn the_variables_override_the_defaults_unless_empty() {
        let env = FakeEnv::default()
            .with_var("NVM_NODEJS_ORG_MIRROR", "http://127.0.0.1:1")
            .with_var("NVM_IOJS_ORG_MIRROR", "");
        let node = from_env(&env, Flavor::Node).unwrap();
        assert_eq!(node.index_url(), "http://127.0.0.1:1/index.tab");
        let iojs = from_env(&env, Flavor::IoJs).unwrap();
        assert_eq!(iojs.index_url(), "https://iojs.org/dist/index.tab");
    }

    #[test]
    fn an_invalid_variable_is_an_error_for_that_flavor_only() {
        let env = FakeEnv::default().with_var("NVM_NODEJS_ORG_MIRROR", "ftp://x/y");
        assert_eq!(from_env(&env, Flavor::Node), Err(MirrorError));
        assert!(from_env(&env, Flavor::IoJs).is_ok());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find struct `MirrorUrl`"
and "cannot find function `from_env`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/domain/mirror.rs`:

```rust
//! Where releases are listed and downloaded from: `NVM_NODEJS_ORG_MIRROR` and
//! `NVM_IOJS_ORG_MIRROR`, validated the way `nvm_get_mirror` does.

use thiserror::Error;

use crate::domain::version::Flavor;
use crate::ports::Env;

const NODE_DEFAULT: &str = "https://nodejs.org/dist";
const IOJS_DEFAULT: &str = "https://iojs.org/dist";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("$NVM_NODEJS_ORG_MIRROR and $NVM_IOJS_ORG_MIRROR may only contain a URL")]
pub struct MirrorError;

/// An `http://` or `https://` URL made only of the characters a mirror needs:
/// letters, digits and `. _ ~ : / @ % [ ] -`. Anything else (spaces, quotes,
/// backticks, `?`, `#`, `;`, `$`...) is rejected, because `nvm.sh` appends
/// paths to it and passes it to shell commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorUrl(String);

fn is_allowed(character: char) -> bool {
    character.is_ascii_alphanumeric() || "._~:/@%[]-".contains(character)
}

impl MirrorUrl {
    /// # Errors
    /// Returns [`MirrorError`] for anything that is not a plain mirror URL.
    pub fn parse(raw: &str) -> Result<Self, MirrorError> {
        let rest = raw
            .strip_prefix("https://")
            .or_else(|| raw.strip_prefix("http://"))
            .ok_or(MirrorError)?;
        if rest.is_empty() || !rest.chars().all(is_allowed) {
            return Err(MirrorError);
        }
        Ok(Self(raw.to_owned()))
    }

    /// The release index: `<mirror>/index.tab`, appended as is (a trailing
    /// slash in the mirror stays, as in `nvm.sh`).
    #[must_use]
    pub fn index_url(&self) -> String {
        format!("{}/index.tab", self.0)
    }
}

/// The mirror for `flavor`: its variable when set and not empty, else the
/// default.
///
/// # Errors
/// Returns [`MirrorError`] when the configured value is not a plain URL.
pub fn from_env(env: &dyn Env, flavor: Flavor) -> Result<MirrorUrl, MirrorError> {
    let (variable, default) = match flavor {
        Flavor::Node => ("NVM_NODEJS_ORG_MIRROR", NODE_DEFAULT),
        Flavor::IoJs => ("NVM_IOJS_ORG_MIRROR", IOJS_DEFAULT),
    };
    let configured = env.var(variable).filter(|value| !value.is_empty());
    MirrorUrl::parse(configured.as_deref().unwrap_or(default))
}

```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 255 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(domain): validate mirror urls like nvm_get_mirror"
```

---

### Task 4: The release index and the `lts/*` aliases

**Files:**

- Create: `src/domain/index.rs`
- Modify: `src/domain/mod.rs`

**Interfaces:**

- Consumes: `domain::version::{Version, Flavor}`.
- Produces: `domain::index::{Release, parse_index, lts_aliases}` where
  `Release { version: Version, lts: Option<String> }`;
  `parse_index(&str, Flavor) -> Vec<Release>` (skips the header line; column 1
  is the version and column 10 the LTS codename, `-` meaning none; io.js rows
  get the `iojs-` prefix); `lts_aliases(&[Release]) -> Vec<(alias, target)>`.
- Behaviour (from the `awk` of `nvm_ls_remote_index_tab`): the list starts with
  `lts/*` pointing at `lts/<newest codename>`, then one `lts/<name>` per
  codename (lowercase) pointing at the newest release of that codename. Names
  that do not match `^[a-z0-9][a-z0-9._-]*$` are skipped.
- [ ] **Step 1: Declare the new modules**

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -4,6 +4,7 @@
 pub mod floor;
 pub mod http_header;
 pub mod implicit;
+pub mod index;
 pub mod listing;
 pub mod mirror;
 pub mod nvmrc;
```

- [ ] **Step 2: Write the failing tests**

Create `src/domain/index.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn row(version: &str, lts: &str) -> String {
        format!("{version}\t2023-01-01\tlinux-x64\t10.0.0\t11.0.0\t1.0.0\t1.3\t3.0\t100\t{lts}\t-")
    }

    fn index(rows: &[(&str, &str)]) -> String {
        let header = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity";
        let body: Vec<String> = rows.iter().map(|(v, l)| row(v, l)).collect();
        format!("{header}\n{}\n", body.join("\n"))
    }

    fn node_index() -> String {
        index(&[
            ("v21.2.0", "-"),
            ("v21.1.0", "-"),
            ("v20.10.0", "Iron"),
            ("v20.9.0", "Iron"),
            ("v18.19.0", "Hydrogen"),
            ("v18.18.0", "Hydrogen"),
            ("v16.20.2", "Gallium"),
            ("v14.21.3", "Fermium"),
            ("v4.9.1", "Argon"),
            ("v4.0.0", "-"),
            ("v0.12.18", "-"),
            ("v0.10.48", "-"),
        ])
    }

    #[test]
    fn the_header_is_skipped_and_file_order_is_kept() {
        let releases = parse_index(&node_index(), Flavor::Node);
        assert_eq!(releases.len(), 12);
        assert_eq!(releases[0].version.to_string(), "v21.2.0");
        assert_eq!(releases[11].version.to_string(), "v0.10.48");
    }

    #[test]
    fn the_lts_column_is_read_and_a_dash_means_none() {
        let releases = parse_index(&node_index(), Flavor::Node);
        assert_eq!(releases[0].lts, None);
        assert_eq!(releases[2].lts.as_deref(), Some("Iron"));
    }

    #[test]
    fn io_js_rows_get_the_iojs_flavor() {
        let releases = parse_index(&index(&[("v3.3.1", "-"), ("v1.0.0", "-")]), Flavor::IoJs);
        let versions: Vec<String> = releases.iter().map(|r| r.version.to_string()).collect();
        assert_eq!(versions, ["iojs-v3.3.1", "iojs-v1.0.0"]);
    }

    #[test]
    fn blank_lines_short_rows_and_junk_are_tolerated() {
        let text = "header\n\nv20.0.0\nnot-a-version\tx\nv18.0.0\t2023\r\n";
        let releases = parse_index(text, Flavor::Node);
        let versions: Vec<String> = releases.iter().map(|r| r.version.to_string()).collect();
        assert_eq!(versions, ["v20.0.0", "v18.0.0"]);
        assert!(releases.iter().all(|release| release.lts.is_none()));
    }

    #[test]
    fn lts_aliases_follow_the_order_nvm_sh_writes_them() {
        let aliases = lts_aliases(&parse_index(&node_index(), Flavor::Node));
        let pairs: Vec<(&str, &str)> = aliases
            .iter()
            .map(|(alias, target)| (alias.as_str(), target.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("lts/*", "lts/iron"),
                ("lts/iron", "v20.10.0"),
                ("lts/hydrogen", "v18.19.0"),
                ("lts/gallium", "v16.20.2"),
                ("lts/fermium", "v14.21.3"),
                ("lts/argon", "v4.9.1"),
            ]
        );
    }

    #[test]
    fn a_codename_points_at_its_newest_release_and_names_are_lowercased() {
        let aliases = lts_aliases(&parse_index(
            &index(&[
                ("v20.10.0", "Iron"),
                ("v20.9.0", "iron"),
                ("v18.1.0", "Hydrogen"),
            ]),
            Flavor::Node,
        ));
        assert_eq!(aliases[1], ("lts/iron".to_owned(), "v20.10.0".to_owned()));
        assert_eq!(aliases.len(), 3);
    }

    #[test]
    fn names_that_are_not_plain_words_are_skipped() {
        let aliases = lts_aliases(&parse_index(
            &index(&[
                ("v20.0.0", "Iron!"),
                ("v18.0.0", "-bad"),
                ("v16.0.0", "Gallium"),
            ]),
            Flavor::Node,
        ));
        assert_eq!(
            aliases,
            [
                ("lts/*".to_owned(), "lts/gallium".to_owned()),
                ("lts/gallium".to_owned(), "v16.0.0".to_owned()),
            ]
        );
    }

    #[test]
    fn without_any_lts_release_there_are_no_aliases() {
        let aliases = lts_aliases(&parse_index(&index(&[("v21.0.0", "-")]), Flavor::Node));
        assert!(aliases.is_empty());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function
`parse_index`" and "cannot find function `lts_aliases`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/domain/index.rs`:

```rust
//! The release index of a mirror, `<mirror>/index.tab`: a tab-separated table
//! with a header line, newest release first. Column 1 is the version, column
//! 10 the LTS codename (`-` when there is none).

use std::collections::HashSet;

use crate::domain::version::{Flavor, Version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// The LTS codename as the index spells it (`Iron`).
    pub lts: Option<String>,
}

/// The releases of `index_tab`, in file order. The header, blank lines and
/// rows whose version is not one are skipped.
#[must_use]
pub fn parse_index(index_tab: &str, flavor: Flavor) -> Vec<Release> {
    index_tab
        .lines()
        .skip(1)
        .filter_map(|line| parse_row(line, flavor))
        .collect()
}

fn parse_row(line: &str, flavor: Flavor) -> Option<Release> {
    let columns: Vec<&str> = line.split_whitespace().collect();
    let raw = columns.first()?;
    let text = match flavor {
        Flavor::Node => (*raw).to_owned(),
        Flavor::IoJs => format!("iojs-{raw}"),
    };
    let lts = columns
        .get(9)
        .filter(|name| **name != "-")
        .map(|name| (*name).to_owned());
    Some(Release {
        version: text.parse().ok()?,
        lts,
    })
}

fn is_alias_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
        && characters.all(|rest| rest.is_ascii_alphanumeric() || "._-".contains(rest))
}

/// The `lts/*` aliases a listing refreshes, as `(alias, target)` in the order
/// `nvm.sh` writes them: `lts/*` first (the newest codename), then one alias
/// per codename pointing at the newest release of that codename. Names that
/// are not plain lowercase words are skipped.
#[must_use]
pub fn lts_aliases(releases: &[Release]) -> Vec<(String, String)> {
    let mut seen = HashSet::new();
    let mut aliases: Vec<(String, String)> = Vec::new();
    for release in releases {
        let Some(name) = release.lts.as_deref().map(str::to_lowercase) else {
            continue;
        };
        if !is_alias_name(&name) || !seen.insert(name.clone()) {
            continue;
        }
        let alias = format!("lts/{name}");
        if aliases.is_empty() {
            aliases.push(("lts/*".to_owned(), alias.clone()));
        }
        aliases.push((alias, release.version.to_string()));
    }
    aliases
}

```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 263 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(domain): parse the release index and derive the lts aliases"
```

---

### Task 5: Which releases `ls-remote` lists

**Files:**

- Create: `src/domain/remote/mod.rs`, `src/domain/remote/tests.rs`
- Modify: `src/domain/mod.rs`

**Interfaces:**

- Consumes: `domain::index::Release`, `domain::version::{Version, Flavor}`.
- Produces: `domain::remote::{RemoteRow, Query, Listing, RemoteError, list,
  normalize_lts, LtsError}`. `RemoteRow { version, lts, latest_lts }` with
  `line()` (`v18.19.0 Hydrogen *`); `Query { pattern, lts }` (`lts` is the
  normalized codename, or `*`); `list(Option<&[Release]>, Option<&[Release]>,
  &Query) -> Result<Listing { rows, missing }, RemoteError>` (a `None` slice is
  an index that could not be had); `normalize_lts(&str, &[String]) ->
  Result<String, LtsError>` (`-N` is the codename `N` places before the last of
  the sorted `alias/lts` names; `LtsError::status()` is 3 for an uppercase name
  and 2 for going back too far).
- Behaviour (from `nvm_remote_versions` and `nvm_ls_remote_index_tab`, checked
  against the real script): the words `node`, `iojs` and `io.js` pick a flavor;
  `stable` and `unstable` are the error `Implicit aliases are not supported in
  nvm_remote_versions.`; the io.js index is skipped under an LTS filter or the
  `node` flavor; Node rows come first up to `v4.0.0`, then io.js, then the rest
  of Node; a pattern is `grep -w` over the whole row text (`v20.10.0 Iron *`):
  it matches at any word-bounded position, so `Iron` lists the Iron rows; an
  LTS filter keeps rows whose codename contains the name (case-insensitive)
  and the `*` filter every LTS row; the newest release of a
  codename is marked, in index order and before the pattern is applied;
  `missing` is true when any part that ran listed nothing, because `nvm.sh`
  then exits 3 even if other rows were found.
- `src/domain/remote/` is a directory so its tests stay under 300 lines.
- [ ] **Step 1: Declare the new modules**

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -9,4 +9,5 @@
 pub mod mirror;
 pub mod nvmrc;
 pub mod path_search;
+pub mod remote;
 pub mod version;
```

- [ ] **Step 2: Write the failing tests**

Create `src/domain/remote/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/domain/remote/tests.rs`:

```rust
use super::*;
use crate::domain::index::parse_index;

fn index(rows: &[(&str, &str)]) -> String {
    let header = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity";
    let body: Vec<String> = rows
        .iter()
        .map(|(version, lts)| {
            format!(
                "{version}\t2023-01-01\tlinux-x64\t10.0.0\t11.0.0\t1.0.0\t1.3\t3.0\t100\t{lts}\t-"
            )
        })
        .collect();
    format!("{header}\n{}\n", body.join("\n"))
}

/// The fixture mirrors the real nvm.sh was run against.
fn node() -> Vec<Release> {
    parse_index(
        &index(&[
            ("v21.2.0", "-"),
            ("v21.1.0", "-"),
            ("v20.10.0", "Iron"),
            ("v20.9.0", "Iron"),
            ("v18.19.0", "Hydrogen"),
            ("v18.18.0", "Hydrogen"),
            ("v16.20.2", "Gallium"),
            ("v14.21.3", "Fermium"),
            ("v4.9.1", "Argon"),
            ("v4.0.0", "-"),
            ("v0.12.18", "-"),
            ("v0.10.48", "-"),
        ]),
        Flavor::Node,
    )
}

fn iojs() -> Vec<Release> {
    parse_index(
        &index(&[
            ("v3.3.1", "-"),
            ("v3.0.0", "-"),
            ("v2.5.0", "-"),
            ("v1.0.0", "-"),
        ]),
        Flavor::IoJs,
    )
}

fn query(pattern: Option<&str>, lts: Option<&str>) -> Query {
    Query {
        pattern: pattern.map(str::to_owned),
        lts: lts.map(str::to_owned),
    }
}

fn listing(pattern: Option<&str>, lts: Option<&str>) -> (Vec<String>, bool) {
    let (node, iojs) = (node(), iojs());
    let result = list(Some(&node), Some(&iojs), &query(pattern, lts)).unwrap();
    let lines = result.rows.iter().map(RemoteRow::line).collect();
    (lines, result.missing)
}

#[test]
fn everything_is_listed_with_io_js_between_node_0_12_and_4_0() {
    let (lines, missing) = listing(None, None);
    assert_eq!(
        lines,
        [
            "v0.10.48",
            "v0.12.18",
            "iojs-v1.0.0",
            "iojs-v2.5.0",
            "iojs-v3.0.0",
            "iojs-v3.3.1",
            "v4.0.0",
            "v4.9.1 Argon *",
            "v14.21.3 Fermium *",
            "v16.20.2 Gallium *",
            "v18.18.0 Hydrogen",
            "v18.19.0 Hydrogen *",
            "v20.9.0 Iron",
            "v20.10.0 Iron *",
            "v21.1.0",
            "v21.2.0",
        ]
    );
    assert!(!missing);
}

#[test]
fn lts_lists_only_lts_releases_and_never_asks_io_js() {
    let (lines, missing) = listing(None, Some("*"));
    assert_eq!(
        lines,
        [
            "v4.9.1 Argon *",
            "v14.21.3 Fermium *",
            "v16.20.2 Gallium *",
            "v18.18.0 Hydrogen",
            "v18.19.0 Hydrogen *",
            "v20.9.0 Iron",
            "v20.10.0 Iron *",
        ]
    );
    assert!(!missing);
}

#[test]
fn an_lts_name_filters_by_substring_ignoring_case() {
    let expected = ["v20.9.0 Iron", "v20.10.0 Iron *"];
    assert_eq!(listing(None, Some("iron")).0, expected);
    assert_eq!(listing(None, Some("ir")).0, expected);
    assert_eq!(listing(None, Some("IRON")).0, expected);
}

#[test]
fn an_unknown_lts_name_finds_nothing() {
    assert_eq!(listing(None, Some("foo")), (Vec::new(), true));
}

#[test]
fn a_pattern_matches_whole_version_words() {
    let both = ["v20.9.0 Iron", "v20.10.0 Iron *"];
    assert_eq!(listing(Some("20"), None).0, both);
    assert_eq!(listing(Some("v20"), None).0, both);
    assert_eq!(listing(Some("20."), None).0, both);
    assert_eq!(listing(Some("18.19"), None).0, ["v18.19.0 Hydrogen *"]);
    assert_eq!(listing(Some("18.18"), None).0, ["v18.18.0 Hydrogen"]);
    assert_eq!(listing(Some("v20.10.0"), None).0, ["v20.10.0 Iron *"]);
    // `v2` is a whole word in `iojs-v2.5.0`, so io.js answers; node has no v2.
    assert_eq!(
        listing(Some("2"), None),
        (vec!["iojs-v2.5.0".to_owned()], true)
    );
    assert_eq!(
        listing(Some("1"), None),
        (vec!["iojs-v1.0.0".to_owned()], true)
    );
    assert_eq!(listing(Some("v"), None), (Vec::new(), true));
    assert_eq!(listing(Some("99"), None), (Vec::new(), true));
}

#[test]
fn a_pattern_without_io_js_matches_exits_missing_even_with_rows() {
    // nvm.sh exits 3 here: its io.js part found nothing for `20`.
    let (lines, missing) = listing(Some("20"), None);
    assert_eq!(lines.len(), 2);
    assert!(missing);
    // With --lts the io.js part does not run, so the same pattern succeeds.
    assert!(!listing(Some("20"), Some("*")).1);
}

#[test]
fn a_plain_number_also_finds_io_js() {
    let (lines, missing) = listing(Some("3"), None);
    assert_eq!(lines, ["iojs-v3.0.0", "iojs-v3.3.1"]);
    assert!(missing, "the node part found nothing");
}

#[test]
fn the_words_node_and_iojs_pick_one_flavor_and_succeed() {
    let (node_lines, node_missing) = listing(Some("node"), None);
    assert_eq!(node_lines.len(), 12);
    assert!(node_lines.iter().all(|line| !line.starts_with("iojs-")));
    assert!(!node_missing);
    let expected = ["iojs-v1.0.0", "iojs-v2.5.0", "iojs-v3.0.0", "iojs-v3.3.1"];
    assert_eq!(
        listing(Some("iojs"), None),
        (expected.map(String::from).to_vec(), false)
    );
    assert_eq!(listing(Some("io.js"), None).0, expected);
}

#[test]
fn an_iojs_flavor_with_an_lts_filter_finds_nothing() {
    assert_eq!(listing(Some("iojs"), Some("*")), (Vec::new(), true));
}

#[test]
fn stable_and_unstable_are_not_supported_remotely() {
    let (node, iojs) = (node(), iojs());
    for word in ["stable", "unstable"] {
        let result = list(Some(&node), Some(&iojs), &query(Some(word), None));
        assert_eq!(result, Err(RemoteError::ImplicitAlias), "{word}");
    }
}

#[test]
fn an_unreachable_index_is_missing_and_the_other_one_still_lists() {
    let iojs = iojs();
    let result = list(None, Some(&iojs), &query(None, None)).unwrap();
    assert_eq!(result.rows.len(), 4);
    assert!(result.missing);
    let neither = list(None, None, &query(None, None)).unwrap();
    assert_eq!(
        neither,
        Listing {
            rows: Vec::new(),
            missing: true
        }
    );
}

#[test]
fn without_a_v4_row_io_js_comes_after_all_node_rows() {
    let node = parse_index(&index(&[("v20.0.0", "-"), ("v18.0.0", "-")]), Flavor::Node);
    let iojs = iojs();
    let result = list(Some(&node), Some(&iojs), &query(None, None)).unwrap();
    let lines: Vec<String> = result.rows.iter().map(RemoteRow::line).collect();
    assert_eq!(lines[..2], ["v18.0.0", "v20.0.0"]);
    assert_eq!(lines[2], "iojs-v1.0.0");
}

fn names() -> Vec<String> {
    ["*", "argon", "fermium", "gallium", "hydrogen", "iron"]
        .map(String::from)
        .to_vec()
}

#[test]
fn a_back_count_picks_the_codename_before_the_last() {
    assert_eq!(normalize_lts("-1", &names()).unwrap(), "hydrogen");
    assert_eq!(normalize_lts("-2", &names()).unwrap(), "gallium");
    assert_eq!(normalize_lts("-4", &names()).unwrap(), "argon");
}

#[test]
fn going_back_past_the_first_codename_is_an_error_with_status_2() {
    for back in ["-5", "-9", "-12"] {
        let error = normalize_lts(back, &names()).unwrap_err();
        assert_eq!(error, LtsError::TooFarBack, "{back}");
        assert_eq!(error.status(), 2);
    }
    assert_eq!(normalize_lts("-1", &[]), Err(LtsError::TooFarBack));
}

#[test]
fn names_must_be_lowercase_and_stay_as_they_are() {
    let error = normalize_lts("Iron", &names()).unwrap_err();
    assert_eq!(error, LtsError::NotLowercase);
    assert_eq!(error.status(), 3);
    assert_eq!(error.to_string(), "LTS names must be lowercase");
    assert_eq!(normalize_lts("iron", &names()).unwrap(), "iron");
    assert_eq!(normalize_lts("*", &names()).unwrap(), "*");
    assert_eq!(normalize_lts("-0", &names()).unwrap(), "-0");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function `list`",
"cannot find struct `Query`" and "cannot find function `normalize_lts`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/domain/remote/mod.rs`:

```rust
//! Which remote releases `nvm ls-remote` lists, as `nvm_remote_versions` picks
//! them, before they are formatted into rows.

use thiserror::Error;

use crate::domain::index::Release;
use crate::domain::version::{Flavor, Version};

/// A release to list. [`Self::line`] is the text `nvm.sh` passes on to its
/// formatter: the version, then the LTS codename, then `*` on the newest
/// release of that codename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRow {
    pub version: Version,
    pub lts: Option<String>,
    pub latest_lts: bool,
}

impl RemoteRow {
    #[must_use]
    pub fn line(&self) -> String {
        match (&self.lts, self.latest_lts) {
            (None, _) => self.version.to_string(),
            (Some(name), false) => format!("{} {name}", self.version),
            (Some(name), true) => format!("{} {name} *", self.version),
        }
    }
}

/// What the user asked for. `lts` is the normalized codename filter: `*` for
/// any LTS release, or a (lowercase) name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub pattern: Option<String>,
    pub lts: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RemoteError {
    #[error("Implicit aliases are not supported in nvm_remote_versions.")]
    ImplicitAlias,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub rows: Vec<RemoteRow>,
    /// Some part that ran found nothing (or could not be fetched): `nvm.sh`
    /// then exits 3, even when other rows were found.
    pub missing: bool,
}

/// # Errors
/// Returns [`RemoteError::ImplicitAlias`] for `stable` and `unstable`.
pub fn list(
    node: Option<&[Release]>,
    iojs: Option<&[Release]>,
    query: &Query,
) -> Result<Listing, RemoteError> {
    let mut flavor = query.lts.as_ref().map(|_| Flavor::Node);
    let mut pattern = query.pattern.as_deref().filter(|text| !text.is_empty());
    match pattern {
        Some("iojs" | "io.js") => (flavor, pattern) = (Some(Flavor::IoJs), None),
        Some("node") => (flavor, pattern) = (Some(Flavor::Node), None),
        Some("stable" | "unstable") => return Err(RemoteError::ImplicitAlias),
        _ => {}
    }
    let node_runs = flavor != Some(Flavor::IoJs);
    let iojs_runs = query.lts.is_none() && flavor != Some(Flavor::Node);

    let mut missing = false;
    let mut node_rows = Vec::new();
    let mut iojs_rows = Vec::new();
    if node_runs {
        node_rows = node
            .map(|releases| select(releases, pattern, query.lts.as_deref(), false))
            .unwrap_or_default();
        missing |= node_rows.is_empty();
    }
    if iojs_runs {
        iojs_rows = iojs
            .map(|releases| select(releases, pattern, None, true))
            .unwrap_or_default();
        missing |= iojs_rows.is_empty();
    }
    let rows = merge(node_rows, iojs_rows);
    Ok(Listing {
        missing: missing || rows.is_empty(),
        rows,
    })
}

/// Node rows come first up to `v4.0.0`, then io.js, then the rest of Node.
/// Without a `v4.0.0` row all Node rows come before io.js.
fn merge(node_rows: Vec<RemoteRow>, iojs_rows: Vec<RemoteRow>) -> Vec<RemoteRow> {
    let split = node_rows
        .iter()
        .position(|row| row.version.to_string() == "v4.0.0")
        .unwrap_or(node_rows.len());
    let mut merged = node_rows;
    let after = merged.split_off(split);
    merged.extend(iojs_rows);
    merged.extend(after);
    merged
}

/// The part of `nvm_ls_remote_index_tab` for one flavor: keep the releases
/// that fit the LTS filter and mark the newest of each codename, then keep
/// those whose version matches the pattern, ascending.
fn select(
    releases: &[Release],
    pattern: Option<&str>,
    lts: Option<&str>,
    iojs: bool,
) -> Vec<RemoteRow> {
    let pattern = pattern
        .map(|text| normalize_pattern(text, iojs))
        .filter(|text| !text.is_empty());
    let mut previous: Option<&str> = None;
    let mut rows = Vec::new();
    for release in releases {
        if !fits_lts(release, lts) {
            continue;
        }
        let name = release.lts.as_deref();
        rows.push(RemoteRow {
            version: release.version,
            lts: release.lts.clone(),
            latest_lts: name.is_some() && name != previous,
        });
        previous = name;
    }
    rows.retain(|row| {
        pattern
            .as_deref()
            .is_none_or(|text| matches_word(row, text))
    });
    rows.sort_by_key(|row| row.version);
    rows
}

fn fits_lts(release: &Release, lts: Option<&str>) -> bool {
    match (lts, release.lts.as_deref()) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some("*"), Some(_)) => true,
        (Some(wanted), Some(name)) => name.to_lowercase().contains(&wanted.to_lowercase()),
    }
}

/// Like `nvm_ensure_version_prefix`: a trailing `.` and `*` are dropped, a
/// leading digit gets the `v`, and for io.js the `iojs-` prefix is dropped.
fn normalize_pattern(pattern: &str, iojs: bool) -> String {
    let trimmed = pattern.strip_suffix('.').unwrap_or(pattern);
    if trimmed == "*" {
        return String::new();
    }
    let bare = if iojs {
        trimmed.strip_prefix("iojs-").unwrap_or(trimmed)
    } else {
        trimmed
    };
    if bare.starts_with(|first: char| first.is_ascii_digit()) {
        format!("v{bare}")
    } else {
        bare.to_owned()
    }
}

/// `grep -w`: the pattern starts the version and is not followed by a word
/// character, so `v20` and `v20.10` match `v20.10.0` but `v2` does not.
fn matches_word(row: &RemoteRow, pattern: &str) -> bool {
    let text = row.version.directory_name();
    text.strip_prefix(pattern).is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(|next| !(next.is_ascii_alphanumeric() || next == '_'))
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LtsError {
    #[error("LTS names must be lowercase")]
    NotLowercase,
    #[error("That many LTS releases do not exist yet.")]
    TooFarBack,
}

impl LtsError {
    /// The exit status `nvm.sh` gives it.
    #[must_use]
    pub fn status(&self) -> u8 {
        match self {
            Self::NotLowercase => 3,
            Self::TooFarBack => 2,
        }
    }
}

/// Like `nvm_normalize_lts`: `-N` is the codename `N` places before the last in
/// `names` (the entries of `alias/lts`, sorted), any other name must be
/// lowercase. Returns the codename, or `*`.
///
/// # Errors
/// [`LtsError::TooFarBack`] when there are not that many codenames, and
/// [`LtsError::NotLowercase`] for a name with uppercase letters.
pub fn normalize_lts(wanted: &str, names: &[String]) -> Result<String, LtsError> {
    if let Some(back) = wanted.strip_prefix('-').and_then(back_count) {
        let index = names.len().saturating_sub(back + 1);
        return match names.get(index) {
            Some(name) if name != "*" => Ok(name.clone()),
            _ => Err(LtsError::TooFarBack),
        };
    }
    if wanted != wanted.to_lowercase() {
        return Err(LtsError::NotLowercase);
    }
    Ok(wanted.to_owned())
}

/// `1`..`9`, optionally followed by more digits.
fn back_count(digits: &str) -> Option<usize> {
    let valid = digits.chars().all(|digit| digit.is_ascii_digit())
        && !digits.starts_with('0')
        && !digits.is_empty();
    valid.then(|| digits.parse().ok()).flatten()
}

```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 278 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(domain): select the releases ls-remote lists"
```

---

### Task 6: Formatting the rows

**Files:**

- Create: `src/domain/remote_format/mod.rs`,
  `src/domain/remote_format/tests.rs`, `src/domain/fixtures.rs` (test only)
- Modify: `src/domain/mod.rs`, `src/domain/remote/tests.rs`

**Interfaces:**

- Consumes: `domain::remote::RemoteRow`, `domain::listing::{format_row,
  RowKind}`.
- Produces: `domain::remote_format::{FormatInput, format_remote_rows}` where
  `FormatInput { rows, installed, current, latest_alias, named_aliases }` and
  `format_remote_rows(&FormatInput) -> Vec<String>`; test-only
  `domain::fixtures::{index_text, node_releases, iojs_releases}`, the mirror
  every later test uses (`remote/tests.rs` is changed to use it, DRY).
- Behaviour (the `awk` of `nvm_print_versions`, checked against the real
  script): the version is right-aligned like `nvm ls` rows, with `*` for an
  installed release and `->` for the current one; then up to three annotation
  columns, each starting at the same place on every row: `(LTS: X)` or
  `(Latest LTS: X)`, `(Latest: node)` on the newest stable release, and
  `(Aliases: a, b)`. The latest stable and unstable releases are found from
  the row order (`iojs-` prefix, `v0.<odd>.` old unstable, `v<digit>` stable).
  A pattern or LTS listing passes no `latest_alias` and no aliases.
- [ ] **Step 1: Declare the new modules**

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -1,6 +1,8 @@
 pub mod alias;
 pub mod alias_format;
 pub mod current;
+#[cfg(test)]
+pub(crate) mod fixtures;
 pub mod floor;
 pub mod http_header;
 pub mod implicit;
@@ -10,4 +12,5 @@
 pub mod nvmrc;
 pub mod path_search;
 pub mod remote;
+pub mod remote_format;
 pub mod version;
```

- [ ] **Step 2: Write the failing tests**

Create `src/domain/fixtures.rs`:

```rust
//! The mirror fixture the real `nvm.sh` was run against, for tests of the
//! remote listing.

use crate::domain::index::{Release, parse_index};
use crate::domain::version::Flavor;

#[must_use]
pub fn index_text(rows: &[(&str, &str)]) -> String {
    let header = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity";
    let body: Vec<String> = rows
        .iter()
        .map(|(version, lts)| {
            format!(
                "{version}\t2023-01-01\tlinux-x64\t10.0.0\t11.0.0\t1.0.0\t1.3\t3.0\t100\t{lts}\t-"
            )
        })
        .collect();
    format!("{header}\n{}\n", body.join("\n"))
}

#[must_use]
pub fn node_releases() -> Vec<Release> {
    parse_index(
        &index_text(&[
            ("v21.2.0", "-"),
            ("v21.1.0", "-"),
            ("v20.10.0", "Iron"),
            ("v20.9.0", "Iron"),
            ("v18.19.0", "Hydrogen"),
            ("v18.18.0", "Hydrogen"),
            ("v16.20.2", "Gallium"),
            ("v14.21.3", "Fermium"),
            ("v4.9.1", "Argon"),
            ("v4.0.0", "-"),
            ("v0.12.18", "-"),
            ("v0.10.48", "-"),
        ]),
        Flavor::Node,
    )
}

#[must_use]
pub fn iojs_releases() -> Vec<Release> {
    parse_index(
        &index_text(&[
            ("v3.3.1", "-"),
            ("v3.0.0", "-"),
            ("v2.5.0", "-"),
            ("v1.0.0", "-"),
        ]),
        Flavor::IoJs,
    )
}
```

Apply to `src/domain/remote/tests.rs`:

```diff
--- a/src/domain/remote/tests.rs
+++ b/src/domain/remote/tests.rs
@@ -1,51 +1,6 @@
 use super::*;
+use crate::domain::fixtures::{index_text, iojs_releases, node_releases};
 use crate::domain::index::parse_index;
-
-fn index(rows: &[(&str, &str)]) -> String {
-    let header = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity";
-    let body: Vec<String> = rows
-        .iter()
-        .map(|(version, lts)| {
-            format!(
-                "{version}\t2023-01-01\tlinux-x64\t10.0.0\t11.0.0\t1.0.0\t1.3\t3.0\t100\t{lts}\t-"
-            )
-        })
-        .collect();
-    format!("{header}\n{}\n", body.join("\n"))
-}
-
-/// The fixture mirrors the real nvm.sh was run against.
-fn node() -> Vec<Release> {
-    parse_index(
-        &index(&[
-            ("v21.2.0", "-"),
-            ("v21.1.0", "-"),
-            ("v20.10.0", "Iron"),
-            ("v20.9.0", "Iron"),
-            ("v18.19.0", "Hydrogen"),
-            ("v18.18.0", "Hydrogen"),
-            ("v16.20.2", "Gallium"),
-            ("v14.21.3", "Fermium"),
-            ("v4.9.1", "Argon"),
-            ("v4.0.0", "-"),
-            ("v0.12.18", "-"),
-            ("v0.10.48", "-"),
-        ]),
-        Flavor::Node,
-    )
-}
-
-fn iojs() -> Vec<Release> {
-    parse_index(
-        &index(&[
-            ("v3.3.1", "-"),
-            ("v3.0.0", "-"),
-            ("v2.5.0", "-"),
-            ("v1.0.0", "-"),
-        ]),
-        Flavor::IoJs,
-    )
-}
 
 fn query(pattern: Option<&str>, lts: Option<&str>) -> Query {
     Query {
@@ -55,7 +10,7 @@
 }
 
 fn listing(pattern: Option<&str>, lts: Option<&str>) -> (Vec<String>, bool) {
-    let (node, iojs) = (node(), iojs());
+    let (node, iojs) = (node_releases(), iojs_releases());
     let result = list(Some(&node), Some(&iojs), &query(pattern, lts)).unwrap();
     let lines = result.rows.iter().map(RemoteRow::line).collect();
     (lines, result.missing)
@@ -179,7 +134,7 @@
 
 #[test]
 fn stable_and_unstable_are_not_supported_remotely() {
-    let (node, iojs) = (node(), iojs());
+    let (node, iojs) = (node_releases(), iojs_releases());
     for word in ["stable", "unstable"] {
         let result = list(Some(&node), Some(&iojs), &query(Some(word), None));
         assert_eq!(result, Err(RemoteError::ImplicitAlias), "{word}");
@@ -188,7 +143,7 @@
 
 #[test]
 fn an_unreachable_index_is_missing_and_the_other_one_still_lists() {
-    let iojs = iojs();
+    let iojs = iojs_releases();
     let result = list(None, Some(&iojs), &query(None, None)).unwrap();
     assert_eq!(result.rows.len(), 4);
     assert!(result.missing);
@@ -204,8 +159,11 @@
 
 #[test]
 fn without_a_v4_row_io_js_comes_after_all_node_rows() {
-    let node = parse_index(&index(&[("v20.0.0", "-"), ("v18.0.0", "-")]), Flavor::Node);
-    let iojs = iojs();
+    let node = parse_index(
+        &index_text(&[("v20.0.0", "-"), ("v18.0.0", "-")]),
+        Flavor::Node,
+    );
+    let iojs = iojs_releases();
     let result = list(Some(&node), Some(&iojs), &query(None, None)).unwrap();
     let lines: Vec<String> = result.rows.iter().map(RemoteRow::line).collect();
     assert_eq!(lines[..2], ["v18.0.0", "v20.0.0"]);
```

Create `src/domain/remote_format/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/domain/remote_format/tests.rs`:

```rust
use super::*;
use crate::domain::fixtures::{iojs_releases, node_releases};
use crate::domain::remote::{Query, list};

fn rows(pattern: Option<&str>) -> Vec<RemoteRow> {
    let (node, iojs) = (node_releases(), iojs_releases());
    let query = Query {
        pattern: pattern.map(str::to_owned),
        lts: None,
    };
    list(Some(&node), Some(&iojs), &query).unwrap().rows
}

fn versions(names: &[&str]) -> Vec<Version> {
    names.iter().map(|name| name.parse().unwrap()).collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(target, name)| ((*target).to_owned(), (*name).to_owned()))
        .collect()
}

/// Every expected line below is what the real nvm.sh printed for the same
/// mirror, installed versions and aliases.
#[test]
fn a_plain_listing_matches_nvm_sh() {
    let rows = rows(None);
    let input = FormatInput {
        rows: &rows,
        installed: &[],
        current: "none",
        latest_alias: Some("node"),
        named_aliases: &[],
    };
    assert_eq!(
        format_remote_rows(&input),
        [
            "       v0.10.48",
            "       v0.12.18",
            "    iojs-v1.0.0",
            "    iojs-v2.5.0",
            "    iojs-v3.0.0",
            "    iojs-v3.3.1",
            "         v4.0.0",
            "         v4.9.1   (Latest LTS: Argon)",
            "       v14.21.3   (Latest LTS: Fermium)",
            "       v16.20.2   (Latest LTS: Gallium)",
            "       v18.18.0   (LTS: Hydrogen)",
            "       v18.19.0   (Latest LTS: Hydrogen)",
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0   (Latest LTS: Iron)",
            "        v21.1.0",
            "        v21.2.0                            (Latest: node)",
        ]
    );
}

#[test]
fn installed_versions_and_aliases_match_nvm_sh() {
    let rows = rows(None);
    let installed = versions(&["v20.10.0", "v18.19.0", "iojs-v3.3.1"]);
    let aliases = pairs(&[
        ("node", "default"),
        ("v99.0.0", "future"),
        ("v21.1.0", "pinned"),
        ("v20.10.0", "prod"),
        ("v18", "work"),
    ]);
    let input = FormatInput {
        rows: &rows,
        installed: &installed,
        current: "none",
        latest_alias: Some("node"),
        named_aliases: &aliases,
    };
    assert_eq!(
        format_remote_rows(&input),
        [
            "       v0.10.48",
            "       v0.12.18",
            "    iojs-v1.0.0",
            "    iojs-v2.5.0",
            "    iojs-v3.0.0",
            "    iojs-v3.3.1 *",
            "         v4.0.0",
            "         v4.9.1   (Latest LTS: Argon)",
            "       v14.21.3   (Latest LTS: Fermium)",
            "       v16.20.2   (Latest LTS: Gallium)",
            "       v18.18.0   (LTS: Hydrogen)",
            "       v18.19.0 * (Latest LTS: Hydrogen)                    (Aliases: work)",
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0 * (Latest LTS: Iron)                        (Aliases: prod)",
            "        v21.1.0                                             (Aliases: pinned)",
            "        v21.2.0                            (Latest: node)   (Aliases: default)",
        ]
    );
}

#[test]
fn the_current_version_gets_an_arrow() {
    let rows = rows(None);
    let installed = versions(&["v20.10.0", "v18.19.0", "iojs-v3.3.1"]);
    let aliases = pairs(&[("node", "default")]);
    let input = FormatInput {
        rows: &rows,
        installed: &installed,
        current: "v18.19.0",
        latest_alias: Some("node"),
        named_aliases: &aliases,
    };
    assert_eq!(
        format_remote_rows(&input),
        [
            "       v0.10.48",
            "       v0.12.18",
            "    iojs-v1.0.0",
            "    iojs-v2.5.0",
            "    iojs-v3.0.0",
            "    iojs-v3.3.1 *",
            "         v4.0.0",
            "         v4.9.1   (Latest LTS: Argon)",
            "       v14.21.3   (Latest LTS: Fermium)",
            "       v16.20.2   (Latest LTS: Gallium)",
            "       v18.18.0   (LTS: Hydrogen)",
            "->     v18.19.0 * (Latest LTS: Hydrogen)",
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0 * (Latest LTS: Iron)",
            "        v21.1.0",
            "        v21.2.0                            (Latest: node)   (Aliases: default)",
        ]
    );
}

#[test]
fn several_aliases_on_one_release_are_joined_in_order() {
    let rows = rows(None);
    let aliases = pairs(&[
        ("node", "default"),
        ("node", "latest"),
        ("v20.10.0", "prod"),
        ("v20", "twenty"),
    ]);
    let input = FormatInput {
        rows: &rows,
        installed: &[],
        current: "none",
        latest_alias: Some("node"),
        named_aliases: &aliases,
    };
    assert_eq!(
        format_remote_rows(&input),
        [
            "       v0.10.48",
            "       v0.12.18",
            "    iojs-v1.0.0",
            "    iojs-v2.5.0",
            "    iojs-v3.0.0",
            "    iojs-v3.3.1",
            "         v4.0.0",
            "         v4.9.1   (Latest LTS: Argon)",
            "       v14.21.3   (Latest LTS: Fermium)",
            "       v16.20.2   (Latest LTS: Gallium)",
            "       v18.18.0   (LTS: Hydrogen)",
            "       v18.19.0   (Latest LTS: Hydrogen)",
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0   (Latest LTS: Iron)                        (Aliases: prod, twenty)",
            "        v21.1.0",
            "        v21.2.0                            (Latest: node)   (Aliases: default, latest)",
        ]
    );
}

#[test]
fn a_pattern_listing_has_no_latest_or_alias_columns() {
    let rows = rows(Some("20"));
    let installed = versions(&["v20.10.0", "v18.19.0", "iojs-v3.3.1"]);
    let input = FormatInput {
        rows: &rows,
        installed: &installed,
        current: "system",
        latest_alias: None,
        named_aliases: &[],
    };
    assert_eq!(
        format_remote_rows(&input),
        [
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0 * (Latest LTS: Iron)",
        ]
    );
}

#[test]
fn old_unstable_releases_are_not_the_latest_stable() {
    assert!(is_old_unstable("v0.11.16"));
    assert!(is_old_unstable("v0.9.1"));
    assert!(!is_old_unstable("v0.10.48"));
    assert!(!is_old_unstable("v0.12.18"));
    assert!(!is_old_unstable("v4.0.0"));
    assert!(!is_old_unstable("v0.1"));
}

#[test]
fn no_rows_format_to_nothing() {
    let input = FormatInput {
        rows: &[],
        installed: &[],
        current: "none",
        latest_alias: Some("node"),
        named_aliases: &[],
    };
    assert!(format_remote_rows(&input).is_empty());
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function
`format_remote_rows`", "cannot find struct `FormatInput`" and "unresolved
import `crate::domain::fixtures`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/domain/remote_format/mod.rs`:

```rust
//! The rows `nvm ls-remote` prints, as the `awk` of `nvm_print_versions`
//! prints them when stdout is not a terminal (no colors): the version
//! right-aligned in 15 columns, then up to three annotation columns that each
//! start at the same place on every row.

use std::collections::HashMap;

use crate::domain::listing::{RowKind, format_row};
use crate::domain::remote::RemoteRow;
use crate::domain::version::Version;

pub struct FormatInput<'a> {
    pub rows: &'a [RemoteRow],
    /// Everything installed: those rows get a `*`.
    pub installed: &'a [Version],
    /// What `nvm current` prints: that row gets an arrow.
    pub current: &'a str,
    /// `node` for a plain listing: the newest stable release is annotated
    /// `(Latest: node)`.
    pub latest_alias: Option<&'a str>,
    /// `(target, name)` of the aliases to show: `(Aliases: name, ...)` marks
    /// the release each target resolves to.
    pub named_aliases: &'a [(String, String)],
}

#[derive(Default)]
struct Latest {
    stable: Option<String>,
    unstable: Option<String>,
    iojs: Option<String>,
}

/// `^v0\.[0-9]*[13579]\.`: an odd minor of the old 0.x scheme.
fn is_old_unstable(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("v0.") else {
        return false;
    };
    let minor: String = rest.chars().take_while(char::is_ascii_digit).collect();
    rest[minor.len()..].starts_with('.')
        && minor
            .chars()
            .last()
            .is_some_and(|digit| "13579".contains(digit))
}

fn latest_of_each_kind(rows: &[RemoteRow]) -> Latest {
    let mut latest = Latest::default();
    for row in rows {
        let text = row.version.to_string();
        if text.starts_with("iojs-") {
            latest.iojs = Some(text);
        } else if is_old_unstable(&text) {
            latest.unstable = Some(text);
        } else if text.starts_with('v') {
            latest.stable = Some(text);
        }
    }
    latest
}

/// The release each alias target stands for, and the aliases on each release.
fn aliases_by_version(input: &FormatInput<'_>, latest: &Latest) -> HashMap<String, Vec<String>> {
    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    for (target, name) in input.named_aliases {
        let version = match target.as_str() {
            "node" | "stable" => latest.stable.clone(),
            "unstable" => latest.unstable.clone(),
            "iojs" | "iojs-" => latest.iojs.clone(),
            _ => input
                .rows
                .iter()
                .map(|row| row.version.to_string())
                .rfind(|text| text == target || text.starts_with(&format!("{target}."))),
        };
        if let Some(version) = version {
            named.entry(version).or_default().push(name.clone());
        }
    }
    named
}

struct Cells {
    version: String,
    padding: &'static str,
    width: usize,
    text: [String; 3],
}

fn cells_for(
    row: &RemoteRow,
    input: &FormatInput<'_>,
    latest: &Latest,
    named: &HashMap<String, Vec<String>>,
) -> Cells {
    let text = row.version.to_string();
    let installed = input.installed.contains(&row.version);
    let kind = if text == input.current {
        RowKind::Current
    } else if installed {
        RowKind::Installed
    } else {
        RowKind::Plain
    };
    let version = format_row(&text, kind);
    let padding = if installed { "" } else { "  " };
    let lts = row.lts.as_deref().map(|name| {
        if row.latest_lts {
            format!(" (Latest LTS: {name})")
        } else {
            format!(" (LTS: {name})")
        }
    });
    let newest = input
        .latest_alias
        .filter(|_| latest.stable.as_deref() == Some(text.as_str()))
        .map(|alias| format!(" (Latest: {alias})"));
    let aliases = named
        .get(&text)
        .map(|names| format!(" (Aliases: {})", names.join(", ")));
    Cells {
        width: version.len() + padding.len(),
        version,
        padding,
        text: [
            lts.unwrap_or_default(),
            newest.unwrap_or_default(),
            aliases.unwrap_or_default(),
        ],
    }
}

fn spaces(count: usize) -> String {
    " ".repeat(count)
}

#[must_use]
pub fn format_remote_rows(input: &FormatInput<'_>) -> Vec<String> {
    let latest = latest_of_each_kind(input.rows);
    let named = aliases_by_version(input, &latest);
    let cells: Vec<Cells> = input
        .rows
        .iter()
        .map(|row| cells_for(row, input, &latest, &named))
        .collect();
    let widest_version = cells.iter().map(|cell| cell.width).max().unwrap_or(0);
    let widths: [usize; 3] = std::array::from_fn(|column| {
        cells
            .iter()
            .map(|cell| cell.text[column].len())
            .max()
            .unwrap_or(0)
    });
    cells
        .iter()
        .map(|cell| assemble(cell, widest_version, &widths))
        .collect()
}

/// One row: the version, then each annotation column that any row has, padded
/// so that the columns line up; nothing is added after the last annotation.
fn assemble(cell: &Cells, widest_version: usize, widths: &[usize; 3]) -> String {
    let Some(last) = cell.text.iter().rposition(|text| !text.is_empty()) else {
        return cell.version.clone();
    };
    let mut row = format!("{}{}", cell.version, cell.padding);
    if widths[1] > 0 || widths[2] > 0 {
        row.push_str(&spaces(widest_version - cell.width));
    }
    let mut gap = String::new();
    for (text, &width) in cell.text.iter().zip(widths).take(last + 1) {
        if width > 0 {
            row.push_str(&gap);
            row.push_str(text);
            gap = format!("{}  ", spaces(width - text.len()));
        }
    }
    row
}

```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 285 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(domain): format remote rows like nvm_print_versions"
```

---

### Task 7: `nvm ls-remote` and the shared index fetcher

**Files:**

- Create: `src/commands/remote_index/mod.rs`,
  `src/commands/remote_index/tests.rs`, `src/commands/ls_remote/mod.rs`,
  `src/commands/ls_remote/named_aliases.rs`,
  `src/commands/ls_remote/tests.rs`
- Modify: `src/commands/mod.rs`, `src/domain/remote/mod.rs`, `src/cli/mod.rs`,
  `src/cli/tests.rs`

**Interfaces:**

- Consumes: everything from Tasks 1 to 6, `Context::{alias_dir,
  installed_versions}`, `commands::current::detect`.
- Produces: `domain::remote::{scope, Scope}` (`list` now uses it: which
  indexes a query reads); `commands::remote_index::{Fetched, fetch}` (downloads
  one flavor's index and refreshes `alias/lts/*`); `commands::ls_remote::
  {Options, parse_options, run}`; the `ls-remote` (alias `list-remote`)
  subcommand, whose words are taken verbatim; the real HTTP stack in
  `run_from_env` (`UreqHttp` with `NVM_AUTH_HEADER`, wrapped in `RetryingHttp`
  with `StdSleeper`).
- Behaviour (verified against `nvm.sh` with 14 captured cases): the options are
  `--lts`, `--lts=<name>`, `--no-colors` (accepted) and `--`; the first
  non-empty word is the pattern, and `lts/*` or `lts/<name>` as the pattern
  mean the LTS filter when no `--lts` was given before it; any other `--x` is
  `Unsupported option "--x".` with exit 55. An LTS name that is uppercase or
  too far back prints its message on stderr, then `N/A`, exit 3.
  Nothing listed is `N/A`, exit 3. Rows are printed with exit 0, or
  with exit 3 when a part could not be listed (an unreachable io.js mirror
  still lists Node). The `Latest` and `Aliases` columns appear only for a
  plain listing that succeeded. Listing always creates `alias/lts` and, when
  the node index was downloaded, writes `alias/lts/*` and one file per
  codename; failures to write them are ignored, as in `nvm.sh`.
- Deviation: `nvm.sh` downloads the node index up to three times for some
  invocations; this downloads it once.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -2,6 +2,8 @@
 pub mod aliases;
 pub mod current;
 pub mod ls;
+pub mod ls_remote;
+pub mod remote_index;
 pub mod resolve;
 pub mod unalias;
 pub mod version;
```

- [ ] **Step 2: Write the failing tests**

Apply to `src/cli/tests.rs`:

```diff
--- a/src/cli/tests.rs
+++ b/src/cli/tests.rs
@@ -1,5 +1,6 @@
 use super::*;
-use crate::fakes::{FakeEnv, FakeFileSystem};
+use crate::domain::fixtures::index_text;
+use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};
 use crate::ports::FileSystem;
 
 fn run_cli(args: &[&str]) -> (u8, String, String) {
@@ -247,3 +248,21 @@
         assert!(!out.is_empty() && err.is_empty(), "{flag}");
     }
 }
+
+#[test]
+fn ls_remote_and_list_remote_print_the_releases_with_the_exit_status() {
+    let index = index_text(&[("v20.10.0", "Iron"), ("v20.9.0", "Iron")]);
+    let http = FakeHttp::default()
+        .with_body("https://nodejs.org/dist/index.tab", &index)
+        .with_status("https://iojs.org/dist/index.tab", 404);
+    let fs = FakeFileSystem::default();
+    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
+    for command in ["ls-remote", "list-remote"] {
+        let (mut out, mut err) = (Vec::new(), Vec::new());
+        let context = Context::new(&fs, &env).with_http(&http);
+        let code = run(["nvm", command, "--lts"], &context, &mut out, &mut err);
+        let expected = "        v20.9.0   (LTS: Iron)\n       v20.10.0   (Latest LTS: Iron)\n";
+        assert_eq!(String::from_utf8(out).unwrap(), expected);
+        assert_eq!(code, 0);
+    }
+}
```

Create `src/commands/ls_remote/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/ls_remote/tests.rs`:

```rust
use super::*;
use crate::domain::fixtures::index_text;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};
use crate::ports::FileSystem;

const NODE_INDEX: &str = "https://nodejs.org/dist/index.tab";
const IOJS_INDEX: &str = "https://iojs.org/dist/index.tab";

fn node_text() -> String {
    index_text(&[
        ("v21.2.0", "-"),
        ("v21.1.0", "-"),
        ("v20.10.0", "Iron"),
        ("v20.9.0", "Iron"),
        ("v18.19.0", "Hydrogen"),
        ("v18.18.0", "Hydrogen"),
        ("v16.20.2", "Gallium"),
        ("v14.21.3", "Fermium"),
        ("v4.9.1", "Argon"),
        ("v4.0.0", "-"),
        ("v0.12.18", "-"),
        ("v0.10.48", "-"),
    ])
}

fn iojs_text() -> String {
    index_text(&[
        ("v3.3.1", "-"),
        ("v3.0.0", "-"),
        ("v2.5.0", "-"),
        ("v1.0.0", "-"),
    ])
}

fn mirror() -> FakeHttp {
    FakeHttp::default()
        .with_body(NODE_INDEX, &node_text())
        .with_body(IOJS_INDEX, &iojs_text())
}

fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

fn run_with(fs: &FakeFileSystem, env: &FakeEnv, http: &FakeHttp, line: &str) -> Output {
    let context = Context::new(fs, env).with_http(http);
    run(&context, &words(line)).unwrap()
}

fn env() -> FakeEnv {
    FakeEnv::default().with_var("NVM_DIR", "/n")
}

fn lines(output: &Output) -> Vec<&str> {
    output.stdout.lines().collect()
}

/// Expected rows are what the real nvm.sh printed against the same mirrors.
#[test]
fn a_plain_listing_has_every_release_and_the_latest_marker() {
    let output = run_with(&FakeFileSystem::default(), &env(), &mirror(), "");
    assert_eq!(
        lines(&output),
        [
            "       v0.10.48",
            "       v0.12.18",
            "    iojs-v1.0.0",
            "    iojs-v2.5.0",
            "    iojs-v3.0.0",
            "    iojs-v3.3.1",
            "         v4.0.0",
            "         v4.9.1   (Latest LTS: Argon)",
            "       v14.21.3   (Latest LTS: Fermium)",
            "       v16.20.2   (Latest LTS: Gallium)",
            "       v18.18.0   (LTS: Hydrogen)",
            "       v18.19.0   (Latest LTS: Hydrogen)",
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0   (Latest LTS: Iron)",
            "        v21.1.0",
            "        v21.2.0                            (Latest: node)",
        ]
    );
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(output.stderr.is_empty());
}

#[test]
fn lts_lists_only_the_lts_releases_from_the_node_index() {
    let http = mirror();
    let output = run_with(&FakeFileSystem::default(), &env(), &http, "--lts");
    assert_eq!(
        lines(&output),
        [
            "         v4.9.1   (Latest LTS: Argon)",
            "       v14.21.3   (Latest LTS: Fermium)",
            "       v16.20.2   (Latest LTS: Gallium)",
            "       v18.18.0   (LTS: Hydrogen)",
            "       v18.19.0   (Latest LTS: Hydrogen)",
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0   (Latest LTS: Iron)",
        ]
    );
    assert_eq!(http.requests(), [NODE_INDEX]);
}

#[test]
fn lts_with_a_name_or_as_a_pattern_is_the_same_filter() {
    let fs = FakeFileSystem::default();
    let expected = [
        "       v18.18.0   (LTS: Hydrogen)",
        "       v18.19.0   (Latest LTS: Hydrogen)",
    ];
    let by_flag = run_with(&fs, &env(), &mirror(), "--lts=hydrogen");
    let by_pattern = run_with(&fs, &env(), &mirror(), "lts/hydrogen");
    assert_eq!(lines(&by_flag), expected);
    assert_eq!(lines(&by_pattern), expected);
}

#[test]
fn lts_minus_one_is_the_codename_before_the_last() {
    let output = run_with(&FakeFileSystem::default(), &env(), &mirror(), "--lts=-1");
    assert_eq!(
        lines(&output),
        [
            "       v18.18.0   (LTS: Hydrogen)",
            "       v18.19.0   (Latest LTS: Hydrogen)",
        ]
    );
}

#[test]
fn an_uppercase_lts_name_is_an_error_row_with_status_3() {
    let output = run_with(&FakeFileSystem::default(), &env(), &mirror(), "--lts=Iron");
    assert_eq!(output.stderr, "LTS names must be lowercase");
    assert_eq!(output.stdout, "            N/A");
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}

#[test]
fn going_back_too_far_is_an_error_row_with_status_3() {
    let output = run_with(&FakeFileSystem::default(), &env(), &mirror(), "--lts=-9");
    assert_eq!(output.stderr, "That many LTS releases do not exist yet.");
    assert_eq!(output.stdout, "            N/A");
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}

#[test]
fn a_pattern_that_matches_nothing_is_n_a_with_status_3() {
    let output = run_with(&FakeFileSystem::default(), &env(), &mirror(), "99");
    assert_eq!(output.stdout, "            N/A");
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}

#[test]
fn an_implicit_alias_is_an_error_without_any_download() {
    let http = mirror();
    let output = run_with(&FakeFileSystem::default(), &env(), &http, "stable");
    assert_eq!(
        output.stderr,
        "Implicit aliases are not supported in nvm_remote_versions."
    );
    assert_eq!(output.stdout, "            N/A");
    assert!(http.requests().is_empty());
}

#[test]
fn an_unsupported_option_is_status_55() {
    let error = parse_options(&words("--bogus")).unwrap_err();
    assert_eq!(error.to_string(), "Unsupported option \"--bogus\".");
    assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption);
}

#[test]
fn no_colors_and_the_separator_are_accepted() {
    let options = parse_options(&words("--no-colors -- 18")).unwrap();
    assert_eq!(options.pattern.as_deref(), Some("18"));
    assert_eq!(options.lts, None);
}

#[test]
fn only_the_first_word_is_the_pattern() {
    let options = parse_options(&words("18 20")).unwrap();
    assert_eq!(options.pattern.as_deref(), Some("18"));
}

#[test]
fn a_failed_iojs_download_still_lists_node_but_exits_3_without_markers() {
    let http = FakeHttp::default()
        .with_body(NODE_INDEX, &node_text())
        .with_status(IOJS_INDEX, 503);
    let output = run_with(&FakeFileSystem::default(), &env(), &http, "");
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert!(!output.stdout.contains("iojs-"));
    assert!(!output.stdout.contains("(Latest: node)"));
    assert!(output.stdout.contains("v21.2.0"));
}

#[test]
fn a_mirror_that_is_not_a_url_is_reported_and_leaves_that_flavor_out() {
    let env = env().with_var("NVM_NODEJS_ORG_MIRROR", "http://x y");
    let output = run_with(&FakeFileSystem::default(), &env, &mirror(), "");
    assert_eq!(
        output.stderr,
        "$NVM_NODEJS_ORG_MIRROR and $NVM_IOJS_ORG_MIRROR may only contain a URL"
    );
    assert!(output.stdout.contains("iojs-v3.3.1"));
    assert!(!output.stdout.contains("v21.2.0"));
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}

#[test]
fn nothing_downloadable_is_n_a_with_status_3() {
    let output = run_with(&FakeFileSystem::default(), &env(), &FakeHttp::default(), "");
    assert_eq!(output.stdout, "            N/A");
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}

#[test]
fn installed_current_and_aliased_releases_are_marked() {
    let fs = FakeFileSystem::default()
        .with_file("/n/versions/node/v20.10.0/bin/node", "")
        .with_file("/n/alias/prod", "v20.10.0\n")
        .with_file("/n/alias/default", "node\n");
    let env = env().with_var("PATH", "/n/versions/node/v20.10.0/bin");
    let output = run_with(&fs, &env, &mirror(), "");
    let rows = lines(&output);
    assert!(
        rows.contains(
            &"->     v20.10.0 * (Latest LTS: Iron)                        (Aliases: prod)"
        )
    );
    assert!(
        rows.iter()
            .any(|row| row.ends_with("(Latest: node)   (Aliases: default)"))
    );
}

#[test]
fn a_pattern_listing_has_no_alias_or_latest_columns() {
    let fs = FakeFileSystem::default().with_file("/n/alias/prod", "v20.10.0\n");
    let output = run_with(&fs, &env(), &mirror(), "20");
    assert_eq!(lines(&output).len(), 2);
    assert!(!output.stdout.contains("Aliases"));
}

#[test]
fn listing_refreshes_the_lts_aliases() {
    let fs = FakeFileSystem::default();
    run_with(&fs, &env(), &mirror(), "");
    let read = |name: &str| fs.read_to_string(std::path::Path::new(name)).unwrap();
    assert_eq!(read("/n/alias/lts/*"), "lts/iron\n");
}
```

Create `src/commands/remote_index/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/remote_index/tests.rs`:

```rust
use std::path::Path;

use super::*;
use crate::domain::fixtures::index_text;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};
use crate::ports::FileSystem;

const NODE_INDEX: &str = "https://nodejs.org/dist/index.tab";

fn node_text() -> String {
    index_text(&[
        ("v20.10.0", "Iron"),
        ("v20.9.0", "Iron"),
        ("v18.19.0", "Hydrogen"),
    ])
}

fn env() -> FakeEnv {
    FakeEnv::default().with_var("NVM_DIR", "/n")
}

#[test]
fn it_returns_the_releases_and_writes_the_lts_aliases() {
    let fs = FakeFileSystem::default();
    let env = env();
    let http = FakeHttp::default().with_body(NODE_INDEX, &node_text());
    let context = Context::new(&fs, &env).with_http(&http);
    let fetched = fetch(&context, Flavor::Node).unwrap();
    assert_eq!(fetched.releases.unwrap().len(), 3);
    assert_eq!(fetched.warning, None);
    let read = |name: &str| fs.read_to_string(Path::new(&format!("/n/alias/lts/{name}")));
    assert_eq!(read("*").unwrap(), "lts/iron\n");
    assert_eq!(read("iron").unwrap(), "v20.10.0\n");
    assert_eq!(read("hydrogen").unwrap(), "v18.19.0\n");
}

#[test]
fn a_failed_download_gives_no_releases_and_no_aliases() {
    let fs = FakeFileSystem::default();
    let env = env();
    let http = FakeHttp::default().with_status(NODE_INDEX, 404);
    let context = Context::new(&fs, &env).with_http(&http);
    let fetched = fetch(&context, Flavor::Node).unwrap();
    assert!(fetched.releases.is_none());
    assert_eq!(fetched.warning, None);
    assert!(fs.read_dir(Path::new("/n/alias/lts")).unwrap().is_empty());
}

#[test]
fn a_mirror_that_is_not_a_url_is_a_warning_and_no_request() {
    let fs = FakeFileSystem::default();
    let env = env().with_var("NVM_NODEJS_ORG_MIRROR", "not a url");
    let http = FakeHttp::default();
    let context = Context::new(&fs, &env).with_http(&http);
    let fetched = fetch(&context, Flavor::Node).unwrap();
    assert!(fetched.releases.is_none());
    assert!(fetched.warning.unwrap().contains("may only contain a URL"));
    assert!(http.requests().is_empty());
}

#[test]
fn the_iojs_index_is_read_from_its_own_mirror() {
    let fs = FakeFileSystem::default();
    let env = env().with_var("NVM_IOJS_ORG_MIRROR", "https://m.example/iojs");
    let body = index_text(&[("v3.3.1", "-")]);
    let http = FakeHttp::default().with_body("https://m.example/iojs/index.tab", &body);
    let context = Context::new(&fs, &env).with_http(&http);
    let releases = fetch(&context, Flavor::IoJs).unwrap().releases.unwrap();
    assert_eq!(releases[0].version.to_string(), "iojs-v3.3.1");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function `fetch`",
"cannot find module `ls_remote`" and "cannot find function `scope`".

- [ ] **Step 4: Write the implementation**

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -5,12 +5,17 @@
 
 use clap::{Parser, Subcommand};
 
+use crate::adapters::retrying_http::RetryingHttp;
 use crate::adapters::std_env::StdEnv;
 use crate::adapters::std_fs::StdFileSystem;
 use crate::adapters::std_process::StdProcess;
+use crate::adapters::std_sleeper::StdSleeper;
+use crate::adapters::ureq_http::UreqHttp;
 use crate::commands::{self, Output};
 use crate::context::Context;
+use crate::domain::http_header::sanitize_auth_header;
 use crate::error::{CliError, NvmExitCode};
+use crate::ports::Env;
 
 #[derive(Parser)]
 #[command(name = "nvm", version, about = "Node Version Manager, in Rust")]
@@ -33,6 +38,13 @@
         #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
         args: Vec<String>,
     },
+    /// List the versions a mirror offers (`--lts[=name]` keeps the LTS
+    /// releases; a pattern keeps the versions matching it).
+    #[command(name = "ls-remote", visible_alias = "list-remote")]
+    LsRemote {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
     /// Print the path to the node binary of a version or alias.
     Which { version: Option<String> },
     /// List aliases, show those starting with a name, or create an alias for
@@ -52,6 +64,7 @@
         }
         Command::Current => commands::current::run(context),
         Command::Ls { args } => commands::ls::run_command(context, args),
+        Command::LsRemote { args } => commands::ls_remote::run(context, args),
         Command::Which { version } => commands::which::run(context, version.as_deref()),
         Command::Alias { args } => commands::aliases::run(context, args),
         Command::Unalias { names } => commands::unalias::run(context, names),
@@ -123,7 +136,15 @@
 #[must_use]
 pub fn run_from_env() -> u8 {
     let process = StdProcess::default();
-    let context = Context::new(&StdFileSystem, &StdEnv).with_process(&process);
+    let auth_header = StdEnv
+        .var("NVM_AUTH_HEADER")
+        .filter(|value| !value.is_empty())
+        .map(|value| sanitize_auth_header(&value));
+    let network = UreqHttp::new(auth_header);
+    let http = RetryingHttp::new(&network, &StdSleeper);
+    let context = Context::new(&StdFileSystem, &StdEnv)
+        .with_process(&process)
+        .with_http(&http);
     run(
         std::env::args_os(),
         &context,
```

Insert above the `#[cfg(test)]` line of `src/commands/ls_remote/mod.rs`:

```rust
//! `nvm ls-remote [pattern]`: the releases a mirror offers, one row each.
//!
//! Output is always plain, as `nvm.sh` prints when stdout is not a terminal.
//! Every flavor that is listed is downloaded again each time, and the `lts/*`
//! aliases are refreshed from the node index, as in `nvm.sh`.

mod named_aliases;

use crate::commands::Output;
use crate::commands::current;
use crate::commands::remote_index::{Fetched, fetch};
use crate::context::Context;
use crate::domain::listing::{RowKind, format_row};
use crate::domain::remote::{Query, list, normalize_lts, scope};
use crate::domain::remote_format::{FormatInput, format_remote_rows};
use crate::domain::version::Flavor;
use crate::error::{CliError, NvmExitCode};

/// The command line of `nvm ls-remote`, as `nvm.sh` reads it: the first
/// non-empty word is the pattern, `lts/*` and `lts/<name>` mean the LTS
/// filter, and `--no-colors` is accepted (output is always plain).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub pattern: Option<String>,
    pub lts: Option<String>,
}

/// # Errors
/// [`CliError::Unsupported`] for an unknown `--option`.
pub fn parse_options(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    for arg in args {
        match arg.as_str() {
            "--" | "--no-colors" => {}
            "--lts" => options.lts = Some("*".to_owned()),
            option if option.starts_with("--lts=") => {
                options.lts = Some(option["--lts=".len()..].to_owned());
            }
            option if option.starts_with("--") => {
                let message = format!("Unsupported option \"{option}\".");
                return Err(CliError::Unsupported(message));
            }
            word if options.pattern.is_none() && !word.is_empty() => {
                options.pattern = Some(word.to_owned());
                options.take_lts_pattern();
            }
            _ => {}
        }
    }
    Ok(options)
}

impl Options {
    /// `lts/*` and `lts/<name>` given as the pattern are the LTS filter,
    /// unless `--lts` was already given.
    fn take_lts_pattern(&mut self) {
        if self.lts.as_deref().is_some_and(|lts| !lts.is_empty()) {
            return;
        }
        let Some(name) = self.pattern.as_deref().and_then(|p| p.strip_prefix("lts/")) else {
            return;
        };
        self.lts = Some(name.to_owned());
        self.pattern = Some(String::new());
    }
}

/// # Errors
/// As [`parse_options`], and [`CliError::NvmDirUnresolved`] when `$NVM_DIR`
/// cannot be found.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let options = parse_options(args)?;
    let mut query = Query {
        pattern: options.pattern.clone().filter(|text| !text.is_empty()),
        lts: options.lts.clone().filter(|text| !text.is_empty()),
    };
    let mut warnings = Vec::new();
    let (node_runs, iojs_runs) = match scope(&query) {
        Ok(scope) => (scope.node_runs, scope.iojs_runs),
        Err(error) => return Ok(not_available(vec![error.to_string()])),
    };
    let node = fetch_if(context, node_runs, Flavor::Node, &mut warnings)?;
    if let Some(wanted) = query.lts.as_deref() {
        match lts_filter(context, wanted) {
            Ok(name) => query.lts = Some(name),
            Err(message) => {
                warnings.push(message);
                return Ok(not_available(warnings));
            }
        }
    }
    let iojs = fetch_if(context, iojs_runs, Flavor::IoJs, &mut warnings)?;
    let listing = list(node.as_deref(), iojs.as_deref(), &query)
        .map_err(|error| CliError::InvalidArgument(error.to_string()))?;
    if listing.rows.is_empty() {
        return Ok(not_available(warnings));
    }
    let plain = listing.missing || query.lts.is_some() || options.pattern.is_some();
    let lines = format_rows(context, &listing.rows, plain)?;
    let status = if listing.missing {
        NvmExitCode::InvalidVersion
    } else {
        NvmExitCode::Success
    };
    Ok(Output::stdout(lines.join("\n"))
        .with_stderr(warnings.join("\n"))
        .with_status(status))
}

/// The row `nvm.sh` prints when nothing is listed, with exit status 3.
fn not_available(warnings: Vec<String>) -> Output {
    Output::stdout(format_row("N/A", RowKind::Plain))
        .with_stderr(warnings.join("\n"))
        .with_status(NvmExitCode::InvalidVersion)
}

fn fetch_if(
    context: &Context<'_>,
    wanted: bool,
    flavor: Flavor,
    warnings: &mut Vec<String>,
) -> Result<Option<Vec<crate::domain::index::Release>>, CliError> {
    if !wanted {
        return Ok(None);
    }
    let Fetched { releases, warning } = fetch(context, flavor)?;
    warnings.extend(warning);
    Ok(releases)
}

/// The codename a `--lts` argument stands for, once the aliases are fresh.
fn lts_filter(context: &Context<'_>, wanted: &str) -> Result<String, String> {
    let directory = context
        .alias_dir()
        .map_err(|error| error.to_string())?
        .join("lts");
    let mut names: Vec<String> = context
        .fs
        .read_dir(&directory)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    names.sort();
    normalize_lts(wanted, &names).map_err(|error| error.to_string())
}

fn format_rows(
    context: &Context<'_>,
    rows: &[crate::domain::remote::RemoteRow],
    plain: bool,
) -> Result<Vec<String>, CliError> {
    let installed = context.installed_versions()?;
    let current = current::detect(context)?.to_string();
    let named = if plain {
        Vec::new()
    } else {
        named_aliases::collect(context)?
    };
    Ok(format_remote_rows(&FormatInput {
        rows,
        installed: &installed,
        current: &current,
        latest_alias: (!plain).then_some("node"),
        named_aliases: &named,
    }))
}

```

Create `src/commands/ls_remote/named_aliases.rs`:

```rust
//! The aliases `ls-remote` shows next to a release, as `nvm_get_remote_aliases`
//! collects them.

use crate::context::Context;
use crate::domain::alias::resolve;
use crate::error::CliError;

/// `(target, name)` for every alias file directly in `$NVM_DIR/alias`, by name.
/// The built-in names, aliases that loop and values with control characters
/// are left out.
///
/// # Errors
/// [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn collect(context: &Context<'_>) -> Result<Vec<(String, String)>, CliError> {
    let directory = context.alias_dir()?;
    let store = context.alias_store()?;
    let mut names: Vec<String> = context
        .fs
        .read_dir(&directory)
        .unwrap_or_default()
        .into_iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.name)
        .filter(|name| !matches!(name.as_str(), "node" | "iojs" | "stable" | "unstable"))
        .filter(|name| !name.chars().any(char::is_control))
        .collect();
    names.sort();
    Ok(names
        .into_iter()
        .filter_map(|name| {
            let target = resolve(&store, &name).ok()?;
            let usable = !target.is_empty() && !target.chars().any(char::is_control);
            usable.then_some((target, name))
        })
        .collect())
}
```

Insert above the `#[cfg(test)]` line of `src/commands/remote_index/mod.rs`:

```rust
//! Downloading a mirror's release index, and refreshing the `lts/*` aliases
//! from it, as `nvm_ls_remote_index_tab` does.

use std::path::Path;

use crate::context::Context;
use crate::domain::index::{Release, lts_aliases, parse_index};
use crate::domain::mirror;
use crate::domain::version::Flavor;
use crate::error::CliError;

/// The releases of one mirror, or `None` when they could not be had (a
/// mirror that is not a plain URL, or a download that failed): `nvm.sh` then
/// lists nothing for that flavor and exits 3.
pub struct Fetched {
    pub releases: Option<Vec<Release>>,
    /// What `nvm.sh` prints on stderr about it.
    pub warning: Option<String>,
}

/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn fetch(context: &Context<'_>, flavor: Flavor) -> Result<Fetched, CliError> {
    let alias_lts_dir = context.alias_dir()?.join("lts");
    // nvm.sh creates the directory whatever the download does.
    let _ = context.fs.create_dir_all(&alias_lts_dir);
    let mirror = match mirror::from_env(context.env, flavor) {
        Ok(mirror) => mirror,
        Err(error) => {
            return Ok(Fetched {
                releases: None,
                warning: Some(error.to_string()),
            });
        }
    };
    let Ok(text) = context.http().get_text(&mirror.index_url()) else {
        return Ok(Fetched {
            releases: None,
            warning: None,
        });
    };
    let releases = parse_index(&text, flavor);
    refresh_lts_aliases(context, &alias_lts_dir, &releases);
    Ok(Fetched {
        releases: Some(releases),
        warning: None,
    })
}

/// Writes `alias/lts/*` and one alias per codename. `nvm.sh` hides every
/// failure of this, and so does it.
fn refresh_lts_aliases(context: &Context<'_>, directory: &Path, releases: &[Release]) {
    for (name, target) in lts_aliases(releases) {
        let file = directory.join(name.trim_start_matches("lts/"));
        let _ = context.fs.write_file(&file, &format!("{target}\n"));
    }
}

```

Apply to `src/domain/remote/mod.rs` (above the test module):

```diff
--- a/src/domain/remote/mod.rs
+++ b/src/domain/remote/mod.rs
@@ -56,16 +56,11 @@
     iojs: Option<&[Release]>,
     query: &Query,
 ) -> Result<Listing, RemoteError> {
-    let mut flavor = query.lts.as_ref().map(|_| Flavor::Node);
-    let mut pattern = query.pattern.as_deref().filter(|text| !text.is_empty());
-    match pattern {
-        Some("iojs" | "io.js") => (flavor, pattern) = (Some(Flavor::IoJs), None),
-        Some("node") => (flavor, pattern) = (Some(Flavor::Node), None),
-        Some("stable" | "unstable") => return Err(RemoteError::ImplicitAlias),
-        _ => {}
-    }
-    let node_runs = flavor != Some(Flavor::IoJs);
-    let iojs_runs = query.lts.is_none() && flavor != Some(Flavor::Node);
+    let Scope {
+        pattern,
+        node_runs,
+        iojs_runs,
+    } = scope(query)?;
 
     let mut missing = false;
     let mut node_rows = Vec::new();
@@ -86,6 +81,33 @@
     Ok(Listing {
         missing: missing || rows.is_empty(),
         rows,
+    })
+}
+
+/// Which indexes a query reads, and the pattern left once a flavor word
+/// (`node`, `iojs`) has been taken out of it.
+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+pub struct Scope<'a> {
+    pub pattern: Option<&'a str>,
+    pub node_runs: bool,
+    pub iojs_runs: bool,
+}
+
+/// # Errors
+/// Returns [`RemoteError::ImplicitAlias`] for `stable` and `unstable`.
+pub fn scope(query: &Query) -> Result<Scope<'_>, RemoteError> {
+    let mut flavor = query.lts.as_ref().map(|_| Flavor::Node);
+    let mut pattern = query.pattern.as_deref().filter(|text| !text.is_empty());
+    match pattern {
+        Some("iojs" | "io.js") => (flavor, pattern) = (Some(Flavor::IoJs), None),
+        Some("node") => (flavor, pattern) = (Some(Flavor::Node), None),
+        Some("stable" | "unstable") => return Err(RemoteError::ImplicitAlias),
+        _ => {}
+    }
+    Ok(Scope {
+        pattern,
+        node_runs: flavor != Some(Flavor::IoJs),
+        iojs_runs: query.lts.is_none() && flavor != Some(Flavor::Node),
     })
 }
 
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 307 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

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
git commit -S -m "feat(commands): add ls-remote and refresh the lts aliases"
```

---

### Task 8: `nvm version-remote`

**Files:**

- Create: `src/domain/remote/resolve/mod.rs`,
  `src/domain/remote/resolve/tests.rs`, `src/commands/version_remote/mod.rs`,
  `src/commands/version_remote/tests.rs`
- Modify: `src/domain/remote/mod.rs`, `src/commands/mod.rs`,
  `src/commands/remote_index/mod.rs`, `src/commands/ls_remote/mod.rs`,
  `src/cli/mod.rs`

**Interfaces:**

- Consumes: `domain::remote::{list, select}`, `domain::implicit::derive`,
  `commands::remote_index::fetch`.
- Produces: `domain::remote::resolve::resolve(Option<&[Release]>,
  Option<&[Release]>, &Query) -> Option<Version>`;
  `commands::remote_index::lts_filter` (moved from `ls_remote` so both commands
  share it); `commands::version_remote::{Options, parse_options, run}`; the
  `version-remote` subcommand.
- Behaviour (verified against `nvm.sh` with 33 captured cases): no pattern
  means `node`. `node` and `stable` are the newest release of the highest
  stable release line, `unstable` the newest of the highest unstable line (or
  `N/A`), `iojs` the newest io.js release, and anything else the newest release
  `ls-remote` would list, kept only when its version contains the pattern
  (`io.js` is therefore `N/A`). `--lts` narrows all of these. The result is
  printed alone; nothing found is `N/A`, exit 3. Unlike `ls-remote`,
  `--no-colors` is an unsupported option here, and `lts/*` or `lts/<name>` as
  the pattern replace `--lts`.
- Deviation: for an invalid `--lts` name `nvm.sh` repeats the message, prints a
  blank line and exits 0 when the pattern is `node` or `iojs` (an upstream bug).
  This prints the message once, then `N/A`, exit 3, for every pattern.
- Deviation: `grep -q "$PATTERN"` treats the pattern as a regular expression;
  this treats it as plain text.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -7,6 +7,7 @@
 pub mod resolve;
 pub mod unalias;
 pub mod version;
+pub mod version_remote;
 pub mod which;
 
 use crate::error::NvmExitCode;
```

- [ ] **Step 2: Write the failing tests**

Create `src/commands/version_remote/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/version_remote/tests.rs`:

```rust
use super::*;
use crate::domain::fixtures::index_text;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};

const NODE_INDEX: &str = "https://nodejs.org/dist/index.tab";
const IOJS_INDEX: &str = "https://iojs.org/dist/index.tab";

fn mirror() -> FakeHttp {
    let node = index_text(&[
        ("v21.2.0", "-"),
        ("v20.10.0", "Iron"),
        ("v18.19.0", "Hydrogen"),
        ("v4.0.0", "-"),
    ]);
    let iojs = index_text(&[("v3.3.1", "-"), ("v2.5.0", "-")]);
    FakeHttp::default()
        .with_body(NODE_INDEX, &node)
        .with_body(IOJS_INDEX, &iojs)
}

fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

fn run_with(http: &FakeHttp, line: &str) -> Output {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    run(&Context::new(&fs, &env).with_http(http), &words(line)).unwrap()
}

fn printed(line: &str) -> (String, NvmExitCode) {
    let output = run_with(&mirror(), line);
    (output.stdout, output.status)
}

fn found(version: &str) -> (String, NvmExitCode) {
    (version.to_owned(), NvmExitCode::Success)
}

fn missing() -> (String, NvmExitCode) {
    ("N/A".to_owned(), NvmExitCode::InvalidVersion)
}

#[test]
fn no_pattern_is_the_newest_node_release_from_the_node_index_only() {
    let http = mirror();
    let output = run_with(&http, "");
    assert_eq!(output.stdout, "v21.2.0");
    assert_eq!(http.requests(), [NODE_INDEX]);
}

#[test]
fn patterns_resolve_as_the_real_nvm_does() {
    assert_eq!(printed("20"), found("v20.10.0"));
    assert_eq!(printed("--lts"), found("v20.10.0"));
    assert_eq!(printed("lts/iron"), found("v20.10.0"));
    assert_eq!(printed("lts/*"), found("v20.10.0"));
    assert_eq!(printed("--lts=-1"), found("v18.19.0"));
    assert_eq!(printed("iojs"), found("iojs-v3.3.1"));
    assert_eq!(printed("2"), found("iojs-v2.5.0"));
    assert_eq!(printed("stable"), found("v21.2.0"));
}

#[test]
fn what_matches_nothing_is_n_a_with_status_3() {
    assert_eq!(printed("99"), missing());
    assert_eq!(printed("unstable"), missing());
    assert_eq!(printed("lts/foo"), missing());
    assert_eq!(printed("--lts iojs"), missing());
}

#[test]
fn a_bad_lts_name_prints_its_message_once_and_n_a() {
    let output = run_with(&mirror(), "--lts=Iron");
    assert_eq!(output.stderr, "LTS names must be lowercase");
    assert_eq!(
        (output.stdout.as_str(), output.status),
        ("N/A", NvmExitCode::InvalidVersion)
    );
    let output = run_with(&mirror(), "--lts=-9");
    assert_eq!(output.stderr, "That many LTS releases do not exist yet.");
}

#[test]
fn the_lts_pattern_replaces_the_flag_and_the_first_word_is_the_pattern() {
    let options = parse_options(&words("--lts=gallium lts/iron 18")).unwrap();
    assert_eq!(options.pattern, None);
    assert_eq!(options.lts.as_deref(), Some("iron"));
    assert_eq!(
        parse_options(&words("18 20")).unwrap().pattern.as_deref(),
        Some("18")
    );
}

#[test]
fn options_other_than_lts_are_unsupported_even_no_colors() {
    for option in ["--bogus", "--no-colors"] {
        let error = parse_options(&words(option)).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("Unsupported option \"{option}\".")
        );
        assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption);
    }
}

#[test]
fn an_unreachable_mirror_is_n_a() {
    let output = run_with(&FakeHttp::default(), "");
    assert_eq!(output.stdout, "N/A");
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}
```

Create `src/domain/remote/resolve/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/domain/remote/resolve/tests.rs`:

```rust
use super::*;
use crate::domain::fixtures::{iojs_releases, node_releases};

fn resolve_text(pattern: Option<&str>, lts: Option<&str>) -> Option<String> {
    let (node, iojs) = (node_releases(), iojs_releases());
    let query = Query {
        pattern: pattern.map(str::to_owned),
        lts: lts.map(str::to_owned),
    };
    resolve(Some(&node), Some(&iojs), &query).map(|version| version.to_string())
}

fn pattern(text: &str) -> Option<String> {
    resolve_text(Some(text), None)
}

/// Every expectation is what the real `nvm version-remote` printed against
/// the same mirrors.
#[test]
fn no_pattern_and_node_and_stable_are_the_newest_release() {
    for text in [None, Some("node"), Some("stable")] {
        assert_eq!(resolve_text(text, None).as_deref(), Some("v21.2.0"));
    }
}

#[test]
fn unstable_is_n_a_when_no_release_line_is_unstable() {
    assert_eq!(pattern("unstable"), None);
}

#[test]
fn iojs_is_the_newest_iojs_release() {
    assert_eq!(pattern("iojs").as_deref(), Some("iojs-v3.3.1"));
}

#[test]
fn lts_narrows_the_implicit_aliases() {
    assert_eq!(resolve_text(None, Some("*")).as_deref(), Some("v20.10.0"));
    assert_eq!(
        resolve_text(Some("stable"), Some("gallium")).as_deref(),
        Some("v16.20.2")
    );
    assert_eq!(
        resolve_text(None, Some("hydrogen")).as_deref(),
        Some("v18.19.0")
    );
    assert_eq!(resolve_text(Some("unstable"), Some("iron")), None);
}

#[test]
fn lts_has_no_iojs_release() {
    assert_eq!(resolve_text(Some("iojs"), Some("*")), None);
}

#[test]
fn a_partial_version_is_its_newest_release() {
    assert_eq!(pattern("20").as_deref(), Some("v20.10.0"));
    assert_eq!(pattern("v4").as_deref(), Some("v4.9.1"));
    assert_eq!(pattern("0.10").as_deref(), Some("v0.10.48"));
    assert_eq!(pattern("20.9").as_deref(), Some("v20.9.0"));
    assert_eq!(pattern("v20.9.0").as_deref(), Some("v20.9.0"));
    assert_eq!(pattern("4.0").as_deref(), Some("v4.0.0"));
    assert_eq!(pattern("18.").as_deref(), Some("v18.19.0"));
    assert_eq!(pattern("0").as_deref(), Some("v0.12.18"));
}

#[test]
fn io_js_releases_are_found_by_their_number() {
    assert_eq!(pattern("3").as_deref(), Some("iojs-v3.3.1"));
    assert_eq!(pattern("2").as_deref(), Some("iojs-v2.5.0"));
    assert_eq!(pattern("iojs-v2").as_deref(), Some("iojs-v2.5.0"));
}

#[test]
fn a_pattern_with_lts_keeps_only_lts_releases() {
    assert_eq!(
        resolve_text(Some("18"), Some("*")).as_deref(),
        Some("v18.19.0")
    );
    assert_eq!(resolve_text(Some("21"), Some("*")), None);
}

#[test]
fn what_matches_nothing_is_n_a() {
    for text in ["99", "x", "v", "99.1", "lts/foo"] {
        assert_eq!(pattern(text), None, "{text}");
    }
}

#[test]
fn io_js_is_not_the_alias_iojs_and_is_n_a() {
    assert_eq!(pattern("io.js"), None);
}

#[test]
fn a_missing_index_resolves_to_nothing() {
    let query = Query::default();
    assert_eq!(resolve(None, None, &query), None);
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find function
`resolve`", "cannot find module `version_remote`" and "cannot find function
`lts_filter`".

- [ ] **Step 4: Write the implementation**

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -45,6 +45,13 @@
         #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
         args: Vec<String>,
     },
+    /// Print the newest release a version, alias or `--lts[=name]` stands for
+    /// on the mirror (`N/A` when there is none).
+    #[command(name = "version-remote")]
+    VersionRemote {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
     /// Print the path to the node binary of a version or alias.
     Which { version: Option<String> },
     /// List aliases, show those starting with a name, or create an alias for
@@ -65,6 +72,7 @@
         Command::Current => commands::current::run(context),
         Command::Ls { args } => commands::ls::run_command(context, args),
         Command::LsRemote { args } => commands::ls_remote::run(context, args),
+        Command::VersionRemote { args } => commands::version_remote::run(context, args),
         Command::Which { version } => commands::which::run(context, version.as_deref()),
         Command::Alias { args } => commands::aliases::run(context, args),
         Command::Unalias { names } => commands::unalias::run(context, names),
```

Apply to `src/commands/ls_remote/mod.rs` (above the test module):

```diff
--- a/src/commands/ls_remote/mod.rs
+++ b/src/commands/ls_remote/mod.rs
@@ -8,10 +8,10 @@
 
 use crate::commands::Output;
 use crate::commands::current;
-use crate::commands::remote_index::{Fetched, fetch};
+use crate::commands::remote_index::{Fetched, fetch, lts_filter};
 use crate::context::Context;
 use crate::domain::listing::{RowKind, format_row};
-use crate::domain::remote::{Query, list, normalize_lts, scope};
+use crate::domain::remote::{Query, list, scope};
 use crate::domain::remote_format::{FormatInput, format_remote_rows};
 use crate::domain::version::Flavor;
 use crate::error::{CliError, NvmExitCode};
@@ -128,23 +128,6 @@
     Ok(releases)
 }
 
-/// The codename a `--lts` argument stands for, once the aliases are fresh.
-fn lts_filter(context: &Context<'_>, wanted: &str) -> Result<String, String> {
-    let directory = context
-        .alias_dir()
-        .map_err(|error| error.to_string())?
-        .join("lts");
-    let mut names: Vec<String> = context
-        .fs
-        .read_dir(&directory)
-        .unwrap_or_default()
-        .into_iter()
-        .map(|entry| entry.name)
-        .collect();
-    names.sort();
-    normalize_lts(wanted, &names).map_err(|error| error.to_string())
-}
-
 fn format_rows(
     context: &Context<'_>,
     rows: &[crate::domain::remote::RemoteRow],
```

Apply to `src/commands/remote_index/mod.rs` (above the test module):

```diff
--- a/src/commands/remote_index/mod.rs
+++ b/src/commands/remote_index/mod.rs
@@ -6,6 +6,7 @@
 use crate::context::Context;
 use crate::domain::index::{Release, lts_aliases, parse_index};
 use crate::domain::mirror;
+use crate::domain::remote::normalize_lts;
 use crate::domain::version::Flavor;
 use crate::error::CliError;
 
@@ -56,3 +57,24 @@
     }
 }
 
+/// The codename a `--lts` argument stands for, once the aliases are fresh;
+/// the message `nvm.sh` prints when it stands for none.
+///
+/// # Errors
+/// The message to print.
+pub fn lts_filter(context: &Context<'_>, wanted: &str) -> Result<String, String> {
+    let directory = context
+        .alias_dir()
+        .map_err(|error| error.to_string())?
+        .join("lts");
+    let mut names: Vec<String> = context
+        .fs
+        .read_dir(&directory)
+        .unwrap_or_default()
+        .into_iter()
+        .map(|entry| entry.name)
+        .collect();
+    names.sort();
+    normalize_lts(wanted, &names).map_err(|error| error.to_string())
+}
+
```

Insert above the `#[cfg(test)]` line of `src/commands/version_remote/mod.rs`:

```rust
//! `nvm version-remote [pattern]`: the one release a description stands for
//! (the newest, with no pattern), downloaded from the mirror.
//!
//! Unlike `nvm.sh`, an invalid `--lts` name (`--lts=Iron`) prints its message
//! once and `N/A` with status 3, whatever the pattern; `nvm.sh` repeats the
//! message and exits 0 after a blank line for `node` and `iojs`.

use crate::commands::Output;
use crate::commands::remote_index::{Fetched, fetch, lts_filter};
use crate::context::Context;
use crate::domain::index::Release;
use crate::domain::remote::resolve::resolve;
use crate::domain::remote::{Query, scope};
use crate::domain::version::Flavor;
use crate::error::{CliError, NvmExitCode};

/// The command line of `nvm version-remote`, as `nvm.sh` reads it: the first
/// non-empty word is the pattern, and `lts/*` or `lts/<name>` as the pattern
/// replace any `--lts`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub pattern: Option<String>,
    pub lts: Option<String>,
}

/// # Errors
/// [`CliError::Unsupported`] for an unknown `--option` (`--no-colors` too).
pub fn parse_options(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    for arg in args {
        match arg.as_str() {
            "--" => {}
            "--lts" => options.lts = Some("*".to_owned()),
            option if option.starts_with("--lts=") => {
                options.lts = Some(option["--lts=".len()..].to_owned());
            }
            option if option.starts_with("--") => {
                let message = format!("Unsupported option \"{option}\".");
                return Err(CliError::Unsupported(message));
            }
            word if options.pattern.is_none() && !word.is_empty() => {
                options.pattern = Some(word.to_owned());
            }
            _ => {}
        }
    }
    match options.pattern.as_deref() {
        Some("lts/*") => (options.lts, options.pattern) = (Some("*".to_owned()), None),
        Some(text) if text.starts_with("lts/") => {
            options.lts = Some(text["lts/".len()..].to_owned());
            options.pattern = None;
        }
        _ => {}
    }
    Ok(options)
}

/// Which indexes the pattern needs: `iojs` the io.js one, the implicit node
/// aliases the node one, anything else as `ls-remote` does.
fn needs(query: &Query) -> (bool, bool) {
    match query.pattern.as_deref() {
        Some("iojs") => (false, true),
        None | Some("node" | "stable" | "unstable") => (true, false),
        Some(_) => scope(query).map_or((false, false), |scope| (scope.node_runs, scope.iojs_runs)),
    }
}

fn fetch_if(
    context: &Context<'_>,
    wanted: bool,
    flavor: Flavor,
    warnings: &mut Vec<String>,
) -> Result<Option<Vec<Release>>, CliError> {
    if !wanted {
        return Ok(None);
    }
    let Fetched { releases, warning } = fetch(context, flavor)?;
    warnings.extend(warning);
    Ok(releases)
}

fn not_available(warnings: &[String]) -> Output {
    Output::stdout("N/A")
        .with_stderr(warnings.join("\n"))
        .with_status(NvmExitCode::InvalidVersion)
}

/// # Errors
/// As [`parse_options`], and [`CliError::NvmDirUnresolved`] when `$NVM_DIR`
/// cannot be found.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let options = parse_options(args)?;
    let mut query = Query {
        pattern: options.pattern,
        lts: options.lts.filter(|text| !text.is_empty()),
    };
    let (node_runs, iojs_runs) = needs(&query);
    let mut warnings = Vec::new();
    let node = fetch_if(context, node_runs, Flavor::Node, &mut warnings)?;
    if let Some(wanted) = query.lts.as_deref() {
        match lts_filter(context, wanted) {
            Ok(name) => query.lts = Some(name),
            Err(message) => {
                warnings.push(message);
                return Ok(not_available(&warnings));
            }
        }
    }
    let iojs = fetch_if(context, iojs_runs, Flavor::IoJs, &mut warnings)?;
    match resolve(node.as_deref(), iojs.as_deref(), &query) {
        Some(version) => Ok(Output::stdout(version.to_string()).with_stderr(warnings.join("\n"))),
        None => Ok(not_available(&warnings)),
    }
}

```

Apply to `src/domain/remote/mod.rs` (the new line goes directly above the
`#[cfg(test)]` line that declares `mod tests;`, not after it):

```diff
--- a/src/domain/remote/mod.rs
+++ b/src/domain/remote/mod.rs
@@ -247,3 +247,5 @@
     valid.then(|| digits.parse().ok()).flatten()
 }
 
+pub mod resolve;
+
```

Insert above the `#[cfg(test)]` line of `src/domain/remote/resolve/mod.rs`:

```rust
//! `nvm version-remote`: the one release a description stands for.

use super::{Query, RemoteRow, list, select};
use crate::domain::implicit::derive;
use crate::domain::index::Release;
use crate::domain::version::Version;

/// The newest release `query` stands for, or `None` for `N/A`.
///
/// `node`, `stable` and `unstable` pick the newest release of the newest
/// stable (or unstable) line of the node index, `iojs` the newest io.js
/// release, and anything else the newest release `ls-remote` would list,
/// provided its version contains the pattern. No pattern means `node`.
#[must_use]
pub fn resolve(
    node: Option<&[Release]>,
    iojs: Option<&[Release]>,
    query: &Query,
) -> Option<Version> {
    let lts = query.lts.as_deref();
    match query.pattern.as_deref().filter(|text| !text.is_empty()) {
        Some("iojs") => newest(select(iojs?, None, lts, true)),
        Some("unstable") => newest_of_line(node?, lts, false),
        None | Some("node" | "stable") => newest_of_line(node?, lts, true),
        Some(pattern) => by_pattern(node, iojs, query, pattern),
    }
}

fn newest(rows: Vec<RemoteRow>) -> Option<Version> {
    rows.last().map(|row| row.version)
}

/// The newest release of the highest stable or unstable release line.
fn newest_of_line(node: &[Release], lts: Option<&str>, stable: bool) -> Option<Version> {
    let versions: Vec<Version> = select(node, None, lts, false)
        .iter()
        .map(|row| row.version)
        .collect();
    let implicit = derive(&versions);
    let line = if stable {
        implicit.stable
    } else {
        implicit.unstable
    }?;
    newest(select(node, Some(&line.to_string()), lts, false))
}

fn by_pattern(
    node: Option<&[Release]>,
    iojs: Option<&[Release]>,
    query: &Query,
    pattern: &str,
) -> Option<Version> {
    let listing = list(node, iojs, query).ok()?;
    let version = newest(listing.rows)?;
    version.to_string().contains(pattern).then_some(version)
}

```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 325 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(commands): add version-remote"
```

---

### Task 9: `nvm cache` and `remove_dir_all`

**Files:**

- Create: `src/commands/cache.rs`
- Modify: `src/ports/mod.rs`, `src/adapters/std_fs.rs`,
  `src/fakes/file_system.rs`, `src/context.rs`, `src/commands/mod.rs`,
  `src/cli/mod.rs`

**Interfaces:**

- Produces: `FileSystem::remove_dir_all(&self, &Path) -> io::Result<()>` (like
  `rm -rf`: a missing path is fine, a file or a symlink is removed without
  following it); `Context::cache_dir()` (`$NVM_DIR/.cache`);
  `commands::cache::run(&Context, &[String]) -> Result<Output, CliError>`; the
  `cache` subcommand.
- Behaviour (verified against `nvm.sh`): `cache dir` prints `$NVM_DIR/.cache`;
  `cache clear` removes it, recreates it and prints `nvm cache cleared.`
  (failing with `Unable to clear nvm cache: <dir>`, exit 1); anything else
  prints the three-line usage on stderr with exit 127. Extra words after `dir`
  are ignored.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -1,5 +1,6 @@
 pub mod alias;
 pub mod aliases;
+pub mod cache;
 pub mod current;
 pub mod ls;
 pub mod ls_remote;
```

- [ ] **Step 2: Write the failing tests**

Apply to the test module of `src/adapters/std_fs.rs`:

```diff
--- a/src/adapters/std_fs.rs
+++ b/src/adapters/std_fs.rs
@@ -0,0 +1,30 @@
+#[cfg(test)]
+mod tests {
+    use super::*;
+
+    #[test]
+    fn remove_dir_all_removes_a_tree_a_file_and_tolerates_a_missing_path() {
+        let root = tempfile::tempdir().unwrap();
+        let tree = root.path().join("tree");
+        fs::create_dir_all(tree.join("a/b")).unwrap();
+        fs::write(tree.join("a/b/file"), "x").unwrap();
+        let file = root.path().join("file");
+        fs::write(&file, "x").unwrap();
+        for path in [&tree, &file, &root.path().join("missing")] {
+            StdFileSystem.remove_dir_all(path).unwrap();
+            assert!(!path.exists());
+        }
+    }
+
+    #[cfg(unix)]
+    #[test]
+    fn remove_dir_all_removes_a_symlink_and_leaves_its_target() {
+        let root = tempfile::tempdir().unwrap();
+        let target = root.path().join("target");
+        fs::create_dir(&target).unwrap();
+        let link = root.path().join("link");
+        std::os::unix::fs::symlink(&target, &link).unwrap();
+        StdFileSystem.remove_dir_all(&link).unwrap();
+        assert!(!link.exists() && target.exists());
+    }
+}
```

Create `src/commands/cache.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::error::NvmExitCode;
    use crate::fakes::{FakeEnv, FakeFileSystem};
    use crate::ports::FileSystem;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    fn env() -> FakeEnv {
        FakeEnv::default().with_var("NVM_DIR", "/n/")
    }

    #[test]
    fn dir_prints_the_cache_directory_under_nvm_dir() {
        let (fs, env) = (FakeFileSystem::default(), env());
        let output = run(&Context::new(&fs, &env), &words("dir")).unwrap();
        assert_eq!(output.stdout, "/n/.cache");
    }

    #[test]
    fn clear_empties_the_directory_and_recreates_it() {
        let fs = FakeFileSystem::default()
            .with_file("/n/.cache/bin/node-v20.1.0/node-v20.1.0.tar.gz", "x")
            .with_file("/n/alias/default", "node");
        let env = env();
        let output = run(&Context::new(&fs, &env), &words("clear")).unwrap();
        assert_eq!(output.stdout, "nvm cache cleared.");
        assert_eq!(fs.read_dir(Path::new("/n/.cache")).unwrap(), []);
        assert!(fs.is_file(Path::new("/n/alias/default")));
    }

    #[test]
    fn anything_else_is_the_usage_with_status_127() {
        let (fs, env) = (FakeFileSystem::default(), env());
        for line in ["", "list", "--help"] {
            let error = run(&Context::new(&fs, &env), &words(line)).unwrap_err();
            assert_eq!(error.exit_code(), NvmExitCode::NotFound);
            assert_eq!(error.to_string(), USAGE);
        }
    }
}
```

Apply to the test module of `src/fakes/file_system.rs`:

```diff
--- a/src/fakes/file_system.rs
+++ b/src/fakes/file_system.rs
@@ -19,6 +19,17 @@
         assert_eq!(root, [entry("a", true), entry("b", false)]);
         let nested = fs.read_dir(Path::new("/d/a")).unwrap();
         assert_eq!(nested, [entry("x", false), entry("y", false)]);
+    }
+
+    #[test]
+    fn fake_file_system_removes_a_whole_tree_and_not_its_siblings() {
+        let fs = FakeFileSystem::default()
+            .with_file("/d/a/x", "1")
+            .with_dir("/d/a/empty")
+            .with_file("/d/ab", "2");
+        fs.remove_dir_all(Path::new("/d/a")).unwrap();
+        assert_eq!(fs.read_dir(Path::new("/d")).unwrap(), [entry("ab", false)]);
+        fs.remove_dir_all(Path::new("/d/missing")).unwrap();
     }
 
     #[test]
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "no method named
`remove_dir_all` found for struct `FakeFileSystem`" and "no method named
`cache_dir` found for struct `Context`".

- [ ] **Step 4: Write the implementation**

Apply to `src/adapters/std_fs.rs` (above the test module):

```diff
--- a/src/adapters/std_fs.rs
+++ b/src/adapters/std_fs.rs
@@ -31,6 +31,15 @@
         fs::remove_file(path)
     }
 
+    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
+        match fs::symlink_metadata(path) {
+            Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
+            Ok(_) => fs::remove_file(path),
+            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
+            Err(error) => Err(error),
+        }
+    }
+
     fn write_file(&self, path: &Path, contents: &str) -> io::Result<()> {
         fs::write(path, contents)
     }
@@ -39,3 +48,4 @@
         fs::create_dir_all(path)
     }
 }
+
```

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -45,6 +45,11 @@
         #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
         args: Vec<String>,
     },
+    /// Print the cache directory (`dir`) or empty it (`clear`).
+    Cache {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
     /// Print the newest release a version, alias or `--lts[=name]` stands for
     /// on the mirror (`N/A` when there is none).
     #[command(name = "version-remote")]
@@ -73,6 +78,7 @@
         Command::Ls { args } => commands::ls::run_command(context, args),
         Command::LsRemote { args } => commands::ls_remote::run(context, args),
         Command::VersionRemote { args } => commands::version_remote::run(context, args),
+        Command::Cache { args } => commands::cache::run(context, args),
         Command::Which { version } => commands::which::run(context, version.as_deref()),
         Command::Alias { args } => commands::aliases::run(context, args),
         Command::Unalias { names } => commands::unalias::run(context, names),
```

Insert above the `#[cfg(test)]` line of `src/commands/cache.rs`:

```rust
//! `nvm cache dir` and `nvm cache clear`: where downloads are kept, and
//! emptying that directory.

use crate::commands::Output;
use crate::context::Context;
use crate::error::CliError;

const USAGE: &str =
    "Usage: nvm cache dir\n       nvm cache clear\n  Run `nvm --help` for full help.";

/// # Errors
/// - [`CliError::Usage`] for anything but `dir` and `clear`.
/// - [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
/// - [`CliError::InvalidArgument`] when the directory cannot be cleared.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    match args.first().map(String::as_str) {
        Some("dir") => Ok(Output::stdout(context.cache_dir()?.display().to_string())),
        Some("clear") => clear(context),
        _ => Err(CliError::Usage(USAGE.to_owned())),
    }
}

fn clear(context: &Context<'_>) -> Result<Output, CliError> {
    let directory = context.cache_dir()?;
    let cleared = context
        .fs
        .remove_dir_all(&directory)
        .and_then(|()| context.fs.create_dir_all(&directory));
    if cleared.is_err() {
        let message = format!("Unable to clear nvm cache: {}", directory.display());
        return Err(CliError::InvalidArgument(message));
    }
    Ok(Output::stdout("nvm cache cleared."))
}

```

Apply to `src/context.rs` (above the test module):

```diff
--- a/src/context.rs
+++ b/src/context.rs
@@ -79,6 +79,14 @@
         Ok(self.nvm_dir()?.join("alias"))
     }
 
+    /// `$NVM_DIR/.cache`, where downloads are kept.
+    ///
+    /// # Errors
+    /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
+    pub fn cache_dir(&self) -> Result<PathBuf, CliError> {
+        Ok(self.nvm_dir()?.join(".cache"))
+    }
+
     /// The alias files under `$NVM_DIR/alias`.
     ///
     /// # Errors
```

Apply to `src/fakes/file_system.rs` (above the test module):

```diff
--- a/src/fakes/file_system.rs
+++ b/src/fakes/file_system.rs
@@ -77,6 +77,14 @@
             .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
     }
 
+    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
+        self.files
+            .borrow_mut()
+            .retain(|file, _| !file.starts_with(path));
+        self.dirs.borrow_mut().retain(|dir| !dir.starts_with(path));
+        Ok(())
+    }
+
     fn write_file(&self, path: &Path, contents: &str) -> io::Result<()> {
         self.files
             .borrow_mut()
```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -31,6 +31,13 @@
     /// # Errors
     /// Propagates the underlying I/O error (for example when `path` is missing).
     fn remove_file(&self, path: &Path) -> io::Result<()>;
+
+    /// Removes `path` and everything under it, like `rm -rf`: a path that is
+    /// missing is fine.
+    ///
+    /// # Errors
+    /// Propagates the underlying I/O error.
+    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
 
     /// Creates or replaces the file at `path`.
     ///
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 331 unit tests plus the end-to-end tests.

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
git commit -S -m "feat(commands): add cache dir and cache clear"
```

---

### Task 10: End-to-end tests and the gate

**Files:**

- Create: `tests/remote_cli.rs`

**Interfaces:**

- Consumes: the `nvm` binary (`CARGO_BIN_EXE_nvm`).
- Produces: acceptance tests that run the real binary against a mirror served
  from a `TcpListener` on `127.0.0.1` (the node mirror serves a fixed
  `index.tab`, the io.js mirror an empty one): `ls-remote --lts`, the `lts/*`
  alias files written on disk and read back by `nvm alias lts/iron`,
  `version-remote` found and `N/A`, an unreachable mirror (`N/A`, exit 3, after
  the retries), and `cache clear` / `cache dir`.
- [ ] **Step 1: Write the end-to-end tests**

Create `tests/remote_cli.rs`:

```rust
//! End-to-end: the real binary against a real temporary `$NVM_DIR` and a mirror
//! served on a local port.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Output};
use std::thread;

const NODE_INDEX: &str = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n\
v20.10.0\t2023-11-22\tlinux-x64\t10.2.3\t11.3\t1.46\t1.3\t3.0\t115\tIron\t-\n\
v20.9.0\t2023-10-24\tlinux-x64\t10.1.0\t11.3\t1.46\t1.3\t3.0\t115\tIron\t-\n\
v18.19.0\t2023-11-29\tlinux-x64\t10.2.3\t10.2\t1.44\t1.3\t3.0\t108\tHydrogen\t-\n\
v4.0.0\t2015-09-08\tlinux-x64\t2.14.2\t4.5\t1.7\t1.2\t1.0\t46\t-\t-\n";

/// Serves `index.tab` to every request, and 404 to the rest, until the test
/// process ends. Returns the mirror URL.
fn serve_mirror(index: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut request = [0_u8; 1024];
            let read = stream.read(&mut request).unwrap_or(0);
            let found = String::from_utf8_lossy(&request[..read]).contains("GET /index.tab");
            let (status, body) = if found {
                ("200 OK", index)
            } else {
                ("404 Not Found", "")
            };
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
        }
    });
    format!("http://127.0.0.1:{port}")
}

/// An io.js mirror that offers nothing, so only Node releases are listed.
const IOJS_INDEX: &str =
    "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity\n";

fn nvm(nvm_dir: &Path, mirror: &str, args: &[&str]) -> Output {
    let iojs_mirror = serve_mirror(IOJS_INDEX);
    Command::new(env!("CARGO_BIN_EXE_nvm"))
        .args(args)
        .env("NVM_DIR", nvm_dir)
        .env("PATH", "/nonexistent")
        .env("NVM_NODEJS_ORG_MIRROR", mirror)
        .env("NVM_IOJS_ORG_MIRROR", iojs_mirror)
        .output()
        .expect("run the binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn ls_remote_lts_lists_the_lts_releases_of_the_mirror() {
    let (home, mirror) = (tempfile::tempdir().unwrap(), serve_mirror(NODE_INDEX));
    let output = nvm(home.path(), &mirror, &["ls-remote", "--lts"]);
    let expected = "       v18.19.0   (Latest LTS: Hydrogen)\n        v20.9.0   (LTS: Iron)\n       v20.10.0   (Latest LTS: Iron)\n";
    assert_eq!(stdout(&output), expected);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn ls_remote_refreshes_the_lts_aliases_on_disk() {
    let (home, mirror) = (tempfile::tempdir().unwrap(), serve_mirror(NODE_INDEX));
    nvm(home.path(), &mirror, &["ls-remote"]);
    let alias = |name: &str| fs::read_to_string(home.path().join("alias/lts").join(name));
    assert_eq!(alias("*").unwrap(), "lts/iron\n");
    assert_eq!(alias("hydrogen").unwrap(), "v18.19.0\n");
    let listed = nvm(home.path(), &mirror, &["alias", "lts/iron"]);
    assert_eq!(stdout(&listed), "v20.10.0\n");
}

#[test]
fn version_remote_prints_one_release_or_n_a() {
    let (home, mirror) = (tempfile::tempdir().unwrap(), serve_mirror(NODE_INDEX));
    let found = nvm(home.path(), &mirror, &["version-remote", "18"]);
    assert_eq!(
        (stdout(&found).as_str(), found.status.code()),
        ("v18.19.0\n", Some(0))
    );
    let missing = nvm(home.path(), &mirror, &["version-remote", "99"]);
    assert_eq!(
        (stdout(&missing).as_str(), missing.status.code()),
        ("N/A\n", Some(3))
    );
}

#[test]
fn an_unreachable_mirror_is_n_a_with_status_3() {
    let home = tempfile::tempdir().unwrap();
    let output = nvm(home.path(), "http://127.0.0.1:1", &["ls-remote"]);
    assert_eq!(stdout(&output), "            N/A\n");
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn cache_clear_empties_the_cache_and_keeps_the_rest() {
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(home.path().join(".cache/bin")).unwrap();
    fs::write(home.path().join(".cache/bin/file.tar.gz"), "x").unwrap();
    fs::create_dir_all(home.path().join("alias")).unwrap();
    fs::write(home.path().join("alias/default"), "node").unwrap();
    let output = nvm(home.path(), "http://127.0.0.1:1", &["cache", "clear"]);
    assert_eq!(stdout(&output), "nvm cache cleared.\n");
    assert_eq!(fs::read_dir(home.path().join(".cache")).unwrap().count(), 0);
    assert!(home.path().join("alias/default").is_file());
    let dir = nvm(home.path(), "http://127.0.0.1:1", &["cache", "dir"]);
    assert_eq!(stdout(&dir), format!("{}/.cache\n", home.path().display()));
}
```

- [ ] **Step 2: Run them**

Run: `cargo test --test remote_cli`
Expected: PASS, 5 tests. They pass at once because the behaviour already
exists; they are the acceptance net for Tasks 1 to 9. The unreachable-mirror
test takes about a second, the time of the retries. If one fails, fix the
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

Expected: formatting clean, zero clippy warnings, 331 unit tests plus 6 + 10 +
5 + 1 end-to-end tests passing (7 + 10 + 5 + 1 on Linux), no advisories, and
the crate still builds on MSRV 1.88.

Also check that no file is over 300 lines:

```bash
find src tests -name '*.rs' | xargs wc -l | sort -n | tail -3
```

- [ ] **Step 4: Run the tests on Linux**

The repository's `Dockerfile` runs `cargo test`:

```bash
docker buildx build -f Dockerfile -t nvmrc:trixie .
docker run --rm nvmrc:trixie
```

Expected: the same test counts, all passing.

- [ ] **Step 5: Compare with the real `nvm.sh` (optional, manual)**

With the reference `nvm.sh` and two local mirrors serving the same `index.tab`
files, run the same `ls-remote` and `version-remote` commands with both and
diff the output and the exit status. The expected texts of Tasks 5 to 9 were
captured this way.

- [ ] **Step 6: Commit**

```bash
git add tests
git commit -S -m "test(cli): run ls-remote, version-remote and cache end to end"
```

---

## Self-review against the spec

- **Spec coverage:** section 3 layering is kept (`ports/` gains `Http` and
  `Sleeper`; `adapters/` holds the only network code; `domain/` has the
  mirror, index, selection and formatting rules; `commands/` assembles them);
  section 4.1 gains `alias/lts/*` and `.cache`; section 6 exit codes are used
  as before (`nvm.sh` prints status 3 with `N/A` for every listing that finds
  nothing, including a bad `--lts` name); section 10 gains `ureq`; section 11
  step 4 is complete for `ls-remote`, `version-remote`, `cache` and mirror validation.
  Checksums belong to Plan 5 with `install`.
- **Deliberate deviations from `nvm.sh`, each to be pinned by the Plan 9
  compatibility contract:**
  - `ureq` replaces `curl`/`wget`; only `NVM_AUTH_HEADER` is carried over.
  - The node index is downloaded once per command; `nvm.sh` repeats it for
    implicit aliases.
  - `version-remote --lts=<bad>` prints its message once and `N/A` with status
    3 for every pattern (see Task 8).
  - The `version-remote` pattern check is a plain-text match, not a regular
    expression, and the `ls-remote` `grep -w` is a literal word match over the
    whole row (a `.` or `*` in a pattern keeps its literal meaning).
  - Rows are sorted by version number; `nvm.sh` sorts io.js rows as text,
    which is the same for every release that exists.
  - The mirror error text goes to stderr once per index that is refused.
- **Type consistency:** names match across tasks (`Http`, `HttpError`,
  `Sleeper`, `Release`, `Query`, `Scope`, `Listing`, `RemoteRow`,
  `FormatInput`, `Fetched`, `lts_filter`, `Context::with_http`).
