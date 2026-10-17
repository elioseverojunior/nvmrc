# nvmrc Plan 13: npm, yarn and pnpm Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver step 3 of the security policy design: pin npm, yarn and
pnpm in `.nvmrc`, install them from the npm registry into a shared store with
integrity checks, put them on `PATH` in `use`, `exec` and `run`, check
`packageManager` in `package.json`, and enforce the policy rules (and the
`--min-npm`, `--min-yarn`, `--min-pnpm` overrides) for every tool in effect.

**Architecture:** The node-only rule types become per-tool (`Tool`,
`ToolLayer`, `ToolPolicy`) and the mode merges four of them. Pure code reads
pins (`domain/nvmrc/pins.rs`), `packageManager` (`domain/package_json.rs`),
registry documents (`domain/registry.rs`), integrity strings
(`domain/integrity.rs`) and edits `PATH` (`domain/tool_path.rs`). Commands
gain one pins reader (`commands/project_pins.rs`), a registry client and a
store (`commands/package_managers/`), and the policy gate checks every tool
in one report. `Http` gains `get_secure_bytes`; `FileSystem` gains
`make_executable`.

**Tech Stack:** Rust 2024 (MSRV 1.85); existing `serde_json`, `sha2`
(`Sha512`), `nodejs-semver`, `flate2`/`tar` behind the `Archive` port; no new
dependency (base64 is encoded by hand in `domain/integrity.rs`).

**Spec:** `docs/superpowers/specs/2026-10-17-security-policy-design.md`
(sections 6.2, 6.3, 6.5, 6.7, 6.10, 8; delivery step 3). Starts from
`feat/policy-tools` at `bd1d526` (Plans 11 and 12 merged).

**What the registry looks like** (checked on 2026-10-17 at
`https://registry.npmjs.org`): `GET /<name>/<version>` returns the version
manifest with `bin` (an object: npm `{npm: bin/npm-cli.js, npx:
bin/npx-cli.js}`, pnpm `{pnpm: bin/pnpm.cjs, pnpx: bin/pnpx.cjs}`, yarn and
`@yarnpkg/cli-dist` `{yarn: bin/yarn.js, yarnpkg: bin/yarn.js}`),
`dist.tarball` (https) and `dist.integrity` (`sha512-<base64>`). `GET
/<name>` returns the packument whose `time` object maps each version to an
ISO timestamp (`2024-03-09T21:35:40.868Z`); it is large (npm 26 MB, over the
16 MiB text limit), so it is fetched as bytes and only when a
`minimumReleaseAgeDays` rule needs it. Scoped names work both as
`@yarnpkg/cli-dist` and `@yarnpkg%2fcli-dist`; this plan always encodes the
`/` as `%2f`. The tarballs unpack into one top directory (`package/`).

## Global Constraints

- Pins: `npm=`, `yarn=`, `pnpm=` lines in the nearest `.nvmrc`, each an exact
  `major.minor.patch` of digits only (no `v`, no prerelease, no range). Yarn
  and pnpm together is an error. Pins are read even when the node version
  comes from the command line.
- Pin errors and the `packageManager` divergence exit **3** (the
  `InvalidVersion` code, `.nvmrc` or `package.json` declares a version that
  is not valid); policy violations exit 7; failures to know exit 9.
- `packageManager` is checked only when yarn or pnpm is pinned and a
  `package.json` sits in the same directory as that `.nvmrc`: it must be
  `<name>@<version>`, a `+<hash>` suffix ignored. nvmrc never writes
  `package.json` in this plan.
- Registry: `NVMRC_NPM_REGISTRY`, default `https://registry.npmjs.org`, a
  trailing `/` trimmed; anything that is not https is refused, and every
  request (manifest, packument, tarball) goes through `get_secure_*`.
- Packages: npm → `npm`; pnpm → `pnpm`; yarn below 2.0.0 → `yarn`; yarn
  2.0.0 and later → `@yarnpkg/cli-dist`.
- Integrity: the tarball must match a `sha512-` entry of `dist.integrity`;
  no `sha512` entry, or a mismatch, refuses the install.
- Store: `$NVM_DIR/package-managers/<tool>/<version>/package/` (the unpacked
  package) and `.../<version>/bin/<command>` (symlinks to
  `../package/<bin path>`, targets made executable). A tool is installed
  when `package/package.json` and `bin/<tool>` exist. The store is shared by
  all node versions.
- `PATH`: `use`, `exec` and `run` on an installed node put each pinned
  tool's `bin` ahead of node's and drop any other
  `$NVM_DIR/package-managers/` entry; `deactivate` drops them too. `system`
  gets no pinned tools on `PATH`.
- Policy: the sources for npm, yarn and pnpm are central policy, repository
  policy, `--min-<tool>` and `NVMRC_MIN_<TOOL>`, merged tighten-only like
  node; `ltsOnly` never applies to them; their CVEs are not checked. A tool
  is in effect when pinned; npm without a pin is the npm bundled with the
  node in use, looked up only when an npm rule exists.
- Automatic paths (`__auto`, the init snippet's `use`/`install`) warn and
  never block: pin errors, missing pinned installs and tool violations are
  warnings.
- No pins and no policy → behaviour exactly as today; the nvm.sh compat
  suite passes unchanged.
- Files under 300 lines, functions under 30, complexity under 10, no
  unwrap/expect on external input, `cargo fmt`, `cargo clippy --all-targets
  --all-features -- -D warnings`, `rumdl` on changed Markdown, no network in
  tests, TDD; commits Conventional, GPG-signed (`git commit -S`), no AI
  attribution.

## Review Focus

- A manifest whose `bin` points outside the package (`../../evil`,
  `/etc/x`) or names a command with a `/`: refused before anything is
  written (Task 4 test `a_bin_path_outside_the_package_is_refused`).
- A tarball whose sha512 differs from `dist.integrity`, or an integrity with
  only `sha1-`: install refused, nothing left in the store (Task 5 test
  `a_tarball_that_fails_integrity_leaves_nothing_in_the_store`).
- `pnpm=9.12`, `pnpm=v9.12.0`, `pnpm=latest` in `.nvmrc`: exit 3 naming the
  file and the line, and nvm.sh's bare-version reading is unchanged (Task 3
  test `a_partial_pin_is_rejected_with_the_file_and_line`).
- `use` from a project pinning pnpm to one with no pins: the old pnpm `bin`
  leaves `PATH` (Task 8 test `use_drops_the_tool_bins_of_another_project`).
- A `pnpm@9.12.0+sha512.abc` `packageManager` against `pnpm=9.12.0`: passes;
  no `package.json` at all: passes (Task 3 test
  `a_corepack_hash_suffix_is_ignored`).

---

### Task 1: Per-tool rule types

**Files:**

- Create: `src/domain/tool.rs` (with its `#[cfg(test)] mod tests`)
- Modify: `src/domain/mod.rs`, `src/domain/policy/rules.rs`,
  `src/domain/policy/verdict.rs`, `src/domain/policy/mod.rs`,
  `src/domain/policy/rules_tests.rs`, `src/domain/policy/verdict_tests.rs`,
  every user of `NodeLayer`/`NodePolicy`/`merge` in `src/commands/policy/`

**Interfaces:**

- Produces:
  - `enum Tool { Node, Npm, Yarn, Pnpm }` (`Debug, Clone, Copy, PartialEq,
    Eq, Ord, PartialOrd`) with `pub const PACKAGE_MANAGERS: [Tool; 3]`
    (`Npm, Yarn, Pnpm`), `fn name(self) -> &'static str` (`node`, `npm`,
    `yarn`, `pnpm`) and `fn show(self, version: &Version) -> String` (node:
    `v20.11.1`, others: `10.9.0`)
  - `ToolLayer` (renamed `NodeLayer`, same fields) with
    `fn from_node(source, &NodeRules) -> Result<Self, DocumentError>`,
    `fn from_package(tool: Tool, source, &PackageRules) -> Result<Self,
    DocumentError>` (field names `tools.<tool>.minimum`, `.allow`,
    `.deny[<i>]`; `lts_only` `None`) and `fn minimum_only(source, Version)`
  - `ToolPolicy` (renamed `NodePolicy`) gains `pub tool: Tool`; `Default` is
    removed; `fn unrestricted(tool: Tool) -> Self` and `fn is_unrestricted(&self)
    -> bool` (no rule at all)
  - `fn merge(tool: Tool, layers: Vec<ToolLayer>) -> (ToolPolicy, Vec<String>)`
  - `Violation::BelowMinimum(Sourced<String>)`: the minimum as
    `tool.show(..)`; `Violation::NotNode` is produced only for `Tool::Node`
- [ ] **Step 1: Write the failing tests** — in `src/domain/tool.rs`:
  `shows_node_with_a_v_and_package_managers_without` (`Tool::Node.show(v)` is
  `"v20.11.1"`, `Tool::Npm.show(v)` is `"10.9.0"`). In `rules_tests.rs`:
  `a_package_layer_names_its_own_fields` (`deny: ["10.x"]` under npm fails
  with field `tools.npm.deny[0]`), `package_minimums_tighten_like_node` (two
  npm layers `10.2.4` and `10.8.0` → winner `10.8.0`, the looser override
  warns). In `verdict_tests.rs`: `an_npm_minimum_is_shown_without_a_v`
  (violation text `below the minimum 10.2.4 (from policy p)`). Keep every
  existing node expectation (`below the minimum v20.11.1 (from --min-node)`).
- [ ] **Step 2: Run** `cargo test domain::` — expected: FAIL (no `Tool`).
- [ ] **Step 3: Implement** the renames and the `tool` field, then fix every
  user (`commands/policy/mode.rs` passes `Tool::Node`).
- [ ] **Step 4: Run** `cargo test` — expected: PASS, no node message changes.
- [ ] **Step 5: Commit** — `refactor(policy): make the rule types per tool`.

### Task 2: npm, yarn and pnpm rules in the mode and the flags

**Files:**

- Modify: `src/domain/policy/flags.rs` (`POLICY_FLAGS`),
  `src/commands/policy/mode.rs` (split into helpers; move package layers to a
  new `src/commands/policy/tool_layers.rs` if `mode.rs` passes 250 lines),
  `src/commands/policy/tests/mode.rs`, `src/shell/init/tests.rs`,
  `src/cli/tests/policy.rs`

**Interfaces:**

- Consumes: Task 1's `Tool`, `ToolLayer::from_package`, `merge(tool, ..)`.
- Produces:
  - `POLICY_FLAGS: [PolicyFlag; 6]`, appended in this order:
    `--min-npm`/`NVMRC_MIN_NPM`, `--min-yarn`/`NVMRC_MIN_YARN`,
    `--min-pnpm`/`NVMRC_MIN_PNPM`
  - `EnforcedPolicy { pub node: ToolPolicy, pub npm: ToolPolicy, pub yarn:
    ToolPolicy, pub pnpm: ToolPolicy, pub cve: CvePolicy }` with
    `fn tool(&self, tool: Tool) -> &ToolPolicy`
  - For each package manager the layers are, in order: central policy,
    repository policy, flag, variable (a tie names the central policy).
- [ ] **Step 1: Write the failing tests** — in `tests/mode.rs`:
  `npm_rules_merge_from_the_policy_and_the_variable` (central `npm.minimum
  10.2.4`, `NVMRC_MIN_NPM=10.8.0` → npm minimum `10.8.0` from
  `NVMRC_MIN_NPM`), `a_min_pnpm_flag_alone_turns_policy_mode_on`,
  `a_blank_min_yarn_variable_is_unset`,
  `an_invalid_min_npm_flag_is_a_failure_to_know` (`--min-npm=ten` → exit 9
  naming `--min-npm`), `yarn_rules_from_the_repository_policy_apply`. In
  `shell/init/tests.rs`: the POSIX and fish functions route
  `nvm --min-pnpm 9 use 20` and `nvm --min-yarn=4 use 20` as `use`. In
  `cli/tests/policy.rs`: `nvm --min-npm 10 version 20` strips the flag.
- [ ] **Step 2: Run** `cargo test policy` — expected: FAIL.
- [ ] **Step 3: Implement** the flags, the package layers and
  `EnforcedPolicy`; keep `resolve_mode` under 30 lines.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `feat(policy): merge npm, yarn and pnpm rules and
  their minimum flags`.

### Task 3: Pins in `.nvmrc` and the `packageManager` check

**Files:**

- Create: `src/domain/nvmrc/pins.rs`, `src/domain/nvmrc/pins_tests.rs`,
  `src/domain/package_json.rs` (with tests), `src/commands/project_pins.rs`
  (with tests)
- Modify: `src/domain/nvmrc/mod.rs` (share the comment and trim rules),
  `src/domain/mod.rs`, `src/commands/mod.rs`, `src/error.rs`

**Interfaces:**

- Consumes: Task 1's `Tool`.
- Produces:
  - `struct Pins { pub npm: Option<Version>, pub yarn: Option<Version>, pub
    pnpm: Option<Version> }` (`Default`) with `fn iter(&self) -> impl
    Iterator<Item = (Tool, Version)>` (npm, yarn, pnpm order),
    `fn get(&self, tool: Tool) -> Option<Version>` and
    `fn package_manager(&self) -> Option<(Tool, Version)>` (yarn or pnpm)
  - `fn parse_pins(contents: &str) -> Result<Pins, PinError>`; `PinError::
    NotExact { line: String }` and `PinError::YarnAndPnpm`. Lines are the
    ones `process_content` sees (same `#` and trim rules, one shared helper);
    keys other than `npm`, `yarn`, `pnpm` are ignored
  - `fn check_package_manager(text: &str, tool: Tool, version: &Version) ->
    Result<(), PackageManagerError>`; `PackageManagerError::{Malformed(String),
    Missing, Different { found: String }}`
  - `struct ProjectPins { pub nvmrc: Option<PathBuf>, pub pins: Pins }`
    (`Default`)
  - `fn read(context: &Context<'_>) -> Result<ProjectPins, CliError>`: the
    nearest `.nvmrc` from the working directory (none → default), its pins,
    and the `package.json` beside it checked when a yarn or pnpm pin exists
  - `fn read_for(context: &Context<'_>, transcript: &mut Transcript) ->
    Result<ProjectPins, CliError>`: like `read`, but in an advisory context
    an error is printed as `nvm: warning: <message>` and the result is empty
  - `CliError::Pins(PinsError)` → `NvmExitCode::InvalidVersion`, where
    `PinsError` (in `error.rs`) holds the paths and prints, one per case:

    ```text
    nvm: /p/.nvmrc: 'pnpm=9.12' is not an exact version (major.minor.patch).
    nvm: /p/.nvmrc pins both yarn and pnpm; pin at most one.
    nvm: /p/package.json has no packageManager; set it to "pnpm@9.12.0" to match /p/.nvmrc.
    nvm: /p/package.json has packageManager "yarn@4.5.0", but /p/.nvmrc pins yarn 4.5.1; set it to "yarn@4.5.1".
    nvm: /p/package.json cannot be read as JSON: <reason>
    ```

- [ ] **Step 1: Write the failing tests** — `pins_tests.rs`:
  `reads_the_three_pins` (`"20.11.1\nnpm=10.9.0\npnpm=9.12.0\n"`),
  `a_partial_pin_is_rejected_with_the_file_and_line` (`pnpm=9.12`,
  `pnpm=v9.12.0`, `pnpm=latest` each → `NotExact` with that line),
  `yarn_and_pnpm_together_are_rejected`, `comments_and_spaces_are_ignored`
  (`" npm = 10.9.0 # pinned"`), `other_keys_are_ignored`; plus a test that
  `process_content` still returns the bare version for a file with pins.
  `package_json.rs`: `a_matching_package_manager_passes`,
  `a_corepack_hash_suffix_is_ignored`, `a_missing_field_is_reported`,
  `another_name_or_version_is_different` (`found` holds the field as
  written), `malformed_json_is_reported`. `project_pins.rs`:
  `no_nvmrc_means_no_pins`, `no_package_json_passes`,
  `a_divergent_package_json_exits_3`,
  `an_advisory_context_warns_and_returns_no_pins`.
- [ ] **Step 2: Run** `cargo test pins package_json` — expected: FAIL.
- [ ] **Step 3: Implement** the domain functions and the reader.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `feat(nvmrc): read npm, yarn and pnpm pins and
  check packageManager`.

### Task 4: Registry documents, integrity and secure byte downloads

**Files:**

- Create: `src/domain/registry.rs` (+ `registry_tests.rs`),
  `src/domain/integrity.rs` (with tests),
  `src/commands/package_managers/mod.rs`,
  `src/commands/package_managers/registry.rs` (with tests)
- Modify: `src/ports/http.rs`, `src/adapters/ureq_http/mod.rs` (+ tests),
  `src/adapters/retrying_http.rs`, `src/adapters/no_http.rs`,
  `src/fakes/http.rs`, `src/domain/mod.rs`, `src/commands/mod.rs`

**Interfaces:**

- Consumes: Task 1's `Tool`.
- Produces:
  - `Http::get_secure_bytes(&self, url: &str) -> Result<Vec<u8>, HttpError>`:
    `get_bytes` with the download size limit and the https rule of
    `get_secure_text` (the URL and every redirect); the retrying client
    retries it like `get_bytes`; the fake mirrors `get_secure_text`
  - `fn package_name(tool: Tool, version: &Version) -> &'static str`
  - `fn manifest_url(registry: &str, tool: Tool, version: &Version) -> String`
    (`<registry>/<name>/<version>`, `/` in the name as `%2f`) and
    `fn packument_url(registry: &str, tool: Tool, version: &Version) -> String`
  - `struct Manifest { pub tarball: String, pub integrity: String, pub
    commands: Vec<(String, String)> }` and
    `fn parse_manifest(text: &str) -> Result<Manifest, RegistryError>`: `bin`
    must be a non-empty object; a command name must be non-empty without `/`;
    a target must be relative with no `..` or root component
  - `fn release_day(packument: &str, version: &Version) -> Result<Option<i64>,
    RegistryError>`: the date part of `time[<version>]` as days since
    1970-01-01 (`days_from_iso_date`); `None` when absent
  - `fn verify(integrity: &str, bytes: &[u8]) -> Result<(), IntegrityError>`:
    space-separated entries, only `sha512-` ones count; base64 (standard
    alphabet, padded) of the computed SHA-512 must equal one of them;
    `IntegrityError::{NoSha512, Mismatch}`
  - `fn registry_base(context: &Context<'_>) -> Result<String, FetchError>`
  - `fn fetch_manifest(context: &Context<'_>, tool: Tool, version: &Version)
    -> Result<Manifest, FetchError>` (via `context.http()`)
  - `fn fetch_release_day(context: &Context<'_>, http: &dyn Http, tool: Tool,
    version: &Version) -> Result<Option<i64>, FetchError>`
  - `struct FetchError { pub location: String, pub reason: String }`
    displayed `<location>: <reason>`
- [ ] **Step 1: Write the failing tests** — `registry_tests.rs`:
  `yarn_one_and_berry_come_from_different_packages` (`1.22.22` → `yarn`,
  `2.0.0` and `4.5.0` → `@yarnpkg/cli-dist`),
  `scoped_names_are_encoded_in_urls`
  (`https://r/@yarnpkg%2fcli-dist/4.5.0`), `parses_the_npm_manifest` (the
  real shape quoted above), `a_bin_path_outside_the_package_is_refused`
  (`../../evil`, `/etc/x`, and the command `a/b`), `a_missing_bin_is_refused`,
  `release_day_reads_the_time_entry` (`2024-03-09T21:35:40.868Z` →
  `days_from_iso_date("2024-03-09")`). `integrity.rs`:
  `base64_matches_rfc4648_vectors` (`""`, `"f"`, `"fo"`, `"foo"`, `"foob"`,
  `"fooba"`, `"foobar"`), `a_matching_sha512_passes`,
  `a_sha1_only_integrity_is_refused`, `a_mismatch_is_refused`.
  `registry.rs` (commands): `the_default_registry_is_npmjs`,
  `a_trailing_slash_is_trimmed`, `an_http_registry_is_refused`.
  `ureq_http/tests.rs` and `fakes/http.rs`: `get_secure_bytes` refuses an
  `http://` URL and a redirect to one.
- [ ] **Step 2: Run** `cargo test registry integrity http` — expected: FAIL.
- [ ] **Step 3: Implement** the port method in every adapter, then the
  domain and the client.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `feat(http): download bytes over https only` and
  `feat(registry): read npm registry manifests and verify integrity`.

### Task 5: The package store

**Files:**

- Create: `src/commands/package_managers/store.rs`,
  `src/commands/package_managers/store_tests.rs`
- Modify: `src/ports/fs.rs`, `src/adapters/std_fs/mod.rs` (+ tests),
  `src/fakes/file_system/mod.rs`, `src/commands/package_managers/mod.rs`

**Interfaces:**

- Consumes: Task 4's `fetch_manifest`, `verify`, `Manifest`;
  `commands::install::place::unpack`.
- Produces:
  - `FileSystem::make_executable(&self, path: &Path) -> io::Result<()>`:
    adds the execute bits wherever read bits are set (Unix); the fake marks
    the path executable
  - `fn tool_dir(context: &Context<'_>, tool: Tool, version: &Version) ->
    Result<PathBuf, CliError>` (`$NVM_DIR/package-managers/<tool>/<version>`)
    and `fn bin_dir(..) -> Result<PathBuf, CliError>` (`<tool_dir>/bin`)
  - `fn is_installed(context: &Context<'_>, tool: Tool, version: &Version) ->
    bool`
  - `fn install_tool(context: &Context<'_>, tool: Tool, version: &Version) ->
    Result<(), String>`: manifest → tarball over https → integrity → the
    bytes written to `<tool_dir>.partial/download.tgz` → unpacked into
    `<tool_dir>.partial/files` → the top directory moved to
    `<tool_dir>.partial/out/package` → one symlink per command in
    `out/bin` → targets made executable → an existing `<tool_dir>` removed
    → `out` renamed to `<tool_dir>` → `.partial` removed. Any failure
    removes `.partial` and leaves `<tool_dir>` as it was.
- [ ] **Step 1: Write the failing tests** — `store_tests.rs` (fake HTTP,
  `FakeArchive`, fake FS): `installs_pnpm_with_its_commands_linked`
  (`bin/pnpm` → `../package/bin/pnpm.cjs`, `bin/pnpx`, target executable,
  `is_installed` true), `yarn_berry_comes_from_cli_dist` (request to
  `.../@yarnpkg%2fcli-dist/4.5.0`),
  `a_tarball_that_fails_integrity_leaves_nothing_in_the_store`,
  `a_broken_install_is_replaced`, `the_registry_variable_is_used`.
  `std_fs` tests: `make_executable_adds_execute_bits` (`0o644` → `0o755`).
- [ ] **Step 2: Run** `cargo test store make_executable` — expected: FAIL.
- [ ] **Step 3: Implement** the port method and the store.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `feat(store): install npm, yarn and pnpm into a
  shared store`.

### Task 6: `install` installs the pinned tools

**Files:**

- Create: `src/commands/install/pinned.rs`,
  `src/commands/install/tests/pinned.rs`
- Modify: `src/commands/install/mod.rs`, `src/commands/install/tests/mod.rs`

**Interfaces:**

- Consumes: Task 3's `read_for`, `ProjectPins`; Task 5's `is_installed`,
  `install_tool`.
- Produces:
  - `install()` reads the pins once with `read_for` right after the options,
    before resolving (a pin error ends the install, exit 3, before any
    download)
  - `fn install_pinned(context: &Context<'_>, pins: &Pins, offline: bool,
    transcript: &mut Transcript) -> Step<()>`, called in both
    `already_installed` and `fresh_install` after the node is in place and
    before it is activated. Per pinned tool, in `Pins::iter` order:
    installed → `pnpm 9.12.0 is already installed.`; else offline →
    `pnpm 9.12.0 is not installed, and --offline does not download.` and
    exit 1; else `Installing pnpm 9.12.0...` then `install_tool`, a failure
    printed as `nvm: cannot install pnpm 9.12.0: <reason>` and exit 1
- [ ] **Step 1: Write the failing tests** — `tests/pinned.rs`:
  `install_installs_the_pinned_tools_after_node`,
  `an_installed_tool_is_not_downloaded_again`,
  `offline_with_a_missing_tool_fails`,
  `pins_come_from_nvmrc_even_with_a_version_argument`,
  `a_pin_error_stops_before_any_download` (no HTTP request recorded),
  `no_pins_change_nothing` (the existing install transcript, byte for byte).
- [ ] **Step 2: Run** `cargo test install::` — expected: FAIL.
- [ ] **Step 3: Implement** `install_pinned` and the calls.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `feat(install): install the npm, yarn and pnpm
  pins`.

### Task 7: The policy gate checks every tool in effect

**Files:**

- Create: `src/commands/policy/tools.rs`, `src/commands/policy/report.rs`
  (the report moved out of `enforce.rs`), `src/commands/policy/bundled.rs`,
  `src/commands/policy/tests/tools.rs`
- Modify: `src/commands/policy/enforce.rs`, `src/commands/policy/gate.rs`,
  `src/commands/policy/mod.rs`, `src/domain/index.rs`,
  `src/commands/install/resolve.rs`, `src/commands/use_version/mod.rs`,
  `src/commands/exec/launch.rs`

**Interfaces:**

- Consumes: Task 2's `EnforcedPolicy::tool`; Task 3's `ProjectPins`; Task
  4's `fetch_release_day`.
- Produces:
  - `Release` gains `pub npm: Option<Version>` (index column 4, `npm`)
  - `Requested` gains `pub pins: ProjectPins`
  - `gate_installed(context, version, origin, pins: &ProjectPins,
    transcript)` and `gate_system(context, pins: &ProjectPins, transcript)`;
    `use` and `exec` (and so `run`) read the pins once with `read_for` and
    pass them; `install` passes the pins of Task 6
  - `enum ToolOrigin { Nvmrc(PathBuf), Bundled(Version) }`
  - `fn tool_violations(context: &Context<'_>, requested: &Requested, policy:
    &EnforcedPolicy) -> Result<Vec<(Tool, Version, ToolOrigin,
    Vec<Violation>)>, PolicyError>`: each pinned tool; npm without a pin
    only when `!policy.npm.is_unrestricted()`; release days from the
    registry (policy client) only for a tool with `minimumReleaseAgeDays`,
    unavailable or offline → `PolicyError::ReleaseInfoUnavailable(<url or
    "offline">)`
  - `fn bundled_npm(context: &Context<'_>, requested: &Requested) ->
    Result<Version, PolicyError>`: `version` of
    `<node dir>/lib/node_modules/npm/package.json` when that node is
    installed; for the system node, `npm --version` of the `npm` beside the
    system `node`; else the index's `npm` column (offline →
    `ReleaseInfoUnavailable("offline")`); nothing readable →
    `ReleaseInfoUnavailable`
  - The report prints one block per offending tool, node first, then npm,
    yarn, pnpm:
    `nvm: npm 10.1.0 (from /p/.nvmrc) is not allowed by policy:` /
    `nvm: npm 10.1.0 (bundled with node v20.10.0) is not allowed by policy:`,
    the bullets, then the fix: `Raise /p/.nvmrc to an allowed version.` or
    `Pin an allowed npm in .nvmrc (npm=<version>).`; advisory prefixes
    `nvm: warning:` and passes; any violation stops with exit 7
- [ ] **Step 1: Write the failing tests** — `tests/tools.rs`:
  `a_pinned_pnpm_below_the_minimum_exits_7`,
  `a_tool_that_is_not_pinned_is_not_checked` (yarn rule, no yarn pin →
  pass, no request), `bundled_npm_of_an_installed_node_is_read_from_its_package_json`,
  `bundled_npm_for_install_comes_from_the_index`,
  `bundled_npm_is_not_looked_up_without_an_npm_rule`,
  `an_age_rule_fetches_the_packument_and_fails_when_too_recent`,
  `an_unreachable_registry_with_an_age_rule_exits_9`,
  `offline_with_an_age_rule_exits_9`,
  `node_and_pnpm_violations_are_reported_together`,
  `the_auto_path_warns_about_a_tool_and_passes`. In `domain/index.rs`:
  `reads_the_npm_column`.
- [ ] **Step 2: Run** `cargo test policy` — expected: FAIL.
- [ ] **Step 3: Implement** the tool checks, the bundled npm lookup, the
  report and the call-site changes.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `feat(policy): enforce the rules of npm, yarn and
  pnpm`.

### Task 8: Pinned tools on `PATH`

**Files:**

- Create: `src/domain/tool_path.rs` (with tests),
  `src/commands/package_managers/path.rs` (with tests)
- Modify: `src/commands/use_version/apply/mod.rs`,
  `src/commands/use_version/mod.rs`, `src/commands/exec/launch.rs`,
  `src/commands/deactivate/mod.rs`, `src/error.rs` (`PinsError::NotInstalled`),
  their tests

**Interfaces:**

- Consumes: Task 3's `ProjectPins`; Task 5's `bin_dir`, `is_installed`;
  Task 7's pins read in `use` and `exec`.
- Produces:
  - `fn strip_tool_bins(path: &str, nvm_dir: &str) -> String`: drops every
    entry that starts with `<nvm_dir>/package-managers/`
  - `fn prepend(path: &str, directories: &[String]) -> String`
  - `fn pinned_path(context: &Context<'_>, path: &str, pins: &ProjectPins,
    transcript: &mut Transcript) -> Result<String, CliError>`: strip, then
    prepend each pinned, installed tool's `bin` in `Pins::iter` order. A
    pinned tool that is not installed: explicit →
    `CliError::Pins(PinsError::NotInstalled { .. })` printing
    `nvm: pnpm 9.12.0 (from /p/.nvmrc) is not installed. Run \`nvm install\`.`
    (exit 3); advisory → that text as a warning, the tool left out
  - `use` (installed) and `exec`/`run` (installed) pass the switched `PATH`
    through `pinned_path` before the script, the npm suffix and the child
    environment; `deactivate` also applies `strip_tool_bins` to `PATH`
    without changing its messages
- [ ] **Step 1: Write the failing tests** — `tool_path.rs`:
  `strips_only_package_manager_entries`, `prepends_in_order`.
  `path.rs`: `pins_go_ahead_of_node`, `a_missing_tool_fails_explicitly`,
  `a_missing_tool_is_a_warning_when_advisory`. `use` tests:
  `use_puts_pinned_pnpm_on_path` (the exported `PATH` starts with the pnpm
  `bin`, then node's), `use_drops_the_tool_bins_of_another_project`,
  `now_using_shows_the_pinned_npm`. `exec` tests:
  `exec_gives_the_child_the_pinned_tools`. `deactivate` tests:
  `deactivate_drops_tool_bins`.
- [ ] **Step 2: Run** `cargo test use_version exec deactivate tool_path` —
  expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** `cargo test` — expected: PASS; the nvm.sh compat
  tests unchanged.
- [ ] **Step 5: Commit** — `feat(use): put the pinned npm, yarn and pnpm on
  PATH`.

### Task 9: Documentation

**Files:**

- Modify: `README.md`, `docs/deviations.md`
- [ ] **Step 1: README** — a "npm, yarn and pnpm" section: the pin lines, the
  exact-version rule, yarn or pnpm but not both, the `packageManager` check,
  the store path, `NVMRC_NPM_REGISTRY` (https only), integrity, `PATH`
  order, `system` without pins, offline behaviour; add `--min-npm`,
  `--min-yarn`, `--min-pnpm` and their variables to the policy section; exit
  code 3 for pin and `packageManager` errors.
- [ ] **Step 2: deviations.md** — `npm=`/`yarn=`/`pnpm=` lines change what
  `use`, `exec`, `run`, `install` and `deactivate` do (nvm.sh ignores them);
  `packageManager` divergence exits 3; the store and `PATH` entries.
- [ ] **Step 3: Verify** — `rumdl fmt README.md docs/deviations.md`, `rumdl
  check README.md docs/deviations.md`, `cargo test`.
- [ ] **Step 4: Commit** — `docs: describe npm, yarn and pnpm pins and their
  policy`.
