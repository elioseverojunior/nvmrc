# nvmrc

A native Rust port of [nvm](https://github.com/nvm-sh/nvm). Three binaries are
built from one library: `nvmrc`, `nvm` (the same program, under the name
scripts, CI and tools call) and `nvm-exec`.

nvmrc uses the same `$NVM_DIR` layout as nvm, so versions installed by either
tool work with both. It follows nvm.sh's commands, options, messages and exit
codes, checked against the test scenarios of the author's fork of nvm
(`tests/compat/`); the deviations are in [docs/deviations.md](docs/deviations.md).

## Install

Releases are built from a `v*` tag by `.github/workflows/release.yml`, on
native runners. The version is the one `nvm --version` prints.

The repository is private, and a release publishes its files anyway: the
GitHub release, the GHCR image and, once configured, the Homebrew formula,
each readable only by those who can read the repository. While it is
private, a release has checksums only: provenance attestations and
signatures are unavailable until the repository is public (see "Verifying
a release"). There is no published release yet, and the channels below
describe what a release will contain.

- Archives, `nvmrc-<version>-<target>.tar.gz`, each with `nvmrc`, `nvm`,
  `nvm-exec`, `LICENSE` and `README.md`:
  - macOS: `aarch64-apple-darwin`, `x86_64-apple-darwin`.
  - Linux, static (any distribution): `x86_64-unknown-linux-musl`,
    `aarch64-unknown-linux-musl`.
  - Linux, glibc 2.34 or newer: `x86_64-unknown-linux-gnu`,
    `aarch64-unknown-linux-gnu`.
- Packages, built from the static binaries (the binaries go to
  `/usr/bin`). Their one dependency is `ca-certificates`, the trust store
  HTTPS needs, so install them with the package manager, which pulls it in.
  They are not signed with a distribution key: check them as "Verifying a
  release" shows.
  - Debian, Ubuntu:
    `sudo apt-get install ./nvmrc_<version>-1_<amd64|arm64>.deb`
  - Fedora, RHEL, Amazon Linux 2023:
    `sudo dnf install ./nvmrc-<version>-1.<x86_64|aarch64>.rpm`
  - Alpine:
    `apk add --update --allow-untrusted nvmrc_<version>-r1_<x86_64|aarch64>.apk`
  - Arch: `sudo pacman -U nvmrc-<version>-1-<x86_64|aarch64>.pkg.tar.zst`
- Docker (linux/amd64, linux/arm64): the image holds the static binaries
  and CA certificates only, no shell and no libc, so it is for one-off
  commands and for copying the binaries into another image:
  `docker run --rm ghcr.io/elioseverojunior/nvmrc:<version> nvmrc --version`,
  or in a Dockerfile
  `COPY --from=ghcr.io/elioseverojunior/nvmrc:<version> /usr/bin/nvm* /usr/local/bin/`.
- Nix: `nix profile install github:elioseverojunior/nvmrc`, or
  `nix run github:elioseverojunior/nvmrc -- --version`.
- Homebrew: each release renders a formula; it is pushed to the tap named
  by the repository variable `NVMRC_HOMEBREW_TAP` once the owner sets it.
  None is published yet.
- From source: `cargo install --locked --git
  https://github.com/elioseverojunior/nvmrc` installs the three binaries
  into `~/.cargo/bin` (nvmrc is never published to crates.io),
  or `cargo build --release --locked` from a clone puts them in
  `target/release/`. Rust 1.85 or newer (MSRV); development uses the 1.99
  toolchain of `rust-toolchain.toml`. The release profile (fat LTO,
  `strip = "symbols"`, `panic = "abort"`) lives in `.cargo/config.toml`;
  with rustup, the `llvm-tools` component that `rust-toolchain.toml`
  declares provides the `rust-objcopy` that stripping uses.
- Windows is not supported; building for it stops with a compile error.
- An `nvm` function left by nvm.sh in a shell startup file shadows the
  `nvm` binary in interactive shells: `nvmrc doctor` finds it and
  `nvm migrate` replaces it (see "Moving from nvm.sh").
- For tools that run `$NVM_DIR/nvm-exec` by path, link it:
  `ln -s "$(command -v nvm-exec)" "$NVM_DIR/nvm-exec"`.

## Verifying a release

A release made while the repository is private is checked with its
checksums only, from a download over authenticated HTTPS; it proves the
files match the release's `SHA256SUMS`, not who built them
([docs/release.md](docs/release.md) has the ruling):

```sh
gh release download v0.0.1 --repo elioseverojunior/nvmrc
shasum -a 256 --check --ignore-missing SHA256SUMS # macOS and Linux
```

Once the repository is public, every file of a release is attested by
GitHub (SLSA build provenance), each binary inside the archives is
attested on its own, and the SBOM
(`nvmrc-<version>.spdx.json`, the dependency graph of `Cargo.lock`) is
attested as such. Only a tagged release is signed or attested; the checks
below also pin the release workflow and the tag, so nothing a manual run
or another workflow attested passes. With the GitHub CLI:

```sh
release=(--repo elioseverojunior/nvmrc
  --signer-workflow elioseverojunior/nvmrc/.github/workflows/release.yml
  --source-ref refs/tags/v0.0.1)
gh attestation verify nvmrc-0.0.1-x86_64-unknown-linux-musl.tar.gz "${release[@]}"
gh attestation verify /usr/bin/nvm "${release[@]}"
gh attestation verify oci://ghcr.io/elioseverojunior/nvmrc:0.0.1 "${release[@]}"
gh attestation verify nvmrc_0.0.1-1_amd64.deb "${release[@]}" \
  --predicate-type https://spdx.dev/Document/v2.3
```

Every file is also signed with Sigstore by the release workflow (keyless;
the bundle is `<file>.sigstore.json`). With cosign, check `SHA256SUMS`,
then the files against it, and the image:

```sh
identity='^https://github\.com/elioseverojunior/nvmrc/\.github/workflows/release\.yml@refs/tags/v'
issuer=https://token.actions.githubusercontent.com
cosign verify-blob --bundle SHA256SUMS.sigstore.json \
  --certificate-identity-regexp "$identity" --certificate-oidc-issuer "$issuer" SHA256SUMS
shasum -a 256 --check --ignore-missing SHA256SUMS # macOS and Linux
cosign verify ghcr.io/elioseverojunior/nvmrc:0.0.1 \
  --certificate-identity-regexp "$identity" --certificate-oidc-issuer "$issuer"
```

The commands use bash arrays (bash, zsh). The cosign identity names the
release workflow at a `v*` tag; `gh` checks the exact tag. A macOS archive
downloaded with a browser is quarantined by Gatekeeper: after verifying it,
`xattr -d com.apple.quarantine nvmrc nvm nvm-exec` on the unpacked binaries.

None of these can run before the first published release; the commands
are the ones the workflow's identities and file names produce. How the
release is built and what it trusts: [docs/release.md](docs/release.md).

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
number, `self-install`/`self-update` (v2), legacy platforms (v2),
Windows.

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

Releasing: set `version` in `Cargo.toml`, run `cargo check` (it updates
`Cargo.lock`), commit both (`chore(release): v0.2.0`), then
`git tag -s v0.2.0 -m "nvmrc 0.2.0"` and push the tag. The workflow
refuses a tag that is not `v` plus the `Cargo.toml` version
(`mise run release:check`). A manual run of the release workflow is a dry
run by default, and `mise run release:snapshot` runs the same build,
packaging and smoke tests locally into `dist/` (Docker required).

Commits follow Conventional Commits and are GPG-signed. CI is in
`.github/workflows/ci.yml`.

## License

MIT, see `LICENSE`; nvm's copyright notice is kept.
