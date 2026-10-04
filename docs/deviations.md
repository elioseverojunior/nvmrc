# Deviations from nvm.sh

Each item is pinned by a test; the contract scenarios that pin one name its
id.

## The contract (D1 to D17)

- D1: when stdout and stderr go to the same place (`2>&1`), nvmrc writes all
  of stderr first; nvm.sh interleaves (`Found '.nvmrc'` first). Separate
  streams are identical. (Scenarios N7, X7.)
- D2: `--help` prints clap's help, not nvm's text.
- D3: fixed in Plan 9: a bare `nvm` prints the help with status 0.
- D4: `--version` prints `nvm <nvmrc version>`, not nvm's version number.
- D5: there is no `nvm unload` (status 127); remove the init line instead.
  (S8)
- D6: an unknown command gets clap's error, not nvm's help dump; the status
  is 127 for both. (S9)
- D7: fixed in Plan 9: `nvm alias` creates `$NVM_DIR/alias/lts`; a failure to
  create it is silent (nvm.sh's `mkdir` complains and goes on).
- D8: fixed in Plan 9: `nvm ls v0.1.` lists v0.1.x.
- D9: `ls-remote` with nothing to list prints `N/A` without the star that
  nvm.sh prints after it. (I5, I11)
- D10: `install -b` of a version before v0.12 says the legacy layout is not
  supported (nvm.sh: "Binary download is not available"); status 3 for both.
- D11: `set-colors` with an invalid setting prints the message without the
  help dump. (S10)
- D12: fixed in Plan 9: the system node is the one `nvm deactivate` leaves on
  `PATH` (`$NVM_DIR/../sys` included).
- D13: a v0.x version placed under `versions/node` (nvm puts it in `$NVM_DIR`
  itself) is listed by `ls` and `alias` but not used by `use`.
- D14: there is no `nvm debug`, so exit 42 is never produced.
- D15: a command that `exec` or `run` cannot start prints
  `nvm: <cmd>: not found` (`nvm-exec: ...` for nvm-exec), not bash's `exec`
  message; status 127 for both. (X6)
- D16: fixed in Plan 9: the floor messages name the real `min-version` path.
- D17: when `$NVM_DIR` ends with `/`, nvm.sh warns on stderr that it should
  not have trailing slashes; nvmrc removes the slash and prints nothing.
  (S7)

Two more differences are visible in the scenarios without an id of their
own:

- Scenario L12: with an empty `versions/node`, `nvm ls` on nvm.sh prints
  `find: .../versions/node/*: No such file...` on stderr; nvmrc prints
  nothing. The scenario hides stderr, as nvm's own test does.
- `nvm exec` and `nvm run` wait for their command as a child process, so
  their status is passed on after it ends; only `nvm-exec` replaces itself
  with the command (on Unix).

## By area

### Listing

- `ls` shows no phantom blank row with only a system node.
- io.js and node sharing a number are both listed.
- `ls unstable` prints one `N/A` row.
- Versions are sorted by bytes.
- `ls` does not read `.nvmrc` as a pattern and does not see a system io.js.
- `unalias` does not create `alias/`.
- A bad alias name exits 1.
- Non-UTF-8 paths are shown lossy.

### Remote

- ureq replaces curl and wget; only `NVM_AUTH_HEADER` is carried.
- The index is fetched once.
- `version-remote --lts=<bad>` prints one message and `N/A`, status 3.
- Patterns are literal text, not regexes.
- Rows are sorted by version.
- A refused mirror is reported once.

### Install

- The archive type follows the version and the OS; there is no `.tar.gz`
  fallback and no external `xz`.
- There is no progress bar and no `Computing checksum` line.
- Archives are read into memory up to 1 GiB; xz uses a 256 MiB dictionary.
- A checksum mismatch fails and the archive is removed.
- There is no npm bootstrap for a node without npm.
- `--default` and `--alias` with `--lts` alias `lts/<name>`.
- `--save` writes `$PWD/.nvmrc` after a fresh install.
- No installs before v0.12, and none before v0.8.6 or without binary and
  source: status 3. A platform without source builds says so, status 1.
- `uninstall` does no permission check and removes aliases by literal match.
- Build jobs come from the CPU count; command output is shown at the end.
- Only binary installs are one atomic rename.
- Connect and header timeouts are 30 s.

### Shell

- `use` of a resolvable but missing version is status 3 (nvm.sh: 0).
- `-h` and `help` are ordinary arguments.
- `deactivate` with an unresolvable `NVM_DIR` fails with 1.
- A command that cannot be executed is 127; bash says 126.
- The function needs `nvm` on `PATH` and runs the first one there, a global
  npm package named `nvm` included.
- `install` activates only through the function.
- Only bash, zsh, sh, dash, ksh and fish are supported.
- `--delete-prefix` without npm warns.
- `hash -r` is always emitted.
- Non-UTF-8 arguments are replaced lossily.
- The function needs `/dev/fd`.

### Colors

- An invalid `NVM_COLORS` letter warns once; an unknown `TERM` is silent.
- There is no color legend in `--help`.
- `NVM_NO_COLORS` does not leak.
- Invalid `.nvmrc` lines and alias targets are printed raw, control bytes
  included.
- `NVM_COLORS` is read by character.
- The install `--alias`/`--default` row is never colored.
- `set-colors x --help` sets the colors.

### Conflicts

- There is no ripgrep and no `LineScanner`.
- The runtime check is in the init snippet, so a late `source nvm.sh` is only
  found by `doctor`.
- Lazy-loader files are never edited.
- `OmzPlugin` and `HelperCall` are hints.
- `source` must be in command position; a `zsh-defer` or `time` before it
  counts; `eval "$(cat nvm.sh)"` is not seen.
- Here-documents are not tracked.
- Relative source paths resolve from the sourcing file's directory.
- Only loader and completion lines are migrated; compound lines are manual,
  and so is a line inside a block or continued.
- An unterminated BEGIN marker is not a block.
- The load line calls `nvmrc`.
- The block's shell comes from the file name.
- The diff is part of the prompt.
- Backups keep the file's permissions but not its owner, extended attributes
  or ACLs, and a read-only profile is replaced.
- `--undo` backs up first, refuses an empty backup and prefers backups not
  dated in the future.
- An unreadable file is exit 1.
- A file the syntax checker already refuses is left alone; a missing checker
  writes with a warning.
- fish is verified in the Docker image and in CI (installed from apt).
- `doctor`, `migrate` and `__conflict` are additions.

## Platforms

v1 runs on Linux and macOS (x86_64 and arm64). Windows is not supported:
the shell channel is an inherited file descriptor, `nvm-exec` replaces
itself with `exec`, and installs set Unix permissions, so building for
Windows stops with a compile error naming this file. Not handled:

- AIX gets `.tar.xz` (nodejs.org publishes `.tar.gz` there).
- armv6 and Rosetta are not detected as nvm.sh does.
- An install lock left by Ctrl-C waits for its stale timeout.
- `.nvmrc` is searched from `$PWD` as the shell gives it.
