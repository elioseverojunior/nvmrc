# nvmrc Plan 12: CVE Check Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver step 2 of the security policy design: check the node
version against the Node Security WG vulnerability index and fail at or above
the policy's severity limit (default `high`), with `--fail-on-severity` and
`NVMRC_FAIL_ON_SEVERITY`, the repository policy's `cve.source` rule, and the
carry-overs from Plan 11's final review.

**Architecture:** Pure code in `src/domain/policy/`: `advisories.rs` parses
the index and finds the advisories that affect a version on a platform;
`cve.rs` merges the severity limit and the index location across sources.
`commands/policy/mode.rs` returns both the node rules and the CVE settings;
`commands/policy/enforce.rs` fetches the index (quick client on automatic
paths) and turns findings into a violation or a warning. The gate call sites
from Plan 11 do not change.

**Tech Stack:** Rust 2024 (MSRV 1.85), existing `serde_json` and
`nodejs-semver` 7; no new dependency.

**Spec:** `docs/superpowers/specs/2026-10-17-security-policy-design.md`
(sections 6.1 `cve`, 6.3, 6.5, 6.6, 6.10; delivery step 2). Starts from
`feat/policy-cve` at `8ab9999` (Plan 11 merged).

**What the real index looks like** (checked on 2026-10-17, 194 entries):
an object keyed by id (`"192"`), each entry with `cve` (array, empty in 14
entries), `vulnerable` and `patched` (npm ranges; `vulnerable` names whole
lines such as `22.x || 24.x`, `patched` names the fix such as `^22.23.2 ||
^24.18.1`; some ranges carry leading spaces, one is `<= 10`),
`affectedEnvironments` (`all`, `linux`, `darwin`, `win32`, `aix`, `freebsd`,
`openbsd`, `sunos`, `android`), `severity` (`critical`, `high`, `medium`,
`low`, and `unknown` in 13 entries), plus fields this plan ignores
(`description`, `overview`, `ref`, `author`, `cvss`, ...).

## Global Constraints

- A version is affected by an advisory when it satisfies `vulnerable` and
  does **not** satisfy `patched`.
- An advisory counts when its `affectedEnvironments` contains `all` or the
  current platform's OS slug (`Os::slug()`); with no known platform it counts.
- Severity ranking: `low` < `medium` < `high` < `critical`. A finding fails
  when its rank is at or above the limit; limit `none` fails nothing but still
  reports. Default limit `high`.
- A missing `severity` and the index's `unknown` count as `high`; any other
  value, an unparseable range, or an index that is not an object of entries is
  a failure to know (exit 9).
- Severity limit sources: `--fail-on-severity`, `NVMRC_FAIL_ON_SEVERITY`,
  central `cve.failOnSeverity`, repository `cve.failOnSeverity`; the strictest
  wins; a looser override is a warning; a tie names the central policy.
- Index location: central `cve.source` when a central policy is configured
  (else the default even if the repository names one); repository
  `cve.source` only without a central policy; default
  `https://raw.githubusercontent.com/nodejs/security-wg/main/vuln/core/index.json`.
- The index is read like a policy (`read_location`): https only, never
  `http://`; quick client on automatic paths.
- Explicit commands: a finding at or above the limit exits 7; a failure to
  know exits 9. Automatic paths warn and pass. Below-limit findings print a
  warning and never change the status.
- No policy configuration → behaviour exactly as today; legacy mode never
  fetches the index.
- Files under 300 lines, functions under 30, complexity under 10, no
  unwrap/expect on external input, `cargo fmt`, `cargo clippy --all-targets
  --all-features -- -D warnings`, `rumdl` on changed Markdown, no network in
  tests, TDD; commits Conventional, GPG-signed, no AI attribution.

## Review Focus

- An advisory with an empty `cve` array: named `advisory <id>` in messages,
  never an empty bullet (Task 2 test `an_advisory_without_cve_ids_is_named_by_id`).
- A version on a patched line (`v22.23.2` under `22.x` / `^22.23.2`): not
  affected (Task 2 test `a_patched_version_is_not_affected`).
- `--fail-on-severity HIGH` or `critial`: exit 9 naming the flag, not a pass
  with the default (Task 4 test `an_invalid_severity_flag_is_a_failure_to_know`).
- A relative `cve.source` in a repository policy, run from a subdirectory:
  resolved against the policy file's directory (Task 4 test
  `a_relative_index_location_is_read_next_to_its_policy`).
- `install --offline` with the default index: exit 9, no network request
  (Task 5 test `offline_with_a_remote_index_is_a_failure_to_know`).

---

### Task 1: Carry-overs that touch no policy code

**Files:**

- Modify: `Cargo.lock` (rustls), `README.md:251` (exec/run flag wording)
- [ ] **Step 1: Update rustls** — `cargo update -p rustls` (0.23.45 →
  0.23.46 resolves for MSRV 1.85). Run `cargo deny check advisories`. If
  RUSTSEC-2026-0346/0347/0348 remain, run `cargo update -p rustls --precise
  <first fixed version>` per the advisories' "Solution" line, staying on a
  version whose `rust-version` ≤ 1.85; if none exists, revert the lockfile
  change and record that in the report.
- [ ] **Step 2: Fix the README wording** — after `exec` and `run` the policy
  flags are taken only immediately after the subcommand (any other option in
  between ends them), matching `docs/deviations.md`.
- [ ] **Step 3: Verify** — `cargo test`, `cargo deny check`, `rumdl check
  README.md` — expected: tests pass; deny clean or the residual recorded.
- [ ] **Step 4: Commit** — `fix(deps): update rustls for RUSTSEC-2026-0346
  to 0348` and `docs: say exactly where exec and run take policy flags`.

### Task 2: Parsing the vulnerability index

**Files:**

- Create: `src/domain/policy/advisories.rs`,
  `src/domain/policy/advisories_tests.rs`
- Modify: `src/domain/policy/mod.rs` (module, re-exports, test module),
  `src/domain/policy/document.rs` (`Severity::rank`)

**Interfaces:**

- Produces:
  - `impl Severity { pub fn rank(self) -> u8 }` — `None` 0, `Low` 1, `Medium`
    2, `High` 3, `Critical` 4
  - `struct Advisory { pub id: String, pub cves: Vec<String>, pub vulnerable:
    nodejs_semver::Range, pub patched: Option<nodejs_semver::Range>, pub
    severity: Severity, pub environments: Vec<String> }`
  - `enum IndexError { NotAnIndex(String), BadEntry { id: String, reason:
    String } }` (thiserror, PartialEq)
  - `pub fn parse_advisories(text: &str) -> Result<Vec<Advisory>, IndexError>`
  - `pub struct Finding { pub name: String, pub severity: Severity }` —
    `name` is the CVE id, or `advisory <id>` when `cves` is empty; one finding
    per CVE id
  - `pub fn findings(advisories: &[Advisory], version: &Version, os:
    Option<&str>) -> Vec<Finding>`

- [ ] **Step 1: Write the failing tests** (`advisories_tests.rs`), with an
  inline fixture copying real entries: `"192"` (`22.x || 24.x || 26.x` /
  `^22.23.2 || ^24.18.1 || ^26.5.1`, high), one `win32`-only entry, one with
  `"cve": []`, one with `"severity": "unknown"`, one with a leading-space range
  (`" 10.x || 12.x"`), one `"<= 10"`.

```rust
#[test]
fn a_vulnerable_unpatched_version_is_affected() {
    let found = findings(&fixture(), &v("v22.20.0"), Some("linux"));
    assert!(found.iter().any(|f| f.name == "CVE-2026-58043" && f.severity == Severity::High));
}

#[test]
fn a_patched_version_is_not_affected() {
    assert!(findings(&fixture(), &v("v22.23.2"), Some("linux")).iter().all(|f| f.name != "CVE-2026-58043"));
}

#[test]
fn a_platform_specific_advisory_counts_only_on_its_platform() {
    assert!(findings(&fixture(), &v(WIN_VERSION), Some("linux")).iter().all(|f| f.name != WIN_CVE));
    assert!(findings(&fixture(), &v(WIN_VERSION), None).iter().any(|f| f.name == WIN_CVE));
}

#[test]
fn an_advisory_without_cve_ids_is_named_by_id() {
    assert!(findings(&fixture(), &v(NO_CVE_VERSION), Some("linux")).iter().any(|f| f.name == "advisory 12"));
}

#[test]
fn unknown_and_missing_severities_count_as_high() {
    let unknown = parse_advisories(&entry_with(r#""severity": "unknown""#)).unwrap();
    let missing = parse_advisories(&entry_without_severity()).unwrap();
    assert_eq!((unknown[0].severity, missing[0].severity), (Severity::High, Severity::High));
}

#[test]
fn anything_else_is_a_failure_to_know() {
    assert!(matches!(parse_advisories("[]"), Err(IndexError::NotAnIndex(_))));
    assert!(matches!(parse_advisories(&entry_with(r#""severity": "urgent""#)), Err(IndexError::BadEntry { .. })));
    assert!(matches!(parse_advisories(&entry_with(r#""vulnerable": ">>22""#)), Err(IndexError::BadEntry { .. })));
}

#[test]
fn ranges_with_spaces_and_comparators_parse() {
    assert!(parse_advisories(FIXTURE).is_ok());
}

#[test]
fn severities_rank_from_none_to_critical() {
    assert!(Severity::None.rank() < Severity::Low.rank() && Severity::High.rank() < Severity::Critical.rank());
}
```

- [ ] **Step 2: Run them** — `cargo test domain::policy::advisories` —
  expected: compile failure.
- [ ] **Step 3: Implement.** Deserialize into `BTreeMap<String, RawEntry>`
  (unknown fields allowed — the index is not ours); trim ranges before
  `Range::parse`; `patched` empty or missing is `None`; io.js versions never
  match (callers skip them, as for the other rules). Compare through
  `nodejs_semver::Version` built from `major.minor.patch`.
- [ ] **Step 4: Run** `cargo test domain::policy` — expected: pass.
- [ ] **Step 5: Commit** — `feat(policy): parse the vulnerability index and
  find what affects a version`

### Task 3: Merging the CVE settings

**Files:**

- Create: `src/domain/policy/cve.rs`, `src/domain/policy/cve_tests.rs`
- Modify: `src/domain/policy/source.rs` (`Source::Default`),
  `src/domain/policy/mod.rs`

**Interfaces:**

- Consumes: `Severity` (Task 2), `Source`, `Sourced`.
- Produces:
  - `Source::Default` — Display `the default`; `is_override()` false
  - `pub const DEFAULT_INDEX: &str =
    "https://raw.githubusercontent.com/nodejs/security-wg/main/vuln/core/index.json";`
  - `struct CveLayer { pub source: Source, pub limit: Option<Severity>, pub
    index: Option<String> }`
  - `struct CvePolicy { pub limit: Sourced<Severity>, pub index:
    Sourced<String> }`
  - `pub fn merge_cve(layers: Vec<CveLayer>) -> (CvePolicy, Vec<String>)`
- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn without_any_setting_the_limit_is_high_and_the_index_is_the_default() {
    let (policy, warnings) = merge_cve(vec![]);
    assert_eq!(policy.limit, Sourced { value: Severity::High, source: Source::Default });
    assert_eq!(policy.index.value, DEFAULT_INDEX);
    assert!(warnings.is_empty());
}

#[test]
fn the_strictest_limit_wins_and_a_looser_override_warns() {
    let (policy, warnings) = merge_cve(vec![central(Some(Severity::High), None), flag(Some(Severity::Critical))]);
    assert_eq!(policy.limit.value, Severity::High);
    assert_eq!(warnings, vec!["nvm: --fail-on-severity critical is looser than high from policy /p.json; using high."]);
    let (policy, _) = merge_cve(vec![central(Some(Severity::High), None), variable(Some(Severity::Low))]);
    assert_eq!(policy.limit, Sourced { value: Severity::Low, source: Source::Variable("NVMRC_FAIL_ON_SEVERITY") });
}

#[test]
fn none_is_the_loosest_limit() {
    let (policy, warnings) = merge_cve(vec![central(Some(Severity::Medium), None), repository(Some(Severity::None), None)]);
    assert_eq!(policy.limit.value, Severity::Medium);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn the_central_index_wins_and_the_repository_index_counts_only_alone() {
    let (with_central, _) = merge_cve(vec![central(None, None), repository(None, Some("/w/vuln.json"))]);
    assert_eq!(with_central.index.value, DEFAULT_INDEX);
    let (alone, _) = merge_cve(vec![repository(None, Some("/w/vuln.json"))]);
    assert_eq!(alone.index, Sourced { value: "/w/vuln.json".into(), source: Source::RepositoryPolicy("/w/.nvmrc-policy.json".into()) });
}
```

- [ ] **Step 2: Run them** — expected: compile failure.
- [ ] **Step 3: Implement.** Strictest = highest `rank()`, except that `None`
  (rank 0) is looser than everything; ties go to the first layer (callers pass
  the central layer first). Warning text, exact: `nvm: {source} {value} is
  looser than {winner} from {winner_source}; using {winner}.` (values
  lowercase, as in JSON). The index: the central layer's `index` when a
  central layer is present (else `DEFAULT_INDEX` with `Source::Default`), the
  repository layer's only when no central layer is present.
- [ ] **Step 4: Run** `cargo test domain::policy` — expected: pass.
- [ ] **Step 5: Commit** — `feat(policy): merge the severity limit and the
  index location`

### Task 4: The severity flag and the mode

**Files:**

- Modify: `src/domain/policy/flags.rs` (table), `src/commands/policy/load.rs`
  (policy mode on any `NVMRC_` policy variable), `src/commands/policy/mode.rs`
  (`Mode::Enforced` carries `EnforcedPolicy`), `src/commands/policy/enforce.rs`
  (adapt to the new mode shape only)
- Test: `src/commands/policy/tests/mode.rs`, `src/domain/policy/flags_tests.rs`,
  `src/shell/init/` snippet tests

**Interfaces:**

- Consumes: `CveLayer`, `CvePolicy`, `merge_cve` (Task 3), `parse_document`.
- Produces:
  - `POLICY_FLAGS` gains `{ name: "--fail-on-severity", variable:
    "NVMRC_FAIL_ON_SEVERITY" }` (the shell patterns follow automatically)
  - `pub struct EnforcedPolicy { pub node: NodePolicy, pub cve: CvePolicy }`
  - `pub enum Mode { Legacy, Enforced(EnforcedPolicy) }`
  - `resolve_mode` signature unchanged
- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn the_severity_flag_and_variable_turn_policy_mode_on() {
    assert!(policy_mode_requested(&ctx_with_var("NVMRC_FAIL_ON_SEVERITY", "low")));
    assert!(policy_mode_requested(&ctx_with_flag("--fail-on-severity", "low")));
}

#[test]
fn an_invalid_severity_flag_is_a_failure_to_know() {
    let error = resolve_mode(&ctx_with_flag("--fail-on-severity", "critial")).unwrap_err();
    assert_eq!(error, PolicyError::Invalid {
        location: "--fail-on-severity".into(),
        reason: "'critial' is not one of critical, high, medium, low, none".into(),
    });
}

#[test]
fn the_cve_settings_of_both_policies_take_part() {
    // central {cve: {failOnSeverity: medium}}, repository {cve: {failOnSeverity: low, source: "v.json"}}
    let (Mode::Enforced(policy), _) = resolve_mode(&both_policies_ctx()).unwrap() else { panic!() };
    assert_eq!(policy.cve.limit.value, Severity::Low);
    assert_eq!(policy.cve.index.value, DEFAULT_INDEX);
}

#[test]
fn a_relative_index_location_is_read_next_to_its_policy() {
    // /w/.nvmrc-policy.json {cve: {source: "sec/vuln.json"}}, PWD=/w/app/src
    let (Mode::Enforced(policy), _) = resolve_mode(&repo_ctx_with_relative_index()).unwrap() else { panic!() };
    assert_eq!(policy.cve.index.value, "/w/sec/vuln.json");
}

#[test]
fn a_relative_index_in_a_remote_policy_is_invalid() {
    // NVMRC_POLICY=https://sec/p.json with cve.source "vuln.json"
    assert!(matches!(resolve_mode(&remote_ctx_with_relative_index()).unwrap_err(), PolicyError::Invalid { .. }));
}
```

Plus the flag test `the_severity_flag_is_taken_like_the_others` (before and
after the subcommand, `=` form) and the snippet test that the POSIX and fish
patterns include `--fail-on-severity`.

- [ ] **Step 2: Run them** — expected: failures.
- [ ] **Step 3: Implement.** `policy_mode_requested` adds: any `POLICY_FLAGS`
  variable starting with `NVMRC_` that is set and non-blank. `resolve_mode`
  builds CVE layers in the order central, repository, flag, variable (central
  first, as for node). A relative `cve.source` in a repository policy becomes
  `<policy directory>/<source>`; in a central policy read from a path,
  `<that file's directory>/<source>`; in a central policy read from https, it
  is `PolicyError::Invalid` naming the policy location. An invalid severity
  value from the flag or the variable is `PolicyError::Invalid` with the exact
  reason above. A document without `tools.node` still contributes its CVE
  layer.
- [ ] **Step 4: Run** `cargo test` — expected: pass.
- [ ] **Step 5: Commit** — `feat(policy): add --fail-on-severity and carry
  the CVE settings in the mode`

### Task 5: Enforcing the CVE check

**Files:**

- Modify: `src/commands/policy/enforce.rs` (fetch, findings, report),
  `src/domain/policy/verdict.rs` (`Violation::Vulnerable`)
- Create: `src/commands/policy/advisories.rs` (fetching and judging, to keep
  `enforce.rs` under 300 lines)
- Test: `src/commands/policy/tests/advisories.rs`,
  `src/commands/install/tests/policy.rs`, `src/commands/use_version/tests*`

**Interfaces:**

- Consumes: `parse_advisories`, `findings`, `Finding` (Task 2), `CvePolicy`
  (Task 3), `EnforcedPolicy` (Task 4), `read_location`,
  `Context::platform()`.
- Produces:
  - `Violation::Vulnerable { findings: Vec<Finding>, limit: Sourced<Severity> }`
    — Display `CVE-2024-27983 (high), CVE-2024-22019 (high); the limit is high
    (from policy /p.json)`
  - `pub(super) fn judge(context: &Context<'_>, requested: &Requested, cve:
    &CvePolicy) -> Result<Judgement, PolicyError>` with `struct Judgement {
    pub failing: Vec<Finding>, pub below: Vec<Finding> }`
  - `PolicyError::IndexInvalid { location: String, reason: String }` —
    message `nvm: cannot check the policy: the vulnerability index {location}
    is not valid: {reason}`
- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_finding_at_the_limit_stops_with_7() {
    // --min-node unset, NVMRC_FAIL_ON_SEVERITY=high, FakeHttp serves the fixture at DEFAULT_INDEX
    let (gate, transcript) = gate(&ctx_with_index(), "v22.20.0");
    assert!(matches!(gate, Gate::Stop(NvmExitCode::BelowVersionFloor)));
    assert!(stderr(transcript).contains("  - CVE-2026-58043 (high)"));
    assert!(stderr(transcript).contains("the limit is high (from NVMRC_FAIL_ON_SEVERITY)"));
}

#[test]
fn findings_below_the_limit_only_warn() {
    let (gate, transcript) = gate(&ctx_with_limit("critical"), "v22.20.0");
    assert!(matches!(gate, Gate::Pass));
    assert!(stderr(transcript).starts_with("nvm: warning: node v22.20.0 has known vulnerabilities below the limit critical:"));
}

#[test]
fn limit_none_reports_and_passes() {
    let (gate, transcript) = gate(&ctx_with_limit("none"), "v22.20.0");
    assert!(matches!(gate, Gate::Pass));
    assert!(stderr(transcript).contains("CVE-2026-58043 (high)"));
}

#[test]
fn an_unreachable_index_is_a_failure_to_know() {
    let error = enforce_node(&ctx_without_index(), &requested("v22.20.0"), &mut Transcript::default()).unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
}

#[test]
fn an_invalid_index_names_its_location() {
    let error = enforce_node(&ctx_with_body("[]"), &requested("v22.20.0"), &mut Transcript::default()).unwrap_err();
    assert!(error.to_string().contains("the vulnerability index https://raw.githubusercontent.com/nodejs/security-wg/main/vuln/core/index.json is not valid"));
}

#[test]
fn offline_with_a_remote_index_is_a_failure_to_know() {
    let error = enforce_node(&ctx_with_index(), &offline("v22.20.0"), &mut Transcript::default()).unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
    assert!(http_requests().is_empty());
}

#[test]
fn offline_with_a_local_index_is_checked() { /* repository policy cve.source = local file → judged, no request */ }

#[test]
fn an_automatic_path_warns_on_a_finding_and_passes() { /* .advisory() context → Pass, warning header */ }

#[test]
fn legacy_mode_never_fetches_the_index() { /* NVM_MIN_VERSION only → no request */ }

#[test]
fn iojs_versions_skip_the_cve_check() { /* NotNode only, no index request */ }
```

In `install/tests/policy.rs`: `install_of_a_vulnerable_version_stops_before_downloading`.
In the `use` tests: `use_of_a_vulnerable_installed_version_stops_without_switching`.

- [ ] **Step 2: Run them** — expected: failures.
- [ ] **Step 3: Implement.** In `check`, after the node violations: skip the
  CVE check for io.js; otherwise `judge`. `judge` reads the index with
  `read_location(context, &cve.index.value)` (offline and an `https://`
  index → `ReleaseInfoUnavailable("offline")` before any request; reuse the
  existing variant), parses it (`IndexError` → `PolicyError::IndexInvalid`),
  takes `context.platform().map(|p| p.os.slug())`, and splits `findings` by
  `severity.rank() >= limit.rank()` (limit `None`: everything is `below`).
  `failing` non-empty → one `Violation::Vulnerable` appended to the node
  violations (same header and fix line). `below` non-empty → one warning line
  `nvm: warning: node {version} has known vulnerabilities below the limit
  {limit}: {names with severities}` printed whatever the gate decides.
  Fetching happens once per command.
- [ ] **Step 4: Run** `cargo test` — expected: pass, compat suite included.
- [ ] **Step 5: Commit** — `feat(policy): fail on known vulnerabilities at or
  above the severity limit`

### Task 6: `use --save` gates before it saves

**Files:**

- Modify: `src/commands/use_version/mod.rs`,
  `src/commands/use_version/target/mod.rs` (only as needed)
- Test: `src/commands/use_version/tests*`

**Interfaces:**

- Consumes: `policy_mode_requested`, the gate from Plan 11
  (`commands::policy::gate`).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_refused_use_save_leaves_nvmrc_untouched() {
    // installed v20.1.0, --min-node 22, /w/.nvmrc contains "v22.11.0"
    let output = run(&ctx, &args(&["--save", "20"])).unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert_eq!(fs.read_to_string(Path::new("/w/.nvmrc")).unwrap(), "v22.11.0\n");
}

#[test]
fn an_allowed_use_save_still_writes_nvmrc() { /* --min-node 18 → .nvmrc becomes v20.1.0 */ }

#[test]
fn legacy_use_save_keeps_its_order() { /* no policy: today's output byte for byte */ }
```

- [ ] **Step 2: Run them** — expected: the first fails.
- [ ] **Step 3: Implement.** In policy mode only, resolve the target with the
  save deferred, run the gate, and write `.nvmrc` only when it passes; legacy
  mode keeps nvm.sh's order (save before the checks).
- [ ] **Step 4: Run** `cargo test` — expected: pass.
- [ ] **Step 5: Commit** — `fix(policy): write .nvmrc only after the policy
  allows the version`

### Task 7: Documentation

**Files:**

- Modify: `README.md` (CVE check, `--fail-on-severity`,
  `NVMRC_FAIL_ON_SEVERITY`, the index and its mirror, offline, `use --save`),
  `docs/deviations.md` (each entry with its pinning test)
- [ ] **Step 1: Write the sections** in the existing style; state the
  vulnerable-and-not-patched rule, `unknown` = `high`, platform filtering, the
  default index URL, and the `cve.source` rule. Remove the "CVE: accepted but
  not enforced" wording.
- [ ] **Step 2: Run** `rumdl fmt` then `rumdl check README.md docs/`, then
  `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
  warnings`, `cargo test`, `cargo deny check` — expected: clean.
- [ ] **Step 3: Commit** — `docs: document the CVE check and
  --fail-on-severity`
