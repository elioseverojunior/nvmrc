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
  so every signature and attestation names a tagged release.
- Publishing is ordered so a failure never leaves a public release half
  done: the GitHub release is created as a draft
  (`mise run release:github:draft`), then the image is pushed, run by
  digest, signed and attested, then the formula goes to the tap, and the
  last step makes the release public. A re-run of the job updates the draft
  (files replaced with `--clobber`) and finds the formula already committed.
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

## The repository must be public

The repository is private today. GitHub artifact attestations need a public
repository (private ones only on Enterprise Cloud), and cosign's public
Sigstore log would record the repository name. So the workflow refuses to
publish until the repository is public. A manual dry run never signs or
attests, so it runs the same way in a private repository.

## Protecting the tags

The ancestry check stops a tag on an unmerged commit, but not someone who
can push tags from moving or deleting one. The owner should add a tag
ruleset for `v*` in the repository settings (Rules, Rulesets) that
restricts creation to maintainers and blocks updates and deletion. It is
not configured from this repository.

## Supply chain

The release build jobs run the owner's
`elioseverojunior/rust-toolchain@v0`, pinned by a loose tag, as the
workflows' other actions are. Whoever can move that tag decides what runs
inside the jobs that produce the signed binaries, so it is the main
binary-integrity risk. The mitigation is tag protection on `v0` in that
repository (or pinning the action to a commit SHA in `release.yml`).
Release jobs restore no cache: the Rust toolchain action runs without one,
and every `jdx/mise-action` step sets `cache: false`, so cosign, nfpm, syft
and git-cliff are downloaded and checked against `mise.lock` each run.
Nothing a previous run left can reach a signed archive. Checkouts set
`persist-credentials: false`, except the tap checkout, which pushes.

The image's CA certificates come from `alpine:3.24.2`, pinned by version
tag in `Dockerfile.release`: a newer Alpine is a reviewed bump, not a
silent change. The digest is not pinned, so a rebuilt tag is taken as is;
instead the release job runs the image it pushed, by digest, before it
signs and attests that digest. Builds are not bit-for-bit reproducible
(archive and package timestamps differ between runs); the attestations
name the exact bytes that were published.
