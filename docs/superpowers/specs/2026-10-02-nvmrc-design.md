# nvmrc — Native Rust port of nvm

Date: 2026-10-02
Status: Draft for review
Reference implementation: `elioseverojunior/nvm` (`nvm.sh`, 5,456 lines,
about 129 functions, plus `nvm-exec`, `install.sh`, `bash_completion`).

## 1. Goal and scope

Reimplement nvm natively in Rust as the `nvmrc` crate, with no shell logic
ported line by line. The result must be a drop-in replacement for day-to-day
use: same commands, same flags, same on-disk layout, same exit codes.

Delivery is phased.

- **v1:** domain model, `install` from pre-built binaries (with checksum),
  `use`, `deactivate`, `init`, `ls`, `ls-remote`, `alias`, `unalias`,
  `current`, `which`, `exec`, `run`, `uninstall`, `cache`, `version`,
  io.js support, `.nvmrc` handling, the minimum-version floor from the
  `spike` work, shell-conflict detection and migration.
- **v2 (out of scope here):** install from source (`-s`), legacy platforms
  (SmartOS, AIX, BSD, ARM variants), `self-install` and `self-update`.

Non-goals: a POSIX shell interpreter, async I/O, a plugin system, a
multi-crate workspace.

### Why no async I/O

Async improves throughput when many I/O operations wait concurrently. It does
not make a single operation faster, and nvmrc's workload is not that shape:

- Install is sequential and bound by bandwidth, disk and CPU (download,
  SHA-256, gzip/xz extraction). Extraction is CPU-bound and would need
  `spawn_blocking` inside an async runtime anyway.
- Most commands (`use`, `current`, `which`, `alias`) read a few local files
  and run on every `cd` or prompt, where startup time matters.
- Cost: a heavy dependency tree (`tokio`, usually `reqwest`/`hyper`), longer
  builds, a larger binary, more `cargo audit` and `cargo deny` surface, `async`
  leaking into every port (`Http`, `FileSystem`), harder `&dyn` fakes, and a
  lock guard whose `Drop` cannot await.

The few genuinely concurrent spots use `std::thread::scope` with blocking
`ureq`: fetching the node and io.js `index.tab` together, and fetching
`SHASUMS256.txt` alongside the tarball. `Http` is a port, so the adapter can
be replaced later without touching the domain.

Revisit this decision if a feature needs many simultaneous connections (such
as parallel multi-version installs or a daemon mode), or if profiling real
`install` and `ls-remote` runs shows the network is the bottleneck and
concurrency fixes it. Per the profile-before-optimizing rule, measure first.

## 2. Binaries and shell integration

A binary cannot change its parent shell's environment, so the design splits
the work between Rust and a tiny generated shell snippet.

- Three binaries share one library: `nvmrc`, `nvm` and `nvm-exec`. `nvm` and
  `nvmrc` accept the same subcommands; `nvm` exists for compatibility with
  scripts, CI and tools that invoke `nvm` as an executable.
- `nvmrc init <shell>` prints a snippet defining a shell function `nvm`. For
  `use`, `deactivate`, `install` and the automatic `use` at start, the
  function opens descriptor 3 on a pipe, names it in `NVMRC_SCRIPT_FD=3`, and
  `eval`s what the binary writes there. Messages stay on their normal
  streams, as in `nvm.sh`. Every other command goes through the function's
  pass-through branch, which never opens descriptor 3 and only exports
  `MANPATH`, `NODE_PATH`, `NVM_SYMLINK_CURRENT` and `PREFIX` when they are
  set.
- The binary takes descriptor 3 over at startup as a close-on-exec copy, so
  the programs it starts never inherit it or the variable. These commands
  write shell code there (`export PATH=...`, `export NVM_BIN=...`,
  `hash -r`). Rust decides the values; the snippet is generated, never
  hand-maintained.
- Run on its own, without the variable, the binary prints that code on
  stdout and moves its stdout messages to stderr, so
  `eval "$(nvm use 18)"` still works by hand.

Known, accepted limits:

- `source nvm.sh` has no equivalent, and the internal `nvm_*` helper
  functions disappear. Scripts that depend on them must use subcommands.
- If `nvm.sh` is still loaded in a shell, its function shadows the binary.
  Section 7 covers detection and migration.

## 3. Architecture

One crate, thin `main.rs`, logic in a library. Dependencies point downward.

```text
cli        clap parsing, exit-code translation
commands/  one file per subcommand
domain/    pure rules, no I/O
  version.rs    parse, ordering, lts/*, v prefix, io.js
  floor.rs      VersionFloor (NVM_MIN_VERSION, $NVM_DIR/min-version)
  alias.rs      resolution with cycle detection
  nvmrc.rs      upward .nvmrc search, CR stripping
  conflict.rs   rules and findings for shell conflicts
  migration.rs  plan and edit model
ports/     traits: FileSystem, Http, Env, Clock, Process, LineScanner
adapters/  real implementations (std::fs, ureq, std::env, ripgrep scanner)
shell/     eval script generation and `init <shell>`
```

Principles applied: SRP (one command per file, under 300 lines, functions
under 30 lines), DIP and dependency injection (commands receive `&dyn` ports),
DRY (one exit-code enum, one rule table), KISS and YAGNI (no workspace, no
async, blocking HTTP).

Errors use `thiserror` per module and `anyhow` only in the binaries. One
`From<CliError> for ExitCode` conversion is the only place that maps errors to
exit codes; only `main` calls `std::process::exit`.

## 4. Data and flow

### 4.1 On-disk layout (compatible with nvm)

```text
$NVM_DIR/
  versions/node/vX.Y.Z/          Node binaries
  versions/io.js/vX.Y.Z/         io.js binaries (the iojs- prefix is stripped)
  alias/<name>                   text file, first line is the target
  alias/lts/<codename>           generated from index.tab, plus alias/lts/*
  alias/default
  .cache/bin/  .cache/src/       downloaded tarballs
  .cache/locks/<version>         per-version install lock
  min-version                    version floor
  current -> versions/...        when NVM_SYMLINK_CURRENT is set
```

The legacy layout (`$NVM_DIR/vX.Y.Z`) is read, never created.

### 4.2 Domain types

- `Version { major, minor, patch, flavor }` with `FromStr` accepting `20`,
  `v20.1` and `iojs-v3.0.0`. Ordering is numeric.
- `VersionFloor(Version)` exists only if valid; an invalid floor is a typed
  error that maps to exit code 7.
- `AliasTarget` is `Version`, `Alias(AliasName)`, `System` or `Lts(codename)`.
- `MirrorUrl` validates `NVM_NODEJS_ORG_MIRROR` and `NVM_IOJS_ORG_MIRROR`
  with the same character rules as `nvm_get_mirror` (injection characters
  are rejected).

### 4.3 Version resolution

Input (argument, `.nvmrc` or `default`) is normalised, then the alias chain is
followed while tracking seen names. A cycle resolves to the infinity marker and
exit code 8, matching `nvm_resolve_alias`. The result is matched against
installed or remote versions. The function is pure and takes an `AliasStore`
trait.

### 4.4 Install flow

1. Parse and resolve (alias, `.nvmrc`, LTS).
2. `VersionFloor::check` fails with exit 7 before any network access.
3. Already installed: use it, unless `--reinstall`.
4. Acquire the per-version lock (timeout and stale handling from
   `NVM_INSTALL_LOCK_TIMEOUT` and `NVM_INSTALL_LOCK_STALE`), released by a
   `Drop` guard.
5. Download `SHASUMS256.txt` and the tarball from the mirror.
6. Verify SHA-256.
7. Extract into a temporary directory, then move atomically.
8. Apply `--alias`, `--default`, npm upgrade, default packages.
9. Release the lock.

Without a binary build the install fails with a clear error (equivalent to
`NVM_NO_SOURCE_FALLBACK=1`) until v2 adds source builds. The remote index is
`index.tab` from the mirror, cached under `.cache/`, with an offline mode.

## 5. Version floor (from the `spike` work)

Behaviour taken from the uncommitted changes on the nvm `spike` branch:

- `NVM_MIN_VERSION` accepts `24`, `v24` or `v24.1.0`. When unset, the first
  line of `$NVM_DIR/min-version` is used. The variable wins over the file.
- `install` of a lower version exits 7 before any download.
- An invalid floor also exits 7; it is never silently ignored.
- The spike's test file becomes an acceptance test.

## 6. Errors and exit codes

Exit codes in `nvm.sh` are not a single enum; helper functions reuse numbers.
Only the public contract of each subcommand is fixed. Confirmed so far:

- 0 success; 1 generic failure.
- 3 invalid or unknown version.
- 6 conflicting options (`-s` with `-b`, `--default` with `--alias`).
- 7 below the version floor, or invalid floor.
- 8 alias loop.
- 11 incompatible prefix settings (`nvm use` refusing an npm `prefix`).
- 33 third-party install hook claimed success but failed.
- 55 unsupported option.
- 127 command not found.

Codes 2, 4, 5, 10 and 42 are pending: each is pinned by the acceptance test
of the scenario that produces it, not guessed. Messages go to stderr with the
same text as `nvm.sh` wherever a test compares it. Structured logs use
`tracing`, enabled by `NVMRC_LOG`.

## 7. Shell conflict detection and migration

### 7.1 Conflict sources

1. `install.sh` profile lines: `[ -s "$NVM_DIR/nvm.sh" ] && \. ...` and the
   `bash_completion` line, in `.bashrc`, `.bash_profile`, `.zshrc`,
   `.zprofile` and `.profile`, in the order of `nvm_detect_profile`.
2. Homebrew nvm: `\. "$HOMEBREW_PREFIX/opt/nvm/nvm.sh"` and its completion.
3. Lazy loaders: shell functions `nvm`, `node`, `npm`, `npx` (and similar)
   that source `nvm.sh` on first call. These re-define `nvm` after `init`.

### 7.2 Two levels of checking

- **Runtime (cheap, always on):** the `init` snippet exports `NVMRC_SHELL=1`
  and the binary checks whether `nvm.sh` looks active (`NVM_BIN`, the type of
  `nvm`). On conflict it warns once per session on stderr with the cause and
  the fix command. It never fails a command by itself.
- **On disk (`nvm doctor`, read-only):** scans the profile files and files
  they source (depth 2) and lists each hit with file and line.

### 7.3 Migration (`nvm migrate`, `--dry-run`, `--undo`)

1. `detect()` returns findings (file, line, kind).
2. `plan()` builds edits: comment the old line and add the `init` line.
3. Show the diff; apply only with `--yes` or interactive confirmation.
4. Back up each file as `<file>.nvmrc-backup-<timestamp>`.
5. Apply with an atomic write (temp file, then rename).
6. `verify()` re-runs `detect()`, expecting zero conflicts.

Rules: idempotent via `# >>> nvmrc init >>>` / `# <<< nvmrc init <<<`
markers; reversible (`--undo` restores the latest backup); old lines are
commented as `# [nvmrc-migrated] ...`, never deleted; `$NVM_DIR` data is never
touched. Lazy-loader files (source 3) are never edited automatically: the
exact snippet and a suggested patch are shown, and the file is changed only
with an explicit `--yes` for that file.

### 7.4 Scanning engine

Detection uses the ripgrep library crates (`grep-regex`, `grep-searcher`)
behind a `LineScanner` port. The domain owns a single `RuleSet` table; the
adapter supplies line numbers and binary/encoding handling. Tests feed bytes
through the in-memory `FileSystem` using `search_slice`. The `ignore` walker
is excluded from v1 (YAGNI): only a closed list of files is scanned.

## 8. Testing

TDD throughout: failing test, minimal code, refactor.

1. Unit tests of `domain/` by table: version, floor (spike cases), alias
   cycles, `.nvmrc` CR stripping, conflict rules, migration idempotence.
2. Command tests with `FakeFileSystem`, `FakeHttp` and `FakeEnv`; scenarios
   from `nvm/test/fast` ported to Rust.
3. Compatibility contract: the `nvm/test` scenarios (fast, installation,
   sourcing) run against the Rust `nvm` binary in a temporary `$NVM_DIR`.
4. A local HTTP server imitating the mirror (`index.tab`, `SHASUMS256.txt`, a
   fixture tarball); no dependency on nodejs.org.
5. Security tests: mirror injection characters (exit 2) and `..` path
   components in aliases and versions.

Test scripts and output directories are generated, used and removed; they are
not kept in the repository.

## 9. Quality gates and CI

Before every commit: `cargo fmt --check`,
`cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`,
`cargo deny check`, `cargo audit`. Markdown is checked with `rumdl`. CI uses
GitHub Actions with the loosest published tag (`@v{major}`) on a macOS and
Linux matrix. Commits follow Conventional Commits and are GPG-signed.

## 10. Dependencies (v1)

`clap`, `thiserror`, `anyhow`, `ureq` (blocking HTTP), `tracing`, `grep-regex`
and `grep-searcher`. Checksums and archive extraction crates are chosen in the
implementation plan. MSRV is 1.85 as declared in `Cargo.toml`.

## 11. Implementation order (v1)

1. Foundation: `Version`, exit codes and errors, ports and fakes, real
   `FileSystem` and `Env`.
2. Resolution: aliases with cycles, `.nvmrc`, `VersionFloor`.
3. Read-only commands: `ls`, `current`, `which`, `alias`, `unalias`, `version`.
4. Network: `ls-remote`, `cache`, mirror validation, checksum.
5. Writes: `install` (lock, atomic extraction, `--alias`, `--default`),
   `uninstall`.
6. Shell: `use`, `deactivate`, `exec`, `run`, `init`, the `nvm` and
   `nvm-exec` binaries.
7. Conflicts: `doctor`, `migrate`, `--undo`.
8. Closing: full compatibility contract, CI, README.

## 12. Open items

- Pin the pending exit codes (2, 4, 5, 10, 42) through acceptance tests.
- Choose the checksum and archive crates during planning.
- Decide whether `nvm-exec` stays a separate binary or becomes a subcommand
  plus a symlink.
