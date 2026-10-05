# Releasing nvmrc

How a release is built, and what it trusts. The commands to install and to
verify a release are in the [README](../README.md).

## Shape

- A `v*` tag equal to `v` plus the `Cargo.toml` version triggers
  `.github/workflows/release.yml`; `mise run release:check` enforces it.
- The tagged commit must be on the default branch: `mise run
  release:check:ci` refuses to publish a tag whose commit
  `git merge-base --is-ancestor` does not find there. A manual dry run may
  build any branch.
- A manual run is a dry run by default: it builds, packages and runs the
  smoke tests, and stops there. Only a run that publishes signs and attests,
  so every signature and attestation names a tagged release; a private
  repository publishes without either (see "A private repository").
- Publishing is ordered so a failure never leaves a published release half
  done: the GitHub release is created as a draft
  (`mise run release:github:draft`), then the image is pushed, run by
  digest, signed and attested (in a public repository only), then the
  formula goes to the tap, and the last step publishes the draft. A re-run
  of the job updates the draft (files replaced with `--clobber`) and finds
  the formula already committed.
- The packages depend on `ca-certificates`; the smoke tests install them
  with each distribution's package manager and run `nvm ls-remote` over
  HTTPS in the container.
- Every step is a `mise run release:*` task, so `mise run release:snapshot`
  runs the same build, packaging and smoke tests locally into `dist/`.
- There is no GoReleaser: packages come from nfpm
  (`packaging/nfpm.yaml`), the rest from the mise tasks. GoReleaser builds
  everything on one host, while the release builds on native runners and
  attests the exact bytes it ships.
- The Homebrew tap and its token are not configured, so publishing a formula
  is disabled: the steps run only when the repository variable
  `NVMRC_HOMEBREW_TAP` is set.
- nvmrc is never published to crates.io, by the owner's decision:
  `Cargo.toml` keeps `publish = false`, and `release:check` fails without it.

## A private repository

The repository stays private, and a tag still publishes: the GitHub release
with the archives, packages, SBOM and `SHA256SUMS`, the GHCR image (a
private package, like the repository) and, once configured, the Homebrew
formula. Only people who can read the repository can download them, so the
formula's URLs need a token until the repository is public.

A private release has checksums only, no signatures and no provenance
attestations. `mise run release:check:ci` prints `provenance=false` when
`github.event.repository.private` is true; the `sign-and-attest` job and
the image's `cosign sign` and `actions/attest` steps are then skipped, the
release is published from the unsigned files, and `mise run release:notes
--provenance false` says so in the notes. The ruling:

- `actions/attest` would fail: GitHub artifact attestations in a private
  repository need GitHub Enterprise Cloud (the action's README).
- cosign 3.1.3 (`mise.lock`) can sign without the Rekor log: a signing
  config from `cosign signing-config create --with-default-services
  --no-default-rekor` has no transparency log (`--tlog-upload=false` is
  deprecated in its favour), the bundle carries a timestamp instead, and
  `cosign verify-blob --insecure-ignore-tlog` accepts it. But keyless
  signing still asks Fulcio for a certificate, and Fulcio appends every
  certificate it issues to its public certificate transparency log; for a
  workflow that certificate names the repository, `release.yml` and the
  tag. Keyless signing would therefore publish the repository name anyway.
- A key pair kept in a secret would avoid both logs, but it is a key the
  owner has to keep and rotate, and it proves less than the workflow
  identity. It is not set up.

So a private release is verified with its checksums only, downloaded over
authenticated HTTPS:

```sh
gh release download v0.0.1 --repo elioseverojunior/nvmrc
shasum -a 256 --check --ignore-missing SHA256SUMS # macOS and Linux
```

This shows the files match the release's `SHA256SUMS`, not who built them:
anyone who can write to the release can replace both. Provenance
attestations and signatures are unavailable until the repository is public.
Nothing in the workflow changes then: the first tag after the switch is
signed and attested; earlier private releases stay unsigned.

## Protecting the tags

The ancestry check stops a tag on an unmerged commit, but not someone who
can push tags from moving or deleting one. The owner should add a tag
ruleset for `v*` in the repository settings (Rules, Rulesets) that
restricts creation to maintainers and blocks updates and deletion. It is
not configured from this repository.

## Supply chain

Every third-party action in the workflows, the owner's
`elioseverojunior/rust-toolchain` included, is pinned to a full commit SHA,
with the release it was resolved from as a trailing comment
(`uses: actions/checkout@<sha> # v7.0.1`). Moving a tag upstream therefore
changes nothing that runs in the jobs that produce the signed binaries.
`mise run actions:pins` reports stale pins (the `action-pin-checker`
workflow runs it weekly) and `mise run actions:pins -- --write` moves them
to the latest release within the same major version; a new major version
is only reported, and is applied with `--apply-major` after review.
Dependabot also proposes the bumps weekly.

Release jobs restore no cache: the Rust toolchain action runs without one,
and every `jdx/mise-action` step sets `cache: false`, so cosign, nfpm, syft
and git-cliff are downloaded and checked against `mise.lock` each run.
Nothing a previous run left can reach a signed archive. Checkouts set
`persist-credentials: false`, except the tap checkout, which pushes.

The image's CA certificates come from `alpine:3.24.2`, pinned by version
tag in `Dockerfile.release`: a newer Alpine is a reviewed bump, not a
silent change. The digest is not pinned, so a rebuilt tag is taken as is;
instead the release job runs the image it pushed, by digest, before it
signs and attests that digest (in a public repository). Builds are not
bit-for-bit reproducible (archive and package timestamps differ between
runs); the attestations and checksums name the exact bytes that were
published.
