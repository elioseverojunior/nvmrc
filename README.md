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

nvmrc's own: `NVMRC_SCRIPT_FD`, `NVMRC_SHELL_KIND`, `NVMRC_SHELL`, and
`NVMRC_POLICY` (the central security policy, see
[Security policy](#security-policy)), `NVMRC_NPM_REGISTRY`,
`NVMRC_NPM_AUTH_HEADER` and `NVMRC_MIN_NPM`/`NVMRC_MIN_YARN`/`NVMRC_MIN_PNPM`
(see [npm, yarn and pnpm](#npm-yarn-and-pnpm)).

Not supported: `NVM_NO_PROGRESS` (there is no progress bar), `NVM_OFFLINE`
(use `--offline`), curl and wget settings (downloads use ureq; proxies come
from the usual `*_PROXY` variables).

## Security policy

A security team can decide which Node versions a pipeline may use. The
policy is a JSON file, checked against
[schemas/nvmrc-policy.v1.schema.json](schemas/nvmrc-policy.v1.schema.json);
the design is in
[docs/superpowers/specs/2026-10-17-security-policy-design.md](docs/superpowers/specs/2026-10-17-security-policy-design.md).

```json
{
  "$schema": "https://raw.githubusercontent.com/elioseverojunior/nvmrc/main/schemas/nvmrc-policy.v1.schema.json",
  "schemaVersion": 1,
  "tools": {
    "node": {
      "minimum": "20.11.1",
      "allow": ">=20 <25",
      "deny": ["20.12.0", "22.3.0"],
      "ltsOnly": true,
      "minimumReleaseAgeDays": 14
    }
  }
}
```

The node rules are `minimum`, `allow` (an npm-style range), `deny`,
`ltsOnly` and `minimumReleaseAgeDays`; the `cve` settings are described in
[CVE check](#cve-check). An unknown field is an error, never ignored. The
`tools.npm`, `tools.yarn` and `tools.pnpm` sections take `minimum`, `allow`,
`deny` and `minimumReleaseAgeDays` and are enforced; see
[npm, yarn and pnpm](#npm-yarn-and-pnpm).

These sources take part, in this order:

- `NVMRC_POLICY` or `--policy <path|https-url>`: the central policy. A path
  is read from the working directory; `https://` is fetched, and a redirect
  away from https is refused; `http://` is rejected. An empty
  `NVMRC_POLICY` is treated as unset. When both are given, `--policy`
  wins and `NVMRC_POLICY` is not read at all: there is one central policy.
- `.nvmrc-policy.json`, found from the working directory upward like
  `.nvmrc`: the repository policy.
- `--min-node <version>`: a minimum of its own.
- `--min-npm`, `--min-yarn`, `--min-pnpm <version>`: the same for those
  tools, with `NVMRC_MIN_NPM`, `NVMRC_MIN_YARN` and `NVMRC_MIN_PNPM` as their
  variables.
- `NVM_MIN_VERSION`: a minimum, merged with the others in policy mode.
- `$NVM_DIR/min-version`: the same, from the file.

The flags go before the subcommand or anywhere after it up to a `--`
(`nvm --policy p use 20`, `nvm use 20 --min-node 20`). After `exec` and
`run` they are taken only immediately after the subcommand: any other option
in between ends them, and a later occurrence belongs to the program they run
and is left alone. A flag with no value fails with
`nvm: <flag> needs a value.` and status 127; a flag given twice fails with
`nvm: <flag> is given twice.`.

The sources only tighten each other, so every rule takes the strictest value
across them: the highest minimum wins, the allowed ranges all apply, the
deny lists add up, `ltsOnly` is on when any source sets it, and the largest
release age wins. A tie names the central policy, then the repository
policy, and so on down the list. A source that asks for less is ignored
with a warning.

Two modes:

- Legacy: only `NVM_MIN_VERSION` or `$NVM_DIR/min-version`. This is exactly
  the old behaviour, checked in `install` alone.
- Policy: any central policy, repository policy or policy flag (`--min-node`
  included). `NVM_MIN_VERSION` and `$NVM_DIR/min-version` then count as
  layers too, and can be the source a violation names. The rules
  are enforced in `install`, `use` (`use system` included), `exec` and
  `run`. The shell startup (`__auto`) only warns and goes on; its policy
  and index fetches get one try of at most 5 seconds, so a silent network
  never holds the shell.

A violation stops with status 7:

```text
nvm: node v18.19.0 (from /w/app/.nvmrc) is not allowed by policy:
  - below the minimum v20.11.1 (from --min-node)
Raise /w/app/.nvmrc to an allowed version.
```

The version comes from the `.nvmrc` path, `the command line`, `system` or
`the active version`. A policy that cannot be checked is never let through:
an unreadable or invalid policy, an unreachable index for a release-date or
LTS rule, `--offline` with such a rule, an unreadable system node version
or `min-version` file all stop with status 9.

### CVE check

In policy mode, `install`, `use`, `exec` and `run` also look the node
version up in the Node.js security advisories. The policy can set:

```json
{
  "schemaVersion": 1,
  "cve": { "failOnSeverity": "high", "source": "vuln/index.json" }
}
```

A version is affected by an advisory when it is in `vulnerable` and not in
`patched`. An advisory that lists `affectedEnvironments` counts only when it
names `all` or this OS; with no known platform, or an empty list, it counts.
A missing or `unknown` severity is `high`; any other value that is not
`critical`, `high`, `medium` or `low` means the index cannot be understood.
An advisory without CVE ids is named `advisory <id>`. io.js versions are not
checked.

The limit is the lowest severity that fails. It comes from
`--fail-on-severity <critical|high|medium|low|none>`,
`NVMRC_FAIL_ON_SEVERITY`, and `cve.failOnSeverity` in the central and the
repository policy. The strictest wins (`low` is stricter than `high`, `none`
is the loosest), the default is `high`, and a looser override is ignored
with a warning. A non-blank `NVMRC_FAIL_ON_SEVERITY` turns policy mode on, as a
flag does.

```text
nvm: node v22.20.0 (from the command line) is not allowed by policy:
  - CVE-2026-58043 (high); the limit is high (from NVMRC_FAIL_ON_SEVERITY)
Use an allowed version.
```

A finding at or above the limit stops with status 7. Findings below it only
print `nvm: warning: node vX has known vulnerabilities below the limit
<limit>: ...` and the status is unchanged; with `none`, every finding is
reported that way.

The index is
`https://raw.githubusercontent.com/nodejs/security-wg/main/vuln/core/index.json`
unless `cve.source` (a URL or a path) says otherwise. When a central policy
exists, only its `cve.source` counts, even if the repository policy names
one; without a central policy the repository policy's is used. A relative
`cve.source` is read next to the policy file that holds it, and is invalid
in a policy fetched over https. `use --save` in policy mode writes `.nvmrc`
only after the policy allows the version.

An index that cannot be read or understood stops with status 9
(`nvm: cannot check the policy: the vulnerability index <location> is not
valid: ...` when it is invalid). If node rules already fail, the status stays
7 and a `nvm: warning: cannot check known vulnerabilities: ...` is added.
`--offline` with an https index stops with status 9 before any request; a
local index works offline. The shell startup (`__auto`) warns and goes on.
Legacy mode never fetches the index.

## npm, yarn and pnpm

A `.nvmrc` can pin the package managers next to the node version:

```text
20.11.1
npm=10.9.0
pnpm=9.12.0
```

Each pin is an exact `major.minor.patch` of digits: no `v`, no prerelease, no
range. Yarn and pnpm together is an error. The pins come from the nearest
`.nvmrc` and are read by `install`, `use`, `exec` and `run` with or without
a policy, even when the node version is given on the command line.

When yarn or pnpm is pinned and a `package.json` sits beside that `.nvmrc`,
its `packageManager` must be `<name>@<version>` for the same tool and
version (a `+<hash>` suffix is ignored). nvmrc never writes `package.json`.
A bad pin, a diverging `packageManager` and a pinned tool that is not
installed (in an explicit `use`, `exec` or `run`) stop with status 3; the
automatic paths (`__auto`, the init snippet) only warn.

`install` installs the pinned tools after the node and before activating it.
They go to `$NVM_DIR/package-managers/<tool>/<version>/`, with `package/`
(the unpacked package) and `bin/` (the commands), shared by all node
versions. The packages are `npm`, `pnpm`, `yarn` below 2.0.0 and
`@yarnpkg/cli-dist` from 2.0.0. The tarball must match a `sha512` entry of
`dist.integrity`, a command may not point outside its package, and a failed
install keeps the previous one. With `--offline`, a pinned tool that is not
in the store fails.

`use`, `exec` and `run` on an installed node put each pinned tool's `bin`
ahead of node's on `PATH` and drop the tool entries of other projects.
`use system` adds none, and `deactivate` removes them.

The registry is `NVMRC_NPM_REGISTRY` (default `https://registry.npmjs.org`,
https only). `NVMRC_NPM_AUTH_HEADER` is sent only to the registry's host.
`NVM_AUTH_HEADER` is never sent to the npm registry, a tarball host or the
built-in vulnerability index; it still goes to the Node mirrors, a central
policy and a configured `cve.source`.

The policy rules for these tools (`minimum`, `allow`, `deny`,
`minimumReleaseAgeDays`) come from the same sources as node's, with
`--min-npm`, `--min-yarn` and `--min-pnpm`, and tighten the same way;
`ltsOnly` does not apply and their CVEs are not checked. A tool is checked
when pinned. Without an npm pin, the npm bundled with the node in use is
checked, and only when an npm rule exists. The release age uses the
registry's `time` metadata. A violation exits 7, a check that cannot be made
exits 9.

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
- 3: invalid or unknown version; an invalid `npm=`/`yarn=`/`pnpm=` pin, a
  `packageManager` that disagrees with the pin, or a pinned tool that is not
  installed in an explicit `use`, `exec` or `run`.
- 4: `--reinstall-packages-from` the version being installed.
- 5: `--reinstall-packages-from` a version not installed.
- 6: options that cannot be combined.
- 7: below the version floor, or not allowed by the policy; or an invalid
  floor.
- 8: alias loop.
- 9: the policy could not be checked (see [Security policy](#security-policy)).
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
