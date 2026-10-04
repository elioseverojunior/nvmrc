# nvmrc

A native Rust port of [nvm](https://github.com/nvm-sh/nvm). Three binaries are
built from one library: `nvmrc`, `nvm` (the same program, under the name
scripts, CI and tools call) and `nvm-exec`.

nvmrc uses the same `$NVM_DIR` layout as nvm, so versions installed by either
tool work with both. It follows nvm.sh's commands, options, messages and exit
codes, checked against the test scenarios of the author's fork of nvm
(`tests/compat/`); the deviations are in [docs/deviations.md](docs/deviations.md).

## Install

- `cargo install --locked --git https://github.com/elioseverojunior/nvmrc`
  installs the three binaries into `~/.cargo/bin` (the crate is not on
  crates.io yet: `publish = false`).
- From a clone: `cargo build --release --locked`; the binaries are in
  `target/release/`.
- Requirements: Rust 1.85 or newer (MSRV); development uses the 1.99
  toolchain of `rust-toolchain.toml`. The release profile (fat LTO,
  `strip = "symbols"`, `panic = "abort"`) lives in `.cargo/config.toml`. With
  rustup, the `llvm-tools` component that `rust-toolchain.toml` declares
  provides the `rust-objcopy` that stripping uses; a toolchain without it may
  warn. On x86_64 Linux the linker settings need `clang` and `mold`;
  cross-building for aarch64 Linux needs `gcc-aarch64-linux-gnu`.
- A Homebrew formula is possible (a tap with `depends_on "rust" => :build`
  and `system "cargo", "install", *std_cargo_args`), but none is published.
- For tools that run `$NVM_DIR/nvm-exec` by path, link it:
  `ln -s "$(command -v nvm-exec)" "$NVM_DIR/nvm-exec"`.

## Shell integration

Add one line to the shell's startup file.

bash, in `~/.bashrc`:

```sh
eval "$(nvmrc init bash)"
```

zsh, in `~/.zshrc`:

```sh
eval "$(nvmrc init zsh)"
```

sh, dash and ksh, in `~/.profile` or the file `$ENV` names (use `dash` or
`ksh` in place of `sh` for those shells):

```sh
eval "$(nvmrc init sh)"
```

fish 3.4 or newer, in `~/.config/fish/config.fish`:

```fish
nvmrc init fish | source
```

`--no-use` skips switching to the default version; `--install` installs it
when missing. The snippet defines a shell function `nvm`. For `use`,
`deactivate` and `install` it opens descriptor 3, names it in
`NVMRC_SCRIPT_FD` and evaluates the shell code the binary writes there, while
messages stay on stdout and stderr; every other command runs the binary
directly. Without the function, `eval "$(nvm use 18)"` works by hand. The
load line calls `nvmrc`, which an `nvm` function left by nvm.sh cannot
intercept.

## Moving from nvm.sh

- `nvm doctor [--shell <shell>]` is read-only; it lists every place that
  still loads nvm.sh and exits 1 when it finds one.
- `nvm migrate --dry-run` shows the diff.
- `nvm migrate [--yes]` backs each file up as
  `<file>.nvmrc-backup-<yyyymmddThhmmssZ>`, comments the old lines as
  `# [nvmrc-migrated] ...`, adds the init block between
  `# >>> nvmrc init >>>` and `# <<< nvmrc init <<<`, and checks the syntax
  with the shell first.
- `nvm migrate --undo` restores the latest backup, backing the current file
  up first.

Lazy loaders are reported with a suggested fix and never edited; `$NVM_DIR`
is never touched.

## Environment variables

Read as nvm.sh does:

- `NVM_DIR`
- `NVM_COLORS`, `NVM_NO_COLORS`, `NVM_HAS_COLORS`, `NO_COLOR`, `TERM`
- `NVM_NODEJS_ORG_MIRROR`, `NVM_IOJS_ORG_MIRROR`, `NVM_AUTH_HEADER`
- `NVM_SYMLINK_CURRENT`, `NVM_INSTALL_LOCK_TIMEOUT`,
  `NVM_INSTALL_LOCK_STALE`, `NVM_INSTALL_THIRD_PARTY_HOOK`,
  `NVM_NO_SOURCE_FALLBACK`, `NVM_MAKE_JOBS`
- `NODE_VERSION` (for `nvm-exec`), `NODE_PATH`, `MANPATH`, `PREFIX`,
  `npm_config_prefix`/`NPM_CONFIG_PREFIX`, `CC`/`CXX` (source builds),
  `HOME`, `PWD`, `PATH`

For `doctor` and `migrate`: `ENV`, `ZDOTDIR`, `XDG_CONFIG_HOME`,
`NVM_LAZY_LOAD` (a conflict rule).

The version floor (`NVM_MIN_VERSION`, `$NVM_DIR/min-version`) comes from
the author's fork of nvm, not from nvm-sh/nvm.

nvmrc's own: `NVMRC_SCRIPT_FD`, `NVMRC_SHELL_KIND`, `NVMRC_SHELL`.

Not supported: `NVM_NO_PROGRESS` (there is no progress bar), `NVM_OFFLINE`
(use `--offline`), curl and wget settings (downloads use ureq; proxies come
from the usual `*_PROXY` variables).

## Commands

As nvm.sh: `version`, `current`, `ls`/`list`, `ls-remote`/`list-remote`,
`cache dir|clear`, `install`/`i` (binaries, and `-s` from source),
`install-latest-npm`, `reinstall-packages`/`copy-packages`, `uninstall`,
`version-remote`, `which`, `alias`, `unalias`, `use`, `deactivate`,
`set-colors`, `exec`, `run`.

New: `init`, `doctor`, `migrate`.

Not available: `unload`, `debug`, nvm.sh's `--help` text and `--version`
number, `self-install`/`self-update` (v2), legacy platforms (v2).

## Deviations

The behaviour follows nvm.sh wherever a test can compare it. The known
differences are listed, with the reason, in
[docs/deviations.md](docs/deviations.md). The most visible are the help text,
`--version`, and the order of messages when stdout and stderr are read
together.

## Exit codes

- 0: success.
- 1: failure.
- 2: missing target (binary download failed with no source fallback,
  `alias lts/<missing>`, `reinstall-packages` from the version in use).
- 3: invalid or unknown version.
- 4: `--reinstall-packages-from` the version being installed.
- 5: `--reinstall-packages-from` a version not installed.
- 6: options that cannot be combined.
- 7: below the version floor, or an invalid floor.
- 8: alias loop.
- 11: an npm prefix setting nvm cannot work with.
- 33: the third-party install hook claimed success.
- 55: unsupported option.
- 127: usage error, unknown command, command not found, or no system version.

`exec`, `run`, `nvm-exec` and the install hook pass their program's status on
(128+n after signal n). 10 and 42 are never produced (internal to nvm.sh, and
`nvm debug`).

## Development

`mise run setup` installs the tools (and may use Homebrew). Then:

- `mise run fmt`, `lint` (clippy, rumdl, actionlint, hadolint), `test`,
  `test:msrv`, `deny`, `audit`.
- `mise run check` runs all the gates, as CI does.
- `mise run docker:build` and `docker:test` run the suite in a Debian image
  with bash, zsh, dash, ksh and fish.
- `mise run compat:capture --nvm-sh <path>` re-runs the contract through a
  real nvm.sh and prints what differs.

Commits follow Conventional Commits and are GPG-signed. CI is in
`.github/workflows/ci.yml`.

## License

MIT, see `LICENSE`; nvm's copyright notice is kept.
