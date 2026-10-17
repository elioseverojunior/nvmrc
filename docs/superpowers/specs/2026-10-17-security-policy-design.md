# Security policy, version rules and package managers

Date: 2026-10-17
Status: Design, not yet planned
Builds on: [the nvmrc design](2026-10-02-nvmrc-design.md) and
[deviations.md](../../deviations.md)

## 1. Intent

In enterprise pipelines the security team, not the project team, decides which
versions of the development SDKs are allowed. Today nothing in a pipeline stops
a project whose `.nvmrc` asks for an old, vulnerable or unvetted Node, npm,
yarn or pnpm.

nvmrc becomes the place that enforces that decision. Teams and companies that
adopt nvmrc as their standard toolset publish a policy file in a format nvmrc
owns. From then on every pipeline that runs nvmrc enforces it. A project on a
version the policy does not allow fails the build with an error that says
exactly what is wrong and how to fix it, and a developer can fix it with one
explicit command.

nvmrc also becomes able to install and pin npm, yarn and pnpm next to Node, so
the same file (`.nvmrc`) describes the whole toolchain and the same policy
covers all of it.

The format nvmrc defines is a contract other companies build on. It must be
strict and unambiguous: a typo in a policy must never turn into "everything
allowed", and nothing a project team sets may weaken what the security team
decided.

## 2. Goals

1. A versioned **policy file** format (JSON) with a published **JSON Schema**,
   loadable from a local path or a URL, and also committable in the project
   repository as `.nvmrc-policy.json`.
2. Per-tool **version rules** for **node, npm, yarn and pnpm**: a minimum, an
   allowed range, a deny list, LTS only (node), and a minimum release age.
   Flags and variables can only tighten the policy.
3. A **CVE check** for the Node version in use, failing at or above a severity
   limit the policy sets (default `high`).
4. **Clear errors**: requested version and its source, the rule it breaks and
   that rule's source, the CVEs with their severity, and the fix.
5. **`nvm init --node|--npm|--yarn|--pnpm`** generates a project's `.nvmrc`
   (and `packageManager` in `package.json`) that already satisfies the policy.
6. **`nvm upgrade`** moves an existing project to the newest allowed versions,
   within its current major unless the developer asks to cross it.
7. nvmrc **installs and pins npm, yarn and pnpm**, recorded in `.nvmrc`, with
   yarn and pnpm mirrored into `package.json`'s `packageManager`.

## 3. Non-goals

- Changing a project's versions without being asked. Enforcement reports;
  `init` and `upgrade` change files only when a person runs them.
- CVE checks for npm, yarn and pnpm. Their version rules are enforced; their
  CVEs are out of scope for this design.
- Caching the policy, the vulnerability data or the registry metadata between
  runs (nvmrc caches no remote index today; see deviations.md).
- Reading `.node-version`, `engines`, `volta` or `devEngines`.
- Using corepack. It is no longer bundled with Node from version 25, so relying
  on it would make nvmrc behave differently per Node version.

## 4. Anti-goals

Any of these makes the feature a failure even if it works on the happy path:

- In an explicit command (`install`, `use`, `exec`, `run`, `init`, `upgrade`),
  a policy that cannot be read, parsed or validated, or a CVE source or
  registry that cannot be reached, lets the command through. **Every failure to
  know is a failure.**
- An unknown field in a policy is ignored instead of rejected.
- A flag, environment variable or repository policy weakens a rule set by the
  central policy.
- `upgrade` crosses a major version without being asked.
- The flags break the shell function: `nvm --policy p use 20` must still switch
  the shell's Node.
- Behaviour without the new configuration changes. With no policy and none of
  the new flags or variables, nvmrc behaves exactly as today (including
  `NVM_MIN_VERSION` and `$NVM_DIR/min-version`), and the nvm.sh compat suite
  passes unchanged.
- Opening a shell or changing directory on a developer machine fails because of
  the policy. Automatic paths warn; they never block.
- `nvm init bash` (the existing shell snippet) changes behaviour.

## 5. Constraints from the existing code

- Commands parse their own arguments by hand (`args: Vec<String>`, as nvm.sh
  does); clap only routes subcommands. Env vars are read through the `Env`
  port so tests can fake them, so **clap's `env` feature is not used**.
- The shell function routes on `$1` (`src/shell/init/posix.rs:16-26`,
  `src/shell/init/fish.rs:26`); only `use`, `deactivate`, `install`, `i`,
  `set-colors` and `__auto` get the channel that applies shell code.
- A floor already exists: `VersionFloor` (`src/domain/floor.rs`) from
  `NVM_MIN_VERSION`, falling back to `$NVM_DIR/min-version`, checked only in
  `install`, exit code 7. This design extends it; it does not add a second
  minimum mechanism.
- `nvm init <shell> [--no-use|--install]` exists (`src/commands/init.rs`) and
  rejects any other option today, so the new flags do not collide. There is no
  `upgrade` command.
- `install --offline` exists.
- The `.nvmrc` parser (`src/domain/nvmrc/mod.rs:66-96`) already accepts
  `key=value` lines and ignores keys other than `node`, so `npm=`, `yarn=` and
  `pnpm=` lines stay readable by nvm.sh.
- The remote index is `index.tab`. It carries each release's date and LTS name
  (only the version and LTS columns are parsed today) and a security flag, but
  no severity.
- There are no JSON dependencies. New crates must pass `cargo deny` (crates.io
  only, license allowlist in `deny.toml`). The project's rules still apply:
  ports and adapters, `&dyn` fakes, no network in tests, files under 300
  lines, functions under 30, TDD, every nvm.sh deviation pinned by a test and
  listed in deviations.md.

## 6. Design

### 6.1 The policy file

JSON, validated against
`schemas/nvmrc-policy.v1.schema.json` (published in this repository). Example:

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
    },
    "npm": { "minimum": "10.2.4" },
    "pnpm": { "minimum": "9.1.0", "minimumReleaseAgeDays": 7 },
    "yarn": { "allow": ">=4.1.0 <5" }
  },
  "cve": {
    "failOnSeverity": "high",
    "source": "https://raw.githubusercontent.com/nodejs/security-wg/main/vuln/core/index.json"
  }
}
```

Rules:

- `schemaVersion` is required and must be `1`. Any other value is an error
  naming the versions this nvmrc understands.
- Unknown fields are rejected at every level.
- `tools` and every tool and field inside it are optional; an absent field
  means that rule does not restrict the tool.
- Per-tool fields:
  - `minimum`: the same syntax `NVM_MIN_VERSION` accepts (`20`, `v20`,
    `20.11.1`); a partial version means its lowest release (`20` is `20.0.0`).
  - `allow`: an npm-style semver range (`>=20 <25`, `^20.11.1 || ^22`).
  - `deny`: a list of exact versions.
  - `ltsOnly` (node only): `true` allows only releases that have an LTS name in
    the index. Setting it on another tool is a validation error.
  - `minimumReleaseAgeDays`: a non-negative integer; a release is allowed only
    once it has been published for at least that many days.
- `cve` is optional. `failOnSeverity` is one of `critical`, `high`, `medium`,
  `low`, `none`, default `high`. `none` disables failing but still reports.
  When a policy is configured, the CVE check runs even if `cve` is absent.
- `cve.source` is optional, a URL or a local path like the policy itself. It
  defaults to the Node Security WG core vulnerability index and exists so
  air-gapped pipelines can point at a mirror.
- `$schema` is allowed and ignored, so editors can validate policies.

The serde types with `deny_unknown_fields` are the enforcement. The schema file
is the published contract. A test generates the schema from the types and fails
when it differs from the committed file, so there is one source of truth
(DRY).

### 6.2 When a version is allowed

A version of a tool is **allowed** when all of these hold:

1. it is at or above `minimum`;
2. it is inside `allow`;
3. it is not in `deny`;
4. for node with `ltsOnly`, it has an LTS name;
5. it has been published for at least `minimumReleaseAgeDays`;
6. for node, it has no CVE at or above the severity limit (6.6).

Release dates come from `index.tab` for node and from the registry's `time`
metadata for npm, yarn and pnpm. Rule 5 needs that data, so an unreachable
index or registry is a failure to know when the rule is set.

One function decides this, and enforcement (6.5), `init` (6.8) and `upgrade`
(6.9) all use it.

### 6.3 Where the policy and rules come from

New flags, each with an environment variable:

- `--policy <path|url>`, `NVMRC_POLICY`.
- `--min-node <version>`, `NVM_MIN_VERSION` (the existing variable; the flag
  feeds the existing floor).
- `--min-npm`, `--min-yarn`, `--min-pnpm` with `NVMRC_MIN_NPM`,
  `NVMRC_MIN_YARN`, `NVMRC_MIN_PNPM`.
- `--fail-on-severity <level>`, `NVMRC_FAIL_ON_SEVERITY`.

The other rules (`allow`, `deny`, `ltsOnly`, `minimumReleaseAgeDays`) come only
from policy files.

**The repository policy.** A project can commit a `.nvmrc-policy.json`, with
the same schema as the central policy. nvmrc finds it the way it finds
`.nvmrc`: walking up from the working directory. It is one more source in the
merge below, so it can tighten rules but never weaken the central policy. Its
`cve.source` is used only when no central policy is configured; otherwise the
central policy's source wins, so a project cannot point the check at a source
of its own choosing.

**Overrides only tighten.** The sources are: flag, environment variable,
central policy (`--policy` or `NVMRC_POLICY`), repository policy, and
`$NVM_DIR/min-version` (node only). Each rule is merged across every source
that sets it:

- `minimum`: the highest wins.
- `allow`: the intersection of all ranges (a version must satisfy every one).
- `deny`: the union of all lists.
- `ltsOnly`: on if any source turns it on.
- `minimumReleaseAgeDays`: the largest wins.
- severity limit: the strictest wins (`low` is stricter than `high`; `none` is
  the loosest).

A source that tries to loosen a rule is not an error: its value loses, and
nvmrc prints a warning naming that source and the value that won. The error for
a violation names the source of the rule that was broken.

A value starting with `http://` is rejected; `https://` is fetched through the
existing `Http` port (with `NVM_AUTH_HEADER` handling as today); anything else
is a local path. A policy that cannot be fetched, read, parsed or validated is
an error, never "no policy".

### 6.4 Flag placement and the shell function

The flags are accepted before the subcommand (`nvm --policy p use 20`) and
after it (`nvm use --policy p 20`). They are stripped before the subcommand's
own argument parsing, so no command parses them itself.

The generated shell function (POSIX and fish) skips leading
`--policy`, `--min-*` and `--fail-on-severity` options, in both `--flag value`
and `--flag=value` forms, before deciding the subcommand, so `use`, `install`
and the rest still get the shell channel. In pipelines the environment
variables are the expected interface; the flags are for overrides.

### 6.5 When and where it is enforced

**Legacy mode.** When only `NVM_MIN_VERSION` (as a variable) or
`$NVM_DIR/min-version` is set, nvmrc behaves exactly as today: the node floor
is checked in `install` only, and there is no CVE check.

**Policy mode** is on when a central policy is configured, a repository policy
is found, or any of the new flags or `NVMRC_*` variables is set. A repository
policy alone is enough, so a team can protect itself before the security team
publishes a central policy. `--min-node` counts as a new flag even though it
feeds `NVM_MIN_VERSION`. In policy mode:

- Explicit commands check that every version in effect is allowed (6.2), and
  fail on any violation or failure to know:
  - `install`: before any download.
  - `use`, `exec`, `run`: on the resolved version, because a pipeline may use a
    Node that was already installed. `use system` checks the system Node's
    version.
  - `init` and `upgrade`: before writing anything.
- Automatic paths only warn: `__auto` (the `cd` hook) and the `use` or
  `install` that the `nvm init <shell>` snippet runs at shell start. A
  violation or a failure to know prints a warning and the command proceeds as
  it would without a policy.
- Commands that only report (`ls`, `ls-remote`, `current`, `which`, `version`,
  `version-remote`) do not enforce.

The versions in effect are node's, and npm's, yarn's and pnpm's as described
in 6.7. The npm, yarn and pnpm pins are always read from the nearest `.nvmrc`,
even when the Node version comes from a command-line argument.

`install --offline` in policy mode needs a local policy and a local
`cve.source`, and no `minimumReleaseAgeDays` rule; otherwise it fails as a
failure to know.

### 6.6 CVE check

The format of `cve.source` is the Node Security WG core index
(`vuln/core/index.json`): entries keyed by id, with `cve`, `vulnerable`
(npm-style semver ranges), `patched`, `severity` and, when present,
`affectedEnvironments`. A mirror must serve this format.

nvmrc finds entries whose `vulnerable` range contains the Node version and
whose `affectedEnvironments`, when present, include the current platform. It
compares each entry's severity with the limit:

- At or above the limit: the version is not allowed (exit 7 in explicit
  commands).
- Below the limit: the version is allowed, with a warning listing them.
- An entry with no severity counts as `high`.
- A severity outside the five levels, an unparseable range, or an unreachable
  or unparseable source is a failure to know.

### 6.7 npm, yarn and pnpm in `.nvmrc`

```text
20.11.1
npm=10.9.0
pnpm=9.12.0
```

- `.nvmrc` is the source of truth. `npm=`, `yarn=` and `pnpm=` take an exact
  `major.minor.patch` version, so `use` never needs the network to resolve
  them. A project pins at most one of yarn or pnpm; pinning both is an error.
- When a project pins yarn or pnpm and has a `package.json`, its
  `packageManager` must be `<name>@<version>` with the same name and version.
  A corepack hash suffix (`+sha512.…`) is allowed and ignored by the
  comparison. A missing field or a different value fails with an error showing
  both and naming the file to fix; nvmrc never rewrites `package.json` outside
  `init` and `upgrade`.
- `install` also installs the pinned tools. nvmrc downloads the package tarball
  from the npm registry (`NVMRC_NPM_REGISTRY`, default
  `https://registry.npmjs.org`), verifies the registry's integrity hash, and
  installs it under `$NVM_DIR/package-managers/<name>/<version>/`, shared by
  all Node versions. Packages: `npm` for npm, `pnpm` for pnpm, `yarn` for yarn
  1.x, and `@yarnpkg/cli-dist` for yarn 2 and later.
- `use` puts the pinned versions' `bin` directories on `PATH` ahead of Node's,
  so a pinned npm shadows the one bundled with Node. Two projects on the same
  Node can pin different versions without colliding.
- The rules apply to the version in effect: the pinned one, or for npm without
  a pin, the npm bundled with the Node in use.

### 6.8 `nvm init` project mode

- `nvm init <shell> [--no-use|--install]` is unchanged.
- `nvm init` with one or more of `--node[=<version>]`, `--npm[=<version>]`,
  `--yarn[=<version>]`, `--pnpm[=<version>]` and no shell name enters project
  mode.
- A shell name together with a project flag is a usage error, as are `--yarn`
  and `--pnpm` together.
- Without a version, each tool gets the newest allowed release (6.2); for node
  this is the newest allowed LTS, whether or not the policy sets `ltsOnly`.
  For yarn, candidates come from `@yarnpkg/cli-dist`.
- With a version, it must be exact and allowed, or the command fails before
  writing.
- It writes `.nvmrc` in the working directory, setting only the keys it was
  asked for and keeping the rest. For yarn or pnpm, it sets `packageManager` in
  an existing `package.json`.

### 6.9 `nvm upgrade`

`nvm upgrade [--node] [--npm] [--yarn] [--pnpm] [--major[=<N>]]`

- Without tool flags it upgrades every tool the nearest `.nvmrc` pins; with
  them, only those tools. A tool flag for a tool `.nvmrc` does not pin is an
  error (use `init` to add it).
- **Default:** each tool moves to the newest allowed release (6.2) within the
  major it already uses. It never crosses a major on its own.
- **`--major`:** moves to the newest allowed release in any major, which may
  jump several majors (20 to 25).
- **`--major=<N>`:** moves to the newest allowed release in major `N`. With
  several tool flags, `--major=<N>` is a usage error unless exactly one tool is
  selected.
- When the current major has no allowed release, the command fails before
  writing anything, says so, and suggests `--major` and the lowest major that
  has an allowed release.
- When a tool is already on the newest allowed release, it is left unchanged
  and reported as up to date.
- It rewrites `.nvmrc` and, for yarn or pnpm, `packageManager` in
  `package.json`; then it installs the new versions as `install` would, and
  prints each change (`node 20.11.1 -> 20.18.0`). If the install fails, the
  files are already updated and the error says so.
- It fails before writing anything if any selected tool has no allowed target,
  so a project is never left half upgraded in its files.

### 6.10 Errors and exit codes

- **7**: a policy violation (a version that is not allowed, for any rule in
  6.2), as the floor today; also `upgrade` when no allowed target exists.
- **9** (new, unused by nvm.sh): a failure to know (policy, CVE source or
  registry unreachable, unreadable, invalid or `http://`; an unknown severity;
  offline with a remote source). Pipelines can tell "not allowed" from "could
  not check".

Every violation message names:

- the tool and the version requested, and where it came from (`.nvmrc` path,
  argument, `package.json`, bundled with Node);
- each rule it breaks, and where that rule came from (flag, variable name,
  central policy path or URL, repository policy path, `min-version`);
- for CVEs, each CVE id with its severity;
- the fix, naming the command when there is one (`nvm upgrade --node`, or
  `nvm upgrade --node --major` when the current major has no allowed release).

Example:

```text
nvm: node v18.19.0 (from /work/app/.nvmrc) is not allowed by policy
  https://sec.example.com/nvmrc-policy.json:
  - below the minimum 20.11.1
  - CVE-2024-27983 (high), CVE-2024-22019 (high)
No allowed release in major 18. Run `nvm upgrade --node --major`.
```

## 7. Delivery order

Each step is releasable on its own:

1. Policy file and schema, flags and variables, the repository policy,
   tighten-only merging, legacy and policy modes, the node rules except CVEs at
   every enforcement point, warn-only automatic paths, exit code 9.
2. CVE check.
3. npm, yarn and pnpm pinning and install, the `packageManager` check, rules
   for npm, yarn and pnpm.
4. `nvm init` project mode.
5. `nvm upgrade`.

## 8. Left to the builder

- Crate choice for JSON, schema generation and npm-style semver ranges (likely
  `serde`, `serde_json`, and `schemars` as a dev-dependency), within
  `cargo deny`.
- Module layout, port design for the vulnerability source and the registry,
  and how the loaded policy reaches commands (likely a `Context` field set in
  `with_real_context`).
- Exit code for the `packageManager` divergence and for pinning both yarn and
  pnpm, chosen from the existing codes and documented in the README.
- How the system Node's and the bundled npm's versions are read.
- How "now" is taken for `minimumReleaseAgeDays` (the existing `Clock` port).
- Error and warning wording, beyond the content listed in 6.10.
- How each package's tarball becomes runnable `bin` entries.
- Updating `nvm init`'s `--help`, adding `upgrade`'s, the README (variables,
  commands, exit codes), and deviations.md.

## 9. Calls made without explicit confirmation

- The flag reuses `NVM_MIN_VERSION` for node; new variables use the `NVMRC_`
  prefix.
- Per-tool `--min-*` flags and `--fail-on-severity` exist alongside
  `--min-node`; `allow`, `deny`, `ltsOnly` and `minimumReleaseAgeDays` have no
  flags.
- A loosening override is a warning, not an error.
- The repository policy is named `.nvmrc-policy.json` and found by walking up
  like `.nvmrc`; its `cve.source` is ignored when a central policy exists.
- `NVM_MIN_VERSION` or `min-version` alone stays in legacy mode (install only,
  no CVE check); any new flag or variable switches to policy mode.
- `http://` policy and CVE URLs are rejected.
- The Security WG index format is the contract for `cve.source` mirrors;
  platform-specific CVEs only count on their platform; a CVE entry without
  severity counts as `high`; an unknown severity is a failure to know.
- CVE checks cover Node only.
- Exit code 9 for a failure to know.
- npm, yarn and pnpm live in a shared store, not inside each Node version, and
  their pins are exact versions.
- The rules for npm apply to the bundled npm when npm is not pinned.
- At most one of yarn and pnpm per project; the corepack hash suffix is ignored
  when comparing `packageManager`.
- `init` without a version picks the newest allowed release (LTS for node), and
  updates an existing `.nvmrc` in place.
- `upgrade` is the command name; it installs after rewriting the files, refuses
  to write anything unless every selected tool has a target, and does not add
  tools `.nvmrc` does not pin.
