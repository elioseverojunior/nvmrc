# nvmrc Plan 15: `nvm upgrade` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver step 5 of the security policy design: `nvm upgrade [--node]
[--npm] [--yarn] [--pnpm] [--major[=<N>]]` moves the tools the nearest
`.nvmrc` pins to their newest allowed release — within the current major
unless asked — rewrites `.nvmrc` and `packageManager`, then installs.

**Architecture:** The pieces `nvm init` uses to choose and write versions move
to a shared `commands/project/` module (candidates, the judge-driven
selection, the file writer with its restore). `commands/upgrade/` parses the
flags, reads the current versions from the nearest `.nvmrc`, picks one target
per selected tool, refuses before writing anything when any tool has none,
writes, prints each change, and runs `install` for the new versions.

**Tech Stack:** Rust 2024 (MSRV 1.85); no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-17-security-policy-design.md`
(sections 6.2, 6.5, 6.7, 6.9, 6.10; delivery step 5). Starts from
`feat/policy-upgrade` at `22856d1` (Plans 11–14 merged).

## Global Constraints

- `nvm upgrade` reads the **nearest** `.nvmrc` (walking up, as `use` does) and
  writes that file and the `package.json` beside it.
- Without tool flags, every tool the `.nvmrc` pins is selected (node is pinned
  when the file has a bare version line; npm, yarn, pnpm when they have a
  `<tool>=` line). With tool flags, only those; a flag for a tool the file
  does not pin is a usage error, exit 6 (`nvm upgrade: /p/.nvmrc does not pin
  pnpm; use nvm init --pnpm to add it.`).
- Default: each tool moves to the newest allowed release (spec 6.2) **within
  the major it already uses**; it never crosses a major on its own.
  `--major`: the newest allowed release in any major. `--major=<N>`: the
  newest allowed release in major `N`, and a usage error (exit 6) unless
  exactly one tool is selected.
- For node, "newest allowed release" is any allowed release (an LTS name is
  required only when the policy sets `ltsOnly`); npm, yarn and pnpm use stable
  `major.minor.patch` releases only. Yarn reads the `yarn` package for major
  1 and `@yarnpkg/cli-dist` for major 2 and later (`--major` searches
  `@yarnpkg/cli-dist`).
- A node line that is a partial version (`20`, `v20.11`) uses that major and
  is replaced by the exact target. A node line that is not a version (an
  alias such as `lts/*`, `node`, an `iojs-` version) is refused, exit 3
  (`nvm upgrade: /p/.nvmrc names node as "lts/*", not a version; use nvm init
  --node to pin one.`). No `.nvmrc` at all: exit 3 (`nvm upgrade: no .nvmrc
  found; run nvm init first.`). An invalid pin line: the Plan 13 pin error,
  exit 3.
- A tool already on its target is left unchanged and reported (`pnpm 9.12.0
  is up to date`). A current version that is not allowed moves to the target
  even when the target is lower.
- No allowed target in the major: fail before writing anything, exit 7. Print
  for each such tool `nvm upgrade: no allowed <tool> release in major <M>.`,
  the policy block of the newest release in that major with the rules it
  breaks and their sources (spec 6.10 — the carry-over from Plan 14), then
  `The lowest major with an allowed release is <L>. Run \`nvm upgrade
  --<tool> --major=<L>\`, or \`nvm upgrade --<tool> --major\` for the
  newest.` (or `No major has an allowed <tool> release.`). If any selected
  tool has no target, nothing is written.
- Changes print as `node v20.11.1 -> v20.18.0` / `pnpm 9.1.0 -> 9.12.0`
  (`Tool::show`), in node, npm, yarn, pnpm order. Nothing changed: exit 0,
  nothing written, nothing installed.
- After writing, `install` runs as `nvm install` would with no argument (it
  reads the rewritten `.nvmrc`, installs node and the pins). If it fails, its
  output and status are kept and one line is added: `nvm upgrade: /p/.nvmrc
  is updated, but the install failed; run nvm install to finish.`
- Writing follows Plan 14: both texts computed first; `package.json` restored
  when `.nvmrc` cannot be written; the restore-failure line names `nvm
  upgrade`.
- Exit codes: 6 usage, 3 file problems above, 7 no allowed target, 9 failure
  to know, 1 I/O, the install's status when the install fails.
- `nvm init` behaviour is unchanged by the refactor (its tests stay green).
- Files under 300 lines, functions under 30 (signature to closing brace),
  complexity under 10, no unwrap/expect on external input, `cargo fmt`,
  `cargo clippy --all-targets --all-features -- -D warnings`, `rumdl` on
  changed Markdown, no network in tests, TDD; commits Conventional,
  GPG-signed (`git commit -S`), no AI attribution.

## Review Focus

- A partial node line `20`: upgraded within major 20 to an exact `v20.x.y`
  (Task 4 test `a_partial_node_line_is_upgraded_within_its_major`).
- A `.nvmrc` naming `lts/*`: exit 3, nothing written, no request (Task 4 test
  `an_alias_node_line_is_refused`).
- `yarn=1.22.19`: the default upgrade stays on the `yarn` package in major 1;
  `--yarn --major` moves to `@yarnpkg/cli-dist` (Task 3 test
  `yarn_one_stays_on_yarn_and_major_moves_to_cli_dist`).
- A current version the policy now denies, newer than every allowed release
  in its major: moves down to the newest allowed and prints the change, not
  "up to date" (Task 3 test `a_denied_current_version_moves_to_the_newest_allowed`).
- Two selected tools, one without a target: nothing written, every failing
  tool reported, exit 7 (Task 4 test `one_tool_without_a_target_writes_nothing`).

---

### Task 1: Share the selection and the writer

**Files:**

- Create: `src/commands/project/mod.rs`
- Move: `src/commands/init/candidates.rs` (+ `candidates_tests.rs`),
  `src/commands/init/write.rs` (+ `write_tests.rs`) and the `Stop` enum to
  `src/commands/project/`; `init` imports them from there
- Modify: `src/commands/init/{mod.rs,project.rs,project_tests.rs,test_support.rs}`,
  `src/commands/mod.rs`

**Interfaces:**

- Produces (all under `crate::commands::project`):
  - everything `init` already uses, unchanged in behaviour
  - `fn first_allowed(context, judge: &mut Judge, tool, candidates:
    &[Candidate]) -> Result<Option<Candidate>, PolicyError>` (no LTS filter);
    `newest_allowed` becomes the LTS-filtered call of it for node
  - `fn in_major(candidates: &[Candidate], major: u64) -> Vec<Candidate>`
  - `fn majors(candidates: &[Candidate]) -> Vec<u64>` (ascending, distinct)
  - `save(context, command: &str, files, nvmrc_text, package_edit,
    transcript)`: `command` (`"nvm init"`, `"nvm upgrade"`) prefixes the
    restore-failure line
  - `ProjectFiles::beside(nvmrc: &Path) -> ProjectFiles` (the `.nvmrc` given
    and `package.json` in its directory), next to `in_directory`
- [ ] **Step 1: Write the failing tests** — `first_allowed_ignores_lts`,
  `in_major_keeps_only_that_major`, `majors_are_ascending_and_distinct`,
  `beside_puts_package_json_next_to_the_nvmrc`,
  `the_restore_failure_line_names_the_command`.
- [ ] **Step 2: Run** `cargo test project` — expected: FAIL.
- [ ] **Step 3: Implement** the move and the additions; init output unchanged.
- [ ] **Step 4: Run** `cargo test` — expected: PASS (all init tests green).
- [ ] **Step 5: Commit** — `refactor(project): share version selection and
  file writing`.

### Task 2: `upgrade` arguments

**Files:**

- Create: `src/commands/upgrade/mod.rs`, `src/commands/upgrade/request.rs`
  (with tests)
- Modify: `src/commands/mod.rs`, `src/cli/commands.rs` (a `Command::Upgrade
  { args }` routed like `Init`, with a help doc comment)

**Interfaces:**

- Produces:
  - `enum Major { Same, Any, Exactly(u64) }`
  - `struct UpgradeRequest { pub tools: Vec<Tool>, pub major: Major }`
    (`tools` empty = every pinned tool; order node, npm, yarn, pnpm)
  - `fn parse(args: &[String]) -> Result<UpgradeRequest, CliError>`: flags
    `--node`, `--npm`, `--yarn`, `--pnpm`, `--major`, `--major=<N>` (digits);
    a repeated flag, `--major=` with no digits, or any other argument is
    `CliError::InvalidOptions` (exit 6): `nvm upgrade: --npm is given
    twice.`, `nvm upgrade: --major=x is not a major version.`, `nvm upgrade:
    unsupported argument "x".`; `--major=<N>` with two or more tool flags is
    exit 6: `nvm upgrade: --major=<N> applies to one tool; name exactly one.`
  - `pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output,
    CliError>` — until Task 4 returns `CliError::Unsupported("nvm upgrade is
    not available yet.")` after parsing
- [ ] **Step 1: Write the failing tests** — `no_flags_select_every_pinned_tool`,
  `tool_flags_select_those_tools_in_order`, `major_alone_means_any`,
  `major_with_a_number_means_that_major`, `a_repeated_flag_is_a_usage_error`,
  `a_major_that_is_not_a_number_is_a_usage_error`,
  `major_n_with_two_tools_is_a_usage_error`,
  `an_unknown_argument_is_a_usage_error`; a CLI test that `nvm upgrade
  --help` lists the flags.
- [ ] **Step 2: Run** `cargo test upgrade` — expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `feat(upgrade): parse the upgrade flags`.

### Task 3: One target per tool

**Files:**

- Create: `src/commands/upgrade/target.rs` (+ `target_tests.rs`)

**Interfaces:**

- Consumes: Task 1's `node_candidates`, `tool_candidates`, `first_allowed`,
  `in_major`, `majors`; `Judge`.
- Produces:
  - `enum Target { UpToDate, Move(Version), NoneInMajor { major: u64,
    newest: Option<(Version, Vec<Violation>)>, lowest: Option<u64> } }`
  - `fn target(context: &Context<'_>, judge: &mut Judge, tool: Tool, current:
    &Version, major: Major) -> Result<Target, PolicyError>`: candidates for
    the tool (yarn: the package of `current` for `Major::Same`, of major `N`
    for `Exactly(N)`, `@yarnpkg/cli-dist` for `Any`), filtered by major
    (`Same` → `current.major`), first allowed; `UpToDate` when it equals
    `current`; `NoneInMajor` with the newest candidate in that major and its
    violations, and the lowest major (over all candidates of the tool) that
    has an allowed release
- [ ] **Step 1: Write the failing tests** — `moves_to_the_newest_allowed_in_the_same_major`
  (`20.11.1` with `20.18.0` and `22.11.0` allowed → `Move(20.18.0)`),
  `major_any_crosses_majors` (→ `22.11.0`), `major_n_picks_that_major`,
  `already_newest_is_up_to_date`,
  `a_denied_current_version_moves_to_the_newest_allowed`,
  `no_allowed_release_in_the_major_names_the_newest_and_the_lowest_major`,
  `yarn_one_stays_on_yarn_and_major_moves_to_cli_dist` (request URLs),
  `node_without_lts_only_may_pick_a_non_lts_release`.
- [ ] **Step 2: Run** `cargo test target` — expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `feat(upgrade): pick the newest allowed release in
  the major`.

### Task 4: Rewrite the project files

**Files:**

- Create: `src/commands/upgrade/project.rs` (+ `project_tests.rs`)
- Modify: `src/commands/upgrade/mod.rs`, `src/commands/policy/report.rs` (a
  block for a version from an `.nvmrc`, beside `argument_block`)

**Interfaces:**

- Consumes: Tasks 1–3; `find_nvmrc`, `process_content`, `VersionPattern`,
  `parse_pins`, the Plan 13 pin error mapping.
- Produces: `fn upgrade(context: &Context<'_>, request: &UpgradeRequest) ->
  Result<Upgraded, CliError>` with `enum Upgraded { Nothing, Written {
  nvmrc: PathBuf } }` and its output in a `Transcript` it is given:
  1. nearest `.nvmrc` (none → exit 3), current versions (node line rules and
     pin errors of the Global Constraints, exit 3), the selection (a flag for
     an unpinned tool → exit 6; `--major=<N>` with a selection that is not
     exactly one tool → exit 6) — all before any request
  2. `Judge::load`, then a target per selected tool; any `NoneInMajor` →
     the messages of the Global Constraints for every such tool, exit 7,
     nothing written
  3. changes printed, up-to-date tools reported; no change → `Nothing`
  4. new `.nvmrc` text and `package.json` edit (yarn/pnpm changed) computed,
     then `save(context, "nvm upgrade", ProjectFiles::beside(nvmrc), ..)`
  - `run` prints the transcript; installing comes in Task 5

- [ ] **Step 1: Write the failing tests** — `upgrades_every_pinned_tool_by_default`
  (`.nvmrc` `v20.11.1` + `pnpm=9.1.0`, `package.json` `pnpm@9.1.0` →
  `v20.18.0`, `pnpm=9.12.0`, `pnpm@9.12.0`, the two change lines),
  `a_tool_flag_upgrades_only_that_tool`,
  `a_flag_for_an_unpinned_tool_is_a_usage_error`,
  `a_partial_node_line_is_upgraded_within_its_major`,
  `an_alias_node_line_is_refused`, `no_nvmrc_is_refused`,
  `one_tool_without_a_target_writes_nothing` (exit 7, the block names the
  rule and its source, the `--major=` hint),
  `everything_up_to_date_writes_nothing`,
  `the_nvmrc_in_a_parent_directory_is_the_one_rewritten`,
  `major_n_with_two_pinned_tools_and_no_flag_is_a_usage_error`.
- [ ] **Step 2: Run** `cargo test upgrade` — expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `feat(upgrade): rewrite .nvmrc to the newest
  allowed versions`.

### Task 5: Install after writing

**Files:**

- Modify: `src/commands/upgrade/mod.rs`, `src/commands/upgrade/project_tests.rs`
  (or a new `install_tests.rs`)

**Interfaces:**

- Consumes: Task 4's `Upgraded`; `crate::commands::install::run(context,
  &[])`.
- Produces: `run` → after `Upgraded::Written`, `install::run(context, &[])`;
  its stdout and stderr follow the upgrade's lines; its status is the
  command's status; on a non-success status add `nvm upgrade: /p/.nvmrc is
  updated, but the install failed; run nvm install to finish.`;
  `Upgraded::Nothing` installs nothing.
- [ ] **Step 1: Write the failing tests** — `an_upgrade_installs_the_new_versions`
  (fake mirror, registry and archive: node and pnpm end up installed),
  `a_failed_install_keeps_the_files_and_says_so`,
  `nothing_to_upgrade_installs_nothing` (no request after the targets).
- [ ] **Step 2: Run** `cargo test upgrade` — expected: FAIL.
- [ ] **Step 3: Implement**.
- [ ] **Step 4: Run** `cargo test` — expected: PASS.
- [ ] **Step 5: Commit** — `feat(upgrade): install the upgraded versions`.

### Task 6: Documentation

**Files:**

- Modify: `README.md`, `docs/deviations.md`
- [ ] **Step 1: README** — an "Upgrading a project" section: selection,
  same-major default, `--major`, `--major=<N>`, node may pick a non-LTS
  release unless `ltsOnly`, partial node lines become exact, aliases refused,
  the no-target message and hint, writes and the install, exit codes.
- [ ] **Step 2: deviations.md** — nvm.sh has no `upgrade`.
- [ ] **Step 3: Verify** — `rumdl fmt` + `rumdl check` on both, `cargo test`.
- [ ] **Step 4: Commit** — `docs: describe nvm upgrade`.
