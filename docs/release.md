# Releasing nvmrc

How a release is built, and what it trusts. The commands to install and to
verify a release are in the [README](../README.md).

## Shape

- A `v*` tag equal to `v` plus the `Cargo.toml` version triggers
  `.github/workflows/release.yml`; `mise run release:check` enforces it.
- Every step is a `mise run release:*` task, so `mise run release:snapshot`
  runs the same build, packaging and smoke tests locally into `dist/`.
- There is no GoReleaser: packages come from nfpm
  (`packaging/nfpm.yaml`), the rest from the mise tasks. GoReleaser builds
  everything on one host, while the release builds on native runners and
  attests the exact bytes it ships.
- The Homebrew tap and its token are not configured, so publishing a formula
  is disabled: the steps run only when the repository variable
  `NVMRC_HOMEBREW_TAP` is set.

## The repository must be public

The repository is private today. GitHub artifact attestations need a public
repository (private ones only on Enterprise Cloud), and cosign's public
Sigstore log would record the repository name. So the workflow refuses to
publish until the repository is public. A manual dry run in a private
repository builds and smoke-tests, skips cosign and lets the attestation
steps fail without failing the job.

## Supply chain

The release build jobs run the owner's
`elioseverojunior/rust-toolchain@v0`, pinned by a loose tag, as the
workflows' other actions are. Whoever can move that tag decides what runs
inside the jobs that produce the signed binaries, so it is the main
binary-integrity risk. The mitigation is tag protection on `v0` in that
repository (or pinning the action to a commit SHA in `release.yml`).
Release builds do not use a build cache (`cache: false`), so nothing a
previous run left can reach a signed archive.
