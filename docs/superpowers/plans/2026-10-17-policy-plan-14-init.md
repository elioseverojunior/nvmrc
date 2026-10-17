# nvmrc Plan 14: `nvm init` Project Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver step 4 of the security policy design: `nvm init
--node[=<v>] --npm[=<v>] --yarn[=<v>] --pnpm[=<v>]` writes a project's
`.nvmrc` (and `packageManager` in an existing `package.json`) with versions
that already satisfy the policy — the newest allowed release when no version
is given, or a checked exact version.

**Architecture:** `nvm init` keeps its shell mode and gains a project mode
chosen by its flags. Pure code edits the files (`domain/nvmrc/edit.rs`,
`domain/package_json.rs`) and lists registry versions
(`domain/registry.rs`). One `Judge` (`commands/policy/judge.rs`) loads the
policy and the vulnerability index once and decides whether a version of a
tool is allowed, reusing `ToolPolicy::violations` and the CVE split that
enforcement already uses. `commands/init/` resolves each requested tool,
refuses before writing anything, then writes.

**Tech Stack:** Rust 2024 (MSRV 1.85); existing `serde_json`,
`nodejs-semver`; no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-17-security-policy-design.md`
(sections 6.2, 6.5, 6.7, 6.8, 6.10; delivery step 4). Starts from
`feat/policy-init` at `faabb1c` (Plans 11–13 merged).

## Global Constraints

- Shell mode is unchanged: `nvm init <shell> [--no-use|--install]` prints the
  same snippet; `nvm init` with no shell and no project flag still prints the
  usage with status 127.
- Project mode: `nvm init` with one or more of `--node[=<v>]`,
  `--npm[=<v>]`, `--yarn[=<v>]`, `--pnpm[=<v>]` and no shell name. A shell
  name with a project flag, `--yarn` with `--pnpm`, or a flag given twice is a
  usage error, exit 6 (`InvalidOptions`).
- Without a version, a tool gets the newest allowed release (spec 6.2): node,
  the newest allowed **LTS** release (whether or not `ltsOnly` is set); npm,
  yarn, pnpm, the newest allowed stable `major.minor.patch` in the registry
  (prereleases never). Yarn candidates come from `@yarnpkg/cli-dist`.
- With a version, it must be exact (node: `20.11.1` or `v20.11.1`; npm, yarn,
  pnpm: digits-only `major.minor.patch`, as pins), published (in the node
  index, or in the registry package for that version — yarn below 2.0.0 is
  `yarn`), and allowed; otherwise the command fails before writing.
- "Allowed" is spec 6.2 in full, decided by one function: minimum, allow,
  deny, `ltsOnly` (node), `minimumReleaseAgeDays`, and for node no CVE at or
  above the limit. Without a policy (legacy mode) every published version is
  allowed; the legacy floor applies to `install` only (spec 6.5).
- Exit codes: a version that is not exact or not published, or a pin
  conflict, exits 3; not allowed, or no allowed release, exits 7; a failure
  to know (index, registry or policy unreadable) exits 9; usage errors 6.
- Files: `.nvmrc` in the **working directory** (created when absent): only
  the requested keys change; the bare version line for node, `npm=`/`yarn=`/
  `pnpm=` lines for the others; every other line, comment and order kept.
  `packageManager` is set in `package.json` in the working directory only
  when it exists and yarn or pnpm was requested; nothing else in it changes.
- Both new contents are computed and validated before either file is
  written. Requesting yarn when the `.nvmrc` already pins pnpm (or the
  reverse) is refused (exit 3), never silently switched.
- Network: the node index through `context.http()`, the registry through
  `context.third_party_http()` (credential rules of Plan 13); the packument
  is fetched as bytes.
- No pins and no policy elsewhere: behaviour exactly as today.
- Files under 300 lines, functions under 30 (signature to closing brace),
  complexity under 10, no unwrap/expect on external input, `cargo fmt`,
  `cargo clippy --all-targets --all-features -- -D warnings`, `rumdl` on
  changed Markdown, no network in tests, TDD; commits Conventional,
  GPG-signed (`git commit -S`), no AI attribution.

## Review Focus

- A `package.json` whose nested object also has a `packageManager` key (e.g.
  under `"devEngines"`): only the top-level field is set (Task 2 test
  `only_the_top_level_package_manager_changes`).
- An existing `.nvmrc` with comments, other keys and CRLF line endings: only
  the requested lines change, line endings kept (Task 2 test
  `editing_keeps_comments_keys_and_line_endings`).
- A policy that denies the newest LTS and requires a release age: the next
  allowed LTS is chosen, not the newest (Task 4 test
  `the_newest_allowed_lts_skips_denied_and_too_recent_releases`).
- Prereleases in the packument (`10.0.0-rc.1`) or a `dist-tags.latest` that
  is not allowed: never chosen (Task 4 test
  `prereleases_and_dist_tags_are_ignored`).
- `--node=20`, `--node=iojs-v3.3.1`, `--npm=v10.9.0`, `--pnpm=latest`: exit
  3 before any request (Task 1 test `inexact_versions_are_rejected`).

---

### Task 1: Project-mode arguments

**Files:**

- Create: `src/commands/init/mod.rs` (from `src/commands/init.rs`; shell mode
  moved unchanged), `src/commands/init/request.rs` (with tests)
- Modify: `src/commands/mod.rs`, `src/cli/commands.rs` (pass the context to
  `init::run`), `src/shell/init/mod.rs` (`USAGE`)

**Interfaces:**

- Produces:
  - `enum Wanted { Newest, Exact(Version) }`
  - `struct ProjectRequest { pub node: Option<Wanted>, pub npm:
    Option<Wanted>, pub yarn: Option<Wanted>, pub pnpm: Option<Wanted> }`
    with `fn get(&self, tool: Tool) -> Option<&Wanted>`
  - `enum InitMode { Shell(Shell, InitOptions), Project(ProjectRequest) }`
  - `fn parse(args: &[String]) -> Result<InitMode, CliError>`: shell-mode
    errors as today; `CliError::InvalidOptions` (exit 6) for a shell name with
    a project flag (`nvm init: a shell name cannot be combined with --node,
    --npm, --yarn or --pnpm.`), `--yarn` with `--pnpm` (`nvm init: --yarn and
    --pnpm cannot be used together.`), a flag given twice (`nvm init: --npm is
    given twice.`); and a new `CliError::InvalidVersionArgument(String)`,
    mapped to `NvmExitCode::InvalidVersion` (exit 3), for an inexact version
    (`nvm init: --npm=10 is not an exact version (major.minor.patch).`)
  - `pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output,
    CliError>`: shell mode as today; project mode returns
    `CliError::Unsupported("nvm init: project mode is not available yet.")`
    until Task 5
  - `USAGE` becomes two lines: `Usage: nvm init <shell> [--no-use]
    [--install]` and `nvm init [--node[=<version>]]
    [--npm[=<version>]] [--yarn[=<version>] | --pnpm[=<version>]]`
- [ ] **Step 1: Write the failing tests** — `request.rs`:
  `a_shell_name_is_shell_mode` (`["bash","--no-use"]`),
  `project_flags_without_a_version_want_the_newest`,
  `project_flags_with_a_version_want_it_exactly` (`--node=v20.11.1`,
  `--pnpm=9.12.0`), `a_shell_with_a_project_flag_is_a_usage_error` (exit 6),
  `yarn_and_pnpm_together_are_a_usage_error`, `a_repeated_flag_is_a_usage_error`,
  `inexact_versions_are_rejected` (`--node=20`, `--node=iojs-v3.3.1`,
  `--npm=v10.9.0`, `--pnpm=latest` → exit 3 with the message). Keep the
  existing init CLI tests green (`src/cli/tests/init.rs`).
- [ ] **Step 2: Run** `cargo test init` — expected: FAIL.
- [ ] **Step 3: Implement** the module split, `parse`, the new error variant
  and the dispatch change.
- [ ] **Step 4: Run** `cargo test` — expected: PASS; shell snippets byte-identical.
- [ ] **Step 5: Commit** — `feat(init): parse the project-mode flags`.

### Task 2: Editing `.nvmrc` and `package.json`

**Files:**

- Create: `src/domain/nvmrc/edit.rs` (+ `edit_tests.rs`); move `with_version`
  there from `src/domain/nvmrc/mod.rs` (re-export it)
- Modify: `src/domain/nvmrc/mod.rs`, `src/domain/package_json.rs` (+ tests)

**Interfaces:**

- Consumes: `Tool` (`src/domain/tool.rs`), `package_manager_field`.
- Produces:
  - `fn with_pin(contents: &str, tool: Tool, version: &Version) -> String`:
    replaces the existing `<tool>=` line (same trim and comment rules as
    `parse_pins`, a trailing comment kept) or appends `<tool>=<x.y.z>` after
    the last non-blank line; `with_version` and `with_pin` keep each line's
    own ending (`\n` or `\r\n`) and the file's final newline
  - `fn set_package_manager(text: &str, value: &str) -> Result<String,
    PackageManagerError>`: the top-level object's `"packageManager"` string
    value replaced in place, or the member appended as the last top-level
    member using the indentation of the first member (`"\n  "` when the
    object is empty, `{}` becomes `{\n  "packageManager": "<v>"\n}`);
    everything else byte for byte; `Malformed` when `text` is not a JSON
    object (validate with `serde_json`, then edit by scanning tokens at depth
    1 — strings and escapes respected)
- [ ] **Step 1: Write the failing tests** — `edit_tests.rs`:
  `with_pin_replaces_the_existing_line` (`" pnpm = 9.1.0 # old"` →
  `"pnpm=9.12.0 # old"`), `with_pin_appends_a_new_line`,
  `editing_keeps_comments_keys_and_line_endings` (a CRLF file with a
  comment, `node`-free bare line, `npm=` and an unknown key: after
  `with_version` and `with_pin` only those two lines differ, CRLF kept),
  `with_version_on_an_empty_file_is_just_the_version`.
  `package_json.rs`: `replaces_the_package_manager_value`,
  `appends_the_field_with_the_file_indentation` (2-space and tab files),
  `an_empty_object_gets_the_field`,
  `only_the_top_level_package_manager_changes` (a nested
  `"devEngines": {"packageManager": "x"}` stays),
  `escaped_quotes_in_strings_do_not_confuse_the_scan`,
  `a_top_level_array_is_malformed`.
- [ ] **Step 2: Run** `cargo test nvmrc package_json` — expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** `cargo test` — expected: PASS (the `--save` tests of
  Plan 13 stay green; this also fixes their CRLF mixed-ending minor).
- [ ] **Step 5: Commit** — `feat(nvmrc): edit pins and packageManager in
  place`.

### Task 3: One judge for "allowed"

**Files:**

- Create: `src/commands/policy/judge.rs`, `src/commands/policy/tests/judge.rs`
- Modify: `src/commands/policy/advisories.rs` (share index loading and the
  severity split), `src/commands/policy/mod.rs`

**Interfaces:**

- Consumes: `resolve_mode`, `EnforcedPolicy::tool`, `ToolPolicy::violations`,
  `CvePolicy`, `parse_advisories`, `findings`, `read_location_with`.
- Produces:
  - `pub(super) fn load_advisories(context: &Context<'_>, cve: &CvePolicy) ->
    Result<Vec<Advisory>, PolicyError>` and
    `pub(super) fn failing(findings: Vec<Finding>, limit: Severity) ->
    Judgement` (today's `split`), used by both `judge` and `Judge`
  - `pub struct Judge` with `pub fn load(context: &Context<'_>, transcript:
    &mut Transcript) -> Result<Judge, PolicyError>` (resolves the mode once,
    prints its warnings; legacy mode → a judge that allows everything) and
    `pub fn violations(&mut self, context: &Context<'_>, tool: Tool, version:
    &Version, release: &ReleaseInfo) -> Result<Vec<Violation>, PolicyError>`
    (the tool's rules, then for node the CVE check; the index is read on the
    first node call and reused)
  - `ReleaseInfo` is the one the release data is passed in (Task 4 supplies
    it for every candidate), so `MissingReleaseInfo` cannot occur here
- [ ] **Step 1: Write the failing tests** — `tests/judge.rs`:
  `legacy_mode_allows_everything` (no policy, no request to the index),
  `a_denied_version_is_not_allowed`, `a_node_cve_above_the_limit_is_not_allowed`,
  `the_index_is_read_once_for_many_node_versions` (three calls, one request),
  `a_package_manager_minimum_applies`, `an_unreadable_index_is_a_failure_to_know`.
  Existing enforcement tests stay green.
- [ ] **Step 2: Run** `cargo test policy` — expected: FAIL.
- [ ] **Step 3: Implement**; enforcement keeps its behaviour and output.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `refactor(policy): decide whether a version is
  allowed in one place`.

### Task 4: Candidates and the newest allowed release

**Files:**

- Create: `src/commands/init/candidates.rs` (with tests)
- Modify: `src/domain/registry.rs` (+ `registry_tests.rs`),
  `src/commands/package_managers/registry.rs`

**Interfaces:**

- Consumes: Task 3's `Judge`; `remote_index::fetch(context, Flavor::Node)`;
  `packument_url`, `registry_base`.
- Produces:
  - `fn published_versions(packument: &str) -> Result<Vec<(Version,
    Option<i64>)>, RegistryError>`: every `versions` key that is digits-only
    `major.minor.patch`, with its `time` day; prereleases and `dist-tags`
    ignored
  - `pub fn fetch_packument(context: &Context<'_>, tool: Tool, version:
    &Version) -> Result<String, FetchError>` (bytes via
    `third_party_http()`, as `fetch_release_day` does; `fetch_release_day`
    reuses it)
  - `struct Candidate { pub version: Version, pub release: ReleaseInfo }`
  - `fn node_candidates(context: &Context<'_>) -> Result<Vec<Candidate>,
    PolicyError>` (node flavor only, newest first) and
    `fn tool_candidates(context: &Context<'_>, tool: Tool, package_of:
    &Version) -> Result<Vec<Candidate>, PolicyError>` (`package_of` picks the
    package: yarn `2.0.0` for the newest search, the exact version otherwise;
    newest first)
  - `fn newest_allowed(context: &Context<'_>, judge: &mut Judge, tool: Tool,
    candidates: &[Candidate]) -> Result<Option<Version>, PolicyError>`: for
    node, only candidates with an LTS name; the first with no violation
  - `fn exact(context, judge, tool, candidates, version) ->
    Result<Exact, PolicyError>` with `enum Exact { Allowed, NotPublished,
    NotAllowed(Vec<Violation>) }`
  - An unreachable index or registry is `PolicyError::ReleaseInfoUnavailable(
    <url>)` (exit 9)
- [ ] **Step 1: Write the failing tests** — `registry_tests.rs`:
  `prereleases_and_dist_tags_are_ignored` (a packument with `10.0.0-rc.1`,
  `10.8.0`, `10.9.0` and `dist-tags.latest = 10.9.0` under a policy denying
  `10.9.0` → newest allowed `10.8.0` — assert in candidates.rs),
  `published_versions_reads_the_time_day`. `candidates.rs`:
  `the_newest_allowed_lts_skips_denied_and_too_recent_releases`,
  `newest_node_without_a_policy_is_the_newest_lts`,
  `yarn_candidates_come_from_cli_dist` (request URL),
  `an_exact_yarn_one_reads_the_yarn_package`,
  `an_unpublished_exact_version_is_not_published`,
  `an_unreachable_registry_is_a_failure_to_know`.
- [ ] **Step 2: Run** `cargo test registry candidates` — expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `feat(init): find the newest allowed release of
  each tool`.

### Task 5: Project mode writes the files

**Files:**

- Create: `src/commands/init/project.rs`, `src/commands/init/project_tests.rs`
- Modify: `src/commands/init/mod.rs`, `src/commands/policy/report.rs` (make
  the block printer reusable for an `Origin::Argument` subject, if needed)

**Interfaces:**

- Consumes: Tasks 1–4; `parse_pins` (pin conflicts), `context.fs`
  (`read_to_string`, `write_file`), `context.working_directory()`.
- Produces: `fn run_project(context: &Context<'_>, request: &ProjectRequest)
  -> Result<Output, CliError>` called by `init::run` for project mode:
  1. `Judge::load`; for each requested tool in node, npm, yarn, pnpm order,
     resolve `Newest` or check `Exact` (all before writing anything)
  2. Read `<cwd>/.nvmrc` (missing → empty); refuse yarn over a pnpm pin or
     the reverse: `nvm init: /p/.nvmrc pins pnpm; remove its pnpm= line to
     pin yarn.` (exit 3)
  3. If yarn or pnpm was requested and `<cwd>/package.json` exists, compute
     its new text with `set_package_manager` (malformed → the Plan 13
     `PinsError` malformed message, exit 3)
  4. Write `package.json` (when changed), then `.nvmrc`
  5. stdout: `Wrote /p/.nvmrc: node v22.11.0, pnpm 9.12.0` and, when set,
     `Set packageManager to "pnpm@9.12.0" in /p/package.json`
  - No allowed release: `nvm init: no allowed node LTS release.` /
    `nvm init: no allowed pnpm release.` (exit 7); not published: `nvm init:
    pnpm 9.99.0 is not a published release.` / `nvm init: node v20.99.0 is
    not a node release.` (exit 3); not allowed: the policy report block
    (`nvm: pnpm 9.1.0 (from the command line) is not allowed by policy:`,
    bullets, `Use an allowed version.`), exit 7; failure to know: exit 9

- [ ] **Step 1: Write the failing tests** — `project_tests.rs`:
  `newest_node_and_pnpm_are_written_with_package_manager`,
  `an_existing_nvmrc_keeps_its_other_lines`,
  `an_exact_version_that_is_not_allowed_writes_nothing` (exit 7, files
  untouched), `no_allowed_release_writes_nothing`,
  `yarn_over_a_pnpm_pin_is_refused`, `no_package_json_is_left_absent`,
  `a_failure_to_know_writes_nothing` (exit 9),
  `policy_flags_reach_init` (`nvm --min-pnpm 9.12.0 init --pnpm` through
  the CLI test harness).
- [ ] **Step 2: Run** `cargo test init` — expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `feat(init): write a project's .nvmrc from the
  policy`.

### Task 6: Documentation

**Files:**

- Modify: `README.md`, `docs/deviations.md`, the `init` help text in
  `src/cli/commands.rs` (the doc comment clap shows)
- [ ] **Step 1: Help** — `nvm init --help` and `nvm --help` describe project
  mode (both forms of `USAGE`).
- [ ] **Step 2: README** — a "Starting a project" section: the flags, newest
  allowed (LTS for node), exact versions, what is written (`.nvmrc` in the
  working directory, `packageManager` only in an existing `package.json`),
  yarn/pnpm conflicts, exit codes 3/6/7/9.
- [ ] **Step 3: deviations.md** — nvm.sh has no project mode for `init`.
- [ ] **Step 4: Verify** — `rumdl fmt` + `rumdl check` on both files,
  `cargo test` (help tests).
- [ ] **Step 5: Commit** — `docs: describe nvm init project mode`.
