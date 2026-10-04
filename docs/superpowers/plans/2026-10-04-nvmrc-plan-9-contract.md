# nvmrc Plan 9: Compatibility Contract, CI and README Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close v1 of nvmrc: package metadata and supply-chain checks, a
GitHub Actions CI, the last behaviour gaps the nvm.sh oracle found, the
deferred review minors, a table-driven compatibility contract against
nvm.sh's own test scenarios, and the README.

**Architecture:** Same layering as Plans 1 to 8. Rust changes are small and
local (a CLI argument rule, the alias command, the `ls` pattern, the
system-node lookup, the floor message, the install cache, the process port,
`nvm-exec`, `migrate`/`doctor`). The contract is one new integration-test
crate, `tests/compat_cli.rs`, whose modules under `tests/compat/` hold the
harness (a temporary tree, a real shell, `eval "$(nvm init <shell>)"`) and
one scenario table per topic, with inline expectations captured from the real
`nvm.sh`. An ignored test re-runs the tables through `nvm.sh` when
`NVMRC_ORACLE_NVM_SH` names one, and prints what differs; nothing is written.

**Tech Stack:** Rust 2024 edition (MSRV 1.85, development toolchain 1.99 from
`rust-toolchain.toml`), the dependencies of Plan 8 (no new crate), mise,
cargo-deny 0.20, cargo-audit 0.22, rumdl, actionlint, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-10-02-nvmrc-design.md` (sections 8 and
9, and the open items of section 12, which this plan closes). This plan starts
from `main` at `743b94c` (the end of
`2026-10-04-nvmrc-plan-8-conflicts.md`), on the branch
`feat/plan-9-contract`.

**Reference.** The oracle is the Plan 9 research digest: about 140 scenario
invocations run both through `nvm.sh` (`. nvm.sh --no-use` in `bash`, with an
empty environment, `TERM=dumb`, a temporary `$NVM_DIR` and a refused or local
mirror) and through the Rust `nvm` function, outputs compared byte for byte
after replacing the temporary root by `<W>`. Every expected string in the
tables below comes from those runs; the few marked "capture" were only run
through nvmrc and are confirmed by the capture step of Task 5.

**Revised roadmap:** this is the last v1 plan. After it: v2 (legacy
platforms, `self-install`/`self-update`), decided separately.

**Not in this plan, on purpose:** a release workflow (binaries are built with
`cargo install`; no tag or crates.io publication is decided), a Homebrew
formula (README describes one, nothing is published), an nvm-style `--help`
text, `nvm unload`, `nvm debug`, an ordered stdout/stderr transcript (D1),
Plan 5 M7/M8/M9/M11, Plan 6 Minors 2/4/5/6/8, Plan 7 Minors 1/5/7, Plan 8
M3/M5/M11 (each is recorded in `docs/deviations.md` instead).

## Global Constraints

- Rust edition 2024, `rust-version = "1.85.0"`; no API newer than 1.85. The
  development toolchain is the one of `rust-toolchain.toml` (1.99); MSRV is
  checked with `rustup run 1.85 cargo check --all-targets --locked`. Plain
  `cargo +1.85` does not work on this machine (Homebrew's `cargo` is first on
  `PATH` and is not a rustup proxy).
- `.cargo/config.toml` sets `-D warnings` for rustc and rustdoc. Every commit
  passes `cargo fmt --all --check`,
  `cargo clippy --all-targets --all-features --locked -- -D warnings` and
  `cargo test --locked`.
- No new dependency. `Cargo.toml` changes only in Task 1 (metadata) and Task
  6 (`readme`).
- Files under 300 lines (tests included), functions under 30 lines,
  cyclomatic complexity under 10, meaningful names without abbreviations. A
  module with tests is `foo/mod.rs` plus `foo/tests.rs` (or `*_tests.rs`
  beside it); a file that would pass 300 lines is split first.
- Markdown is formatted and linted with `rumdl` (`rumdl fmt <file>`,
  `rumdl check <file>`; config `rumdl.toml`: MD013 at 80 columns, tables
  included, code blocks exempt). No wide tables: lists instead. Headings and
  lists are surrounded by blank lines.
- GitHub Actions: the loosest published tag (`actions/checkout@v7`,
  `Swatinem/rust-cache@v2`, `taiki-e/install-action@v2`), never a SHA;
  kebab-case job ids, step ids and inputs; `SCREAMING_SNAKE_CASE` only for
  environment variables.
- mise: `mise.toml` follows the global template exactly (settings verbatim
  except the idiomatic tool list, the `hk` platform gate verbatim, the
  `setup` task verbatim); every other task has an alias, a description,
  `shell = "bash -c"`, a `'''` body starting with
  `#!/usr/bin/env bash` + `set -Eeuo pipefail`, and a `usage` with
  `flag "-v --verbose"`. `mise.local.toml` is ignored, `mise.lock` committed.
- Commits: Conventional Commits, GPG-signed with `git commit -S`, files added
  by explicit pathspec (`git add <paths>`, never `-A`), and no AI attribution
  or `Co-Authored-By` trailer in the message.
- Tests never touch the real home, `~/.nvm` or real dotfiles (temporary
  directories and fakes); real-shell tests skip a shell that is not installed.
  No test scripts or test-output directories are kept in the repository.
- The contract compares stdout, stderr and the exit status separately, after
  replacing the temporary root by `<W>`. A scenario whose expectation is
  nvmrc's own output (not nvm.sh's) names its deviation (`D1` .. `D16`) and
  that deviation is listed in `docs/deviations.md`.
- Never weaken an expectation to make a test pass: when a table row and the
  binary disagree and the row is not a listed deviation, stop and report.

## Decisions taken for this plan

These rulings come from the digest and the plan author; each is restated in
the task that implements it and in `docs/deviations.md`.

- **Packaging.** `license = "MIT"`, the license of nvm itself. `LICENSE` holds
  the MIT text with the owner's copyright and keeps nvm's two copyright lines,
  because message texts and behaviour are reproduced from nvm. Metadata for
  crates.io is complete (`description`, `repository`, `readme`, `keywords`,
  `categories`), but `publish = false` until the owner decides to publish.
- **CI.** One workflow, `.github/workflows/ci.yml`: rustfmt, clippy (Linux and
  macOS), tests (Linux with every shell, and macOS), MSRV, cargo-deny plus
  cargo-audit, and rumdl. No Docker job and no release job (YAGNI: the
  Docker image is a local task, and nothing is released yet).
- **Fixed parity gaps:** D3 (bare `nvm` prints the help on stdout, status 0),
  D7 (`nvm alias`, in every form, creates `$NVM_DIR/alias/lts`), D8
  (`nvm ls v0.1.` is the pattern `v0.1`), D12 (the system node is found on
  the `PATH` that `nvm deactivate` would leave, so `$NVM_DIR/../sys` is the
  system's), and D16, found while writing this plan: the floor messages name
  the real `$NVM_DIR/min-version` path, as nvm.sh expands it.
- **Deliberate deviations, documented, not fixed:** D1 (stderr before stdout
  when both are read together), D2 (clap's `--help`), D4 (`--version` prints
  `nvm 0.1.0`), D5 (no `unload`), D6 (clap's unknown-command error), D9
  (`ls-remote` prints `N/A` without the star that nvm.sh adds), D10
  (`install -b` of a legacy version names the legacy layout), D11
  (`set-colors` with an invalid setting prints no help dump), D13 (a v0.x
  version placed under `versions/node` is listed but not used), D14 (no
  `debug`, so exit 42 is never produced), D15 (`nvm: <cmd>: not found`
  instead of bash's `exec` message).
- **Exit codes.** 2, 4, 5, 6, 7 and 11 get end-to-end acceptance scenarios;
  10 is internal to nvm.sh (`nvm_die_on_prefix`, turned into 11 by
  `nvm use`) and 42 only comes from `nvm debug`: neither is produced.
- **`nvm-exec` stays a separate binary** (tools call it by path, as
  `$NVM_DIR/nvm-exec`; the README says to symlink it there). On Unix it now
  replaces itself with the command (`exec`), as the upstream script does.
- **Spec drift** is corrected in Task 5 (sections 1, 3, 4.4, 6, 8, 10, 12) and
  the status in Task 7.

## File map

- Create: `LICENSE`, `deny.toml`, `.github/workflows/ci.yml`, `mise.lock`
  (generated), `README.md`, `docs/deviations.md`, `tests/compat_cli.rs`,
  `tests/compat/mod.rs`, `tests/compat/world.rs`,
  `tests/compat/{alias,codes,exec,init,install,ls,nvmrc,use_version}.rs`,
  `src/cli/tests/help.rs`, `src/commands/resolve/system_tests.rs`,
  `src/commands/install/fetch/offline_tests.rs`,
  `src/commands/use_version/tests.rs`, `src/adapters/std_process/tests.rs`.
- Move: `src/adapters/std_process.rs` to `src/adapters/std_process/mod.rs`.
- Modify: `Cargo.toml`, `.gitignore`, `mise.toml`,
  `docs/superpowers/specs/2026-10-02-nvmrc-design.md`, and the Rust files
  named in Tasks 3 and 4.

---

### Task 1: Packaging, licenses and the mise tasks

**Files:**

- Create: `LICENSE`, `deny.toml`, `mise.lock` (generated by mise)
- Modify: `Cargo.toml:1-5`, `.gitignore`, `mise.toml` (replaced)

**Interfaces:**

- Produces: the mise tasks every later task uses for verification:
  `mise run fmt`, `fmt:check`, `lint`, `lint:rust`, `lint:docs`,
  `lint:actions`, `lint:docker`, `test`, `test:msrv`, `deny`, `audit`,
  `compat:capture` (its test exists from Task 5 on), `docker:build`,
  `docker:test`, `check`.

- [ ] **Step 1: Package metadata**

Replace the `[package]` table of `Cargo.toml` (lines 1-5) with:

```toml
[package]
name = "nvmrc"
version = "0.1.0"
edition = "2024"
rust-version = "1.85.0"
description = "A native Rust port of nvm, the Node Version Manager"
license = "MIT"
repository = "https://github.com/elioseverojunior/nvmrc"
keywords = ["nvm", "node", "nodejs", "version-manager", "shell"]
categories = ["command-line-utilities", "development-tools"]
# Ready for crates.io, but not published until the owner decides to.
publish = false
```

Create `LICENSE` with exactly this text (the MIT license of nvm, with the
owner's copyright first and nvm's notices kept, because nvmrc reproduces
nvm's messages and behaviour):

```text
The MIT License (MIT)

Copyright (c) 2026 Elio Severo Junior

Portions reproduce the messages and behaviour of nvm:
Copyright (c) 2010 Tim Caswell
Copyright (c) 2014 Jordan Harband

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

- [ ] **Step 2: `deny.toml`**

Create `deny.toml` (tested by the digest against this lock file; the license
list is every license in the dependency tree):

```toml
[graph]
all-features = true

[advisories]
version = 2
yanked = "deny"
ignore = []

[licenses]
version = 2
confidence-threshold = 0.93
allow = [
  "0BSD",
  "Apache-2.0",
  "Apache-2.0 WITH LLVM-exception",
  "BSD-3-Clause",
  "CDLA-Permissive-2.0",
  "ISC",
  "MIT",
  "Unicode-3.0",
  "Unlicense",
  "Zlib",
]

[licenses.private]
ignore = true

[bans]
multiple-versions = "warn"
wildcards = "deny"
highlight = "all"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

- [ ] **Step 3: Check the supply chain**

Run: `cargo deny check`
Expected: exit 0; `advisories ok, bans ok, licenses ok, sources ok`. Two
`multiple-versions` warnings are expected and accepted (`syn` 2 and 3 through
`jni-macros`, `windows-sys` 0.52 and 0.61 through `ring`).

Run: `cargo audit`
Expected: exit 0, no vulnerability found.

If either fails on something other than the above, stop and report: do not
add an `ignore` entry.

- [ ] **Step 4: Ignore the machine-specific mise file**

`.gitignore` becomes:

```text
/target
!/docs/**/*
.idea
mise.local.toml
```

- [ ] **Step 5: `mise.toml`**

Replace `mise.toml` with the content below. Everything above the
`# === Tool Installation ===` line follows the global template; the
`[tasks.setup]` table is the template's, verbatim; the project tasks follow
the template's task rules. `bun` and `[deps.bun]` are left out (no
JavaScript). Rust is not pinned here: `idiomatic_version_file_enable_tools =
["rust"]` makes mise read `rust-toolchain.toml`, which stays the only place
the toolchain is pinned.

````toml
[settings]
color = true
experimental = true
idiomatic_version_file = true
idiomatic_version_file_enable_tools = ["rust"]
legacy_version_file = false
lockfile = true
not_found_auto_install = true
quiet = true

[tools]
actionlint = { version = "latest" }
cargo-deny = { version = "0.20" }
gh = { version = "latest" }
gitleaks = { version = "latest" }
hadolint = { version = "latest" }
jq = { version = "latest" }
rumdl = { version = "latest" }
yq = { version = "latest" }

# Cargo Tools
"cargo:cargo-audit" = { version = "0.22" }
"cargo:yamllint-rs" = "latest"

# aqua ships hk for darwin/arm64 but not darwin/amd64, so macOS is narrowed to
# arm64 with the compound "<os>/<arch>" form — mise parses both halves, and this
# is the whole platform gate. The separate `arch` key does NOT filter (verified:
# arch = ["ppc64"] still resolves on an x64 host); it documents intent only.
"aqua:hk" = { version = "latest", os = [
    "linux",
    "windows",
    "macos/arm64",
], arch = [
    "arm64",
    "x64",
], locked = false }

[env]
ARCH = "{{exec(command='uname -m')}}"

# Name the project after the git remote, falling back to the directory.
# `$PWD` is wrong here: run mise from a subdirectory and the project renames
# itself. Tera cannot nest `{{ }}` inside `{{ exec(...) }}`, so config_root is
# spliced in with the `~` concatenation operator. The fallback must sit INSIDE
# the command substitution -- written as `basename $(git ...) .git || <fb>`,
# an empty remote makes basename fail on missing operands and the fallback
# then yields a full path rather than a basename. A TOML multi-line literal
# (''') avoids escaping the double quotes the shell needs.
PROJECT_NAME = '''{{ exec(command='basename "$(git config --get remote.origin.url || echo ' ~ config_root ~ ')" .git') }}'''

[hooks]
enter = { task = "setup" }
postinstall = '''
echo "Installed: $MISE_INSTALLED_TOOLS"
'''

[deps.cargo]
auto = true

[deps.gh]
auto = true

# === Tool Installation ===
[tasks.setup]
alias = ["install", "dev", "dev:setup", "dev:up"]
description = "Install tools, sourcing from Homebrew when it matches the pin"
run = '''
set -eo pipefail

# Homebrew is preferred on macOS when present, but ONLY for a tool whose
# installed version already equals the pin in [tools] above. None of these ship
# a versioned formula (there is no swiftlint@0.65.0), so brew tracks whatever is
# current — honouring it blindly would silently break the pinning this file
# exists to guarantee. On any mismatch the pin wins and mise installs the tool.
#
# Formula names only; versions are read back from [tools] so mise.toml stays the
# single source of truth.
BREW_TOOLS="git-cliff gitleaks gh jq rustup scorecard uv yq"

root="$(git rev-parse --show-toplevel)"
local_cfg="$root/mise.local.toml"

# An empty MISE_DISABLE_TOOLS overrides whatever mise.local.toml sets, so the
# lockfile is always written from the FULL toolset. A disabled tool is omitted
# from mise.lock entirely, and a brew machine committing such a lockfile would
# leave non-brew machines unresolvable. This is why locking no longer needs to
# delete the local config first.
MISE_DISABLE_TOOLS= mise lock

have_brew=no
if [ "$(uname -s)" = "Darwin" ] && command -v brew >/dev/null 2>&1; then
  have_brew=yes
fi

# One batched call: six separate `brew list` invocations cost roughly 5x more.
brew_versions() {
  [ "$have_brew" = yes ] || return 0
  brew list --versions $BREW_TOOLS 2>/dev/null | sort || true
}

# `mise ls --current` merges the global config, so intersecting with BREW_TOOLS
# below keeps this to the project's own tools.
pins="$(MISE_DISABLE_TOOLS= mise ls --current --json | jq -r 'to_entries[] | "\(.key) \(.value[0].requested_version)"' | sort)"

# Fingerprint every input that decides this file's content. Hashing mise.lock
# alone is NOT sufficient: Homebrew moving swiftlint 0.65.0 -> 0.66.0 must force
# a rewrite, and nothing about that touches the lockfile.
fingerprint() {
  printf '%s\n' \
    "v1" "$(uname -s)" "$(uname -m)" "$have_brew" "$pins" "$(brew_versions)" \
    "$(shasum -a 256 "$root/mise.lock" 2>/dev/null | cut -d" " -f1)" \
    | shasum -a 256 | cut -d" " -f1
}

# `mise install` can settle new entries into mise.lock — a first-time install of
# a version, or a tool added since the last lock — which leaves the fingerprint
# just written already stale and costs one redundant rebuild on the next run.
# Re-stamping afterwards makes the cache converge in a single run.
refresh_fp() {
  [ -f "$local_cfg" ] || return 0
  local now tmp
  now="$(fingerprint)"
  if grep -qF "# fingerprint: $now" "$local_cfg"; then
    return 0
  fi
  tmp="$(mktemp)"
  awk -v fp="$now" '/^# fingerprint: /{ print "# fingerprint: " fp; next } { print }' "$local_cfg" > "$tmp"
  mv "$tmp" "$local_cfg"
}

fp="$(fingerprint)"
if [ -f "$local_cfg" ] && grep -qF "# fingerprint: $fp" "$local_cfg"; then
  echo "mise.local.toml up to date ($(printf '%s' "$fp" | cut -c1-12))"
  mise install
  mise deps
  refresh_fp
  exit 0
fi

lookup() { printf '%s\n' "$2" | awk -v t="$1" '$1 == t { print $2 }'; }

brewed=""
installed_any=no
if [ "$have_brew" = yes ]; then
  state="$(brew_versions)"
  missing=""

  for f in $BREW_TOOLS; do
    pin="$(lookup "$f" "$pins")"
    [ -n "$pin" ] || continue
    inst="$(lookup "$f" "$state")"

    if [ -z "$inst" ]; then
      missing="$missing $f"
    elif [ "$pin" = "latest" ] || [ "$pin" = "$inst" ]; then
      brewed="$brewed $f"
      echo "brew $f $inst"
    else
      echo "mise $f $pin (Homebrew has $inst)"
    fi
  done

  # Only tools that are not installed yet need the slow formula API, and they
  # are queried in a single batched call.
  if [ -n "$missing" ]; then
    stables="$(brew info --json=v2 $missing 2>/dev/null | jq -r '.formulae[] | "\(.name) \(.versions.stable)"' || true)"
    for f in $missing; do
      pin="$(lookup "$f" "$pins")"
      stable="$(lookup "$f" "$stables")"
      if [ -n "$stable" ] && { [ "$pin" = "latest" ] || [ "$pin" = "$stable" ]; }; then
        brew install "$f"
        brewed="$brewed $f"
        installed_any=yes
        echo "brew $f $stable (installed)"
      else
        echo "mise $f $pin${stable:+ (Homebrew has $stable)}"
      fi
    done
  fi
fi

# Installing changed what brew reports, so the fingerprint must be recomputed or
# the next run would see a mismatch and redo all of this for nothing.
[ "$installed_any" = yes ] && fp="$(fingerprint)"

# disable_tools makes mise step aside so its shims stop shadowing Homebrew's
# bin. Machine-specific, hence mise.local.toml rather than mise.toml.
{
  echo "# Generated by \`mise run install\` - machine-specific, not committed."
  echo "# fingerprint: $fp"
  if [ -n "$brewed" ]; then
    echo "# Tools Homebrew satisfies at the pinned version; mise handles the rest."
    echo "[settings]"
    printf 'disable_tools = ['
    sep=""
    for f in $brewed; do
      printf '%s"%s"' "$sep" "$f"
      sep=", "
    done
    printf ']\n'
  fi
  # No [tools] section is emitted here: platform gating belongs in mise.toml,
  # where the compound os = [..., "macos/arm64"] form already covers hk. This
  # file is only ever about stepping aside for the system package manager.
} > "$local_cfg"

mise lock
mise install
mise deps
refresh_fp
'''
shell = "bash -c"

# === Formatting ===
[tasks.fmt]
alias = ["f"]
description = "Format the Rust code and the Markdown"
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

cargo fmt --all
rumdl fmt .
'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

[tasks."fmt:check"]
alias = ["fc"]
description = "Fail when the Rust code is not formatted"
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

cargo fmt --all --check
'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

# === Linting ===
[tasks.lint]
alias = ["l"]
description = "Run every linter: clippy, rumdl, actionlint and hadolint"
depends = ["lint:rust", "lint:docs", "lint:actions", "lint:docker"]
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

[tasks."lint:rust"]
alias = ["clippy"]
description = "Clippy on every target and feature, warnings denied"
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

cargo clippy --all-targets --all-features --locked -- -D warnings
'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

[tasks."lint:docs"]
alias = ["md"]
description = "Lint the Markdown with rumdl"
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

rumdl check .
'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

[tasks."lint:actions"]
alias = ["al"]
description = "Lint the GitHub Actions workflows with actionlint"
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

actionlint
'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

[tasks."lint:docker"]
alias = ["hl"]
description = "Lint the Dockerfile with hadolint"
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

hadolint Dockerfile
'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

# === Tests ===
[tasks.test]
alias = ["t"]
description = "Run every unit and end-to-end test"
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

cargo test --locked
'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

[tasks."test:msrv"]
alias = ["msrv"]
description = "Check that everything still builds with the MSRV, Rust 1.85"
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

# `rustup run` beats rust-toolchain.toml and a non-rustup cargo on PATH.
rustup toolchain install 1.85 --profile minimal
rustup run 1.85 cargo check --all-targets --locked
'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

[tasks."compat:capture"]
alias = ["cc"]
description = "Run the compatibility scenarios through nvm.sh and print what differs"
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

nvm_sh="${usage_nvm_sh:?}"
if [ ! -f "$nvm_sh" ]; then
  echo "compat:capture: no nvm.sh at $nvm_sh" >&2
  exit 1
fi
NVMRC_ORACLE_NVM_SH="$(cd "$(dirname "$nvm_sh")" && pwd)/$(basename "$nvm_sh")" \
  cargo test --locked --test compat_cli -- --ignored capture_from_nvm_sh \
  --nocapture --test-threads=1
'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
flag "--nvm-sh <path>" help="The nvm.sh to capture from" default="../nvm/nvm.sh"
'''

# === Supply chain ===
[tasks.deny]
alias = ["d"]
description = "Check licenses, advisories, bans and sources with cargo-deny"
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

cargo deny check
'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

[tasks.audit]
alias = ["a"]
description = "Check the lock file against the RustSec advisories"
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

cargo audit
'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

# === Docker ===
[tasks."docker:build"]
alias = ["dkb", "dkrb", "dkr:build"]
description = "Docker Build"
depends = []
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

docker buildx build -f Dockerfile -t nvmrc:trixie .

'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

[tasks."docker:test"]
alias = ["dkt", "dkr:test"]
description = "Run the whole test suite, every shell included, in the Docker image"
depends = ["docker:build"]
run = '''
#!/usr/bin/env bash

set -Eeuo pipefail

[ "${usage_verbose:-false}" = "true" ] && set -x

docker run --rm nvmrc:trixie
'''
shell = "bash -c"
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''

# === Gate ===
[tasks.check]
alias = ["ci"]
description = "Every gate a commit must pass, as CI runs them"
depends = ["fmt:check", "lint", "test", "test:msrv", "deny", "audit"]
usage = '''
flag "-v --verbose" help="Enable verbose (debug) output"
'''
````

The `[tasks.setup]` table is the template's, character for character
(compare it with the "Main setup task — use verbatim" block of
`~/.claude/CLAUDE.md`; it is the one task exempt from the task template,
because the global rules say to use it verbatim).

- [ ] **Step 6: Verify mise**

Run: `mise trust && mise tasks ls`
Expected: the 16 tasks above, each with its description.

Run:

```sh
mise run fmt:check && mise run deny && mise run audit && mise run test:msrv
```

Expected: every task exits 0 (MSRV: `Finished` with Rust 1.85).

Run: `mise run lint:docker`
Expected: exit 0 (the Dockerfile already follows `.hadolint.yaml`).

Run: `MISE_DISABLE_TOOLS= mise lock`
Expected: `mise.lock` is created (or updated) for the tools above.

`mise run setup` installs tools and may call `brew install`; run it only
when the owner is at the machine. It is not required for this task.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml LICENSE deny.toml .gitignore mise.toml mise.lock
git commit -S -m "build: add the MIT license, cargo-deny policy and the mise tasks"
```

---

### Task 2: Continuous integration

**Files:**

- Create: `.github/workflows/ci.yml`

**Interfaces:**

- Consumes: `rust-toolchain.toml` (rustup installs it with
  `rustup toolchain install`, as the Dockerfile does), `.cargo/config.toml`
  (x86_64 Linux links with `clang` and `mold`), `deny.toml`, `rumdl.toml`.
- Produces: the jobs `fmt`, `clippy`, `test`, `msrv`, `supply-chain`, `docs`.
- [ ] **Step 1: The workflow**

Create `.github/workflows/ci.yml`:

```yaml
name: ci

on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always

jobs:
  fmt:
    name: rustfmt
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - name: Install the toolchain of rust-toolchain.toml
        run: rustup toolchain install
      - run: cargo fmt --all --check

  clippy:
    name: clippy (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v7
      - name: Install the linker of .cargo/config.toml
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends clang mold
      - name: Install the toolchain of rust-toolchain.toml
        run: rustup toolchain install
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --all-targets --all-features --locked -- -D warnings

  test:
    name: test (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v7
      # The end-to-end tests of `nvm init` and the contract run in every
      # shell nvmrc supports and skip the missing ones: on Linux all are
      # installed; macOS has bash, zsh, sh, dash and ksh already.
      - name: Install the linker and every shell
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends clang mold dash fish ksh zsh
      - name: Install the toolchain of rust-toolchain.toml
        run: rustup toolchain install
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --locked

  msrv:
    name: msrv (1.85)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - name: Install the linker of .cargo/config.toml
        run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends clang mold
      - name: Install Rust 1.85
        run: rustup toolchain install 1.85 --profile minimal
      - uses: Swatinem/rust-cache@v2
      - run: rustup run 1.85 cargo check --all-targets --locked

  supply-chain:
    name: cargo deny and cargo audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - name: Install the toolchain of rust-toolchain.toml
        run: rustup toolchain install
      - uses: taiki-e/install-action@v2
        with:
          tool: cargo-deny,cargo-audit
      - run: cargo deny check
      - run: cargo audit

  docs:
    name: markdown (rumdl)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - run: pipx run rumdl check .
```

Notes for the implementer:

- `rustup toolchain install` with no argument installs what
  `rust-toolchain.toml` names (channel 1.99, its components and four
  targets); the Dockerfile relies on the same behaviour.
- No action besides checkout, rust-cache and install-action is used, so no
  other tag has to be chosen. Do not add `dtolnay/rust-toolchain`.
- `pipx` is preinstalled on the GitHub Ubuntu images; `rumdl` is published on
  PyPI.
- [ ] **Step 2: Lint the workflow**

Run: `mise run lint:actions` (or `actionlint` directly)
Expected: no output, exit 0. A `shellcheck` finding inside a `run:` block is
fixed in the block, not silenced.

Run: `rumdl check .`
Expected: `No issues found`.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -S -m "ci: run fmt, clippy, tests, MSRV, deny, audit and rumdl on GitHub Actions"
```

The workflow itself is proven on the first push (Task 7 lists it in the
hand-off; pushing is the owner's decision).

---

### Task 3: The parity gaps the oracle found (D3, D7, D8, D12, D16)

Five small fixes, one commit each. Each sub-step is TDD: the failing test,
the run that shows it fail, the code, the run that shows it pass, the
commit. The end-to-end proof of each fix is a contract scenario in Task 5
(`init.rs` S6, `alias.rs` A14/A15, `ls.rs` L6, `ls.rs` L3, `codes.rs` C7).

**Files:**

- Create: `src/cli/tests/help.rs`, `src/commands/resolve/system_tests.rs`
- Modify: `src/cli/mod.rs:58-77`, `src/cli/tests/mod.rs:292-298`,
  `src/commands/aliases/mod.rs:57-73`, `src/commands/aliases/tests.rs`,
  `src/domain/version_prefix.rs`, `src/commands/ls/mod.rs:182-200`,
  `src/commands/ls/tests.rs`, `src/commands/resolve/mod.rs:1-60`,
  `src/commands/use_version/target/lookup.rs:96-115`,
  `src/commands/install/resolve.rs:112-133`,
  `src/commands/install/tests/failures.rs:22-38`, `src/error.rs:81-86`

**Interfaces:**

- Produces: `domain::version_prefix::without_trailing_group_dot(&str) ->
  &str`; `commands::resolve::path_without_nvm(&Context) ->
  Result<OsString, CliError>` (used by `use_version::target::lookup`).
- Behaviour: `nvm` with no argument prints clap's help on stdout and exits 0;
  `nvm alias ...` (any form, errors included) first creates
  `$NVM_DIR/alias/lts`, ignoring a failure; `nvm ls v0.1.` lists v0.1.x;
  `resolve::system_node` searches the `PATH` that `nvm_strip_path` leaves;
  the floor messages print the real `<NVM_DIR>/min-version`.

#### 3.1 D3: a bare `nvm` is `nvm --help`

nvm.sh (`nvm.sh:3690-3693`): `if [ "$#" -lt 1 ]; then nvm --help; return;
fi`, so `nvm` alone prints the help on stdout with status 0. Today clap
reports a usage error, status 127, and `set -e; eval init; nvm` aborts the
caller (upstream `fast/Sourcing nvm.sh should make the nvm command
available`).

- [ ] **Step 1: The failing test**

Create `src/cli/tests/help.rs`:

```rust
//! A bare `nvm` is `nvm --help`, as in nvm.sh.

use super::*;

#[test]
fn bare_nvm_prints_the_help_on_stdout_with_exit_0() {
    let (code, out, err) = run_cli(&["nvm"]);
    let (_, help, _) = run_cli(&["nvm", "--help"]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, help.as_str(), ""));
}
```

In `src/cli/tests/mod.rs`, add `mod help;` to the module list at the end,
keeping it sorted (`mod exec; mod help; mod init; ...`).

- [ ] **Step 2: Run it**

Run: `cargo test --locked --lib cli::tests::help`
Expected: FAIL, `left: (127, "", "error: ...")`.

- [ ] **Step 3: The code**

In `src/cli/mod.rs`, `run` builds its arguments through a new function
instead of calling `keep_leading_double_dash` directly:

```rust
    let args = arguments(args);
```

and, below `keep_leading_double_dash`, add:

```rust
/// What clap is given: a bare `nvm` is `nvm --help`, as nvm.sh's function
/// does, and a `--` after `exec`, `run` or `set-colors` is kept.
fn arguments<I, T>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    if args.len() == 1 {
        args.push(OsString::from("--help"));
    }
    keep_leading_double_dash(args)
}
```

`run`'s `T: Into<OsString> + Clone` bound stays as it is.

- [ ] **Step 4: Run the tests**

Run: `cargo test --locked --lib cli::tests`
Expected: PASS (all of `cli::tests`, `a_usage_error_exits_127_without_stdout`
included: `nvm bogus` is still 127).

- [ ] **Step 5: Commit**

```bash
git add src/cli/mod.rs src/cli/tests/mod.rs src/cli/tests/help.rs
git commit -S -m "fix(cli): a bare nvm prints the help with status 0, as nvm.sh does"
```

#### 3.2 D7: `nvm alias` creates `$NVM_DIR/alias/lts`

nvm.sh's `alias` branch (`nvm.sh:5021-5027`) runs
`command mkdir -p "${NVM_ALIAS_DIR}/lts"` before it reads its arguments, so
listing, creating, deleting (`nvm alias foo ""`) and even a refused name
leave the directory behind. A failing `mkdir` only complains on stderr and
nvm.sh goes on; nvmrc ignores the failure silently (listed in
`docs/deviations.md` by Task 6).

- [ ] **Step 1: The failing test**

Append to `src/commands/aliases/tests.rs` (249 lines today):

```rust
#[test]
fn every_alias_command_creates_the_lts_directory_first() {
    let cases: [&[&str]; 5] = [&[], &["--no-colors"], &["work"], &["new", "20"], &["a#b", "20"]];
    for words in cases {
        let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "");
        let env = FakeEnv::default().with_var("NVM_DIR", "/n");
        let args: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
        let _ = run(&Context::new(&fs, &env), &args);
        let lts = crate::ports::FileSystem::read_dir(&fs, Path::new("/n/alias/lts"));
        assert!(lts.is_ok(), "{words:?}");
    }
}
```

- [ ] **Step 2: Run it**

Run: `cargo test --locked --lib commands::aliases::tests::every_alias_command`
Expected: FAIL on `[]`.

- [ ] **Step 3: The code**

In `src/commands/aliases/mod.rs`, `run` calls a new function first:

```rust
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    ensure_lts_directory(context);
    let Words {
```

and below `run`:

```rust
/// nvm.sh's `nvm alias` starts with `mkdir -p "$NVM_DIR/alias/lts"`, whatever
/// its arguments. A failure is ignored: the command goes on without it.
fn ensure_lts_directory(context: &Context<'_>) {
    if let Ok(alias_dir) = context.alias_dir() {
        let _ = context.fs.create_dir_all(&alias_dir.join("lts"));
    }
}
```

Update the doc comment of `run` with one more line: "Every form first
creates `$NVM_DIR/alias/lts`, as nvm.sh does."

- [ ] **Step 4: Run the tests**

Run:

```sh
cargo test --locked --lib commands::aliases commands::unalias commands::alias
```

Expected: PASS. (`nvm unalias` is not changed: nvm.sh's `unalias` creates
nothing.)

- [ ] **Step 5: Commit**

```bash
git add src/commands/aliases/mod.rs src/commands/aliases/tests.rs
git commit -S -m "fix(alias): create the alias/lts directory as nvm.sh does"
```

#### 3.3 D8: `nvm ls v0.1.` is the pattern `v0.1`

`nvm_ls` (`nvm.sh:1764-1773`) counts the version groups of the pattern
(`nvm_num_version_groups` ignores one trailing dot) and, for one or two
groups, rewrites the pattern as `${PATTERN%.}.`. So `v0.1.` and `0.1.` list
the v0.1.x versions, while `v0.1.2.` (three groups) matches nothing.

- [ ] **Step 1: The failing tests**

Append to the `tests` module of `src/domain/version_prefix.rs` (it does not
compile yet):

```rust
    #[test]
    fn one_trailing_dot_after_one_or_two_groups_is_dropped() {
        assert_eq!(without_trailing_group_dot("v0.1."), "v0.1");
        assert_eq!(without_trailing_group_dot("0."), "0");
        assert_eq!(without_trailing_group_dot("iojs-v3."), "iojs-v3");
        assert_eq!(without_trailing_group_dot("v0.1"), "v0.1");
        assert_eq!(without_trailing_group_dot("v0.1.2."), "v0.1.2.");
    }
```

Append to `src/commands/ls/tests.rs` (256 lines today):

```rust
#[test]
fn a_pattern_ending_with_a_dot_lists_that_line() {
    assert_eq!(ls(Some("v20.1.")), Output::stdout(rows(&["        v20.1.0 *"])));
    assert_eq!(
        ls(Some("20.")),
        Output::stdout(rows(&["        v20.1.0 *", "       v20.10.0 *"]))
    );
}
```

(`installed()` holds v20.1.0, v20.10.0, v18.9.0 and two io.js versions of
majors 2 and 3: `v20.1` matches only v20.1.0, `20` matches the two v20
versions.)

- [ ] **Step 2: Run them**

Run:

```sh
cargo test --locked --lib version_prefix commands::ls::tests::a_pattern_ending
```

Expected: FAIL to compile (`without_trailing_group_dot` not found).

- [ ] **Step 3: The code**

Add to `src/domain/version_prefix.rs`, after `with_v_prefix`:

```rust
/// `nvm_ls`'s treatment of a pattern with one or two version groups and a
/// trailing dot: `v0.1.` is `v0.1` and `0.` is `0`. With three groups the
/// dot stays (`v0.1.2.` matches nothing), as it does in nvm.sh.
#[must_use]
pub fn without_trailing_group_dot(pattern: &str) -> &str {
    match pattern.strip_suffix('.') {
        Some(rest) if rest.matches('.').count() <= 1 => rest,
        _ => pattern,
    }
}
```

In `src/commands/ls/mod.rs`, `by_name` parses the trimmed text (the alias
check before it keeps the name as given):

```rust
    let Ok(pattern) = without_trailing_group_dot(name).parse::<VersionPattern>() else {
        return Ok(nothing_found());
    };
```

and import it: `use crate::domain::version_prefix::without_trailing_group_dot;`.
Only `ls` changes; `VersionPattern::from_str` stays strict, because the
floor (`NVM_MIN_VERSION=24.`) and `nvm use` must not start accepting it
without their own oracle check.

- [ ] **Step 4: Run the tests**

Run: `cargo test --locked --lib version_prefix commands::ls`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/domain/version_prefix.rs src/commands/ls/mod.rs src/commands/ls/tests.rs
git commit -S -m "fix(ls): a pattern ending with a dot lists that version line, as nvm.sh does"
```

#### 3.4 D12: the system node is the one `nvm deactivate` would leave

`nvm_has_system_node` is `nvm deactivate >/dev/null && command -v node`, and
deactivate strips only nvm's own entries (`nvm_strip_path`:
`$NVM_DIR/[^/]*/bin` and `$NVM_DIR/versions/*/*/bin`). `resolve::system_node`
(`src/commands/resolve/mod.rs:49-60`) instead drops every `PATH` entry that
`Path::starts_with` the unnormalised `$NVM_DIR`, so `$NVM_DIR/../sys` (whose
components start with `$NVM_DIR`) is lost: `nvm which default` (default →
system) gives `N/A`, `nvm which system` 127. `use_version::target::lookup::
system_node` already strips as nvm.sh does; both now share one function.

- [ ] **Step 1: The failing test**

`src/commands/resolve/tests.rs` has 294 lines, so the new tests get their own
file. Create `src/commands/resolve/system_tests.rs`:

```rust
//! `system_node` finds the `node` left on `PATH` once nvm's own entries are
//! stripped from it, as nvm.sh's `nvm_has_system_node` does.

use std::path::PathBuf;

use super::system_node;
use crate::context::Context;
use crate::fakes::{FakeEnv, FakeFileSystem};

fn found_on(path: &str) -> Option<PathBuf> {
    let fs = FakeFileSystem::default()
        .with_file("/w/nvm/versions/node/v20.1.0/bin/node", "")
        // The fake normalises `..` when it looks a file up.
        .with_file("/w/sys/node", "")
        .with_file("/w/nvm/tools/bin/node", "");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/w/nvm")
        .with_var("PATH", path);
    system_node(&Context::new(&fs, &env)).unwrap()
}

#[test]
fn a_directory_whose_text_starts_with_nvm_dir_is_the_system() {
    let found = found_on("/w/nvm/versions/node/v20.1.0/bin:/w/nvm/../sys:/usr/bin");
    assert_eq!(found, Some(PathBuf::from("/w/nvm/../sys/node")));
}

#[test]
fn the_entries_nvm_strips_are_never_the_system() {
    assert_eq!(found_on("/w/nvm/versions/node/v20.1.0/bin"), None);
    assert_eq!(found_on("/w/nvm/tools/bin"), None);
}
```

In `src/commands/resolve/mod.rs`, next to `mod tests;`:

```rust
#[cfg(test)]
mod system_tests;
```

- [ ] **Step 2: Run it**

Run: `cargo test --locked --lib commands::resolve::system_tests`
Expected: `a_directory_whose_text_starts_with_nvm_dir_is_the_system` FAILS
(`left: None`); the other passes.

- [ ] **Step 3: The code**

In `src/commands/resolve/mod.rs`, replace `system_node` with:

```rust
/// The first `node` on `PATH` once nvm's own entries are stripped from it
/// (`nvm_has_system_node`): a directory that is merely inside `$NVM_DIR`, or
/// whose text starts with it (`$NVM_DIR/../sys`), is the system's.
///
/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn system_node(context: &Context<'_>) -> Result<Option<PathBuf>, CliError> {
    Ok(find_in_path(context.fs, &path_without_nvm(context)?, "node"))
}

/// `PATH` as `nvm deactivate` leaves it (`nvm_strip_path`): without the
/// `$NVM_DIR/*/bin` and `$NVM_DIR/versions/*/*/bin` entries.
///
/// # Errors
/// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
pub fn path_without_nvm(context: &Context<'_>) -> Result<OsString, CliError> {
    let nvm_dir = context.nvm_dir()?;
    let path = context.env.var_os("PATH").unwrap_or_default();
    let stripped = strip_path(&path.to_string_lossy(), "/bin", &nvm_dir.to_string_lossy());
    Ok(OsString::from(stripped))
}
```

Imports of `resolve/mod.rs`: add `use std::ffi::OsString;`,
`use crate::domain::path_edit::strip_path;`, and replace
`use crate::domain::path_search::find_in_dirs;` by
`use crate::domain::path_search::find_in_path;`. Update the doc of
`Resolved::System` to "The alias chain ended at `system` and a `node` is left
on `PATH` once nvm's entries are stripped."

In `src/commands/use_version/target/lookup.rs`, `system_node` reuses it:

```rust
pub fn system_node(context: &Context<'_>) -> Result<Option<SystemNode>, CliError> {
    let stripped = path_without_nvm(context)?;
    let find = |name: &str| find_in_path(context.fs, &stripped, name);
```

(the rest of the function is unchanged), import
`crate::commands::resolve::path_without_nvm` next to `Resolved`, and remove
the imports that became unused (`std::ffi::OsStr`,
`crate::domain::path_edit::strip_path`) as clippy reports them.

- [ ] **Step 4: Run the tests**

Run: `cargo test --locked --lib`
Expected: PASS. If an existing test asserted the old rule (a `node` in a
non-`bin` directory under `$NVM_DIR` ignored), it now contradicts nvm.sh:
change its expectation to nvm.sh's rule and name it in the commit body.

- [ ] **Step 5: Commit**

```bash
git add src/commands/resolve/mod.rs src/commands/resolve/system_tests.rs \
  src/commands/use_version/target/lookup.rs
git commit -S -m "fix(resolve): find the system node on the PATH nvm deactivate leaves"
```

#### 3.5 D16: the floor messages name the real `min-version` file

nvm.sh (the `spike` floor) prints the expanded path:
`Invalid minimum version 'bogus' (from NVM_MIN_VERSION or <W>/nvm/min-version).`
and `Lower or unset NVM_MIN_VERSION (or edit <W>/nvm/min-version) to install
it.`; nvmrc prints the literal `$NVM_DIR/min-version` (checked against
nvm.sh with the local mirror while writing this plan).

- [ ] **Step 1: The failing tests**

In `src/commands/install/tests/failures.rs`, change the expected second line
of `a_version_below_the_floor_is_status_7_with_both_lines` to:

```rust
            "Lower or unset NVM_MIN_VERSION (or edit /n/min-version) to install it.",
```

and append:

```rust
#[test]
fn an_invalid_floor_names_the_min_version_file_of_nvm_dir() {
    let mut world = World::new();
    world.env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("NVM_MIN_VERSION", "bogus");
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert_eq!(
        output.stderr,
        "Invalid minimum version 'bogus' (from NVM_MIN_VERSION or /n/min-version)."
    );
}
```

- [ ] **Step 2: Run them**

Run: `cargo test --locked --lib commands::install::tests::failures`
Expected: both FAIL (`$NVM_DIR/min-version` printed).

- [ ] **Step 3: The code**

In `src/error.rs`, document the placeholder on `FloorError::Invalid`:

```rust
    /// `$NVM_DIR/min-version` is replaced by the real path where it is
    /// printed (`install::resolve::check_floor`), as nvm.sh expands it.
    #[error("Invalid minimum version '{0}' (from NVM_MIN_VERSION or $NVM_DIR/min-version).")]
    Invalid(String),
```

In `src/commands/install/resolve.rs`, `check_floor` becomes:

```rust
/// How the floor messages name the file before the real path replaces it.
const MIN_VERSION_FILE: &str = "$NVM_DIR/min-version";

pub(super) fn check_floor(
    context: &Context<'_>,
    version: &Version,
    transcript: &mut Transcript,
) -> Step<()> {
    let file = context.nvm_dir()?.join("min-version");
    let from_file = context.fs.read_to_string(&file).ok();
    let from_env = context.env.var("NVM_MIN_VERSION");
    let floor = VersionFloor::from_sources(from_env.as_deref(), from_file.as_deref())
        .and_then(|floor| floor.map_or(Ok(()), |floor| floor.check(version)));
    let Err(error) = floor else {
        return Ok(());
    };
    let shown = file.display().to_string();
    transcript.err(error.to_string().replace(MIN_VERSION_FILE, &shown));
    if matches!(error, crate::error::FloorError::Below { .. }) {
        transcript.err(format!(
            "Lower or unset NVM_MIN_VERSION (or edit {shown}) to install it."
        ));
    }
    Err(Halt::Exit(NvmExitCode::BelowVersionFloor))
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --locked --lib commands::install`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/error.rs src/commands/install/resolve.rs src/commands/install/tests/failures.rs
git commit -S -m "fix(install): name the real min-version file in the floor messages"
```

- [ ] **Step 6: The task gate**

Run:

```sh
cargo fmt --all --check && cargo clippy --all-targets --all-features --locked -- -D warnings && cargo test --locked
```

Expected: all green.

---

### Task 4: The deferred minors of Plans 5, 6 and 8

Nine small fixes the final reviews deferred, grouped so each commit touches a
few files. File and line references are those of `743b94c` plus Task 3.

**Files:**

- Create: `src/commands/install/fetch/offline_tests.rs`,
  `src/adapters/std_process/tests.rs`, `src/commands/use_version/tests.rs`
- Move: `src/adapters/std_process.rs` to `src/adapters/std_process/mod.rs`
- Modify: `src/commands/install/fetch/{mod.rs,tests.rs}`,
  `src/commands/install/source/{mod.rs,tests.rs}`,
  `src/adapters/std_spawn.rs`, `src/ports/process.rs`,
  `src/fakes/process.rs`, `src/commands/use_version/mod.rs`,
  `src/commands/nvm_exec/mod.rs`, `src/cli/{mod.rs,child.rs}`,
  `src/cli/tests/{nvm_exec.rs,exec.rs}`, `tests/nvm_exec_cli.rs`,
  `src/commands/migrate/{undo.rs,undo_tests.rs,check.rs,apply.rs,apply_tests.rs,write_tests.rs}`,
  `src/domain/migration/{backup.rs,backup_tests.rs}`,
  `src/adapters/std_fs/{atomic.rs,atomic_tests.rs}`,
  `src/commands/conflict/{roots.rs,roots_tests.rs}`,
  `src/commands/doctor/tests.rs`, `src/domain/conflict/{source_path.rs,
  source_path_tests.rs,tests.rs}`

**Interfaces:**

- Produces: `Completed::code` is `Some(128 + n)` for a program killed by
  signal n (Unix); `ProcessOutput::stderr: String` (capped like stdout);
  `Process::hand_over(&self, &Invocation) -> io::Result<i32>` (default:
  `spawn`); `FakeProcess::{handed_over, with_failure_saying}`;
  `use_version::run_and_target(&Context, &[String]) -> Result<(Output,
  Option<Target>), CliError>`; `latest_backup(file_name, candidates, now:
  i64)`; `SyntaxCheck::{original_passes, complaint}`.

#### 4.1 Plan 5 M2 and M12: the cache trusts only verified files

- M2 (`src/commands/install/fetch/mod.rs:213-230`, `verify`): an archive that
  fails its checksum stays in `.cache/bin/<slug>/`, and a later
  `nvm install --offline` takes it unchecked. Fix: remove it with the
  unpack directory.
- M12 (`fetch/mod.rs:114-130`, `cached_only`): `--offline` takes any
  `file_info` success, a directory included; nvm.sh tests `-r`. Fix: a
  directory is not a cached archive.
- [ ] **Step 1: Make room for the offline tests**

`fetch/tests.rs` has 290 lines. Create
`src/commands/install/fetch/offline_tests.rs` and move into it, unchanged,
the `offline` helper and the two tests after it
(`offline_uses_the_cached_archive_without_a_checksum_or_the_network`,
`offline_without_a_cached_archive_fails_naming_the_slug`, today
`fetch/tests.rs:208-247`). The new file starts with:

```rust
//! `--offline`: the cached archive, taken without a checksum, or nothing.

use super::tests::{TARBALL, artifact_of, env, stderr, version};
use super::*;
use crate::fakes::{FakeDigest, FakeFileSystem, FakeHttp};
```

In `fetch/tests.rs`, make the shared items visible to the sibling module:
`pub(super) const TARBALL`, `pub(super) fn artifact_of`, `pub(super) fn
version`, `pub(super) fn env`, `pub(super) fn stderr`. In `fetch/mod.rs`,
after `mod name_tests;`:

```rust
#[cfg(test)]
mod offline_tests;
```

Run: `cargo test --locked --lib commands::install::fetch`
Expected: PASS, same number of tests as before.

- [ ] **Step 2: The failing tests**

In `fetch/tests.rs`, rename
`a_wrong_checksum_fails_and_leaves_the_archive_but_not_the_unpack_directory`
to `a_wrong_checksum_fails_and_removes_the_archive_and_the_unpack_directory`
and change its archive assertion to:

```rust
    assert!(fs.file_info(Path::new(TARBALL)).is_err());
```

Append to `offline_tests.rs`:

```rust
#[test]
fn offline_does_not_take_a_directory_for_the_cached_archive() {
    let fs = FakeFileSystem::default().with_dir(TARBALL);
    let (result, transcript) = offline(&fs, &FakeHttp::default());
    assert_eq!(result, Err(Failed));
    assert_eq!(
        stderr(transcript),
        ["Offline: no cached archive found for node-v20.10.0-linux-x64"]
    );
}
```

Run: `cargo test --locked --lib commands::install::fetch`
Expected: the two tests FAIL.

- [ ] **Step 3: The code**

In `cached_only`:

```rust
    let cached = context.fs.file_info(&artifact.tarball);
    if cached.is_ok_and(|info| !info.is_dir) {
```

In `verify`, the `Err` arm removes the archive too:

```rust
        Err(error) => {
            transcript.err(error.to_string());
            // Never left for a later `--offline` install, which takes the
            // cache without a checksum.
            let _ = context.fs.remove_file(&artifact.tarball);
            let _ = context.fs.remove_dir_all(&artifact.files());
            Err(Failed)
        }
```

Run: `cargo test --locked --lib commands::install`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/commands/install/fetch/mod.rs src/commands/install/fetch/tests.rs \
  src/commands/install/fetch/offline_tests.rs
git commit -S -m "fix(install): drop an archive that fails its checksum and never take a directory offline"
```

#### 4.2 Plan 5 M6: no source build is said, not silent

`src/commands/install/source/mod.rs:84`: when `Artifact::source_of` is
`None` (no known platform, or no cache directory), `build` returns
`BuildFailed` with nothing in the transcript, so `nvm install` exits 1
printing nothing. nvm.sh prints "Installing from source on non-WSL Windows
is not supported" (status 87, a platform nvmrc does not run on). nvmrc keeps
status 1 and says why.

- [ ] **Step 1: The failing test**

Append to `src/commands/install/source/tests.rs` (251 lines today):

```rust
#[test]
fn a_platform_without_source_builds_says_so() {
    let world = World::new();
    let (result, _, stderr) = world.build(&[], None);
    assert_eq!(result, Err(BuildFailed));
    assert_eq!(stderr, "Installing from source is not supported on this platform.");
    assert!(world.ran().is_empty());
}
```

Run:

```sh
cargo test --locked --lib commands::install::source::tests::a_platform_without
```

Expected: FAIL (`left: ""`).

- [ ] **Step 2: The code**

In `source/mod.rs`, add near the top:

```rust
/// What `build` says when this machine has no source archive to build.
const NO_SOURCE_BUILD: &str = "Installing from source is not supported on this platform.";
```

and in `build`, replace the `let artifact = ... .ok_or(BuildFailed)?;` line:

```rust
    let Some(artifact) = Artifact::source_of(context, job.version) else {
        transcript.err(NO_SOURCE_BUILD);
        return Err(BuildFailed);
    };
```

If `build` passes 30 lines, move the three lines that print the parameters
and choose the compiler into a `fn prepare(...) -> (String, Compiler)`
helper.

Run: `cargo test --locked --lib commands::install`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/commands/install/source/mod.rs src/commands/install/source/tests.rs
git commit -S -m "fix(install): say so when this platform has no source build"
```

#### 4.3 Plan 5 M5: a hook killed by a signal passes on 128+n

`src/commands/install/acquire/hook.rs:52-53` passes the hook's
`Completed::code` to `NvmExitCode::passing_on`, which maps `None` (a signal)
to 1; `StdProcess::execute` (`src/adapters/std_process.rs:56-59`) sets
`code: status.code()`. nvm.sh passes `$?`, 128+n. `std_spawn::exit_code`
(`src/adapters/std_spawn.rs:44-56`) already does the mapping for `spawn`:
`execute` reuses it (DRY).

- [ ] **Step 1: Split `std_process.rs`**

It has 295 lines. Run
`git mv src/adapters/std_process.rs src/adapters/std_process/mod.rs`, then
move the body of its `#[cfg(all(test, unix))] mod tests { ... }` block into
`src/adapters/std_process/tests.rs` (the items without the `mod tests {`
wrapper, dedented, keeping `use super::*;` first) and leave in `mod.rs`:

```rust
#[cfg(all(test, unix))]
mod tests;
```

Run: `cargo test --locked --lib adapters::std_process`
Expected: PASS, same tests.

- [ ] **Step 2: The failing test**

Append to `src/adapters/std_process/tests.rs`:

```rust
#[test]
fn execute_reports_128_plus_the_signal_that_killed_the_program() {
    let done = execute("kill -9 $$");
    assert_eq!((done.success, done.code), (false, Some(137)));
}
```

Run:

```sh
cargo test --locked --lib adapters::std_process::tests::execute_reports_128
```

Expected: FAIL (`left: (false, None)`).

- [ ] **Step 3: The code**

In `src/adapters/std_spawn.rs`, make both `exit_code` functions (the `unix`
one and the other) `pub(super)`. In `std_process/mod.rs`, `execute` sets:

```rust
            code: Some(super::std_spawn::exit_code(status)),
```

In `src/ports/process.rs`, the doc of `Completed::code` becomes:

```rust
    /// The exit status; `128 + n` for a program killed by signal `n`
    /// (Unix), as a shell's `$?`. `None` only from fakes.
```

Run: `cargo test --locked --lib`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/adapters/std_process src/adapters/std_spawn.rs src/ports/process.rs
git commit -S -m "fix(process): pass on 128+n for a program killed by a signal"
```

(`git add src/adapters/std_process` stages the moved file and the new
`tests.rs`; `git status` must also show the old path as deleted, staged by
the `git mv`.)

#### 4.4 Plan 6 Minor 9: `nvm-exec` resolves the version once

`src/commands/nvm_exec/mod.rs:97-111`, `switch`, runs `use_version::run` and
then `use_version::plan` again to learn the target: double I/O and a race if
an alias changes in between. `use_version` returns the target it applied.

- [ ] **Step 1: The failing test**

Create `src/commands/use_version/tests.rs`:

```rust
//! `run_and_target`: the switch `nvm use` made, for the callers that run a
//! program in it (`nvm-exec`).

use std::path::Path;

use super::{Target, run_and_target};
use crate::commands::Output;
use crate::commands::exec::tests::{BASE_PATH, N18, env_on, lab, npm_versions, strings};
use crate::context::Context;
use crate::error::NvmExitCode;

fn switch(args: &[&str]) -> (Output, Option<Target>) {
    let (fs, env, process) = (lab(), env_on(BASE_PATH), npm_versions());
    let context = Context::new(&fs, &env).with_process(&process);
    run_and_target(&context, &strings(args)).unwrap()
}

#[test]
fn a_switch_returns_the_target_it_applied() {
    let (output, target) = switch(&["18"]);
    assert_eq!(output.status, NvmExitCode::Success);
    match target {
        Some(Target::Installed { directory, .. }) => assert_eq!(directory, Path::new(N18)),
        other => panic!("not the installed v18: {other:?}"),
    }
}

#[test]
fn a_stop_before_the_switch_returns_no_target() {
    let (output, target) = switch(&["99"]);
    assert_ne!(output.status, NvmExitCode::Success);
    assert!(target.is_none());
}
```

and declare it at the end of `src/commands/use_version/mod.rs`:

```rust
#[cfg(test)]
mod tests;
```

Run: `cargo test --locked --lib commands::use_version::tests`
Expected: FAIL to compile (`run_and_target` not found).

- [ ] **Step 2: The code**

In `src/commands/use_version/mod.rs`, `run` becomes a wrapper:

```rust
/// `nvm use <args>`.
///
/// # Errors
/// [`CliError`] for options that cannot be parsed, or what the switch fails
/// with.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    run_and_target(context, args).map(|(output, _)| output)
}

/// [`run`], also returning the target it switched to (`None` when it stopped
/// before the switch), so that a caller never resolves the version twice.
///
/// # Errors
/// As [`run`].
pub fn run_and_target(
    context: &Context<'_>,
    args: &[String],
) -> Result<(Output, Option<Target>), CliError> {
    let options = options::parse(args)?;
    let mut transcript = Transcript::default();
    match target::resolve(context, &options, &mut transcript) {
        Ok(target) => {
            let output = apply::apply(context, &options, &target, transcript)?;
            Ok((output, Some(target)))
        }
        Err(halt) => Ok((transcript.finish(halt.status), None)),
    }
}
```

(Keep `run`'s existing doc comment if it has one; the text above is for a
missing one.) In `src/commands/nvm_exec/mod.rs`, `switch` becomes:

```rust
/// `nvm use <args>`: its stderr and, when it succeeded, the version it chose.
fn switch(context: &Context<'_>, args: &[String]) -> Result<Selection, CliError> {
    let (output, target) = use_version::run_and_target(context, args)?;
    let target = target.filter(|_| output.status == NvmExitCode::Success);
    Ok(Selection {
        stderr: output.stderr,
        target,
    })
}
```

and drop the imports that became unused (`Transcript` stays: `run` and
`from_nvmrc` use it).

Run: `cargo test --locked --lib commands::use_version commands::nvm_exec cli`
Expected: PASS (the `nvm_exec` tests prove the behaviour did not change).

- [ ] **Step 3: Commit**

```bash
git add src/commands/use_version/mod.rs src/commands/use_version/tests.rs \
  src/commands/nvm_exec/mod.rs
git commit -S -m "refactor(nvm-exec): resolve the version once"
```

#### 4.5 Plan 6 Minor 1: `nvm-exec` hands the process over (`exec`)

nvm's `nvm-exec` ends with `exec "$@"`. nvmrc's waits for the command as a
child (`src/cli/child.rs:14-34`, `src/adapters/std_spawn.rs:10-25`), so
Ctrl-C kills `nvm-exec` while a child that traps INT goes on orphaned. On
Unix `nvm-exec` now replaces itself after its messages are flushed; only the
error path returns (still 127 with `nvm-exec: <cmd>: not found`). `nvm exec`
and `nvm run` keep waiting for a child (in nvm.sh they also run `nvm-exec`
as a child of the shell function); `docs/deviations.md` notes it.

- [ ] **Step 1: The failing tests**

In `src/cli/tests/nvm_exec.rs`, replace
`the_command_runs_and_its_status_is_the_status` with:

```rust
#[test]
fn the_command_takes_the_process_over_and_its_status_is_the_status() {
    let process = FakeProcess::default().with_spawn("node", "a", 9);
    let (code, out, err) = nvm_exec_with(&["node", "a"], &process);
    assert_eq!((code, out.as_str(), err.as_str()), (9, "", ""));
    assert_eq!(process.handed_over().len(), 1);
    assert!(process.spawned().is_empty());
}
```

Append to `src/cli/tests/exec.rs`:

```rust
#[test]
fn exec_waits_for_its_child_instead_of_handing_over() {
    let process = FakeProcess::default().with_spawn("node", "a", 0);
    run_with(&["nvm", "exec", "18", "node", "a"], &process);
    assert_eq!((process.spawned().len(), process.handed_over().len()), (1, 0));
}
```

Append to `tests/nvm_exec_cli.rs` (add `Stdio` to its
`use std::process::{...}`):

```rust
/// Upstream `nvm-exec` ends with `exec "$@"`: the command takes the process
/// over, so it runs with the pid `nvm-exec` was started with.
#[test]
fn the_command_replaces_nvm_exec() {
    let fixture = Fixture::new();
    let child = fixture
        .command()
        .env("NODE_VERSION", "18")
        .args(["sh", "-c", "echo $$"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    let output = child.wait_with_output().unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), pid.to_string());
}
```

Run: `cargo test --locked --lib cli::tests 2>&1 | tail -5`
Expected: FAIL to compile (`handed_over` not found).

- [ ] **Step 2: The port and the fake**

In `src/ports/process.rs`, add to the `Process` trait, after `spawn`:

```rust
    /// Hands this process over to `invocation`, as a shell's `exec` does:
    /// where it can (Unix), the process becomes the program, so signals,
    /// the terminal and the exit status are the program's own, and this
    /// returns only when the program could not be started. Elsewhere it
    /// runs the program as [`Self::spawn`] does and returns its status.
    ///
    /// # Errors
    /// Fails when the program cannot be started.
    fn hand_over(&self, invocation: &Invocation) -> io::Result<i32> {
        self.spawn(invocation)
    }
```

In `src/fakes/process.rs`: a field `handed_over: RefCell<Vec<Invocation>>`;
`spawn`'s lookup moves into a private `fn answer(&self, invocation:
&Invocation) -> io::Result<i32>` shared by both; and:

```rust
    /// Every invocation `hand_over` was given, in order.
    #[must_use]
    pub fn handed_over(&self) -> Vec<Invocation> {
        self.handed_over.borrow().clone()
    }
```

```rust
    /// Records the hand-over and answers as [`Self::with_spawn`] says.
    fn hand_over(&self, invocation: &Invocation) -> io::Result<i32> {
        self.handed_over.borrow_mut().push(invocation.clone());
        self.answer(invocation)
    }
```

Update the doc of `with_spawn`: "The exit code `spawn` and `hand_over`
answer to ...".

- [ ] **Step 3: The real adapter**

In `src/adapters/std_spawn.rs`, split `spawn_inherited` so the command is
built once:

```rust
/// The command `invocation` describes, with stdio inherited.
fn command_for(invocation: &Invocation) -> io::Result<Command> {
    let mut command = Command::new(&invocation.program);
    for name in &invocation.env_remove {
        command.env_remove(name);
    }
    command
        .args(&invocation.args)
        .envs(invocation.env.iter().map(|(name, value)| (name, value)));
    if let Some(dir) = &invocation.dir {
        command.current_dir(dir);
    }
    if let Some(prefix) = &invocation.path_prefix {
        command.env("PATH", path_with_prefix(prefix, invocation)?);
    }
    without_channel(&mut command);
    Ok(command)
}

/// Runs `invocation` with inherited stdio and waits for it.
pub(super) fn spawn_inherited(invocation: &Invocation) -> io::Result<i32> {
    Ok(exit_code(command_for(invocation)?.status()?))
}

/// Replaces this process with `invocation`; returns only the error that
/// kept it from starting.
#[cfg(unix)]
pub(super) fn exec_replacing(invocation: &Invocation) -> io::Result<i32> {
    use std::os::unix::process::CommandExt;
    Err(command_for(invocation)?.exec())
}
```

In `src/adapters/std_process/mod.rs`, in `impl Process for StdProcess`:

```rust
    #[cfg(unix)]
    fn hand_over(&self, invocation: &Invocation) -> io::Result<i32> {
        super::std_spawn::exec_replacing(invocation)
    }
```

- [ ] **Step 4: The CLI**

In `src/cli/child.rs`, replace the `tool: &str` parameter by a launcher:

```rust
/// Who starts the program a command leaves to run, and how.
#[derive(Debug, Clone, Copy)]
pub(super) struct Launcher {
    /// The name its messages start with.
    pub tool: &'static str,
    /// Replace this process (`exec`) instead of waiting for a child.
    pub hand_over: bool,
}

/// `nvm exec` and `nvm run`: a child, waited for.
pub(super) const NVM: Launcher = Launcher { tool: "nvm", hand_over: false };
/// `nvm-exec`: `exec "$@"`, as the upstream script ends.
pub(super) const NVM_EXEC: Launcher = Launcher { tool: "nvm-exec", hand_over: true };
```

`run_child(launcher: Launcher, context, invocation, err)` starts the program
with:

```rust
    let started = if launcher.hand_over {
        context.process().hand_over(invocation)
    } else {
        context.process().spawn(invocation)
    };
    match started {
```

and uses `launcher.tool` in the two messages. In `src/cli/mod.rs`, `finish`
takes `launcher: child::Launcher` instead of `tool: &str` and passes it on;
`run` calls `finish(child::NVM, ...)` and `run_nvm_exec` calls
`finish(child::NVM_EXEC, ...)`. Update `run_child`'s doc comment ("prefixed
by `launcher.tool`").

- [ ] **Step 5: Run the tests**

Run: `cargo test --locked`
Expected: PASS, `the_command_replaces_nvm_exec` included.

- [ ] **Step 6: Commit**

```bash
git add src/ports/process.rs src/fakes/process.rs src/adapters/std_spawn.rs \
  src/adapters/std_process/mod.rs src/cli/child.rs src/cli/mod.rs \
  src/cli/tests/nvm_exec.rs src/cli/tests/exec.rs tests/nvm_exec_cli.rs
git commit -S -m "feat(nvm-exec): replace the process with the command, as exec does"
```

#### 4.6 Plan 8 M4 (rest): undo never restores a cut-short backup

The fix wave made backups `create_new`, synced and `-N` suffixed. Left:

- `src/commands/migrate/undo.rs:68-92`: an empty backup (a crash between
  creating and syncing it) is restored over the profile. Fix: refuse it with
  a message and status 1.
- `src/domain/migration/backup.rs:20-34`: the newest backup is the greatest
  timestamp, so one dated in the future by clock skew always wins. Fix: a
  backup dated after now is only chosen when no other exists.
- `src/adapters/std_fs/atomic.rs:13-29`: the directory is not synced after
  the rename. Fix: sync it, best effort (no observable behaviour to test; the
  existing `atomic_tests` keep passing).
- [ ] **Step 1: The failing tests**

Append to `src/commands/migrate/undo_tests.rs`:

```rust
#[test]
fn an_empty_backup_is_not_restored_over_a_file_with_content() {
    let fs = FakeFileSystem::default()
        .with_file(BASHRC, INSTALL_SH_MIGRATED)
        .with_file(NEWER, "");
    let output = migrate(&fs, &["--undo", "--yes"]);
    assert_eq!(output.status, NvmExitCode::Failure);
    assert_eq!(
        output.stderr,
        format!("nvm migrate: {NEWER}: the backup is empty; {BASHRC} was not restored")
    );
    assert_eq!(read(&fs, BASHRC), INSTALL_SH_MIGRATED);
}
```

In `src/domain/migration/backup_tests.rs`, add after the `use` line:

```rust
/// 2026-10-04 10:34:00 UTC, the time of every lookup below.
const NOW: i64 = 1_791_110_040;
```

pass `NOW` as the new third argument of every existing `latest_backup(...)`
call (lines 46, 50, 53, 60, 61, 74, 78), and append:

```rust
#[test]
fn a_backup_dated_after_now_only_wins_when_it_is_the_only_one() {
    let future = ".bashrc.nvmrc-backup-20991231T000000Z".to_owned();
    let past = ".bashrc.nvmrc-backup-20261001T000000Z".to_owned();
    let both = [future.clone(), past.clone()];
    assert_eq!(latest_backup(".bashrc", &both, NOW), Some(past.as_str()));
    let alone = [future.clone()];
    assert_eq!(latest_backup(".bashrc", &alone, NOW), Some(future.as_str()));
}
```

Run: `cargo test --locked --lib migrat`
Expected: FAIL to compile (`latest_backup` takes two arguments).

- [ ] **Step 2: The code**

`src/domain/migration/backup.rs`:

```rust
/// The newest backup of `file_name` among the file names `candidates`: the
/// greatest timestamp, then the greatest sequence number, among the names of
/// exactly the [`backup_name`] pattern (other files, malformed timestamps or
/// sequence numbers are ignored). A backup dated after `now` (clock skew)
/// loses to every other one.
#[must_use]
pub fn latest_backup<'a>(file_name: &str, candidates: &'a [String], now: i64) -> Option<&'a str> {
    let present = compact_utc(now);
    candidates
        .iter()
        .filter_map(|candidate| {
            let rest = candidate.strip_prefix(file_name)?;
            let (stamp, sequence) = order_key(rest.strip_prefix(BACKUP_INFIX)?)?;
            Some(((stamp <= present.as_str(), stamp, sequence), candidate))
        })
        .max()
        .map(|(_, candidate)| candidate.as_str())
}
```

`src/commands/migrate/undo.rs`: `latest` passes the clock:

```rust
    latest_backup(&name, &names, context.clock().unix_seconds())
        .map(|backup| directory.join(backup))
```

and `restores` delegates each pair to a new function, so both stay short:

```rust
fn restores(
    context: &Context<'_>,
    backups: Vec<(PathBuf, PathBuf)>,
    transcript: &mut Transcript,
) -> (Vec<Restore>, bool) {
    let mut found = Vec::new();
    let mut failed = false;
    for (path, backup) in backups {
        match restore_of(context, path, backup) {
            Ok(Some(restore)) => found.push(restore),
            Ok(None) => {}
            Err(message) => {
                transcript.err(message);
                failed = true;
            }
        }
    }
    (found, failed)
}

/// The restore of `path` from `backup`; `None` when both hold the same text.
/// An error message when either cannot be read, or when the backup is empty
/// and the file is not (a backup cut short by a crash).
fn restore_of(
    context: &Context<'_>,
    path: PathBuf,
    backup: PathBuf,
) -> Result<Option<Restore>, String> {
    let read = |file: &Path| context.fs.read_to_string(file);
    let failure = |error: std::io::Error| format!("nvm migrate: {}: {error}", path.display());
    let old = read(&path).map_err(failure)?;
    let new = read(&backup).map_err(failure)?;
    if old == new {
        return Ok(None);
    }
    if new.is_empty() {
        return Err(format!(
            "nvm migrate: {}: the backup is empty; {} was not restored",
            backup.display(),
            path.display()
        ));
    }
    Ok(Some(Restore {
        change: Change { path, old, new },
        backup,
    }))
}
```

(`failure` is used twice; if the borrow checker objects to the closure
capturing `path` while `path` is moved later, compute
`let shown = path.display().to_string();` first and capture `shown`.)

`src/adapters/std_fs/atomic.rs`: after a successful rename, sync the
directory:

```rust
    let result = fill(file, contents, permissions)
        .and_then(|()| verify(&temporary))
        .and_then(|()| fs::rename(&temporary, &target));
    if result.is_err() {
        // The rename did not happen, so the temporary file is still there.
        let _ = fs::remove_file(&temporary);
    } else {
        sync_directory(&target);
    }
    result
```

```rust
/// Makes the rename itself durable by syncing the directory entry. Best
/// effort: the new contents are already in place and synced.
fn sync_directory(target: &Path) {
    #[cfg(unix)]
    if let Some(directory) = target.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        let _ = File::open(directory).and_then(|handle| handle.sync_all());
    }
}
```

Run: `cargo test --locked --lib`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/commands/migrate/undo.rs src/commands/migrate/undo_tests.rs \
  src/domain/migration/backup.rs src/domain/migration/backup_tests.rs \
  src/adapters/std_fs/atomic.rs
git commit -S -m "fix(migrate): never restore an empty backup, distrust future-dated ones, sync the directory"
```

#### 4.7 Plan 8 M6: name an unfollowable link right and report a looping root

- `src/adapters/std_fs/atomic.rs:44-47`, `dangling`, labels every
  `canonicalize` error "dangling symbolic link", a loop (ELOOP) or a denied
  directory (EACCES) included. Fix: "dangling" only for `NotFound`.
- `src/commands/conflict/roots.rs:37`: a profile that is a link but not a
  file (a self-loop, a dangling stow link) is skipped silently, and
  `nvm doctor` says "no shell profile files found", exit 0. Fix: it is a
  root; the scanner then reports it as not read (doctor exit 1).
- [ ] **Step 1: The failing tests**

Append to `src/adapters/std_fs/atomic_tests.rs`:

```rust
#[test]
fn a_looping_link_is_not_called_dangling() {
    let root = tempfile::tempdir().unwrap();
    let link = root.path().join(".bashrc");
    std::os::unix::fs::symlink(&link, &link).unwrap();
    let message = replace_file(&link, "x\n", &accept).unwrap_err().to_string();
    assert!(message.contains("symbolic link that cannot be followed"), "{message}");
    assert!(!message.contains("dangling"), "{message}");
}
```

Append to `src/commands/conflict/roots_tests.rs`:

```rust
#[test]
fn a_link_that_leads_nowhere_is_still_a_root() {
    let fs = FakeFileSystem::default();
    link(&fs, "/Users/u/.bashrc", "/Users/u/.bashrc");
    let found = roots_of(&fs, &home_env(), Some(Shell::Bash));
    assert_eq!(found, paths(&["/Users/u/.bashrc"]));
}
```

Append to `src/commands/doctor/tests.rs`:

```rust
#[test]
fn a_looping_profile_link_is_reported_not_skipped() {
    let fs = FakeFileSystem::default();
    link(&fs, "/Users/u/.bashrc", "/Users/u/.bashrc");
    let output = report(&fs, &home_env(), &[]);
    assert!(output.stdout.contains("/Users/u/.bashrc: not read:"), "{}", output.stdout);
    assert_eq!(output.status, NvmExitCode::Failure);
}
```

Run: `cargo test --locked --lib looping a_link_that_leads_nowhere`
Expected: the three FAIL.

- [ ] **Step 2: The code**

`atomic.rs`: rename `dangling` to `unfollowable` (both call sites):

```rust
/// The error for a link at `link` that `canonicalize` could not follow:
/// "dangling" only when its target is missing (a loop, ELOOP, or a denied
/// directory is not).
fn unfollowable(link: &Path, error: &io::Error) -> io::Error {
    let what = if error.kind() == io::ErrorKind::NotFound {
        "dangling symbolic link"
    } else {
        "symbolic link that cannot be followed"
    };
    io::Error::new(error.kind(), format!("{}: {what} ({error})", link.display()))
}
```

`roots.rs`, in `roots`:

```rust
        // A link that cannot be followed (a loop, a dangling stow link) is
        // kept: the scan reports it as not read instead of skipping it.
        let present = context.fs.is_file(&candidate) || context.fs.read_link(&candidate).is_ok();
        if !found.contains(&candidate) && present {
            found.push(candidate);
        }
```

and add to its doc comment: "A symbolic link counts even when it cannot be
followed."

Run: `cargo test --locked --lib`
Expected: PASS. If a migrate test now sees an extra root, it is a fixture
with a dangling link: check that the new "not read" outcome is right for it
and update the expectation, naming it in the commit body.

- [ ] **Step 3: Commit**

```bash
git add src/adapters/std_fs/atomic.rs src/adapters/std_fs/atomic_tests.rs \
  src/commands/conflict/roots.rs src/commands/conflict/roots_tests.rs \
  src/commands/doctor/tests.rs
git commit -S -m "fix(doctor): report a profile link that cannot be followed, and call only a missing target dangling"
```

#### 4.8 Plan 8 M7: show what the syntax checker said, and check the original first

`src/adapters/std_process.rs:69-90` (now `std_process/mod.rs`), `run`, sends
stderr to `/dev/null`, so `nvm migrate` cannot say which line the checker
refused (`src/commands/migrate/check.rs:31-50`). And the original is never
checked, so an already broken profile (or one `sh -n` rejects on dash) is
reported as "the edited file would not pass" forever. Fix: `run` keeps
stderr (capped like stdout); `SyntaxCheck` keeps the first three non-blank
lines; `apply` checks the original first and refuses it with its own
message.

- [ ] **Step 1: The failing tests**

Append to `src/adapters/std_process/tests.rs`:

```rust
#[test]
fn run_keeps_what_the_program_said_on_stderr() {
    let output = shell(&StdProcess::default(), "echo oops >&2; exit 2").unwrap();
    assert_eq!((output.success, output.stderr.as_str()), (false, "oops\n"));
}
```

Append to `src/commands/migrate/apply_tests.rs`:

```rust
#[test]
fn a_file_already_broken_is_left_alone_and_said_so() {
    let fs = FakeFileSystem::default().with_file(BASHRC, INSTALL_SH);
    let complaint = "x: line 9: syntax error: unexpected end of file\n";
    let process = checkers().with_failure_saying("bash", complaint);
    let output = run_with(&fs, &process);
    assert_eq!(output.status, NvmExitCode::Failure);
    assert_eq!(
        output.stderr,
        format!(
            "nvm migrate: {BASHRC}: the file does not pass `bash -n` before the edit; \
left unchanged:\n  x: line 9: syntax error: unexpected end of file"
        )
    );
    assert_eq!(read(&fs, BASHRC), INSTALL_SH);
    assert!(fs.replaced().is_empty());
}

#[test]
fn a_refused_edit_shows_what_the_checker_said() {
    let fs = FakeFileSystem::default().with_file(BASHRC, INSTALL_SH);
    let process = checkers()
        .with_failure_saying("bash", "t: line 3: syntax error near `fi'\n")
        .with_run("bash", &format!("-n {BASHRC}"), true, "");
    let output = run_with(&fs, &process);
    assert_eq!(
        output.stderr,
        format!(
            "nvm migrate: {BASHRC}: the edited file would not pass `bash -n`; \
left unchanged:\n  t: line 3: syntax error near `fi'"
        )
    );
}
```

The two existing tests that make the checker refuse the candidate must now
let the original pass. In
`apply_tests.rs::a_file_that_fails_its_check_is_refused_and_the_others_are_migrated`
and `write_tests.rs::a_file_refused_by_its_check_leaves_no_backup`, the
process becomes:

```rust
    let process = checkers()
        .with_failure("bash")
        .with_run("bash", &format!("-n {BASHRC}"), true, "");
```

(their expectations stay; the fake prints nothing on stderr, so no
complaint is appended).

Run: `cargo test --locked --lib 2>&1 | tail -5`
Expected: FAIL to compile (`stderr` field and `with_failure_saying` missing).

- [ ] **Step 2: The port, the fake and the adapter**

`src/ports/process.rs`, `ProcessOutput` gains:

```rust
    /// What the program wrote on stderr, capped like `stdout`.
    pub stderr: String,
```

`src/fakes/process.rs`: `with_output`, `with_failure` and `with_run` set
`stderr: String::new()`, and:

```rust
    /// `run` of `program`, whatever its arguments, fails after printing
    /// `stderr`; [`Self::with_run`] still takes precedence.
    #[must_use]
    pub fn with_failure_saying(mut self, program: &str, stderr: &str) -> Self {
        let output = ProcessOutput {
            success: false,
            stdout: String::new(),
            stderr: stderr.to_owned(),
        };
        self.outputs.insert(PathBuf::from(program), output);
        self
    }
```

`src/adapters/std_process/mod.rs`: `read_in_background` takes any pipe
(`fn read_in_background<R: Read + Send + 'static>(pipe: R, cap: usize)`),
and `run` reads both streams:

```rust
    fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput> {
        let deadline = Instant::now() + self.timeout;
        let mut child = without_channel(&mut Command::new(program))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdout = read_in_background(pipe(child.stdout.take())?, self.output_cap);
        let stderr = read_in_background(pipe(child.stderr.take())?, self.output_cap);
        let status = wait_until(&mut child, deadline)?;
        let stdout = receive_until(&stdout, deadline)??;
        let stderr = receive_until(&stderr, deadline)??;
        Ok(ProcessOutput {
            success: status.success(),
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        })
    }
```

```rust
fn pipe<R>(pipe: Option<R>) -> io::Result<R> {
    pipe.ok_or_else(|| io::Error::other("no pipe"))
}
```

Update `StdProcess`'s doc comment: "stdin is closed, at most `output_cap`
bytes of stdout and of stderr are kept, ...". Remove the `ChildStdout` import
if it became unused.

- [ ] **Step 3: The checker and the report**

`src/commands/migrate/check.rs` (complete new version):

```rust
//! The syntax check a candidate must pass before it replaces a startup file.

use std::cell::{Cell, RefCell};
use std::io;
use std::path::Path;

use crate::domain::migration::syntax_check;
use crate::ports::Process;

/// At most this many lines of what the checker said are shown.
const SHOWN_LINES: usize = 3;

/// The syntax checker of one file, run on the original and on the candidate,
/// remembering what happened.
pub struct SyntaxCheck<'a> {
    process: &'a dyn Process,
    command: Option<(&'static str, Vec<&'static str>)>,
    missing: Cell<bool>,
    rejected: Cell<bool>,
    /// The first lines the checker printed when it last refused a file,
    /// each indented by two spaces.
    said: RefCell<String>,
}

impl<'a> SyntaxCheck<'a> {
    pub fn new(process: &'a dyn Process, path: &Path) -> Self {
        Self {
            process,
            command: syntax_check(&path.display().to_string()),
            missing: Cell::new(false),
            rejected: Cell::new(false),
            said: RefCell::new(String::new()),
        }
    }

    /// Whether the file as it is passes. Only a refusal by the checker
    /// counts against it: a checker that is missing or could not run does
    /// not, so the candidate's check decides.
    pub fn original_passes(&self, original: &Path) -> bool {
        self.check(original).is_ok() || !self.rejected.get()
    }

    /// Runs the checker on `temporary`: a failure or a non-success status is
    /// an error; a checker that is not installed is skipped.
    pub fn verify(&self, temporary: &Path) -> io::Result<()> {
        self.check(temporary)
    }

    fn check(&self, file: &Path) -> io::Result<()> {
        let Some((program, flags)) = &self.command else {
            return Ok(());
        };
        let file = file.to_string_lossy();
        let mut arguments = flags.clone();
        arguments.push(&file);
        match self.process.run(Path::new(program), &arguments) {
            Ok(output) if output.success => Ok(()),
            Ok(output) => {
                self.rejected.set(true);
                *self.said.borrow_mut() = first_lines(&output.stderr);
                Err(io::Error::other("the syntax check failed"))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.missing.set(true);
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// Whether the checker refused the candidate.
    pub fn rejected(&self) -> bool {
        self.rejected.get()
    }

    /// `:` and what the checker said, on the next lines; nothing when it
    /// said nothing.
    pub fn complaint(&self) -> String {
        let said = self.said.borrow();
        if said.is_empty() {
            String::new()
        } else {
            format!(":\n{said}")
        }
    }

    /// The checker's program, when it was not found.
    pub fn missing(&self) -> Option<&'static str> {
        self.command
            .as_ref()
            .filter(|_| self.missing.get())
            .map(|(program, _)| *program)
    }

    /// `bash -n`, as the messages show it.
    pub fn command_line(&self) -> String {
        self.command
            .as_ref()
            .map(|(program, flags)| format!("{program} {}", flags.join(" ")))
            .unwrap_or_default()
    }
}

/// The first [`SHOWN_LINES`] non-blank lines of `stderr`, indented.
fn first_lines(stderr: &str) -> String {
    stderr
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(SHOWN_LINES)
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}
```

`src/commands/migrate/apply.rs`, `apply` checks the original first:

```rust
fn apply(context: &Context<'_>, change: &Change, transcript: &mut Transcript) -> bool {
    let check = SyntaxCheck::new(context.process(), &change.path);
    if !check.original_passes(&change.path) {
        transcript.err(format!(
            "nvm migrate: {}: the file does not pass `{}` before the edit; left unchanged{}",
            change.path.display(),
            check.command_line(),
            check.complaint()
        ));
        return false;
    }
    let written = replace_with_backup(context, change, &|temporary| check.verify(temporary));
    report(change, &check, &written, transcript);
    written.is_ok()
}
```

and the refusal arm of `report` appends the complaint:

```rust
        Err(Refusal::Failed(_)) if check.rejected() => transcript.err(format!(
            "nvm migrate: {path}: the edited file would not pass `{}`; left unchanged{}",
            check.command_line(),
            check.complaint()
        )),
```

Run: `cargo test --locked`
Expected: PASS (`tests/migrate_cli.rs` runs the real checkers on valid
files, so the original check passes there).

- [ ] **Step 4: Commit**

```bash
git add src/ports/process.rs src/fakes/process.rs src/adapters/std_process/mod.rs \
  src/adapters/std_process/tests.rs src/commands/migrate/check.rs \
  src/commands/migrate/apply.rs src/commands/migrate/apply_tests.rs \
  src/commands/migrate/write_tests.rs
git commit -S -m "fix(migrate): check the original first and show what the syntax checker said"
```

#### 4.9 Plan 8 M8: wrapper commands before a loader

`zsh-defer source ~/.nvm/nvm.sh` (romkatv's zsh-defer, common in zsh setups)
and `time . ~/.nvm/nvm.sh` are not seen: `source_path.rs:21-23`
(`COMMAND_KEYWORDS`) only lets reserved words, `builtin` and `command` stand
before the command word. Fix: add `zsh-defer` and `time`. `standalone.rs`
keeps its own list, so such a line is a manual `CompoundLoader` finding,
never edited. `eval "$(cat .../nvm.sh)"` stays a false negative
(`docs/deviations.md`).

- [ ] **Step 1: The failing tests**

Append to `src/domain/conflict/source_path_tests.rs`:

```rust
#[test]
fn a_wrapper_command_may_stand_before_the_loader() {
    assert_eq!(
        source_target("zsh-defer source ~/.nvm/nvm.sh").as_deref(),
        Some("~/.nvm/nvm.sh")
    );
    assert_eq!(
        source_target("time . ~/.nvm/nvm.sh").as_deref(),
        Some("~/.nvm/nvm.sh")
    );
}
```

Append to `src/domain/conflict/tests.rs`:

```rust
#[test]
fn a_loader_behind_a_wrapper_command_is_a_manual_finding() {
    let text = "zsh-defer source ~/.nvm/nvm.sh\ntime . ~/.nvm/nvm.sh\n";
    assert_eq!(found(text), [(1, CompoundLoader), (2, CompoundLoader)]);
}
```

Run: `cargo test --locked --lib domain::conflict`
Expected: both FAIL (`None`, `[]`).

- [ ] **Step 2: The code**

In `src/domain/conflict/source_path.rs`:

```rust
/// The reserved words and prefixes a command may follow; `zsh-defer` and
/// `time` run the command after them.
const COMMAND_KEYWORDS: [&str; 13] = [
    "then", "do", "else", "if", "elif", "while", "until", "{", "!", "builtin", "command",
    "zsh-defer", "time",
];
```

Run:

```sh
cargo test --locked --lib domain::conflict commands::doctor commands::migrate
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/domain/conflict/source_path.rs src/domain/conflict/source_path_tests.rs \
  src/domain/conflict/tests.rs
git commit -S -m "fix(conflict): see a loader behind zsh-defer or time"
```

- [ ] **Step 4: The task gate**

Run:

```sh
cargo fmt --all --check && cargo clippy --all-targets --all-features --locked -- -D warnings && cargo test --locked && mise run test:msrv
```

Expected: all green; every touched file under 300 lines
(`wc -l` on the files of this task).

---

### Task 5: The compatibility contract

One test crate, `tests/compat_cli.rs`, whose modules under `tests/compat/`
are the harness and eight scenario tables. Every scenario runs in a real
shell, from a temporary tree, through the `nvm` function
(`eval "$(nvm init <shell> --no-use)"`), exactly as the oracle ran nvm.sh.
The task ends with the spec corrections.

**Files:**

- Create: `tests/compat_cli.rs`, `tests/compat/mod.rs`,
  `tests/compat/world.rs`, `tests/compat/alias.rs`, `tests/compat/codes.rs`,
  `tests/compat/exec.rs`, `tests/compat/init.rs`, `tests/compat/install.rs`,
  `tests/compat/ls.rs`, `tests/compat/nvmrc.rs`,
  `tests/compat/use_version.rs`
- Modify: `docs/superpowers/specs/2026-10-02-nvmrc-design.md`

**Interfaces:**

- Consumes: `tests/common/mod.rs` (`serve`, `NODE_INDEX`, `IOJS_INDEX`),
  `tests/init_lab/mod.rs` (`find_shell`), the three binaries
  (`CARGO_BIN_EXE_nvm`, `_nvmrc`, `_nvm-exec`), every fix of Tasks 3 and 4.
- Produces: `cargo test --locked --test compat_cli` (run by `cargo test` and
  CI) and the ignored `capture_from_nvm_sh` tests (run by
  `mise run compat:capture`).

**The tree of a scenario** (`<W>` in every expectation; `fs::canonicalize`d,
so macOS's `/var` → `/private/var` link does not leak into outputs):

- `<W>/nvm`: `$NVM_DIR`, also exported as `$D`;
- `<W>/home`: `$HOME`;
- `<W>/proj`: the working directory and `$PWD`;
- `<W>/sys`: `$SYS`, a system node when the fixture asks for one (not on
  `PATH` until the script puts it there, as `PATH="$SYS:$PATH"` or
  `PATH="$D/../sys:$PATH"`);
- `<W>/bin`: first on `PATH` (`<W>/bin:/usr/bin:/bin`); for nvmrc it holds
  `nvm`, `nvmrc` and `nvm-exec` (links to the built binaries); for the
  oracle it is empty, and `$NVM_DIR` holds links to the checkout's `nvm.sh`
  and `nvm-exec` (nvm.sh's `nvm exec` runs `$NVM_DIR/nvm-exec`, which sources
  the `nvm.sh` beside it);
- `$NVM_EXEC`: the `nvm-exec` to call by path (`<W>/bin/nvm-exec` or
  `<W>/nvm/nvm-exec`);
- the environment is empty but `HOME`, `TERM=dumb`, `PATH`, `PWD`,
  `NVM_DIR`, `D`, `SYS`, `NVM_EXEC`, `NVM_NODEJS_ORG_MIRROR` and
  `NVM_IOJS_ORG_MIRROR` (both `http://127.0.0.1:9`, a refused port, unless
  the fixture has `Mirror`: then two local servers with `common::NODE_INDEX`
  (v20.10.0 Iron, v18.19.0 Hydrogen) and the empty `common::IOJS_INDEX`).

A script that contains `@load@` or `@load-no-use@` loads `nvm` itself
(`eval "$(nvm init <shell>)"`, and the same with `--no-use` for the
second; the oracle gets `. '<nvm.sh>'` and `. '<nvm.sh>' --no-use`); any
other script is run after `@load-no-use@`.

#### 5.1 The harness

- [ ] **Step 1: The crate**

Create `tests/compat_cli.rs`:

```rust
//! The compatibility contract with nvm.sh: see `tests/compat/mod.rs`.
#![cfg(unix)]

mod common;
mod compat;
mod init_lab;
```

- [ ] **Step 2: `tests/compat/mod.rs`**

```rust
//! The compatibility contract (spec 8.3): scenarios of nvm's own test suite
//! (`test/fast`, `test/sourcing`) re-expressed as commands of the `nvm`
//! function, each run in a real shell over a temporary tree (see
//! `world.rs`), with what `nvm.sh` printed for it. stdout, stderr and the
//! status are compared separately, the temporary root replaced by `<W>`.
//!
//! A scenario with a deviation (`D1` .. `D16`, `docs/deviations.md`) pins
//! nvmrc's own output instead of nvm.sh's.
//!
//! With `NVMRC_ORACLE_NVM_SH=/path/to/nvm.sh`, the ignored
//! `capture_from_nvm_sh` tests (`mise run compat:capture`) run the same
//! scenarios through `nvm.sh` and print, as literals to paste, every outcome
//! that differs from its table. Nothing is written. bash is the reference:
//! nvm.sh ignores `--no-use` when sourced by dash or sh and does not support
//! ksh, so those captures are informative only.

mod alias;
mod codes;
mod exec;
mod init;
mod install;
mod ls;
mod nvmrc;
mod use_version;
mod world;

use std::path::Path;

pub use world::Fixture;
use world::{Implementation, World};

use crate::init_lab::find_shell;

/// The shell nvm's own suite runs in.
pub const BASH: &[&str] = &["bash"];
/// Every POSIX shell `nvm init` supports (fish has its own syntax).
pub const POSIX: &[&str] = &["bash", "zsh", "sh", "dash", "ksh"];

/// One upstream test, re-expressed; built with [`Scenario::new`] and the
/// chained setters, which default to bash, no fixture, no output, status 0.
pub struct Scenario {
    name: &'static str,
    upstream: &'static str,
    fixture: &'static [Fixture],
    shells: &'static [&'static str],
    script: &'static str,
    stdout: &'static str,
    stderr: &'static str,
    status: i32,
    deviation: Option<&'static str>,
}

impl Scenario {
    /// `name` names it in reports; `upstream` is the file under nvm's
    /// `test/` it comes from (or where nvm.sh produces the behaviour).
    #[must_use]
    pub const fn new(name: &'static str, upstream: &'static str) -> Self {
        Self {
            name,
            upstream,
            fixture: &[],
            shells: BASH,
            script: "",
            stdout: "",
            stderr: "",
            status: 0,
            deviation: None,
        }
    }

    #[must_use]
    pub const fn fixture(mut self, fixture: &'static [Fixture]) -> Self {
        self.fixture = fixture;
        self
    }

    #[must_use]
    pub const fn shells(mut self, shells: &'static [&'static str]) -> Self {
        self.shells = shells;
        self
    }

    #[must_use]
    pub const fn script(mut self, script: &'static str) -> Self {
        self.script = script;
        self
    }

    #[must_use]
    pub const fn stdout(mut self, stdout: &'static str) -> Self {
        self.stdout = stdout;
        self
    }

    #[must_use]
    pub const fn stderr(mut self, stderr: &'static str) -> Self {
        self.stderr = stderr;
        self
    }

    #[must_use]
    pub const fn status(mut self, status: i32) -> Self {
        self.status = status;
        self
    }

    /// The expectation is nvmrc's own output: deviation `id`.
    #[must_use]
    pub const fn deviation(mut self, id: &'static str) -> Self {
        self.deviation = Some(id);
        self
    }

    fn expected(&self) -> Outcome {
        Outcome {
            stdout: self.stdout.to_owned(),
            stderr: self.stderr.to_owned(),
            status: self.status,
        }
    }
}

/// What a shell printed, the root replaced by `<W>`, and its status.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    stdout: String,
    stderr: String,
    status: i32,
}

/// Runs every scenario in each of its shells through nvmrc and fails with
/// every mismatch at once; a shell that is not installed is skipped.
pub fn check_all(scenarios: &[Scenario]) {
    assert_no_system_node();
    let failures: Vec<String> = scenarios
        .iter()
        .flat_map(|scenario| scenario.shells.iter().map(move |shell| (scenario, *shell)))
        .filter_map(|(scenario, shell)| {
            let outcome = run(scenario, shell, &Implementation::Rust)?;
            (outcome != scenario.expected()).then(|| report(scenario, shell, &outcome))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Runs every scenario through the `nvm.sh` that `NVMRC_ORACLE_NVM_SH`
/// names and prints how each outcome compares with its table.
pub fn capture_all(scenarios: &[Scenario]) {
    let nvm_sh = std::env::var_os("NVMRC_ORACLE_NVM_SH")
        .expect("set NVMRC_ORACLE_NVM_SH to the nvm.sh to capture from");
    let oracle = Implementation::Oracle(Path::new(&nvm_sh));
    for scenario in scenarios {
        for shell in scenario.shells {
            if let Some(outcome) = run(scenario, shell, &oracle) {
                print_capture(scenario, shell, &outcome);
            }
        }
    }
}

fn print_capture(scenario: &Scenario, shell: &str, outcome: &Outcome) {
    let label = format!("{} [{shell}]", scenario.name);
    let same = outcome == &scenario.expected();
    match (same, scenario.deviation) {
        (true, None) => println!("// {label}: same as the table"),
        (true, Some(id)) => println!("// {label}: nvm.sh now prints the table: is {id} gone?"),
        (false, deviation) => println!(
            "// {label}: {}\n.stdout({:?})\n.stderr({:?})\n.status({})",
            deviation.map_or("DIFFERS".to_owned(), |id| format!("differs, as {id} says")),
            outcome.stdout,
            outcome.stderr,
            outcome.status
        ),
    }
}

/// Runs `scenario` in `shell`; `None` when the shell is not installed.
fn run(scenario: &Scenario, shell: &str, implementation: &Implementation<'_>) -> Option<Outcome> {
    let Some(program) = find_shell(shell) else {
        eprintln!("skipped {} in {shell}: not installed", scenario.name);
        return None;
    };
    let world = World::new(scenario.fixture, implementation);
    let output = world
        .command(&program, shell, scenario.script, implementation)
        .output()
        .expect("run the shell");
    let root = world.root().display().to_string();
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).replace(&root, "<W>");
    Some(Outcome {
        stdout: text(&output.stdout),
        stderr: text(&output.stderr),
        status: output.status.code().unwrap_or(-1),
    })
}

fn report(scenario: &Scenario, shell: &str, outcome: &Outcome) -> String {
    let deviation = scenario
        .deviation
        .map_or(String::new(), |id| format!(", deviation {id}"));
    format!(
        "{} [{shell}] (nvm test: {}{deviation})\n  expected {:?}\n  actual   {:?}",
        scenario.name,
        scenario.upstream,
        scenario.expected(),
        outcome
    )
}

/// The scenarios assume no system node unless they add one: a `node` in
/// `/usr/bin` or `/bin`, which stay on `PATH`, would break that.
fn assert_no_system_node() {
    for directory in ["/usr/bin", "/bin"] {
        let node = Path::new(directory).join("node");
        assert!(!node.exists(), "the contract needs a machine without {}", node.display());
    }
}
```

- [ ] **Step 3: `tests/compat/world.rs`**

```rust
//! The temporary tree a scenario runs in, and the shell command that runs it.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::common::{IOJS_INDEX, NODE_INDEX, serve};

const LOAD: &str = "@load@";
const LOAD_NO_USE: &str = "@load-no-use@";
/// A port nothing listens on: every download is refused.
const REFUSED: &str = "http://127.0.0.1:9";

/// What a scenario needs before it runs.
#[derive(Debug, Clone, Copy)]
pub enum Fixture {
    /// `$NVM_DIR/versions/node/<v>/bin/node`, printing `<v>`.
    Node(&'static str),
    /// `$NVM_DIR/<v>/bin/node`: where nvm's tests put v0.x versions.
    Legacy(&'static str),
    /// `$NVM_DIR/versions/io.js/<v>/bin/{node,iojs}`, printing `<v>`.
    Iojs(&'static str),
    /// `$NVM_DIR/alias/<name>`, holding the target and a newline.
    Alias(&'static str, &'static str),
    /// A file under the root (`proj/.nvmrc`, `home/.npmrc`), as given.
    File(&'static str, &'static str),
    /// `<W>/sys/node`, printing the version.
    SystemNode(&'static str),
    /// Both mirrors are served locally.
    Mirror,
}

/// Which `nvm` runs a scenario.
pub enum Implementation<'a> {
    Rust,
    /// The `nvm.sh` at this path, sourced.
    Oracle(&'a Path),
}

pub struct World {
    _directory: tempfile::TempDir,
    root: PathBuf,
    /// The node and the io.js mirror.
    mirrors: (String, String),
}

impl World {
    pub fn new(fixtures: &[Fixture], implementation: &Implementation<'_>) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(directory.path()).unwrap();
        for name in ["bin", "home", "proj", "sys", "nvm/alias"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        let refused = (REFUSED.to_owned(), REFUSED.to_owned());
        let mut world = Self {
            _directory: directory,
            root,
            mirrors: refused,
        };
        world.link_programs(implementation);
        fixtures.iter().for_each(|fixture| world.add(*fixture));
        world
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn add(&mut self, fixture: Fixture) {
        match fixture {
            Fixture::Node(version) => {
                self.programs(&format!("nvm/versions/node/{version}/bin"), version, &["node"]);
            }
            Fixture::Legacy(version) => self.programs(&format!("nvm/{version}/bin"), version, &["node"]),
            Fixture::Iojs(version) => {
                let bin = format!("nvm/versions/io.js/{version}/bin");
                self.programs(&bin, version, &["node", "iojs"]);
            }
            Fixture::SystemNode(version) => self.programs("sys", version, &["node"]),
            Fixture::Alias(name, target) => self.file(&format!("nvm/alias/{name}"), &format!("{target}\n")),
            Fixture::File(relative, text) => self.file(relative, text),
            Fixture::Mirror => self.mirrors = (index(NODE_INDEX), index(IOJS_INDEX)),
        }
    }

    /// Executables `names` in `relative`, each printing `version`.
    fn programs(&self, relative: &str, version: &str, names: &[&str]) {
        let directory = self.root.join(relative);
        fs::create_dir_all(&directory).unwrap();
        for name in names {
            let path = directory.join(name);
            fs::write(&path, format!("#!/bin/sh\necho {version}\n")).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    fn file(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn link_programs(&self, implementation: &Implementation<'_>) {
        match implementation {
            Implementation::Rust => {
                let binaries = [
                    ("nvm", env!("CARGO_BIN_EXE_nvm")),
                    ("nvmrc", env!("CARGO_BIN_EXE_nvmrc")),
                    ("nvm-exec", env!("CARGO_BIN_EXE_nvm-exec")),
                ];
                for (name, binary) in binaries {
                    symlink(binary, self.root.join("bin").join(name)).unwrap();
                }
            }
            Implementation::Oracle(nvm_sh) => {
                let checkout = nvm_sh.parent().unwrap();
                for name in ["nvm.sh", "nvm-exec"] {
                    symlink(checkout.join(name), self.root.join("nvm").join(name)).unwrap();
                }
            }
        }
    }

    fn nvm_exec(&self, implementation: &Implementation<'_>) -> PathBuf {
        match implementation {
            Implementation::Rust => self.root.join("bin/nvm-exec"),
            Implementation::Oracle(_) => self.root.join("nvm/nvm-exec"),
        }
    }

    /// `program -c <script>` (`zsh -f -c`) in `<W>/proj` with the scenario's
    /// environment and nothing else.
    pub fn command(
        &self,
        program: &Path,
        shell: &str,
        script: &str,
        implementation: &Implementation<'_>,
    ) -> Command {
        let at = |relative: &str| self.root.join(relative);
        let mut command = Command::new(program);
        if shell == "zsh" {
            command.arg("-f");
        }
        command
            .arg("-c")
            .arg(full_script(shell, script, implementation))
            .env_clear()
            .env("HOME", at("home"))
            .env("TERM", "dumb")
            .env("PATH", format!("{}:/usr/bin:/bin", at("bin").display()))
            .env("PWD", at("proj"))
            .env("NVM_DIR", at("nvm"))
            .env("D", at("nvm"))
            .env("SYS", at("sys"))
            .env("NVM_EXEC", self.nvm_exec(implementation))
            .env("NVM_NODEJS_ORG_MIRROR", &self.mirrors.0)
            .env("NVM_IOJS_ORG_MIRROR", &self.mirrors.1)
            .current_dir(at("proj"));
        command
    }
}

/// A local mirror serving `text` as its `index.tab`.
fn index(text: &str) -> String {
    serve(BTreeMap::from([("/index.tab".to_owned(), text.as_bytes().to_vec())]))
}

/// `script` with its load markers replaced; a script without one starts
/// with `@load-no-use@`.
fn full_script(shell: &str, script: &str, implementation: &Implementation<'_>) -> String {
    let script = if script.contains("@load") {
        script.to_owned()
    } else {
        format!("{LOAD_NO_USE}\n{script}")
    };
    let (load, load_no_use) = match implementation {
        Implementation::Rust => (
            format!("eval \"$(nvm init {shell})\""),
            format!("eval \"$(nvm init {shell} --no-use)\""),
        ),
        Implementation::Oracle(nvm_sh) => {
            let source = format!(". '{}'", nvm_sh.display());
            (source.clone(), format!("{source} --no-use"))
        }
    };
    script.replace(LOAD_NO_USE, &load_no_use).replace(LOAD, &load)
}
```

- [ ] **Step 4: Compile the harness alone**

Temporarily comment out the eight `mod <table>;` lines of `mod.rs` (they do
not exist yet) and run:

Run: `cargo test --locked --test compat_cli`
Expected: compiles, `running 0 tests`. Uncomment the lines again as each
table is added below.

#### 5.2 The tables

Each table file has the same shape: the module doc, the imports it needs,
`const SCENARIOS: &[Scenario] = &[...]`, and these two tests (shown once
here, repeated in every file):

```rust
#[test]
fn the_contract_holds() {
    check_all(SCENARIOS);
}

#[test]
#[ignore = "needs NVMRC_ORACLE_NVM_SH: run `mise run compat:capture`"]
fn capture_from_nvm_sh() {
    capture_all(SCENARIOS);
}
```

The imports are `use super::{Scenario, capture_all, check_all};` plus the
`Fixture` variants and `POSIX` the table uses. Every expectation below is
nvm.sh's output from the oracle runs, unless the scenario calls
`.deviation(..)` (then it is nvmrc's) or carries the comment
`// capture: run through nvmrc only` (confirmed in Step 9).

Multi-line expectations are written with `concat!`, never with a trailing
`\` line continuation: a continuation eats the leading spaces of the next
line, and many expected lines start with spaces (the `ls` rows, the
indented usage lines). Keep every literal byte for byte as shown.

- [ ] **Step 1: `tests/compat/alias.rs`**

```rust
//! `nvm alias` and `nvm unalias` (`test/fast/Aliases` and the top-level
//! alias tests).

use super::Fixture::{Alias, File, Iojs, Legacy, Node};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "A1 alias creates a file",
        "fast/Running 'nvm alias' should create a file in the alias directory",
    )
    .fixture(&[Node("v20.1.0")])
    .script("nvm alias test v20.1.0; echo rc=$?; cat \"$D/alias/test\"")
    .stdout("test -> v20.1.0 *\nrc=0\nv20.1.0\n"),
    Scenario::new(
        "A2 unalias removes the file",
        "fast/Running 'nvm unalias' should remove the alias file",
    )
    .fixture(&[Alias("test", "v0.1.2")])
    .script("nvm unalias test; echo rc=$?; test -e \"$D/alias/test\" && echo still || echo gone")
    .stdout(concat!(
        "Deleted alias test - restore it with `nvm alias \"test\" \"v0.1.2\"`\n",
        "rc=0\ngone\n",
    )),
    Scenario::new(
        "A3 no hash in a name",
        "fast/Aliases/'nvm alias' should not accept aliases with a hash",
    )
    .script("nvm alias foo#bar baz")
    .stderr("Aliases with a comment delimiter (#) are not supported.\n")
    .status(1),
    Scenario::new(
        "A4 no slash in a name",
        "fast/Aliases/'nvm alias' should not accept aliases with slashes",
    )
    .script("nvm alias foo/bar baz")
    .stderr("Aliases in subdirectories are not supported.\n")
    .status(1),
    Scenario::new(
        "A5 unalias: no slash in a name",
        "fast/Aliases/'nvm unalias' should not accept aliases with slashes",
    )
    .script("nvm unalias foo/bar")
    .stderr("Aliases in subdirectories are not supported.\n")
    .status(1),
    Scenario::new(
        "A6 unalias of a built-in alias",
        "fast/Aliases/'nvm unalias' should not accept aliases with names equal to built-in alias",
    )
    .script("for a in node stable unstable iojs system; do nvm unalias $a; echo rc=$?; done")
    .stdout("rc=1\nrc=1\nrc=1\nrc=1\nrc=1\n")
    .stderr(concat!(
        "node is a default (built-in) alias and cannot be deleted.\n",
        "stable is a default (built-in) alias and cannot be deleted.\n",
        "unstable is a default (built-in) alias and cannot be deleted.\n",
        "iojs is a default (built-in) alias and cannot be deleted.\n",
        "system is a default (built-in) alias and cannot be deleted.\n",
    )),
    Scenario::new(
        "A7 unalias of an alias shadowing a built-in one",
        "fast/Aliases/'nvm unalias' should accept aliases when they shadow a built-in alias",
    )
    .script(concat!(
        "nvm alias node stable; echo rc=$?; nvm unalias node; echo rc=$?; ",
        "nvm unalias node; echo rc=$?",
    ))
    .stdout(concat!(
        "node -> stable (-> N/A)\nrc=0\n",
        "Deleted alias node - restore it with `nvm alias \"node\" \"stable\"`\n",
        "rc=0\nrc=1\n",
    ))
    .stderr(concat!(
        "! WARNING: Version 'stable' does not exist.\n",
        "node is a default (built-in) alias and cannot be deleted.\n",
    )),
    Scenario::new(
        "A8 alias again changes the target",
        "fast/Aliases/Running 'nvm alias ˂aliasname˃ ˂target˃' again should change the target",
    )
    .fixture(&[Legacy("v0.0.1"), Legacy("v0.0.2")])
    .script("nvm alias t 0.0.2; nvm alias t; nvm alias t 0.0.1; nvm alias t")
    .stdout(concat!(
        "t -> 0.0.2 (-> v0.0.2 *)\n",
        "t -> 0.0.2 (-> v0.0.2 *)\n",
        "t -> 0.0.1 (-> v0.0.1 *)\n",
        "t -> 0.0.1 (-> v0.0.1 *)\n",
    )),
    Scenario::new(
        "A9 alias <name> lists the names starting with it",
        "fast/Aliases/Running 'nvm alias ˂aliasname˃' should list but one alias",
    )
    .fixture(&[Node("v20.1.0"), Alias("t-1", "20"), Alias("t-10", "20")])
    .script("nvm alias t-1")
    .stdout("t-1 -> 20 (-> v20.1.0 *)\nt-10 -> 20 (-> v20.1.0 *)\n"),
    Scenario::new(
        "A10 implicit aliases",
        "fast/Aliases/Running 'nvm alias' lists implicit aliases when they do not exist",
    )
    .fixture(&[
        Legacy("v0.0.1"),
        Legacy("v0.1.1"),
        Iojs("v0.2.1"),
        Alias("ts", "0.0.1"),
        Alias("tu", "0.1.1"),
    ])
    .script("nvm alias")
    .stdout(concat!(
        "ts -> 0.0.1 (-> v0.0.1 *)\n",
        "tu -> 0.1.1 (-> v0.1.1 *)\n",
        "iojs -> iojs-v0.2 (-> iojs-v0.2.1 *) (default)\n",
        "node -> stable (-> v0.0.1 *) (default)\n",
        "stable -> 0.0 (-> v0.0.1 *) (default)\n",
        "unstable -> 0.1 (-> v0.1.1 *) (default)\n",
    )),
    Scenario::new(
        "A11 default and the whole list",
        "fast/Aliases/Running 'nvm alias' should list all aliases",
    )
    .fixture(&[Node("v20.1.0"), Alias("default", "20")])
    .script("nvm alias default; nvm alias --no-colors")
    .stdout(concat!(
        "default -> 20 (-> v20.1.0 *)\n",
        "default -> 20 (-> v20.1.0 *)\n",
        "iojs -> N/A (default)\n",
        "node -> stable (-> v20.1.0 *) (default)\n",
        "stable -> 20.1 (-> v20.1.0 *) (default)\n",
        "unstable -> N/A (default)\n",
    )),
    Scenario::new(
        "A12 leading blank lines",
        "fast/Aliases/'nvm alias' should ignore leading blank lines in the file",
    )
    .fixture(&[Legacy("v0.0.1"), File("nvm/alias/tb", "\nv0.0.1\n\n")])
    .script("nvm alias tb; nvm which tb")
    .stdout("tb -> v0.0.1 *\n<W>/nvm/v0.0.1/bin/node\n"),
    Scenario::new("A13 uppercase names", "fast/Aliases/uppercase alias names should work")
        .fixture(&[Legacy("v0.0.1")])
        .script("nvm alias UPPER_ALIAS v0.0.1; echo rc=$?; cat \"$D/alias/UPPER_ALIAS\"")
        .stdout("UPPER_ALIAS -> v0.0.1 *\nrc=0\nv0.0.1\n"),
    Scenario::new(
        "A14 the lts alias directory",
        "fast/Aliases/lts/'nvm alias' should ensure LTS alias dir exists",
    )
    .script("nvm alias >/dev/null 2>&1; test -d \"$D/alias/lts\" && echo yes || echo no")
    .stdout("yes\n"),
    Scenario::new("A15 an empty target deletes", "nvm.sh:5021-5027 (no upstream test)")
        .fixture(&[Node("v20.1.0"), Alias("foo", "20")])
        .script("nvm alias foo \"\"; echo rc=$?; ls \"$D/alias\"")
        .stdout(concat!(
            "Deleted alias foo - restore it with `nvm alias \"foo\" \"20\"`\n",
            "rc=0\nlts\n",
        )),
    Scenario::new("A16 a missing target warns", "nvm_make_alias (no upstream test)")
        .script("nvm alias foo 99; echo rc=$?")
        .stdout("foo -> 99 (-> N/A)\nrc=0\n")
        .stderr("! WARNING: Version '99' does not exist.\n"),
    Scenario::new("A17 path traversal", "fast/Unit tests/nvm_alias path traversal (adapted)")
        .fixture(&[Node("v20.1.0")])
        .script(concat!(
            "nvm which ../../outside; echo rc=$?; nvm use ../x; echo rc=$?; ",
            "nvm alias ../x 20; echo rc=$?",
        ))
        .stdout("rc=1\nrc=3\nrc=1\n")
        .stderr(concat!(
            "N/A: version \"../../outside\" is not yet installed.\n",
            "\n",
            "You need to run `nvm install ../../outside` to install and use it.\n",
            "N/A: version \"../x\" is not yet installed.\n",
            "\n",
            "You need to run `nvm install ../x` to install and use it.\n",
            "Aliases in subdirectories are not supported.\n",
        )),
];
```

(The `˂`/`˃` in A8 and A9 are U+02C2/U+02C3, as in nvm's file names.)
Add the two tests, uncomment `mod alias;`, and run:

Run: `cargo test --locked --test compat_cli alias`
Expected: PASS (A14 and A15 need Task 3.2).

- [ ] **Step 2: `tests/compat/ls.rs`**

```rust
//! `nvm ls`, `nvm which` and `nvm version` (`test/fast/Listing versions`,
//! `test/fast/Listing paths`).

use super::Fixture::{Alias, File, Iojs, Legacy, Node, SystemNode};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "L1 which <exact version>",
        "fast/Listing paths/Running 'nvm which 0.0.2' should display only version 0.0.2",
    )
    .fixture(&[Legacy("v0.0.2"), Legacy("v0.0.20"), Node("v0.12.0")])
    .script("nvm which 0.0.2; nvm which 0.0.20; nvm which 0.12.0")
    .stdout(concat!(
        "<W>/nvm/v0.0.2/bin/node\n",
        "<W>/nvm/v0.0.20/bin/node\n",
        "<W>/nvm/versions/node/v0.12.0/bin/node\n",
    )),
    Scenario::new(
        "L2 which of a missing version",
        "fast/Listing paths/Running 'nvm which foo' should return a nonzero exit code when not found",
    )
    .script("nvm which nonexistent_version; echo rc=$?")
    .stdout("rc=1\n")
    .stderr(concat!(
        "N/A: version \"nonexistent_version\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install nonexistent_version` to install and use it.\n",
    )),
    Scenario::new(
        "L3 which of an alias to system",
        "fast/Listing paths/Running 'nvm which' should respect alias pointing to system",
    )
    .fixture(&[Alias("default", "system"), SystemNode("v0.0.0")])
    .script(concat!(
        "PATH=\"$D/../sys:$PATH\"; nvm which default; echo rc=$?; ",
        "nvm which system; echo rc=$?",
    ))
    .stdout("<W>/nvm/../sys/node\nrc=0\n<W>/nvm/../sys/node\nrc=0\n"),
    Scenario::new(
        "L4 --no-alias with a pattern",
        "fast/Listing versions/Running 'nvm ls --no-alias' with a pattern errors",
    )
    .script("nvm ls --no-colors --no-alias pattern; echo rc=$?")
    .stdout("rc=55\n")
    .stderr("`--no-alias` is not supported when a pattern is provided.\n"),
    Scenario::new(
        "L5 ls <exact version>",
        "fast/Listing versions/Running 'nvm ls 0.0.2' should display only version 0.0.2",
    )
    .fixture(&[Legacy("v0.0.2"), Legacy("v0.0.20")])
    .script("nvm ls 0.0.2")
    .stdout("         v0.0.2 *\n"),
    Scenario::new(
        "L6 partial patterns and a trailing dot",
        "fast/Listing versions/Running 'nvm ls' with node-like versioning vx.x.x should only list a matched version",
    )
    .fixture(&[Legacy("v0.1.3"), Legacy("v0.2.3"), Legacy("v0.20.3")])
    .script("nvm ls 0.1; nvm ls 0.2; nvm ls v0.2.; nvm ls v0.1.1; echo rc=$?")
    .stdout(concat!(
        "         v0.1.3 *\n",
        "         v0.2.3 *\n",
        "         v0.2.3 *\n",
        "            N/A\n",
        "rc=3\n",
    )),
    Scenario::new(
        "L7 ls of what does not exist",
        "fast/Listing versions/Running 'nvm ls foo', 'nvm ls io', 'nvm ls node_'",
    )
    .fixture(&[Node("v20.1.0")])
    .script(concat!(
        "nvm ls nonexistent_version; echo rc=$?; nvm ls io; echo rc=$?; ",
        "nvm ls node_; echo rc=$?",
    ))
    .stdout(concat!(
        "            N/A\nrc=3\n",
        "            N/A\nrc=3\n",
        "            N/A\nrc=3\n",
    )),
    Scenario::new(
        "L8 ls stable and unstable",
        "fast/Listing versions/Running 'nvm ls stable' and 'nvm ls unstable' should return the appropriate implicit alias",
    )
    .fixture(&[Node("v0.2.3"), Node("v0.3.3"), Node("v0.1.4")])
    .script("nvm ls stable; nvm ls unstable; nvm alias stable 0.1; nvm ls stable")
    .stdout(concat!(
        "         v0.2.3 *\n",
        "         v0.3.3 *\n",
        "stable -> 0.1 (-> v0.1.4 *)\n",
        "         v0.1.4 *\n",
    )),
    Scenario::new(
        "L9 ls default and system with a system node",
        "fast/Listing versions/Running 'nvm ls default' should show system version when available",
    )
    .fixture(&[Alias("default", "system"), Node("v20.1.0"), SystemNode("v0.0.0")])
    .script(concat!(
        "PATH=\"$SYS:$PATH\"; nvm ls default; echo rc=$?; nvm ls system; echo rc=$?; ",
        "nvm ls --no-colors",
    ))
    .stdout(concat!(
        "->       system * (-> v0.0.0)\n",
        "rc=0\n",
        "->       system * (-> v0.0.0)\n",
        "rc=0\n",
        "        v20.1.0 *\n",
        "->       system * (-> v0.0.0)\n",
        "default -> system *\n",
        "iojs -> N/A (default)\n",
        "node -> stable (-> v20.1.0 *) (default)\n",
        "stable -> 20.1 (-> v20.1.0 *) (default)\n",
        "unstable -> N/A (default)\n",
    )),
    Scenario::new(
        "L10 ls system without a system node",
        "fast/Listing versions/Running 'nvm ls system' should include 'system' when appropriate",
    )
    .fixture(&[Node("v20.1.0")])
    .script("nvm ls system; echo rc=$?")
    .stdout("            N/A\nrc=3\n"),
    Scenario::new(
        "L11 ls lists node and io.js by version",
        "fast/Listing versions/Running 'nvm ls' should display all installed versions",
    )
    .fixture(&[Node("v0.12.87"), Node("v0.12.9"), Iojs("v0.1.2"), Iojs("v0.10.2")])
    .script("nvm ls --no-colors --no-alias")
    .stdout(concat!(
        "    iojs-v0.1.2 *\n",
        "   iojs-v0.10.2 *\n",
        "        v0.12.9 *\n",
        "       v0.12.87 *\n",
    )),
    Scenario::new(
        "L12 ls hides dot directories, 'versions' and slashes",
        "fast/Listing versions/Running 'nvm ls' should filter out '.nvm' and 'versions', and not show a trailing slash",
    )
    .fixture(&[
        Legacy("v0.0.1"),
        File("nvm/.nvm/.keep", ""),
        File("nvm/versions/node/.keep", ""),
        File("nvm/v0.0.3/.keep", ""),
    ])
    .script("nvm ls --no-colors | grep -e '^ *\\.' -e versions -e '/$'; echo rc=$?")
    .stdout("rc=1\n"),
    Scenario::new(
        "L13 ls under set -u",
        "fast/Listing versions/Running 'nvm ls' with nounset should not fail",
    )
    .fixture(&[Node("v0.12.34")])
    .script("set -u; nvm ls 99; echo rc=$?; nvm ls 0.12.00; echo rc=$?")
    .stdout("            N/A\nrc=3\n            N/A\nrc=3\n"),
    Scenario::new(
        "L14 ls with an empty IFS",
        "fast/Listing versions/Using a nonstandard IFS should not break",
    )
    .fixture(&[Node("v0.0.1"), Iojs("v0.1.2")])
    .script("IFS=\"\" nvm ls")
    .stdout(concat!(
        "         v0.0.1 *\n",
        "    iojs-v0.1.2 *\n",
        "iojs -> iojs-v0.1 (-> iojs-v0.1.2 *) (default)\n",
        "node -> stable (-> v0.0.1 *) (default)\n",
        "stable -> 0.0 (-> v0.0.1 *) (default)\n",
        "unstable -> N/A (default)\n",
    )),
    Scenario::new("L15 ls current", "fast/Listing versions/Running 'nvm ls' calls into nvm_alias")
        .fixture(&[Node("v20.1.0"), Node("v18.0.0")])
        .script("nvm use 18 >/dev/null; nvm ls current; nvm ls --no-colors --no-alias")
        .stdout("->      v18.0.0 *\n->      v18.0.0 *\n        v20.1.0 *\n"),
    Scenario::new("L16 version", "nvm version (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script("nvm version 20; nvm version node; nvm version foo; echo rc=$?")
        .stdout("v20.1.0\nv20.1.0\nN/A\nrc=3\n"),
];
```

Add the two tests, uncomment `mod ls;`, and run:

Run: `cargo test --locked --test compat_cli ls`
Expected: PASS (L3 needs Task 3.4, L6 Task 3.3).

- [ ] **Step 3: `tests/compat/use_version.rs`**

```rust
//! `nvm use`, `nvm deactivate` and `nvm current` (the top-level `use` tests).

use super::Fixture::{Alias, Iojs, Legacy, Node, SystemNode};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "U1 current after deactivate",
        "fast/Running 'nvm current' should display current nvm environment",
    )
    .script("nvm deactivate; echo rc=$?; nvm current")
    .stdout("rc=0\nnone\n")
    .stderr("Could not find <W>/nvm/*/bin in ${PATH}\n"),
    Scenario::new(
        "U2 deactivate unsets what use set",
        "fast/Running 'nvm deactivate' should unset the nvm environment variables",
    )
    .fixture(&[Legacy("v0.2.3")])
    .script(concat!(
        "nvm use --delete-prefix v0.2.3; echo rc=$?; nvm deactivate; echo rc=$?; ",
        "echo \"bin=[${NVM_BIN-}]\"",
    ))
    .stdout(concat!(
        "Now using node v0.2.3\n",
        "rc=0\n",
        "<W>/nvm/*/bin removed from ${PATH}\n",
        "<W>/nvm/*/share/man removed from ${MANPATH}\n",
        "rc=0\n",
        "bin=[]\n",
    )),
    Scenario::new(
        "U3 use and deactivate with hashing disabled",
        "fast/Running 'nvm use' and 'nvm deactivate' with hashing disabled",
    )
    .fixture(&[Node("v20.1.0")])
    .script(concat!(
        "set +h; nvm use --delete-prefix 20; echo rc=$?; nvm deactivate; echo rc=$?; ",
        "echo \"bin=[${NVM_BIN-}]\"",
    ))
    .stdout(concat!(
        "Now using node v20.1.0\n",
        "rc=0\n",
        "<W>/nvm/*/bin removed from ${PATH}\n",
        "<W>/nvm/*/share/man removed from ${MANPATH}\n",
        "rc=0\n",
        "bin=[]\n",
    )),
    Scenario::new(
        "U4 no stale hashed node",
        "fast/Running 'nvm use' does not leave a stale hashed node",
    )
    .fixture(&[Node("v20.1.0"), Node("v18.0.0")])
    .script(concat!(
        "nvm use 20 >/dev/null; node; nvm use 18 >/dev/null; node; ",
        "nvm deactivate >/dev/null; command -v node || echo none",
    ))
    .stdout("v20.1.0\nv18.0.0\nnone\n"),
    Scenario::new(
        "U5 the current symlink",
        "fast/Running 'nvm use x' should create and change the 'current' symlink",
    )
    .fixture(&[Legacy("v0.10.29"), Legacy("v0.11.13")])
    .script(concat!(
        "export NVM_SYMLINK_CURRENT=true; nvm use 0.10.29 >/dev/null; readlink \"$D/current\"; ",
        "nvm use 0.11.13 >/dev/null; readlink \"$D/current\"; ",
        "export NVM_SYMLINK_CURRENT=1; rm \"$D/current\"; nvm use 0.10.29 >/dev/null; ",
        "test -L \"$D/current\" && echo link || echo nolink",
    ))
    .stdout("<W>/nvm/v0.10.29\n<W>/nvm/v0.11.13\nnolink\n"),
    Scenario::new(
        "U6 MANPATH keeps the default",
        "fast/Running 'nvm use' should not clobber the default MANPATH",
    )
    .fixture(&[Legacy("v0.10.2"), Legacy("v0.10.3")])
    .script(concat!(
        "unset MANPATH; nvm use v0.10.2 --silent; echo \"[$MANPATH]\"; ",
        "nvm deactivate --silent; echo \"[${MANPATH-unset}]\"; ",
        "MANPATH=/opt/foo/man; nvm use v0.10.2 --silent; echo \"[$MANPATH]\"; ",
        "nvm use v0.10.3 --silent; echo \"[$MANPATH]\"; ",
        "nvm deactivate --silent; echo \"[$MANPATH]\"",
    ))
    .stdout(concat!(
        "[<W>/nvm/v0.10.2/share/man:]\n",
        "[unset]\n",
        "[<W>/nvm/v0.10.2/share/man:/opt/foo/man:]\n",
        "[<W>/nvm/v0.10.3/share/man:/opt/foo/man:]\n",
        "[/opt/foo/man:]\n",
    )),
    Scenario::new(
        "U7 use iojs",
        "fast/Running 'nvm use iojs' uses latest io.js version",
    )
    .fixture(&[Iojs("v3.99.0")])
    .script("nvm use iojs; echo rc=$?; nvm current")
    .stdout("Now using io.js v3.99.0\nrc=0\niojs-v3.99.0\n"),
    Scenario::new(
        "U8 use system without a system node",
        "fast/Running 'nvm use system' should work as expected",
    )
    .script("nvm use system; echo rc=$?; nvm use --silent system; echo rc=$?")
    .stdout("rc=127\nrc=127\n")
    .stderr("System version of node not found.\n"),
    Scenario::new(
        "U9 use of an alias to system",
        "fast/Running 'nvm use' should respect alias pointing to system",
    )
    .fixture(&[Alias("default", "system"), SystemNode("v0.0.0")])
    .script("PATH=\"$D/../sys:$PATH\"; nvm use default; echo rc=$?")
    .stdout("Now using system version of node: v0.0.0\nrc=0\n"),
    Scenario::new(
        "U10 a circular alias",
        "fast/Running 'nvm use foo' where 'foo' is circular aborts",
    )
    .fixture(&[Alias("foo", "foo"), Alias("circ", "circ")])
    .script(concat!(
        "nvm use foo; echo rc=$?; nvm use --silent foo; echo rc=$?; ",
        "nvm which circ; echo rc=$?",
    ))
    .stdout("rc=8\nrc=8\nrc=8\n")
    .stderr(concat!(
        "The alias \"foo\" leads to an infinite loop. Aborting.\n",
        "The alias \"circ\" leads to an infinite loop. Aborting.\n",
    )),
    Scenario::new("U11 use of a version not installed", "nvm_ensure_version_installed")
        .script("nvm use 0.10.29; echo rc=$?; nvm use v18; echo rc=$?")
        .stdout("rc=3\nrc=3\n")
        .stderr(concat!(
            "N/A: version \"v0.10.29\" is not yet installed.\n",
            "\n",
            "You need to run `nvm install 0.10.29` to install and use it.\n",
            "N/A: version \"v18\" is not yet installed.\n",
            "\n",
            "You need to run `nvm install v18` to install and use it.\n",
        )),
    Scenario::new("U12 current of the version in use", "nvm_ls_current (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script("nvm use 20 >/dev/null; nvm current; nvm ls current")
        .stdout("v20.1.0\n->      v20.1.0 *\n"),
];
```

Add the two tests, uncomment `mod use_version;`, and run:

Run: `cargo test --locked --test compat_cli use_version`
Expected: PASS.

- [ ] **Step 4: `tests/compat/nvmrc.rs`**

```rust
//! `.nvmrc` handling by `use`, `which` and `install`.

use super::Fixture::{File, Legacy, Node, SystemNode};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new("N1 a CR is dropped", "fast/Running 'nvm use' should drop CR char automatically")
        .fixture(&[Node("v20.1.0"), File("proj/.nvmrc", "20.1.0\r\n")])
        .script("nvm which; echo rc=$?; nvm use; echo rc=$?")
        .stdout(concat!(
            "Found '<W>/proj/.nvmrc' with version <20.1.0>\n",
            "<W>/nvm/versions/node/v20.1.0/bin/node\n",
            "rc=0\n",
            "Found '<W>/proj/.nvmrc' with version <20.1.0>\n",
            "Now using node v20.1.0\n",
            "rc=0\n",
        )),
    Scenario::new("N2 no version and no .nvmrc", "nvm use without a version (no upstream test)")
        .script("nvm use; echo rc=$?")
        .stdout("rc=127\n")
        .stderr(concat!(
            "No version provided and no .nvmrc file found\n",
            "Please see `nvm --help` or https://github.com/nvm-sh/nvm#nvmrc for more information.\n",
        )),
    Scenario::new("N3 an empty .nvmrc", "nvm_process_nvmrc (no upstream test)")
        .fixture(&[File("proj/.nvmrc", "\n")])
        .script("nvm use; echo rc=$?; nvm which; echo rc=$?; nvm install; echo rc=$?")
        .stdout("rc=127\nrc=127\nrc=127\n")
        .stderr(concat!(
            "invalid .nvmrc!\n",
            "all non-commented content (anything after # is a comment) must be either:\n",
            "  - a single bare nvm-recognized version-ish\n",
            "  - or, multiple distinct key-value pairs, each key/value separated by a single equals sign (=)\n",
            "\n",
            "additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).\n",
            "\n",
            "non-commented content parsed:\n",
            "Please see `nvm --help` or https://github.com/nvm-sh/nvm#nvmrc for more information.\n",
            "invalid .nvmrc!\n",
            "all non-commented content (anything after # is a comment) must be either:\n",
            "  - a single bare nvm-recognized version-ish\n",
            "  - or, multiple distinct key-value pairs, each key/value separated by a single equals sign (=)\n",
            "\n",
            "additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).\n",
            "\n",
            "non-commented content parsed:\n",
            "Usage: nvm which [current | <version>]\n",
            "  Provide a <version>, or run from a directory containing an .nvmrc file.\n",
            "  Run `nvm --help` for full help.\n",
            "invalid .nvmrc!\n",
            "all non-commented content (anything after # is a comment) must be either:\n",
            "  - a single bare nvm-recognized version-ish\n",
            "  - or, multiple distinct key-value pairs, each key/value separated by a single equals sign (=)\n",
            "\n",
            "additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).\n",
            "\n",
            "non-commented content parsed:\n",
            "Usage: nvm install [<version>]\n",
            "  Provide a <version>, or run from a directory containing an .nvmrc file.\n",
            "  Run `nvm --help` for full help.\n",
        )),
    Scenario::new("N4 system in .nvmrc", "fast/Running 'nvm use' should respect system in .nvmrc")
        .fixture(&[File("proj/.nvmrc", "system\n"), SystemNode("v0.0.0")])
        .script("PATH=\"$D/../sys:$PATH\"; nvm use; echo rc=$?")
        .stdout(concat!(
            "Found '<W>/proj/.nvmrc' with version <system>\n",
            "Now using system version of node: v0.0.0\n",
            "rc=0\n",
        )),
    Scenario::new("N5 use --save", "fast/Unit tests/Running 'nvm use --save' works as expected'")
        .fixture(&[Legacy("v0.2.4")])
        .script(concat!(
            "nvm use --save v0.2.4 >/dev/null; cat .nvmrc; rm .nvmrc; ",
            "nvm use -w --silent v0.2.4; echo \"rc=$?\"; cat .nvmrc",
        ))
        .stdout("v0.2.4\nrc=0\nv0.2.4\n"),
    Scenario::new("N6 which of an .nvmrc version not installed", "nvm which (no upstream test)")
        .fixture(&[File("proj/.nvmrc", "99\n")])
        .script("nvm which; echo rc=$?")
        .stdout("Found '<W>/proj/.nvmrc' with version <99>\nrc=1\n")
        .stderr(concat!(
            "N/A: version \"v99\" is not yet installed.\n",
            "\n",
            "You need to run `nvm install 99` to install and use it.\n",
        )),
    Scenario::new("N7 both streams read together", "fast/Running 'nvm-exec' should display required node version")
        .fixture(&[File("proj/.nvmrc", "99\n")])
        .script("nvm use 2>&1; echo rc=$?")
        .stdout(concat!(
            "N/A: version \"v99\" is not yet installed.\n",
            "\n",
            "You need to run `nvm install` to install and use the node version specified in `.nvmrc`.\n",
            "Found '<W>/proj/.nvmrc' with version <99>\n",
            "rc=3\n",
        ))
        .deviation("D1"),
];
```

Add the two tests, uncomment `mod nvmrc;`, and run:

Run: `cargo test --locked --test compat_cli nvmrc`
Expected: PASS.

- [ ] **Step 5: `tests/compat/exec.rs`**

```rust
//! `nvm exec`, `nvm run`, `nvm-exec` and the usage errors.

use super::Fixture::{File, Node};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "X1 digit-named files are arguments",
        "fast/Running 'nvm exec' and 'nvm run' treat digit-named files as arguments, not versions",
    )
    .script(concat!(
        "nvm run 123.js </dev/null; echo rc=$?; nvm exec 1.2.3.js </dev/null; echo rc=$?; ",
        "nvm run v999.0.0-rc.1 app.js; echo rc=$?",
    ))
    .stdout("rc=1\nrc=1\nrc=1\n")
    .stderr(concat!(
        "No version provided and no .nvmrc file found\n",
        "WARNING: `nvm run` was invoked without a version argument and without an .nvmrc file.\n",
        "  Falling back to the active node version; this will become an error in a future release.\n",
        "  Pass `current` explicitly (e.g. `nvm run current ...`) to silence this warning.\n",
        "N/A: version \"none\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install` to install and use the node version specified in `.nvmrc`.\n",
        "No version provided and no .nvmrc file found\n",
        "WARNING: `nvm exec` was invoked without a version argument and without an .nvmrc file.\n",
        "  Falling back to the active node version; this will become an error in a future release.\n",
        "  Pass `current` explicitly (e.g. `nvm exec current ...`) to silence this warning.\n",
        "N/A: version \"current\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install current` to install and use it.\n",
        "N/A: version \"v999.0.0-rc.1\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install v999.0.0-rc.1` to install and use it.\n",
    )),
    Scenario::new(
        "X2 exec without a version",
        "fast/Running 'nvm exec' and 'nvm run' without a resolvable version warn and fall back",
    )
    .script("nvm exec </dev/null; echo rc=$?; nvm exec --silent </dev/null; echo rc=$?")
    .stdout("rc=1\nrc=1\n")
    .stderr(concat!(
        "WARNING: `nvm exec` was invoked without a version argument and without an .nvmrc file.\n",
        "  Falling back to the active node version; this will become an error in a future release.\n",
        "  Pass `current` explicitly (e.g. `nvm exec current ...`) to silence this warning.\n",
        "N/A: version \"current\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install current` to install and use it.\n",
        "N/A: version \"current\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install current` to install and use it.\n",
    )),
    Scenario::new(
        "X3 run of an unknown version",
        "fast/Running 'nvm exec' and 'nvm run' without a resolvable version warn and fall back",
    )
    .script(concat!(
        "nvm run bogusversion </dev/null; echo rc=$?; ",
        "nvm run --silent bogusversion </dev/null; echo rc=$?",
    ))
    .stdout("rc=1\nrc=1\n")
    .stderr(concat!(
        "No version provided and no .nvmrc file found\n",
        "WARNING: `nvm run` was invoked without a version argument and without an .nvmrc file.\n",
        "  Falling back to the active node version; this will become an error in a future release.\n",
        "  Pass `current` explicitly (e.g. `nvm run current ...`) to silence this warning.\n",
        "N/A: version \"none\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install` to install and use the node version specified in `.nvmrc`.\n",
        "N/A: version \"none\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install` to install and use the node version specified in `.nvmrc`.\n",
    )),
    Scenario::new(
        "X4 focused usage errors",
        "fast/Subcommands with missing or invalid args show a focused usage error",
    )
    .script(concat!(
        "for c in \"cache bogus\" \"install\" \"run\" \"which\" \"uninstall\" \"uninstall a b\" ",
        "\"unalias\" \"unalias a b\" \"install-latest-npm extra\" \"reinstall-packages\" ",
        "\"copy-packages a b\"; do nvm $c </dev/null; echo \"rc=$? [$c]\"; done",
    ))
    .stdout(concat!(
        "rc=127 [cache bogus]\n",
        "rc=127 [install]\n",
        "rc=127 [run]\n",
        "rc=127 [which]\n",
        "rc=127 [uninstall]\n",
        "rc=127 [uninstall a b]\n",
        "rc=127 [unalias]\n",
        "rc=127 [unalias a b]\n",
        "rc=127 [install-latest-npm extra]\n",
        "rc=127 [reinstall-packages]\n",
        "rc=127 [copy-packages a b]\n",
    ))
    .stderr(concat!(
        "Usage: nvm cache dir\n",
        "       nvm cache clear\n",
        "  Run `nvm --help` for full help.\n",
        "No version provided and no .nvmrc file found\n",
        "Usage: nvm install [<version>]\n",
        "  Provide a <version>, or run from a directory containing an .nvmrc file.\n",
        "  Run `nvm --help` for full help.\n",
        "No version provided and no .nvmrc file found\n",
        "Usage: nvm run [<version>] [<args>]\n",
        "  Provide a <version>, or run from a directory containing an .nvmrc file.\n",
        "  Run `nvm --help` for full help.\n",
        "No version provided and no .nvmrc file found\n",
        "Usage: nvm which [current | <version>]\n",
        "  Provide a <version>, or run from a directory containing an .nvmrc file.\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm uninstall <version>\n",
        "       nvm uninstall --lts\n",
        "       nvm uninstall --lts=<LTS name>\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm uninstall <version>\n",
        "       nvm uninstall --lts\n",
        "       nvm uninstall --lts=<LTS name>\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm unalias <name>\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm unalias <name>\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm install-latest-npm\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm reinstall-packages <version>\n",
        "  Run `nvm --help` for full help.\n",
        "Usage: nvm copy-packages <version>\n",
        "  Run `nvm --help` for full help.\n",
    )),
    // capture: run through nvmrc only (nvm.sh needs its nvm-exec, linked by the harness)
    Scenario::new("X5 exec and run of an installed version", "nvm exec / nvm run (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script("nvm exec 20 node; echo rc=$?; nvm run 20 --version; echo rc=$?")
        .stdout(concat!(
            "Running node v20.1.0\n",
            "v20.1.0\n",
            "rc=0\n",
            "Running node v20.1.0\n",
            "v20.1.0\n",
            "rc=0\n",
        )),
    Scenario::new("X6 exec of a missing command", "nvm-exec: exec \"$@\" (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script("nvm exec 20 nonexistentcmd; echo rc=$?")
        .stdout("Running node v20.1.0\nrc=127\n")
        .stderr("nvm: nonexistentcmd: not found\n")
        .deviation("D15"),
    Scenario::new(
        "X7 nvm-exec with an .nvmrc version not installed",
        "fast/Running 'nvm-exec' should display required node version",
    )
    .fixture(&[File("proj/.nvmrc", "v0.42\n")])
    .script("\"$NVM_EXEC\" 2>&1; echo rc=$?")
    .stdout(concat!(
        "N/A: version \"v0.42\" is not yet installed.\n",
        "\n",
        "You need to run `nvm install v0.42` to install and use it.\n",
        "nvm-exec: unable to select a node version\n",
        "  Set `NODE_VERSION` (e.g. `NODE_VERSION=default`), or add an `.nvmrc` file.\n",
        "Found '<W>/proj/.nvmrc' with version <v0.42>\n",
        "rc=127\n",
    ))
    .deviation("D1"),
    Scenario::new(
        "X8 nvm-exec without a version",
        "fast/Running 'nvm-exec' should display required node version",
    )
    .script("\"$NVM_EXEC\" node; echo rc=$?")
    .stdout("rc=127\n")
    .stderr(concat!(
        "No version provided and no .nvmrc file found\n",
        "nvm-exec: unable to select a node version\n",
        "  Set `NODE_VERSION` (e.g. `NODE_VERSION=default`), or add an `.nvmrc` file.\n",
    )),
    Scenario::new("X9 nvm-exec with NODE_VERSION", "nvm-exec (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script("NODE_VERSION=20 \"$NVM_EXEC\" node; echo rc=$?")
        .stdout("v20.1.0\nrc=0\n"),
];
```

Add the two tests, uncomment `mod exec;`, and run:

Run: `cargo test --locked --test compat_cli exec`
Expected: PASS (X9 runs the hand-over of Task 4.5).

- [ ] **Step 6: `tests/compat/install.rs`**

```rust
//! `install`, `uninstall`, `ls-remote` and `version-remote`, offline or
//! against the local mirror (`common::NODE_INDEX`: v20.10.0 Iron, v18.19.0
//! Hydrogen; no io.js release).

use super::Fixture::{Alias, File, Legacy, Mirror, Node};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "I1 install of an invalid version",
        "fast/Running 'nvm install' with an invalid version fails nicely",
    )
    .script("nvm install invalid.invalid; echo rc=$?")
    .stdout("rc=3\n")
    .stderr("Version 'invalid.invalid' not found - try `nvm ls-remote` to browse available versions.\n"),
    Scenario::new("I2 install --offline", "fast/Unit tests/nvm install --offline")
        .script("nvm install --offline 999.999.999; echo rc=$?")
        .stdout("rc=3\n")
        .stderr(concat!(
            "Version '999.999.999' not found locally or in cache - try `nvm ls` ",
            "to browse available versions.\n",
        )),
    Scenario::new("I3 LTS names", "fast/Unit tests/nvm install with nonlowercase LTS name")
        .fixture(&[Mirror])
        .script("nvm install lts/ARGON; echo rc=$?; nvm install --lts 0.12; echo rc=$?")
        .stdout("rc=3\nrc=3\n")
        .stderr(concat!(
            "LTS names must be lowercase\n",
            "Version with LTS filter 'ARGON' not found - try `nvm ls-remote --lts=ARGON` ",
            "to browse available versions.\n",
            "Version '0.12' (with LTS filter) not found - try `nvm ls-remote --lts` ",
            "to browse available versions.\n",
        )),
    Scenario::new("I4 version-remote", "fast/Unit tests/nvm version-remote")
        .fixture(&[Mirror])
        .script(concat!(
            "nvm version-remote 20; nvm version-remote --lts; nvm version-remote lts/hydrogen; ",
            "nvm version-remote --lts=hydrogen 18; nvm version-remote lts/foo; echo rc=$?; ",
            "nvm version-remote 99; echo rc=$?; nvm version-remote --foo bar; echo rc=$?",
        ))
        .stdout(concat!(
            "v20.10.0\n",
            "v20.10.0\n",
            "v18.19.0\n",
            "v18.19.0\n",
            "N/A\n",
            "rc=3\n",
            "N/A\n",
            "rc=3\n",
            "rc=55\n",
        ))
        .stderr("Unsupported option \"--foo\".\n"),
    Scenario::new("I5 ls-remote", "fast/Unit tests/nvm ls-remote")
        .fixture(&[Mirror])
        .script(concat!(
            "nvm ls-remote --no-colors; echo rc=$?; nvm ls-remote --lts --no-colors; ",
            "echo rc=$?; nvm ls-remote 99; echo rc=$?",
        ))
        .stdout(concat!(
            "       v18.19.0   (Latest LTS: Hydrogen)\n",
            "       v20.10.0   (Latest LTS: Iron)\n",
            "rc=3\n",
            "       v18.19.0   (Latest LTS: Hydrogen)\n",
            "       v20.10.0   (Latest LTS: Iron)\n",
            "rc=0\n",
            "            N/A\n",
            "rc=3\n",
        ))
        .deviation("D9"),
    // capture: run through nvmrc only
    Scenario::new("I6 ls-remote under set -u", "fast/Unit tests/nvm ls-remote with nounset should not fail")
        .fixture(&[Mirror])
        .script("set -u; nvm ls-remote --lts --no-colors >/dev/null; echo rc=$?")
        .stdout("rc=0\n"),
    Scenario::new(
        "I7 uninstall removes the aliases of the version",
        "fast/Running 'nvm uninstall' should clean up aliases pointing to uninstalled version",
    )
    .fixture(&[Node("v20.1.0"), Node("v18.0.0"), Alias("ta", "v20.1.0")])
    .script("nvm uninstall v20.1.0; echo rc=$?; ls \"$D/alias\"; ls \"$D/versions/node\"")
    .stdout(concat!(
        "Uninstalled node v20.1.0\n",
        "Deleted alias ta - restore it with `nvm alias \"ta\" \"v20.1.0\"`\n",
        "rc=0\n",
        "v18.0.0\n",
    )),
    Scenario::new(
        "I8 uninstall removes the directory",
        "fast/Running 'nvm uninstall' should remove the appropriate directory",
    )
    .fixture(&[Legacy("v0.0.1")])
    .script("nvm uninstall v0.0.1; echo rc=$?; test -d \"$D/v0.0.1\" && echo still || echo gone")
    .stdout("Uninstalled node v0.0.1\nrc=0\ngone\n"),
    Scenario::new(
        "I9 uninstall of an inferred version",
        "fast/Running 'nvm uninstall' with an inferred version shows the inferred message",
    )
    .fixture(&[File("nvm/v0.0.1/.keep", "")])
    .script("nvm uninstall 0.0; echo rc=$?")
    .stdout("rc=0\n")
    .stderr("Version 'v0.0.1' (inferred from 0.0) is not installed.\n"),
    Scenario::new(
        "I10 uninstall of a version not installed",
        "fast/Running 'nvm uninstall' with an uninstalled version shows the requested version",
    )
    .script("nvm uninstall 22; echo rc=$?")
    .stdout("rc=0\n")
    .stderr("Version '22' is not installed.\n"),
    Scenario::new("I11 mirror injection characters", "spec 8.5; nvm_get_mirror")
        .script(concat!(
            "NVM_NODEJS_ORG_MIRROR=\"http://x/;id\" nvm ls-remote; echo rc=$?; ",
            "NVM_NODEJS_ORG_MIRROR=\"http://x/;id\" nvm install 20; echo rc=$?",
        ))
        .stdout("            N/A\nrc=3\nrc=3\n")
        .stderr(concat!(
            "$NVM_NODEJS_ORG_MIRROR and $NVM_IOJS_ORG_MIRROR may only contain a URL\n",
            "$NVM_NODEJS_ORG_MIRROR and $NVM_IOJS_ORG_MIRROR may only contain a URL\n",
            "Version '20' not found - try `nvm ls-remote` to browse available versions.\n",
        ))
        .deviation("D9"),
];
```

Add the two tests, uncomment `mod install;`, and run:

Run: `cargo test --locked --test compat_cli install`
Expected: PASS.

- [ ] **Step 7: `tests/compat/codes.rs` (the exit-code acceptance tests)**

```rust
//! The exit codes the spec left pending, pinned end to end: 2, 4, 5, 6, 7
//! and 11. (10 is internal to nvm.sh, turned into 11 by `nvm use`; 42 only
//! comes from `nvm debug`, which nvmrc does not have.)

use super::Fixture::{Mirror, Node};
use super::{Scenario, capture_all, check_all};

const SCENARIOS: &[Scenario] = &[
    Scenario::new("C2 reinstall-packages from the current version", "nvm.sh:5144 (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script("nvm use 20 >/dev/null; nvm reinstall-packages 20; echo rc=$?")
        .stdout("rc=2\n")
        .stderr("Can not reinstall packages from the current version of node.\n"),
    Scenario::new(
        "C4 C5 C6 --reinstall-packages-from",
        "fast/Running 'nvm install' with '--reinstall-packages-from' requires a valid version",
    )
    .fixture(&[Mirror, Node("v18.19.0")])
    .script(concat!(
        "nvm install v20.10.0 --reinstall-packages-from=0.11; echo rc=$?; ",
        "nvm install v20.10.0 --reinstall-packages-from=20.10.0; echo rc=$?; ",
        "nvm install v20.10.0 --reinstall-packages-from; echo rc=$?; ",
        "nvm install v20.10.0 --reinstall-packages-from=; echo rc=$?",
    ))
    .stdout("rc=5\nrc=4\nrc=6\nrc=6\n")
    .stderr(concat!(
        "If --reinstall-packages-from is provided, it must point to an installed version of node.\n",
        "You can't reinstall global packages from the same version of node you're installing.\n",
        "If --reinstall-packages-from is provided, it must point to an installed version of node using `=`.\n",
        "If --reinstall-packages-from is provided, it must point to an installed version of node.\n",
    )),
    Scenario::new("C6 -s with -b", "fast/Unit tests/nvm install -s and -b conflict")
        .script("nvm install -s -b 20; echo rc=$?; nvm install -b -s 20; echo rc=$?")
        .stdout("rc=6\nrc=6\n")
        .stderr(concat!(
            "-s and -b cannot be set together since they would skip install from both binary and source\n",
            "-s and -b cannot be set together since they would skip install from both binary and source\n",
        )),
    Scenario::new("C7 the version floor", "fast/Unit tests/nvm install NVM_MIN_VERSION")
        .fixture(&[Mirror])
        .script(concat!(
            "NVM_MIN_VERSION=24 nvm install 20; echo rc=$?; ",
            "NVM_MIN_VERSION=bogus nvm install 20; echo rc=$?; ",
            "echo 24 > \"$D/min-version\"; nvm install 18; echo rc=$?",
        ))
        .stdout("rc=7\nrc=7\nrc=7\n")
        .stderr(concat!(
            "Version v20.10.0 is below the minimum allowed version v24.0.0.\n",
            "Lower or unset NVM_MIN_VERSION (or edit <W>/nvm/min-version) to install it.\n",
            "Invalid minimum version 'bogus' (from NVM_MIN_VERSION or <W>/nvm/min-version).\n",
            "Version v18.19.0 is below the minimum allowed version v24.0.0.\n",
            "Lower or unset NVM_MIN_VERSION (or edit <W>/nvm/min-version) to install it.\n",
        )),
    Scenario::new("C11 an npm prefix nvm cannot work with", "nvm.sh:4690 (no upstream test)")
        .fixture(&[Node("v20.1.0")])
        .script(concat!(
            "export PREFIX=/x; nvm use 20; echo rc=$?; unset PREFIX; ",
            "export npm_config_prefix=/y; nvm use 20; echo rc=$?; unset npm_config_prefix; ",
            "echo prefix=/z > \"$HOME/.npmrc\"; nvm use 20; echo rc=$?",
        ))
        .stdout("rc=11\nrc=11\nrc=11\n")
        .stderr(concat!(
            "nvm is not compatible with the \"PREFIX\" environment variable: currently set to \"/x\"\n",
            "Run `unset PREFIX` to unset it.\n",
            "nvm is not compatible with the \"npm_config_prefix\" environment variable: ",
            "currently set to \"/y\"\n",
            "Run `unset npm_config_prefix` to unset it.\n",
            "Your user\u{2019}s .npmrc file (${HOME}/.npmrc)\n",
            "has a `globalconfig` and/or a `prefix` setting, which are incompatible with nvm.\n",
            "Run `nvm use --delete-prefix v20.1.0` to unset it.\n",
        )),
];
```

Add the two tests, uncomment `mod codes;`, and run:

Run: `cargo test --locked --test compat_cli codes`
Expected: PASS (C7 needs Task 3.5).

- [ ] **Step 8: `tests/compat/init.rs`**

```rust
//! Loading `nvm` (`test/sourcing`, in every POSIX shell) and the commands
//! nvmrc answers differently (bash only).

use super::Fixture::{Alias, Node};
use super::{POSIX, Scenario, capture_all, check_all};

/// clap's answer to a command it does not know.
macro_rules! unknown {
    ($name:literal) => {
        concat!(
            "error: unrecognized subcommand '",
            $name,
            "'\n\nUsage: nvm <COMMAND>\n\nFor more information, try '--help'.\n"
        )
    };
}

const SCENARIOS: &[Scenario] = &[
    Scenario::new(
        "S1 the default is used",
        "sourcing/Sourcing nvm.sh should use the default if available and no nvm node is loaded",
    )
    .shells(POSIX)
    .fixture(&[Node("v20.1.0"), Alias("default", "20")])
    .script("@load@\nnvm current\nnvm alias default")
    .stdout("v20.1.0\ndefault -> 20 (-> v20.1.0 *)\n"),
    Scenario::new("S2 --no-use", "sourcing/Sourcing nvm.sh with --no-use should not use anything")
        .shells(POSIX)
        .fixture(&[Node("v20.1.0"), Alias("default", "20")])
        .script("@load-no-use@\nnvm current")
        .stdout("none\n"),
    // capture: run through nvmrc only
    Scenario::new("S3 an active version is kept", "sourcing/Sourcing nvm.sh should keep version if one is active")
        .shells(POSIX)
        .fixture(&[Node("v18.0.0"), Node("v20.1.0"), Alias("default", "20")])
        .script("@load-no-use@\nnvm use 18 >/dev/null\n@load@\nnvm current")
        .stdout("v18.0.0\n"),
    Scenario::new("S4 no default is not a failure", "sourcing/Sourcing nvm.sh with no default should return 0")
        .shells(POSIX)
        .script("set -e\n@load@\necho ok")
        .stdout("ok\n"),
    Scenario::new("S5 the caller's parameters", "fast/Sourcing nvm.sh should not modify parameters of caller")
        .shells(POSIX)
        .script("set -- yes\n@load-no-use@\necho \"$1\"")
        .stdout("yes\n"),
    Scenario::new("S6 a bare nvm", "fast/Sourcing nvm.sh should make the nvm command available")
        .shells(POSIX)
        .script("set -e\n@load-no-use@\nnvm >/dev/null\necho rc=$?")
        .stdout("rc=0\n"),
    Scenario::new("S7 a trailing slash in NVM_DIR", "fast/nvm should remove the last trailing slash in $NVM_DIR")
        .script("export NVM_DIR=\"$D/\"\n@load-no-use@\nnvm cache dir")
        .stdout("<W>/nvm/.cache\n"),
    Scenario::new("S8 unload", "fast/Running 'nvm unload' should unset all function and variables")
        .script("nvm unload; echo rc=$?")
        .stdout("rc=127\n")
        .stderr(unknown!("unload"))
        .deviation("D5"),
    Scenario::new("S9 an unknown command", "nvm.sh: help dump on stderr, status 127")
        .script("nvm bogus; echo rc=$?")
        .stdout("rc=127\n")
        .stderr(unknown!("bogus"))
        .deviation("D6"),
    Scenario::new("S10 set-colors", "fast/Unit tests/nvm set_colors")
        .script(concat!(
            "nvm set-colors rgbyc; echo rc=$? $NVM_COLORS; nvm set-colors rgby; echo rc=$?; ",
            "nvm set-colors p3gq7; echo rc=$?",
        ))
        .stdout(concat!(
            "Setting colors to: r g b y c\n",
            "WARNING: Colors may not display because they are not supported in this shell.\n",
            "rc=0 rgbyc\n",
            "\n",
            "rc=0\n",
            "\n",
            "rc=0\n",
        ))
        .stderr(concat!(
            "\u{1b}[1;37mPlease pass in five \u{1b}[1;31mvalid color codes\u{1b}[1;37m. ",
            "Choose from: rRgGbBcCyYmMkKeW\u{1b}[0m\n",
            "\u{1b}[1;37mPlease pass in five \u{1b}[1;31mvalid color codes\u{1b}[1;37m. ",
            "Choose from: rRgGbBcCyYmMkKeW\u{1b}[0m\n",
        ))
        .deviation("D11"),
];
```

Add the two tests, uncomment `mod init;`, and run:

Run: `cargo test --locked --test compat_cli init`
Expected: PASS in every installed POSIX shell (S6 needs Task 3.1). A shell
that fails only there is a real difference: report it with the shell name,
do not narrow `.shells(..)`.

- [ ] **Step 9: Capture from nvm.sh**

With the nvm checkout next to this repository
(`../nvm/nvm.sh`, the `elioseverojunior/nvm` repository the oracle used):

Run: `mise run compat:capture` (or `mise run compat:capture --nvm-sh <path>`)
Expected, for every scenario in bash:

- `same as the table` for each scenario without a deviation, the three
  `// capture:` rows (X5, I6, S3) included;
- `differs, as D1 says` (N7, X7), `as D5 says` (S8), `as D6 says` (S9),
  `as D9 says` (I5, I11), `as D11 says` (S10), `as D15 says` (X6).

Lines for zsh, sh, dash and ksh are informative only (see the module doc).
If a bash row that has no deviation prints `DIFFERS`: when the printed
nvm.sh output is what the upstream test asserts and nvmrc matches it too
(only a `// capture:` row can be in that state), paste the printed literals
into the table and drop the comment; otherwise stop and report the scenario,
both outputs and your reading of the difference. Remove the `// capture:`
comments of the rows confirmed same.

- [ ] **Step 10: The contract in full**

Run: `cargo test --locked --test compat_cli`
Expected: 8 passed, 8 ignored.

Run: `wc -l tests/compat_cli.rs tests/compat/*.rs`
Expected: every file under 300 lines (`exec.rs` and `nvmrc.rs` are the
longest, about 230 and 130).

- [ ] **Step 11: Commit the contract**

```bash
git add tests/compat_cli.rs tests/compat
git commit -S -m "test(compat): pin nvm.sh's observable contract in a scenario table"
```

#### 5.3 The spec catches up

- [ ] **Step 1: Edit `docs/superpowers/specs/2026-10-02-nvmrc-design.md`**

Make exactly these changes (each old text is quoted from the current file):

1. Section 1, the phase list: in the v1 item, after "`install` from
   pre-built binaries (with checksum)," insert "install from source (`-s`)
   when there is no binary,"; the v2 item becomes "**v2 (out of scope
   here):** legacy platforms (SmartOS, AIX, BSD, ARM variants),
   `self-install` and `self-update`."
2. Section 3, the code block: the `ports/` line becomes
   `ports/     traits: FileSystem, Http, Env, Clock, Process, Prompt, ...`
   and the `adapters/` line
   `adapters/  real implementations (std::fs, ureq, std::env, std::process)`.
   The sentence "Errors use `thiserror` per module and `anyhow` only in the
   binaries." becomes "Errors use `thiserror`; there is no `anyhow`: the
   binaries only turn the CLI's result into an exit code."
3. Section 4.4, the paragraph "Without a binary build the install fails with
   a clear error (equivalent to `NVM_NO_SOURCE_FALLBACK=1`) until v2 adds
   source builds." becomes "Without a binary build the install falls back to
   a source build, as nvm.sh does, unless `-b` or
   `NVM_NO_SOURCE_FALLBACK=1` forbids it (then exit 2)."
4. Section 6: the last bullet "127 command not found." becomes "127 usage
   error, unknown command, command not found, or no system version." Insert
   after "1 generic failure." the bullet "2 missing target: a binary
   download that failed with no source fallback, `nvm alias lts/<missing>`,
   `nvm reinstall-packages` of the current version." and after "3 invalid or
   unknown version." the bullets "4 `--reinstall-packages-from` the version
   being installed." and "5 `--reinstall-packages-from` a version that is not
   installed." Replace the paragraph "Codes 2, 4, 5, 10 and 42 are pending:
   ... enabled by `NVMRC_LOG`." with: "Every code above is pinned by an
   acceptance scenario of `tests/compat/codes.rs`. 10 is internal to nvm.sh
   (`nvm_die_on_prefix`; `nvm use` turns it into 11) and 42 only comes from
   `nvm debug`, which nvmrc does not have: nvmrc produces neither. Messages
   go to stderr with the same text as `nvm.sh` wherever a test compares it.
   There are no structured logs (no `tracing`, no `NVMRC_LOG`)."
5. Section 8, item 3 becomes: "Compatibility contract: the `nvm/test`
   scenarios that can be expressed as commands (`test/fast`,
   `test/sourcing`) run against the Rust `nvm` function in a temporary
   `$NVM_DIR` (`tests/compat_cli.rs`), with nvm.sh's output inline; an
   ignored test re-captures them from a given `nvm.sh`." Item 5 becomes:
   "Security tests: mirror injection characters (`ls-remote` and `install`
   exit 3, as nvm.sh's public commands do; `nvm_get_mirror`'s 2 stays
   internal) and `..` path components in aliases and versions."
6. Section 10 becomes: "`clap`, `thiserror`, `ureq` (blocking HTTP, platform
   certificate verifier), `sha2` (checksums), `flate2` and `lzma-rust2`
   (gzip and xz) and `tar`; `tempfile` for tests. No `anyhow`, no `tracing`
   and no ripgrep crates (the conflict rules are hand-written, see 7.4).
   MSRV is 1.85 as declared in `Cargo.toml`, checked in CI."
7. Section 12 becomes: "None open. The pending exit codes are pinned (section
   6); the checksum and archive crates are chosen (section 10); `nvm-exec`
   stays a separate binary, because tools call it by path as
   `$NVM_DIR/nvm-exec` (the README says to link it there), and on Unix it
   replaces itself with the command, as the upstream script's `exec "$@"`."

Run:

```sh
rumdl fmt docs/superpowers/specs/2026-10-02-nvmrc-design.md && rumdl check docs/superpowers/specs/2026-10-02-nvmrc-design.md
```

Expected: `No issues found`.

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/specs/2026-10-02-nvmrc-design.md
git commit -S -m "docs(spec): pin the exit codes and catch up with the implementation"
```

---

### Task 6: README and the list of deviations

**Files:**

- Create: `README.md`, `docs/deviations.md`
- Modify: `Cargo.toml` (add `readme = "README.md"` after `repository`)

**Interfaces:**

- Consumes: the mise tasks (Task 1), the CI jobs (Task 2), the D-list (Tasks
  3 and 5), the facts below.
- Produces: the user documentation. Both files are prose for people, written
  from the facts below; nothing in them may contradict a test.

Both files: wrapped at 80 columns, no tables (lists instead), every code
block fenced with a language (`sh`, `fish`, `toml`, `text`), headings and
lists surrounded by blank lines, under 300 lines each.

- [ ] **Step 1: `README.md`, in this order**

1. **Title and summary.** `# nvmrc`, then: a native Rust port of
   [nvm](https://github.com/nvm-sh/nvm); three binaries built from one
   library, `nvmrc`, `nvm` (the same program, under the name scripts, CI and
   tools call) and `nvm-exec`; the same `$NVM_DIR` layout as nvm, so versions
   installed by either tool work with both; nvm.sh's commands, options,
   messages and exit codes, checked against nvm.sh's own test scenarios
   (`tests/compat/`), with the deviations in `docs/deviations.md`.
2. **Install.**
   - `cargo install --locked --git https://github.com/elioseverojunior/nvmrc`
     installs the three binaries into `~/.cargo/bin` (the crate is not on
     crates.io yet: `publish = false`).
   - From a clone: `cargo build --release --locked`; the binaries are in
     `target/release/`.
   - Requirements: Rust 1.85 or newer (MSRV); development uses the 1.99
     toolchain of `rust-toolchain.toml`. The release profile (fat LTO,
     `strip = "symbols"`, `panic = "abort"`) lives in `.cargo/config.toml`.
     With rustup, the `llvm-tools` component that `rust-toolchain.toml`
     declares provides the `rust-objcopy` that stripping uses; a toolchain
     without it may warn. On x86_64 Linux the linker settings need `clang`
     and `mold`; cross-building for aarch64 Linux needs
     `gcc-aarch64-linux-gnu`.
   - A Homebrew formula is possible (a tap with `depends_on "rust" =>
     :build` and `system "cargo", "install", *std_cargo_args`), but none is
     published.
   - For tools that run `$NVM_DIR/nvm-exec` by path, link it:
     `ln -s "$(command -v nvm-exec)" "$NVM_DIR/nvm-exec"`.
3. **Shell integration.** One code block per shell:
   - bash, in `~/.bashrc`: `eval "$(nvmrc init bash)"`;
   - zsh, in `~/.zshrc`: `eval "$(nvmrc init zsh)"`;
   - sh, dash and ksh, in `~/.profile` or the file `$ENV` names:
     `eval "$(nvmrc init sh)"` (or `dash`, `ksh`);
   - fish 3.4 or newer, in `~/.config/fish/config.fish`:
     `nvmrc init fish | source`.
   Then: `--no-use` skips switching to the default version, `--install`
   installs it when missing; the snippet defines a shell function `nvm`; for
   `use`, `deactivate` and `install` it opens descriptor 3, names it in
   `NVMRC_SCRIPT_FD` and evaluates the shell code the binary writes there,
   while messages stay on stdout and stderr; every other command runs the
   binary directly. Without the function, `eval "$(nvm use 18)"` works by
   hand. The load line calls `nvmrc`, which an `nvm` function left by nvm.sh
   cannot intercept.
4. **Moving from nvm.sh.** `nvm doctor [--shell <shell>]` (read-only; lists
   every place that still loads nvm.sh, exit 1 when one is found);
   `nvm migrate --dry-run` (shows the diff); `nvm migrate [--yes]` (backs each
   file up as `<file>.nvmrc-backup-<yyyymmddThhmmssZ>`, comments the old lines
   as `# [nvmrc-migrated] ...`, adds the init block between
   `# >>> nvmrc init >>>` and `# <<< nvmrc init <<<`, checks the syntax with
   the shell first); `nvm migrate --undo` (restores the latest backup,
   backing the current file up first). Lazy loaders are reported with a
   suggested fix and never edited; `$NVM_DIR` is never touched.
5. **Environment variables.** A list, grouped:
   - read as nvm.sh does: `NVM_DIR`, `NVM_BIN`, `NVM_INC`, `NVM_CD_FLAGS`,
     `NVM_COLORS`, `NVM_NO_COLORS`, `NVM_HAS_COLORS`, `NVM_NUM_COLORS`,
     `NO_COLOR`, `TERM`, `NVM_MIN_VERSION` (and `$NVM_DIR/min-version`),
     `NVM_NODEJS_ORG_MIRROR`, `NVM_IOJS_ORG_MIRROR`, `NVM_AUTH_HEADER`,
     `NVM_SYMLINK_CURRENT`, `NVM_INSTALL_LOCK_TIMEOUT`,
     `NVM_INSTALL_LOCK_STALE`, `NVM_INSTALL_THIRD_PARTY_HOOK`,
     `NVM_NO_SOURCE_FALLBACK`, `NVM_MAKE_JOBS`, `NVM_DEBUG`, `NVM_SILENT`,
     `NODE_VERSION` (for `nvm-exec`), `NODE_PATH`, `MANPATH`, `PREFIX`,
     `npm_config_prefix`/`NPM_CONFIG_PREFIX`, `CC`/`CXX` (source builds),
     `HOME`, `PWD`, `PATH`;
   - for `doctor` and `migrate`: `SHELL`, `ENV`, `ZDOTDIR`,
     `XDG_CONFIG_HOME`, `NVM_LAZY_LOAD` (a conflict rule);
   - nvmrc's own: `NVMRC_SCRIPT_FD`, `NVMRC_SHELL_KIND`, `NVMRC_SHELL`;
   - not supported: `NVM_NO_PROGRESS` (there is no progress bar),
     `NVM_OFFLINE` (use `--offline`), curl and wget settings (downloads use
     ureq; proxies come from the usual `*_PROXY` variables).
6. **Commands.** A list: `version`, `current`, `ls`/`list`,
   `ls-remote`/`list-remote`, `cache dir|clear`, `install`/`i` (binaries,
   and `-s` from source), `install-latest-npm`,
   `reinstall-packages`/`copy-packages`, `uninstall`, `version-remote`,
   `which`, `alias`, `unalias`, `use`, `deactivate`, `set-colors`, `exec`,
   `run`, as nvm.sh; new: `init`, `doctor`, `migrate`. Not available:
   `unload`, `debug`, nvm.sh's `--help` text and `--version` number,
   `self-install`/`self-update` (v2), legacy platforms (v2).
7. **Deviations.** Three sentences and a link: the behaviour follows nvm.sh
   wherever a test can compare it; the known differences are listed, with
   the reason, in a link to `docs/deviations.md`; the most visible
   are the help text, `--version`, and the order of messages when stdout and
   stderr are read together.
8. **Exit codes.** A list: 0 success; 1 failure; 2 missing target (binary
   download failed with no source fallback, `alias lts/<missing>`,
   `reinstall-packages` from the version in use); 3 invalid or unknown
   version; 4 `--reinstall-packages-from` the version being installed; 5
   `--reinstall-packages-from` a version not installed; 6 options that cannot
   be combined; 7 below the version floor, or an invalid floor; 8 alias loop;
   11 an npm prefix setting nvm cannot work with; 33 the third-party install
   hook claimed success; 55 unsupported option; 127 usage error, unknown
   command, command not found, or no system version. `exec`, `run`,
   `nvm-exec` and the install hook pass their program's status on (128+n
   after signal n). 10 and 42 are never produced (internal to nvm.sh, and
   `nvm debug`).
9. **Development.** `mise run setup` (tools; may use Homebrew), then the
   tasks: `mise run fmt`, `lint` (clippy, rumdl, actionlint, hadolint),
   `test`, `test:msrv`, `deny`, `audit`, `check` (all gates, as CI),
   `docker:build` and `docker:test` (the suite in a Debian image with bash,
   zsh, dash, ksh and fish), `compat:capture --nvm-sh <path>` (re-runs the
   contract through a real nvm.sh and prints what differs). Commits follow
   Conventional Commits and are GPG-signed. CI:
   `.github/workflows/ci.yml`.
10. **License.** MIT, see `LICENSE`; nvm's copyright notice is kept.

- [ ] **Step 2: `docs/deviations.md`**

Title `# Deviations from nvm.sh`, one introductory sentence ("Each item is
pinned by a test; the contract scenarios that pin one name its id."), then
three sections:

**`## The contract (D1 to D16)`** — one bullet per id, "fixed" or the reason:

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
- D7: fixed in Plan 9: `nvm alias` creates `$NVM_DIR/alias/lts`; a failure
  to create it is silent (nvm.sh's `mkdir` complains and goes on).
- D8: fixed in Plan 9: `nvm ls v0.1.` lists v0.1.x.
- D9: `ls-remote` with nothing to list prints `N/A` without the star that
  nvm.sh prints after it.
  (I5, I11)
- D10: `install -b` of a version before v0.12 says the legacy layout is not
  supported (nvm.sh: "Binary download is not available"); status 3 for both.
- D11: `set-colors` with an invalid setting prints the message without the
  help dump. (S10)
- D12: fixed in Plan 9: the system node is the one `nvm deactivate` leaves on
  `PATH` (`$NVM_DIR/../sys` included).
- D13: a v0.x version placed under `versions/node` (nvm puts it in
  `$NVM_DIR` itself) is listed by `ls` and `alias` but not used by `use`.
- D14: there is no `nvm debug`, so exit 42 is never produced.
- D15: a command that `exec` or `run` cannot start prints
  `nvm: <cmd>: not found` (`nvm-exec: ...` for nvm-exec), not bash's `exec`
  message; status 127 for both. (X6)
- D16: fixed in Plan 9: the floor messages name the real `min-version` path.

**`## By area`** — the deliberate deviations recorded by Plans 1 to 8, one
short bullet each, grouped under `###` headings: Listing (`ls` shows no
phantom blank row with only a system node; io.js and node sharing a number
are both listed; one `N/A` row for `ls unstable`; sort by bytes; `ls` does
not read `.nvmrc` as a pattern nor see a system io.js; `unalias` does not
create `alias/`; a bad alias name exits 1; non-UTF-8 paths are shown lossy),
Remote (ureq replaces curl/wget, only `NVM_AUTH_HEADER` carried; the index is
fetched once; `version-remote --lts=<bad>` prints one message and `N/A`,
status 3; patterns are literal text, not regexes; rows sorted by version; a
refused mirror is reported once), Install (archive type by version and OS,
no `.tar.gz` fallback, no external `xz`; no progress bar and no `Computing
checksum` line; archives are read into memory up to 1 GiB, xz with a 256 MiB
dictionary; a checksum mismatch fails and the archive is removed; no npm
bootstrap for a node without npm; `--default`/`--alias` with `--lts` alias
`lts/<name>`; `--save` writes `$PWD/.nvmrc` after a fresh install; no
installs before v0.12, and before v0.8.6 or without binary and source:
status 3; a platform without source builds says so, status 1; `uninstall`
does no permission check and removes aliases by literal match; build jobs
from the CPU count; command output shown at the end; only binary installs
are one atomic rename; 30 s connect and header timeouts), Shell (`use` of a
resolvable but missing version is status 3, nvm.sh 0; `-h` and `help` are
ordinary arguments; `deactivate` with an unresolvable `NVM_DIR` fails 1; a
command that cannot be executed is 127, bash says 126; the function needs
`nvm` on `PATH` and runs the first one there, a global npm package named
`nvm` included; `install` activates only through the function; only bash,
zsh, sh, dash, ksh and fish; `--delete-prefix` without npm warns; `hash -r`
is always emitted; non-UTF-8 arguments are replaced lossily; the function
needs `/dev/fd`; `nvm exec` and `nvm run` wait for their command as a child,
while `nvm-exec` replaces itself with it), Colors (an invalid `NVM_COLORS`
letter warns once; an unknown `TERM` is silent; no color legend in
`--help`; `NVM_NO_COLORS` does not leak; invalid `.nvmrc` lines and alias
targets are printed raw, control bytes included; `NVM_COLORS` is read by
character; the install `--alias`/`--default` row is never colored;
`set-colors x --help` sets the colors), Conflicts (no ripgrep, no
`LineScanner`; the runtime check is in the init snippet, so a late
`source nvm.sh` is only found by `doctor`; lazy-loader files are never
edited; `OmzPlugin` and `HelperCall` are hints; `source` must be in command
position, a `zsh-defer` or `time` before it counts, `eval "$(cat nvm.sh)"` is
not seen; here-documents are not tracked; relative source paths resolve from
the sourcing file's directory; only loader and completion lines are
migrated, compound lines are manual; a line inside a block or continued is
manual; an unterminated BEGIN marker is not a block; the load line calls
`nvmrc`; the block's shell comes from the file name; the diff is part of the
prompt; backups keep the file's permissions but not its owner, extended
attributes or ACLs, and a read-only profile is replaced; `--undo` backs up
first, refuses an empty backup and prefers backups not dated in the future;
an unreadable file is exit 1; a file the syntax checker already refuses is
left alone; a missing checker writes with a warning; fish is verified only
in the Docker image; `doctor`, `migrate` and `__conflict` are additions).

**`## Platforms`** — v1 runs on Linux and macOS (x86_64 and arm64). Not
handled: AIX gets `.tar.xz` (nodejs.org publishes `.tar.gz` there), armv6
and Rosetta are not detected as nvm.sh does, an install lock left by Ctrl-C
waits for its stale timeout, `.nvmrc` is searched from `$PWD` as the shell
gives it.

- [ ] **Step 3: Metadata and checks**

Add `readme = "README.md"` to `Cargo.toml` after the `repository` line.

Run: `rumdl fmt README.md docs/deviations.md && rumdl check .`
Expected: `No issues found`.

Run:

```sh
wc -l README.md docs/deviations.md && cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].readme'
```

Expected: both files under 300 lines, then `README.md`.

Read both files once against the tests: every exit code, command name and
message quoted must match `tests/compat/` or the unit tests.

- [ ] **Step 4: Commit**

```bash
git add README.md docs/deviations.md Cargo.toml
git commit -S -m "docs: add the README and the list of deviations from nvm.sh"
```

---

### Task 7: The final gate

**Files:**

- Modify: `docs/superpowers/specs/2026-10-02-nvmrc-design.md` (status lines)

**Interfaces:**

- Consumes: everything above. Produces: nothing new, only evidence.

- [ ] **Step 1: Every gate on macOS**

Run: `mise run check`
Expected: fmt, clippy, rumdl, actionlint, hadolint, the full test suite,
MSRV 1.85, cargo-deny and cargo-audit all pass.

Run:

```sh
cargo build --release --locked 2>&1 | grep -i -e warning -e error; ls -l target/release/nvm target/release/nvmrc target/release/nvm-exec
```

Expected: no warning or error line; the three binaries exist (about 2.4, 2.4
and 1.8 MB on macOS arm64).

- [ ] **Step 2: Every shell in Docker**

Run: `mise run docker:test`
Expected: the image builds and `cargo test --locked --offline` passes inside
it, with no `skipped:` line for a shell (bash, zsh, sh, dash, ksh and fish
are installed there), the contract in all five POSIX shells included.

- [ ] **Step 3: The contract against nvm.sh, once more**

Run: `mise run compat:capture`
Expected: as Task 5.2 Step 9: bash rows are `same as the table` or `differs,
as D<n> says`; no `DIFFERS` and no `is D<n> gone?`.

- [ ] **Step 4: Size and complexity of what this plan touched**

Run:

```sh
git diff --name-only --diff-filter=d 743b94c -- '*.rs' | xargs wc -l | sort -n | tail -5
```

Expected: every file under 300 lines.

In a scratch directory (never in the repository), create `clippy.toml` with
`too-many-lines-threshold = 30` and `cognitive-complexity-threshold = 10`, then:

Run:

```sh
CLIPPY_CONF_DIR=<scratch dir> cargo clippy --all-targets --locked -- -W clippy::too_many_lines -W clippy::cognitive_complexity 2>&1 | grep -A3 -e too_many_lines -e cognitive_complexity
```

Expected: no finding in a function this plan added or changed (findings in
untouched code are reported in the hand-off, not fixed here). Delete the
scratch file.

- [ ] **Step 5: Read-only sanity on the real home**

The real `$HOME` is only read. Never pass `--yes` and never run `migrate`
without `--dry-run`.

```sh
scratch="$(mktemp -d)"
for f in ~/.bashrc ~/.bash_profile ~/.profile ~/.zshenv ~/.zprofile ~/.zshrc ~/.config/fish/config.fish; do
  [ -e "$f" ] && shasum -a 256 "$f"
done > "$scratch/before.txt"
touch "$scratch/marker"
target/release/nvm doctor; echo "doctor=$?"
target/release/nvm migrate --dry-run; echo "migrate=$?"
for f in ~/.bashrc ~/.bash_profile ~/.profile ~/.zshenv ~/.zprofile ~/.zshrc ~/.config/fish/config.fish; do
  [ -e "$f" ] && shasum -a 256 "$f"
done > "$scratch/after.txt"
diff "$scratch/before.txt" "$scratch/after.txt" && echo unchanged
find ~/.nvm -newer "$scratch/marker" -maxdepth 3 | head -5
rm -rf "$scratch"
```

Expected: `doctor` lists the same findings as at the end of Plan 8 (status
1 if nvm.sh is still loaded somewhere); `migrate --dry-run` prints a diff and
exits 0; `unchanged`; `find` prints nothing (`$NVM_DIR` untouched).

- [ ] **Step 6: The spec's status**

In `docs/superpowers/specs/2026-10-02-nvmrc-design.md`: the line
`Status: Draft for review` becomes `Status: v1 implemented (Plans 1 to 9)`,
and section 11 item 8 becomes "Closing: full compatibility contract, CI,
README (Plan 9, done)."

Run: `rumdl check docs/superpowers/specs/2026-10-02-nvmrc-design.md`
Expected: `No issues found`.

- [ ] **Step 7: Commit**

```bash
git add docs/superpowers/specs/2026-10-02-nvmrc-design.md
git commit -S -m "docs(spec): mark v1 implemented"
```

- [ ] **Step 8: Hand-off**

Report: the commits of the branch (`git log --oneline 743b94c..`), the
gate results of Steps 1 to 5, the capture summary, any untouched-code
complexity findings, and that the branch is not pushed: the first CI run
happens when the owner pushes it.

---

## Deliberate deviations from the spec and from nvm.sh

Each is pinned by a test and listed in `docs/deviations.md`.

- **The contract runs nvmrc only** in CI; nvm.sh is a development-time oracle
  behind `NVMRC_ORACLE_NVM_SH` (spec 8.3 asked for the `nvm/test` scenarios
  "against the Rust binary", which the inline expectations do without making
  CI depend on nvm.sh). Class-B scenarios (those that stub `nvm_*` helpers)
  stay covered by the unit tables; class C (network, `sudo`, real npm) is
  out of the contract.
- **No release workflow, no crates.io publication, no Homebrew formula**:
  `publish = false` until the owner decides.
- **D1, D2, D4, D5, D6, D9, D10, D11, D13, D14, D15** stay as listed above;
  D3, D7, D8, D12 and D16 are fixed.
- **`nvm-exec` replaces itself (`exec`) on Unix**, as upstream; `nvm exec` and
  `nvm run` keep a child, as nvm.sh's function runs `nvm-exec` as one.
- **A cut-short (empty) backup is refused by `--undo`** instead of asking for
  confirmation; a backup dated in the future only wins when it is the only
  one.
- **An already broken profile is left alone** by `migrate`, with the
  checker's first lines; it is no longer blamed on the edit.
- **A platform without source builds** says so and keeps status 1 (nvm.sh's
  87 is for Windows, which nvmrc does not run on).

## Self-review against the spec

- **Spec coverage:** section 8.3 (the contract: `tests/compat/`, Task 5),
  8.5 (mirror injection, corrected to status 3: scenario I11, spec edit),
  section 6 (exit codes 2, 4, 5, 6, 7, 11 pinned end to end in `codes.rs`;
  10 and 42 explained), section 9 (fmt, clippy, tests, deny, audit, rumdl,
  macOS and Linux CI with the loosest tags: Tasks 1 and 2), section 10 (MSRV
  1.85 checked locally and in CI), section 11 item 8 (contract, CI, README:
  Tasks 5, 2, 6), section 12 (every open item closed: Task 5.3), and the
  deferred review items of Plans 5, 6 and 8 that were ruled in (Task 4).
- **Placeholder scan:** none; the `setup` task is reproduced verbatim, every
  expectation is a literal, every Rust change shows its code.
- **Type consistency:** `Scenario`, `Fixture`, `Implementation`, `World`,
  `Outcome`, `check_all`, `capture_all`, `BASH`, `POSIX` (Task 5);
  `Launcher`, `NVM`, `NVM_EXEC`, `Process::hand_over`,
  `FakeProcess::{handed_over, with_failure_saying}`,
  `ProcessOutput::stderr`, `run_and_target`, `path_without_nvm`,
  `without_trailing_group_dot`, `latest_backup(.., now)`,
  `SyntaxCheck::{original_passes, complaint}` (Tasks 3 and 4) are named the
  same everywhere they appear.
- **Size:** every new file is planned under 300 lines (`std_process`,
  `fetch` and `resolve` tests are split first because they were near the
  limit).
