# nvmrc Plan 11: Security Policy Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver step 1 of the security policy design: the policy file and
its JSON Schema, the `--policy` and `--min-node` flags with their variables,
the repository policy, tighten-only merging, legacy and policy modes, every
node rule except CVEs (`minimum`, `allow`, `deny`, `ltsOnly`,
`minimumReleaseAgeDays`) enforced in `install`, `use`, `exec` and `run`,
warn-only automatic paths, and exit code 9.

**Architecture:** The pure part lives in `src/domain/policy/` (the document
types, the sources and their tighten-only merge, the verdict, the flag
table). The impure part lives in `src/commands/policy/` (reading a location,
resolving the mode, the enforcement gate that prints and stops). The CLI
strips the policy flags before clap and hands them to commands through two
new `Context` fields, `policy_flags` and `advisory`. Commands call one gate,
`commands::policy::enforce::enforce_node`, after they know the version.

**Tech Stack:** Rust 2024 (MSRV 1.85, toolchain 1.99), new dependencies
`serde` 1 (derive), `serde_json` 1, `nodejs-semver` 7 (Apache-2.0, MSRV
1.85), dev-dependency `schemars` 1; cargo-deny, rumdl.

**Spec:** `docs/superpowers/specs/2026-10-17-security-policy-design.md`
(sections 4, 5, 6.1 to 6.5, 6.10; delivery step 1 of section 7). This plan
starts from `feat/policy-floor` at `3307f08`. Later plans: 12 (CVE check, with
`--fail-on-severity` and the repository policy's `cve.source` rule), 13 (npm,
yarn and pnpm, with `--min-npm`, `--min-yarn`, `--min-pnpm`), 14 (`nvm init`
project mode), 15 (`nvm upgrade`). This plan parses the whole schema (`cve`
and the package-manager blocks included) because the schema is the contract,
but enforces only node's rules.

## Global Constraints

- With no policy, no `--policy`/`--min-node` flag and no `NVMRC_POLICY`, and
  no `.nvmrc-policy.json` above the working directory, behaviour is exactly
  today's; `cargo test` (the compat suite included) passes unchanged.
- `NVM_MIN_VERSION` or `$NVM_DIR/min-version` alone is legacy mode: checked in
  `install` only, with today's messages.
- Every failure to know in an explicit command exits **9**; every violation
  exits **7**. Automatic paths (`__auto`) never change status because of the
  policy: they print a warning and go on.
- Unknown fields are rejected at every level of a policy; `schemaVersion` must
  be `1`.
- A location starting with `http://` is rejected; `https://` goes through the
  `Http` port; anything else is a path, relative paths resolved against the
  logical working directory (`Context::working_directory`).
- Overrides only tighten: `minimum` highest wins, `allow` intersects, `deny`
  unions, `ltsOnly` on if any source sets it, `minimumReleaseAgeDays` largest
  wins; a looser override is a warning naming its source and the winning value.
- Env vars are read through `context.env`; clap's `env` feature is not used.
- Files under 300 lines, functions under 30, cyclomatic complexity under 10,
  `cargo clippy --all-targets --all-features -- -D warnings`, `cargo fmt`,
  `rumdl check` on changed Markdown, no network in tests, TDD.
- Commits are Conventional Commits, GPG-signed (`git commit -S`), with no AI
  attribution.

## Review Focus

- `nvm exec 20 node --policy x.js`: a `--policy` among the child's arguments
  reaches `node` untouched; only flags before the subcommand or immediately
  after its name are taken (Task 4 test
  `flags_after_the_first_argument_are_left_alone`).
- A policy with a trailing comma, a comment or a byte-order mark: exit 9 with
  the location and serde's line and column (Task 1 test
  `malformed_json_reports_line_and_column`, Task 5 test
  `an_invalid_central_policy_is_a_failure_to_know`).
- `--policy ./sec.json` from a subdirectory: resolved against the logical
  working directory, not the process directory (Task 5 test
  `a_relative_location_is_read_from_the_working_directory`).
- `NVMRC_POLICY=""` (set but empty): treated as unset, as `NVM_MIN_VERSION`
  is (Task 5 test `an_empty_policy_variable_is_unset`).
- `nvm use iojs-v3.3.1` in policy mode: a violation (io.js is not covered by
  any policy), not a crash or a pass (Task 3 test
  `an_iojs_version_is_never_allowed`).

---

### Task 1: Policy document, parsing and the JSON Schema

**Files:**

- Modify: `Cargo.toml` (dependencies)
- Create: `src/domain/policy/mod.rs`, `src/domain/policy/document.rs`,
  `src/domain/policy/document_tests.rs`
- Modify: `src/domain/mod.rs` (add `pub mod policy;`)
- Create: `schemas/nvmrc-policy.v1.schema.json`

**Interfaces:**

- Produces:
  - `PolicyDocument { schema: Option<String> /* "$schema" */, schema_version:
    u32, tools: Tools, cve: Option<CveSettings> }`
  - `Tools { node: Option<NodeRules>, npm: Option<PackageRules>, yarn:
    Option<PackageRules>, pnpm: Option<PackageRules> }` (`Default`)
  - `NodeRules { minimum: Option<String>, allow: Option<String>, deny:
    Option<Vec<String>>, lts_only: Option<bool>, minimum_release_age_days:
    Option<u32> }`
  - `PackageRules` — the same fields without `lts_only`
  - `CveSettings { fail_on_severity: Option<Severity>, source: Option<String> }`
  - `enum Severity { Critical, High, Medium, Low, None }` (lowercase in JSON)
  - `pub fn parse_document(text: &str) -> Result<PolicyDocument,
    DocumentError>`
  - `enum DocumentError { Malformed(String), UnsupportedSchemaVersion(u32),
    InvalidVersion { field: String, value: String }, InvalidRange { field:
    String, value: String } }` (thiserror)

- [ ] **Step 1: Add the dependencies**

`Cargo.toml`: `serde = { version = "1", features = ["derive"] }`,
`serde_json = "1"`, `nodejs-semver = "7"`; dev: `schemars = "1"`. Run
`cargo deny check` — expected: no license or source errors.

- [ ] **Step 2: Write the failing tests** in `document_tests.rs`

```rust
#[test]
fn the_spec_example_parses() {
    let document = parse_document(SPEC_EXAMPLE).unwrap(); // §6.1 example, verbatim
    let node = document.tools.node.unwrap();
    assert_eq!(node.minimum.as_deref(), Some("20.11.1"));
    assert_eq!(node.allow.as_deref(), Some(">=20 <25"));
    assert_eq!(node.deny, Some(vec!["20.12.0".into(), "22.3.0".into()]));
    assert_eq!(node.lts_only, Some(true));
    assert_eq!(node.minimum_release_age_days, Some(14));
    assert_eq!(document.cve.unwrap().fail_on_severity, Some(Severity::High));
}

#[test]
fn an_unknown_field_is_rejected_at_any_level() {
    for text in [
        r#"{"schemaVersion":1,"extra":true}"#,
        r#"{"schemaVersion":1,"tools":{"deno":{}}}"#,
        r#"{"schemaVersion":1,"tools":{"node":{"minimun":"20"}}}"#,
        r#"{"schemaVersion":1,"cve":{"failOn":"high"}}"#,
    ] {
        assert!(matches!(parse_document(text), Err(DocumentError::Malformed(_))), "{text}");
    }
}

#[test]
fn lts_only_is_rejected_outside_node() {
    let text = r#"{"schemaVersion":1,"tools":{"pnpm":{"ltsOnly":true}}}"#;
    assert!(matches!(parse_document(text), Err(DocumentError::Malformed(_))));
}

#[test]
fn only_schema_version_1_is_understood() {
    assert_eq!(parse_document(r#"{"schemaVersion":2}"#),
        Err(DocumentError::UnsupportedSchemaVersion(2)));
    assert!(matches!(parse_document("{}"), Err(DocumentError::Malformed(_))));
}

#[test]
fn versions_and_ranges_are_validated() {
    let bad_minimum = r#"{"schemaVersion":1,"tools":{"node":{"minimum":"twenty"}}}"#;
    let bad_deny = r#"{"schemaVersion":1,"tools":{"node":{"deny":["20"]}}}"#; // deny is exact only
    let bad_range = r#"{"schemaVersion":1,"tools":{"node":{"allow":">>20"}}}"#;
    assert!(matches!(parse_document(bad_minimum), Err(DocumentError::InvalidVersion { .. })));
    assert!(matches!(parse_document(bad_deny), Err(DocumentError::InvalidVersion { .. })));
    assert!(matches!(parse_document(bad_range), Err(DocumentError::InvalidRange { .. })));
}

#[test]
fn malformed_json_reports_line_and_column() {
    let error = parse_document("{\n  \"schemaVersion\": 1,\n}").unwrap_err();
    assert!(error.to_string().contains("line 3"), "{error}");
}

#[test]
fn committed_schema_matches_the_types() {
    let generated = serde_json::to_string_pretty(&schemars::schema_for!(PolicyDocument)).unwrap();
    let committed = include_str!("../../../schemas/nvmrc-policy.v1.schema.json");
    assert_eq!(committed.trim_end(), generated, "regenerate schemas/nvmrc-policy.v1.schema.json");
}
```

- [ ] **Step 3: Run them** — `cargo test domain::policy::document` — expected:
  compile failure (module missing).

- [ ] **Step 4: Implement `document.rs`**

Types derive `Debug, Clone, PartialEq, Eq, Deserialize` and
`#[cfg_attr(test, derive(schemars::JsonSchema))]`, with
`#[serde(deny_unknown_fields, rename_all = "camelCase")]`; `Severity` uses
`rename_all = "lowercase"`. `parse_document` deserializes with
`serde_json::from_str` (map the error to `Malformed(error.to_string())`, which
carries line and column), then validates: `schema_version == 1`; every
`minimum` parses as `VersionPattern`; every `deny` entry parses as `Version`
and has three numeric parts; every `allow` parses as `nodejs_semver::Range`.
`field` names the JSON path (`tools.node.deny[0]`). Add schema annotations so
the schema says what the code enforces: `schemaVersion` minimum and maximum
1, version strings pattern `^v?\d+(\.\d+){0,2}$`, `deny` items pattern
`^v?\d+\.\d+\.\d+$`.

- [ ] **Step 5: Write the schema file** — run the schema test once, paste the
  generated JSON (from the assertion's right side) into
  `schemas/nvmrc-policy.v1.schema.json` with a trailing newline, add `"$id":
  "https://raw.githubusercontent.com/elioseverojunior/nvmrc/main/schemas/nvmrc-policy.v1.schema.json"`
  through a `#[schemars(extend("$id" = ...))]` attribute on `PolicyDocument`
  so the file stays generated, and rerun.

- [ ] **Step 6: Run the tests** — `cargo test domain::policy::document` —
  expected: all pass.

- [ ] **Step 7: Commit** — `feat(policy): parse and validate the policy
  document and publish its schema`

### Task 2: Sources and the tighten-only merge

**Files:**

- Create: `src/domain/policy/source.rs`, `src/domain/policy/rules.rs`,
  `src/domain/policy/rules_tests.rs`

**Interfaces:**

- Consumes: `NodeRules` (Task 1), `Version`, `VersionPattern`.
- Produces:
  - `enum Source { Flag(&'static str), Variable(&'static str),
    CentralPolicy(String), RepositoryPolicy(PathBuf), MinVersionFile(PathBuf) }`
    with `Display` (`--min-node`, `NVM_MIN_VERSION`, `policy
    https://sec/p.json`, `repository policy /w/.nvmrc-policy.json`,
    `/n/min-version`) and `fn is_override(&self) -> bool` (all but
    `CentralPolicy`)
  - `struct Sourced<T> { pub value: T, pub source: Source }`
  - `struct NodeLayer { pub source: Source, pub minimum: Option<Version>, pub
    allow: Option<nodejs_semver::Range>, pub allow_text: Option<String>, pub
    deny: Vec<Version>, pub lts_only: Option<bool>,
    pub minimum_release_age_days: Option<u32> }`
  - `impl NodeLayer { pub fn from_rules(source: Source, rules: &NodeRules) ->
    Result<Self, DocumentError>; pub fn minimum_only(source: Source, minimum:
    Version) -> Self }` (`from_rules` re-parses the strings `parse_document`
    already validated; it returns the error instead of unwrapping)
  - `struct NodePolicy { pub minimum: Option<Sourced<Version>>, pub allow:
    Vec<Sourced<(nodejs_semver::Range, String)>>, pub deny:
    Vec<Sourced<Version>>, pub lts_only: Option<Source>,
    pub minimum_release_age_days: Option<Sourced<u32>> }`
  - `pub fn merge(layers: Vec<NodeLayer>) -> (NodePolicy, Vec<String>)` —
    the policy and the loosening warnings
- [ ] **Step 1: Write the failing tests** in `rules_tests.rs`

```rust
#[test]
fn the_highest_minimum_wins_and_names_its_source() {
    let (policy, _) = merge(vec![central(minimum("20.11.1")), variable(minimum("22"))]);
    let winner = policy.minimum.unwrap();
    assert_eq!(winner.value.to_string(), "v22.0.0");
    assert_eq!(winner.source, Source::Variable("NVM_MIN_VERSION"));
}

#[test]
fn a_lower_override_loses_with_a_warning() {
    let (policy, warnings) = merge(vec![central(minimum("20.11.1")), flag(minimum("16"))]);
    assert_eq!(policy.minimum.unwrap().value.to_string(), "v20.11.1");
    assert_eq!(warnings, vec![
        "nvm: --min-node 16 is looser than v20.11.1 from policy /p.json; using v20.11.1."
    ]);
}

#[test]
fn allow_ranges_all_apply_and_deny_lists_add_up() {
    let (policy, _) = merge(vec![central(allow(">=20 <25").deny(&["20.12.0"])),
                                 repository(allow("^22").deny(&["22.3.0"]))]);
    assert_eq!(policy.allow.len(), 2);
    assert_eq!(policy.deny.len(), 2);
}

#[test]
fn lts_only_is_on_when_any_source_sets_it_and_an_explicit_false_warns() {
    let (policy, warnings) = merge(vec![central(lts_only(true)), repository(lts_only(false))]);
    assert!(policy.lts_only.is_some());
    assert_eq!(warnings.len(), 1);
}

#[test]
fn the_largest_release_age_wins() {
    let (policy, warnings) = merge(vec![central(age(14)), repository(age(7))]);
    assert_eq!(policy.minimum_release_age_days.unwrap().value, 14);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn the_central_policy_never_warns_about_itself() {
    let (_, warnings) = merge(vec![central(minimum("18")), repository(minimum("20"))]);
    assert!(warnings.is_empty());
}
```

(`central`, `flag`, `variable`, `repository` and the rule builders are small
test helpers in the test file returning `NodeLayer`; `central` uses
`Source::CentralPolicy("/p.json".into())`, `flag` uses
`Source::Flag("--min-node")`.)

- [ ] **Step 2: Run them** — `cargo test domain::policy::rules` — expected:
  compile failure.

- [ ] **Step 3: Implement** `source.rs` and `rules.rs`. Warning text, exact:
  `nvm: {source} {value} is looser than {winner} from {winner_source}; using
  {winner}.` (for `ltsOnly`: value `false`, winner `true`; for the age: day
  counts). Only an override source (`is_override`) that loses produces a
  warning. `allow` keeps the original text for messages.

- [ ] **Step 4: Run the tests** — expected: pass.

- [ ] **Step 5: Commit** — `feat(policy): merge rule sources so overrides only
  tighten`

### Task 3: The verdict, with release dates from the index

**Files:**

- Create: `src/domain/policy/verdict.rs`, `src/domain/policy/verdict_tests.rs`
- Modify: `src/domain/index.rs` (`Release` gains `published_day`),
  `src/domain/timestamp.rs` (`days_from_iso_date`),
  `src/domain/fixtures.rs` (`index_text_dated`)

**Interfaces:**

- Consumes: `NodePolicy`, `Sourced`, `Source` (Task 2), `Version`, `Flavor`.
- Produces:
  - `Release { version, lts, pub published_day: Option<i64> }` — days since
    1970-01-01 from column 2 (`YYYY-MM-DD`); `None` when absent or invalid
  - `pub fn days_from_iso_date(text: &str) -> Option<i64>` (inverse of the
    existing `civil_from_days`)
  - `pub fn index_text_dated(rows: &[(&str, &str, &str)]) -> String` —
    `(version, date, lts)`
  - `struct ReleaseInfo { pub lts: Option<String>, pub published_day:
    Option<i64> }`
  - `enum Violation { BelowMinimum(Sourced<Version>), OutsideRange(Sourced<String>),
    Denied(Source), NotLts(Source), TooRecent { age_days: i64, required:
    Sourced<u32> }, NotNode }` with `Display` giving the bullet text
  - `impl NodePolicy { pub fn needs_release_info(&self) -> bool; pub fn
    violations(&self, version: &Version, release: Option<&ReleaseInfo>,
    today_day: i64) -> Result<Vec<Violation>, MissingReleaseInfo> }`
  - `struct MissingReleaseInfo;` (the version is not in the index although a
    rule needs it)
- [ ] **Step 1: Write the failing tests**

```rust
// timestamp.rs tests
#[test]
fn iso_dates_round_trip_through_days() {
    assert_eq!(days_from_iso_date("1970-01-01"), Some(0));
    assert_eq!(days_from_iso_date("2024-02-29"), Some(19_782));
    assert_eq!(days_from_iso_date("2024-02-30"), None);
    assert_eq!(days_from_iso_date("-"), None);
}

// index.rs tests
#[test]
fn the_release_date_is_read_from_column_two() {
    let text = index_text_dated(&[("v22.11.0", "2024-10-29", "Jod")]);
    assert_eq!(parse_index(&text, Flavor::Node)[0].published_day, days_from_iso_date("2024-10-29"));
}

// verdict_tests.rs
#[test]
fn each_rule_reports_its_own_violation() {
    let policy = policy_with_all_rules(); // min 20.11.1, allow ">=20 <25", deny 20.12.0, ltsOnly, 14 days
    let today = days_from_iso_date("2024-11-01").unwrap();
    let release = ReleaseInfo { lts: None, published_day: days_from_iso_date("2024-10-29") };
    let found = policy.violations(&v("v25.1.0"), Some(&release), today).unwrap();
    let text: Vec<String> = found.iter().map(ToString::to_string).collect();
    assert_eq!(text, vec![
        "outside the allowed range >=20 <25 (from policy /p.json)",
        "not an LTS release (from policy /p.json)",
        "published 3 days ago; at least 14 required (from policy /p.json)",
    ]);
    assert_eq!(policy.violations(&v("v18.0.0"), Some(&lts("Hydrogen")), today).unwrap()[0].to_string(),
        "below the minimum v20.11.1 (from policy /p.json)");
    assert_eq!(policy.violations(&v("v20.12.0"), Some(&lts("Iron")), today).unwrap()[0].to_string(),
        "denied (from policy /p.json)");
}

#[test]
fn an_allowed_version_has_no_violations() {
    let today = days_from_iso_date("2024-11-01").unwrap();
    let old_lts = ReleaseInfo { lts: Some("Iron".into()), published_day: days_from_iso_date("2024-07-08") };
    assert!(policy_with_all_rules().violations(&v("v20.15.1"), Some(&old_lts), today).unwrap().is_empty());
}

#[test]
fn release_info_is_needed_only_for_lts_and_age_rules() {
    assert!(!minimum_only("20").needs_release_info());
    assert!(policy_with_all_rules().needs_release_info());
    assert_eq!(policy_with_all_rules().violations(&v("v20.15.1"), None, 0), Err(MissingReleaseInfo));
}

#[test]
fn an_iojs_version_is_never_allowed() {
    let found = minimum_only("1").violations(&v("iojs-v3.3.1"), None, 0).unwrap();
    assert_eq!(found, vec![Violation::NotNode]);
    assert_eq!(found[0].to_string(), "io.js is not covered by policy");
}
```

- [ ] **Step 2: Run them** — expected: compile failure.

- [ ] **Step 3: Implement** `days_from_iso_date` (Hinnant's
  `days_from_civil`, rejecting impossible dates by round-tripping through
  `civil_from_days`), `published_day` in `parse_row`, `index_text_dated`, and
  `verdict.rs`. Violation order: `NotNode` alone (stop), then minimum, range,
  deny, LTS, age. Age is `today_day - published_day`; a release without a date
  under an age rule is `MissingReleaseInfo`. Range checks convert `Version` to
  `nodejs_semver::Version` through its `major.minor.patch` text.

- [ ] **Step 4: Run** `cargo test domain::` — expected: pass (existing index
  tests unchanged).

- [ ] **Step 5: Commit** — `feat(policy): decide whether a node version is
  allowed`

### Task 4: Policy flags in the CLI and the context

**Files:**

- Create: `src/domain/policy/flags.rs`, `src/domain/policy/flags_tests.rs`
- Modify: `src/context/mod.rs` (derive `Clone`; fields `policy_flags`,
  `advisory`), `src/cli/mod.rs` (`run` strips the flags),
  `src/cli/tests/policy.rs` (new), `src/cli/tests/mod.rs` (`mod policy;`)

**Interfaces:**

- Produces:
  - `struct PolicyFlag { pub name: &'static str, pub variable: &'static str }`
  - `pub const POLICY_FLAGS: [PolicyFlag; 2] = [{ "--policy", "NVMRC_POLICY" },
    { "--min-node", "NVM_MIN_VERSION" }]` — later plans append to this table
  - `#[derive(Debug, Clone, Default, PartialEq, Eq)] struct PolicyFlags` with
    `pub fn get(&self, name: &str) -> Option<&str>` and `pub fn is_empty(&self)
    -> bool`
  - `pub fn extract(args: Vec<OsString>) -> Result<(PolicyFlags,
    Vec<OsString>), FlagError>`; `FlagError` (thiserror) messages exactly
    `nvm: {flag} needs a value.`, `nvm: {flag} is given twice.`,
    `nvm: the value of {flag} is not valid UTF-8.`
  - `Context::with_policy_flags(self, PolicyFlags) -> Self`,
    `Context::policy_flags(&self) -> &PolicyFlags`, `Context::advisory(self)
    -> Self`, `Context::is_advisory(&self) -> bool`

- [ ] **Step 1: Write the failing tests** in `flags_tests.rs`

```rust
#[test]
fn flags_before_the_subcommand_are_taken() {
    let (flags, rest) = extract(args(&["nvm", "--policy", "/p.json", "use", "20"])).unwrap();
    assert_eq!(flags.get("--policy"), Some("/p.json"));
    assert_eq!(rest, args(&["nvm", "use", "20"]));
}

#[test]
fn flags_right_after_the_subcommand_are_taken_in_both_forms() {
    let (flags, rest) = extract(args(&["nvm", "use", "--min-node=20", "--policy", "p", "20"])).unwrap();
    assert_eq!(flags.get("--min-node"), Some("20"));
    assert_eq!(flags.get("--policy"), Some("p"));
    assert_eq!(rest, args(&["nvm", "use", "20"]));
}

#[test]
fn flags_after_the_first_argument_are_left_alone() {
    let (flags, rest) = extract(args(&["nvm", "exec", "20", "node", "--policy", "x.js"])).unwrap();
    assert!(flags.is_empty());
    assert_eq!(rest, args(&["nvm", "exec", "20", "node", "--policy", "x.js"]));
}

#[test]
fn a_missing_or_repeated_value_is_an_error() {
    assert_eq!(extract(args(&["nvm", "--policy"])).unwrap_err().to_string(), "nvm: --policy needs a value.");
    assert_eq!(extract(args(&["nvm", "--policy=a", "--policy", "b", "ls"])).unwrap_err().to_string(),
        "nvm: --policy is given twice.");
}
```

In `src/cli/tests/policy.rs`:

```rust
#[test]
fn a_flag_error_is_a_usage_error() {
    let (code, _, err) = run_cli(&["nvm", "--policy"]);
    assert_eq!(code, 127);
    assert_eq!(err.trim_end(), "nvm: --policy needs a value.");
}

#[test]
fn stripped_flags_do_not_reach_clap() {
    let (code, out, _) = run_cli(&["nvm", "--min-node", "18", "version", "20"]);
    assert_eq!((code, out.trim_end()), (0, "v20.1.0"));
}
```

- [ ] **Step 2: Run them** — expected: compile failure.

- [ ] **Step 3: Implement** `extract`: from index 1, take policy flags while
  they come; keep the next argument (the subcommand); then take policy flags
  again while they come; copy everything else verbatim. In `cli::run`, call
  it on the raw arguments **before** `arguments(...)`, so a bare
  `nvm --policy p` still becomes `nvm --help` and the `--` rule of
  `keep_leading_double_dash` sees the real subcommand at index 1; on error
  print the message and return 127; else run clap on `arguments(rest)` with
  `context.clone().with_policy_flags(flags)`. `PolicyFlags`, `FlagError`,
  `PolicyError` and `MissingReleaseInfo` derive `PartialEq` for the tests.

- [ ] **Step 4: Run** `cargo test` — expected: all pass.

- [ ] **Step 5: Commit** — `feat(policy): accept --policy and --min-node
  around the subcommand`

### Task 5: Reading locations and resolving the mode

**Files:**

- Create: `src/commands/policy/mod.rs`, `src/commands/policy/load.rs`,
  `src/commands/policy/mode.rs`, `src/commands/policy/tests/` (`load.rs`,
  `mode.rs`, `mod.rs`)
- Modify: `src/commands/mod.rs` (`pub mod policy;`), `src/domain/nvmrc/mod.rs`
  (generalise the walk into `pub fn find_up(fs, start, file_name) ->
  Option<PathBuf>`; `find_nvmrc` calls it), `src/error.rs` (`PolicyError`,
  `CliError::Policy`, `NvmExitCode::PolicyUnverifiable` = 9)

**Interfaces:**

- Consumes: `parse_document`, `NodeLayer`, `merge`, `Source` (Tasks 1–2),
  `Context::policy_flags` (Task 4), `VersionFloor::parse`.
- Produces:
  - `pub const REPOSITORY_POLICY_FILE: &str = ".nvmrc-policy.json";`
  - `pub fn read_location(context: &Context<'_>, location: &str) ->
    Result<String, PolicyError>`
  - `pub fn policy_mode_requested(context: &Context<'_>) -> bool` — a policy
    flag, a non-empty `NVMRC_POLICY`, or a repository policy file found
  - `pub enum Mode { Legacy, Enforced(NodePolicy) }`
  - `pub fn resolve_mode(context: &Context<'_>) -> Result<(Mode, Vec<String>),
    PolicyError>`
  - `enum PolicyError { InsecureUrl(String), Unreadable { location: String,
    reason: String }, Invalid { location: String, reason: String },
    ReleaseInfoUnavailable(String) }` — messages start `nvm: cannot check the
    policy:` and name the location
- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn an_http_location_is_rejected() {
    assert_eq!(read_location(&ctx(), "http://sec/p.json"), Err(PolicyError::InsecureUrl("http://sec/p.json".into())));
}

#[test]
fn an_https_location_is_fetched_and_a_failure_is_unreadable() {
    let http = FakeHttp::default().with_body("https://sec/p.json", POLICY);
    assert_eq!(read_location(&ctx().with_http(&http), "https://sec/p.json").unwrap(), POLICY);
    assert!(matches!(read_location(&ctx(), "https://sec/missing.json"), Err(PolicyError::Unreadable { .. })));
}

#[test]
fn a_relative_location_is_read_from_the_working_directory() {
    // FakeEnv PWD=/w/app (with current dir /w/app), file /w/app/sec/p.json
    assert_eq!(read_location(&ctx_in("/w/app"), "sec/p.json").unwrap(), POLICY);
}

#[test]
fn legacy_mode_without_any_new_configuration() {
    // NVM_MIN_VERSION=20 and $NVM_DIR/min-version set, nothing else
    assert!(matches!(resolve_mode(&legacy_ctx()).unwrap().0, Mode::Legacy));
}

#[test]
fn an_empty_policy_variable_is_unset() {
    assert!(!policy_mode_requested(&ctx_with_var("NVMRC_POLICY", "")));
}

#[test]
fn the_min_node_flag_alone_enforces() {
    let Mode::Enforced(policy) = resolve_mode(&ctx_with_flag("--min-node", "20")).unwrap().0 else { panic!() };
    assert_eq!(policy.minimum.unwrap().source, Source::Flag("--min-node"));
}

#[test]
fn a_repository_policy_is_found_above_the_working_directory_and_enforces() {
    // /w/.nvmrc-policy.json with node.minimum 22, PWD=/w/app/src
    let Mode::Enforced(policy) = resolve_mode(&repo_ctx()).unwrap().0 else { panic!() };
    assert_eq!(policy.minimum.unwrap().source, Source::RepositoryPolicy("/w/.nvmrc-policy.json".into()));
}

#[test]
fn an_invalid_central_policy_is_a_failure_to_know() {
    let error = resolve_mode(&ctx_with_var("NVMRC_POLICY", "/bad.json")).unwrap_err(); // "{ \"schemaVersion\": 1, }"
    assert!(matches!(error, PolicyError::Invalid { .. }));
    assert_eq!(CliError::from(error).exit_code().code(), 9);
}

#[test]
fn every_source_takes_part_in_the_merge() {
    // central minimum 20, NVM_MIN_VERSION 22, min-version file 18, flag --min-node 21
    let (Mode::Enforced(policy), warnings) = resolve_mode(&all_sources_ctx()).unwrap() else { panic!() };
    assert_eq!(policy.minimum.unwrap().value.to_string(), "v22.0.0");
    assert_eq!(warnings.len(), 2); // the flag and the file are looser
}
```

- [ ] **Step 2: Run them** — expected: compile failure.

- [ ] **Step 3: Implement.** Layer order passed to `merge`: flag
  (`--min-node`), variable (`NVM_MIN_VERSION`), central policy (flag
  `--policy` value, else `NVMRC_POLICY`; `Source::CentralPolicy(location)`),
  repository policy (`find_up` from `working_directory`), min-version file
  (first line, `Source::MinVersionFile`). Empty values are unset. A flag or
  variable that is not a version is `PolicyError::Invalid` naming it. Map
  `PolicyError` to exit 9 in `CliError::exit_code`, and add 9 to the
  `exit_codes_match_the_nvm_contract` test.

- [ ] **Step 4: Run** `cargo test` — expected: pass.

- [ ] **Step 5: Commit** — `feat(policy): load central and repository
  policies and resolve the mode`

### Task 6: The enforcement gate and `install`

**Files:**

- Create: `src/commands/policy/enforce.rs`, `src/commands/policy/tests/enforce.rs`
- Modify: `src/commands/install/mod.rs:57` (call the gate),
  `src/commands/install/tests/` (new policy cases)

**Interfaces:**

- Consumes: `resolve_mode`, `policy_mode_requested`, `Mode`, `PolicyError`
  (Task 5), `NodePolicy::violations`, `ReleaseInfo` (Task 3),
  `remote_index::fetch`, `Context::is_advisory` (Task 4).
- Produces:
  - `pub enum Origin { Nvmrc(PathBuf), Argument, System }`
  - `pub struct Requested { pub version: Version, pub origin: Origin, pub
    offline: bool }`
  - `pub enum Gate { Pass, Stop(NvmExitCode) }`
  - `pub fn enforce_node(context: &Context<'_>, requested: &Requested,
    transcript: &mut Transcript) -> Result<Gate, CliError>`
- [ ] **Step 1: Write the failing tests** (`tests/enforce.rs`)

```rust
#[test]
fn legacy_mode_always_passes() {
    assert!(matches!(gate(&legacy_ctx(), "v16.0.0"), (Gate::Pass, _)));
}

#[test]
fn a_violation_stops_with_7_and_names_everything() {
    let (gate, transcript) = gate_from_nvmrc(&min_ctx("20.11.1"), "v18.19.0", "/w/app/.nvmrc");
    assert!(matches!(gate, Gate::Stop(NvmExitCode::BelowVersionFloor)));
    assert_eq!(stderr(transcript), [
        "nvm: node v18.19.0 (from /w/app/.nvmrc) is not allowed by policy:",
        "  - below the minimum v20.11.1 (from --min-node)",
        "Raise /w/app/.nvmrc to an allowed version.",
    ].join("\n"));
}

#[test]
fn an_advisory_context_warns_and_passes() {
    let (gate, transcript) = gate(&min_ctx("20.11.1").advisory(), "v18.19.0");
    assert!(matches!(gate, Gate::Pass));
    assert!(stderr(transcript).starts_with("nvm: warning: node v18.19.0 is not allowed by policy:"));
}

#[test]
fn rules_needing_release_info_fetch_the_index_once() {
    // ltsOnly policy, FakeHttp serving index_text_dated at https://nodejs.org/dist/index.tab
    let (gate, _) = gate(&lts_ctx_with_index(), "v23.1.0");
    assert!(matches!(gate, Gate::Stop(_)));
    assert_eq!(http_requests().len(), 1);
}

#[test]
fn an_unreachable_index_is_a_failure_to_know() {
    let error = enforce_node(&lts_ctx_without_index(), &requested("v20.15.1"), &mut Transcript::default()).unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
}

#[test]
fn offline_with_a_release_rule_is_a_failure_to_know() {
    let error = enforce_node(&lts_ctx_with_index(), &offline("v20.15.1"), &mut Transcript::default()).unwrap_err();
    assert_eq!(error.exit_code().code(), 9);
}
```

In `src/commands/install/tests/`:

```rust
#[test]
fn install_below_a_policy_minimum_stops_before_downloading() { /* --min-node 22 via flags, install 20 → status 7, no tarball request */ }

#[test]
fn install_in_legacy_mode_keeps_todays_floor_message() { /* NVM_MIN_VERSION=22 only → today's two lines, status 7 */ }
```

- [ ] **Step 2: Run them** — expected: compile failure.

- [ ] **Step 3: Implement** `enforce_node`: legacy → `Pass`; resolve the mode
  (print its warnings); fetch release info only when `needs_release_info()`
  (offline → `ReleaseInfoUnavailable("offline")`; no releases or version not
  listed → `ReleaseInfoUnavailable(<index url>)`); today is
  `context.clock().unix_seconds().div_euclid(86_400)`. Violations: blocking
  prints the header, one `-` line per violation, and the fix line, then
  `Stop(BelowVersionFloor)`; advisory prefixes the header with
  `nvm: warning:` and passes. A `PolicyError` blocks as `Err(CliError::Policy)`
  or, advisory, prints `nvm: warning: {error}` and passes. Origin text:
  `(from <path>)` for `.nvmrc`, `(from the command line)` for an argument,
  `(system)` for the system node; the fix line names the `.nvmrc` when there
  is one, else `Use an allowed version.` In `install`, replace
  `check_floor(...)?` with: when `policy_mode_requested`, call the gate (a
  `Stop(status)` becomes `Halt::Exit(status)`), else the existing
  `check_floor`.

- [ ] **Step 4: Run** `cargo test` — expected: pass.

- [ ] **Step 5: Commit** — `feat(policy): enforce node rules in install`

### Task 7: `use`, `exec`, `run` and the automatic paths

**Files:**

- Modify: `src/commands/use_version/mod.rs` (`run_and_target`),
  `src/commands/exec/launch.rs` (`launch`), `src/commands/auto/mod.rs`
  (advisory context)
- Test: `src/commands/use_version/tests.rs`, `src/commands/exec/tests/`,
  `src/commands/auto/tests/`, `src/commands/run/tests.rs`

**Interfaces:**

- Consumes: `enforce_node`, `Requested`, `Origin`, `Gate` (Task 6),
  `commands::resolve::system_version`, `Context::advisory` (Task 4).

- [ ] **Step 1: Write the failing tests**

```rust
// use_version/tests.rs
#[test]
fn use_of_a_denied_installed_version_stops_without_switching() {
    // installed v20.12.0, repository policy deny ["20.12.0"]
    let output = run(&ctx, &["20.12.0".into()]).unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert!(output.script.is_none());
}

#[test]
fn use_system_checks_the_system_node_version() {
    // system node prints v18.0.0, --min-node 20
    assert_eq!(run(&ctx, &["system".into()]).unwrap().status, NvmExitCode::BelowVersionFloor);
}

#[test]
fn use_without_policy_is_unchanged() { /* existing fixture, no policy → same output as before */ }

// exec/tests
#[test]
fn exec_with_a_disallowed_version_runs_nothing() {
    let output = exec::run(&ctx_min_22, &args(&["20", "node", "-v"])).unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert!(output.spawn.is_none());
}

// run/tests.rs
#[test]
fn run_goes_through_the_same_gate() { /* nvm run 20 with --min-node 22 → 7, no spawn */ }

// auto/tests
#[test]
fn auto_use_warns_but_still_switches() {
    // default alias → v20.1.0, --min-node 22
    let output = auto::run(&ctx, &["use".into()]).unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(output.script.is_some());
    assert!(output.stderr.contains("nvm: warning: node v20.1.0 is not allowed by policy:"));
}

#[test]
fn auto_use_with_an_unreadable_policy_warns_and_switches() { /* NVMRC_POLICY=/missing.json → status 0, warning */ }
```

- [ ] **Step 2: Run them** — expected: the new cases fail.

- [ ] **Step 3: Implement.** `use`: after `target::resolve` succeeds and before
  `apply`, build `Requested` from the target (`Installed` → its version,
  `Origin::Nvmrc(find_nvmrc(...))` when `from_nvmrc`, else `Argument`;
  `System` → `system_version` parsed as `Version`, `Origin::System`; no
  readable system version with policy mode on is
  `PolicyError::ReleaseInfoUnavailable("system node version")`); a `Stop`
  finishes the transcript with that status. `exec`: the same gate in `launch`
  once `nvm_version` gives `Version` or `System`, before the `Running` line.
  `run` needs no change beyond its test (it hands over to `exec`). `auto`:
  `auto_use` and `auto_install` pass `&context.clone().advisory()` to
  `use_version::run` and `install::run`.

- [ ] **Step 4: Run** `cargo test` — expected: pass, compat suite included.

- [ ] **Step 5: Commit** — `feat(policy): enforce node rules in use, exec and
  run; warn on automatic paths`

### Task 8: The shell function skips leading policy flags

**Files:**

- Modify: `src/shell/init/posix.rs`, `src/shell/init/fish.rs`,
  `src/shell/init/mod.rs` (a helper building the patterns from
  `POLICY_FLAGS`)
- Test: `src/shell/init/` unit tests, `tests/init_lab` (real shells)

**Interfaces:**

- Consumes: `POLICY_FLAGS` (Task 4).
- Produces: `pub(super) fn policy_flag_patterns() -> (String, String)` — the
  `--policy | --min-node` and `--policy=* | --min-node=*` case patterns
  (fish uses the names list).
- [ ] **Step 1: Write the failing tests**

```rust
// tests/init_lab (each shell: bash, zsh, sh, dash, ksh, fish)
#[test]
fn use_after_leading_policy_flags_switches_the_shell() {
    // eval "$(nvm init <shell>)"; nvm --min-node 18 use 20; node -v → v20.1.0, status 0
}

#[test]
fn use_after_a_joined_policy_flag_switches_the_shell() {
    // nvm --policy=/p.json use 20 (p.json allows 20) → node -v prints v20.1.0
}

#[test]
fn a_disallowed_use_leaves_the_shell_alone() {
    // nvm --min-node 22 use 20 → status 7; node -v still the previous version
}
```

- [ ] **Step 2: Run them** — expected: fail (the function routes on `$1`).

- [ ] **Step 3: Implement.** POSIX: before the `case`, find the first
  argument that is not a policy flag (skipping the value after a separate
  flag), store it in `__nvmrc_command`, route on it instead of `${1-}`, and
  unset `__nvmrc_command` and the loop variables on both branches. All
  arguments are still passed to the binary unchanged, which strips the flags
  itself. Fish: the same with a `for` loop over `$argv` and a local
  variable. Build the patterns from `POLICY_FLAGS` so later plans only extend
  the table.

- [ ] **Step 4: Run** `cargo test` (init lab included) — expected: pass in
  every shell the lab finds.

- [ ] **Step 5: Commit** — `feat(shell): route the nvm function past leading
  policy flags`

### Task 9: Documentation

**Files:**

- Modify: `README.md` (variables: `NVMRC_POLICY`; flags `--policy`,
  `--min-node`; the repository policy; exit code 9; legacy and policy modes),
  `docs/deviations.md` (the new flags, `.nvmrc-policy.json`, enforcement in
  `use`/`exec`/`run` in policy mode, exit 9, the shell function routing)

- [ ] **Step 1: Write the sections** in the README's existing style, pointing
  at `schemas/nvmrc-policy.v1.schema.json` and the spec. Every deviation
  entry names the test that pins it (from Tasks 4–8).

- [ ] **Step 2: Run** `rumdl fmt` then `rumdl check README.md docs/` —
  expected: no issues.

- [ ] **Step 3: Run the pre-commit set** — `cargo fmt --check`,
  `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`,
  `cargo deny check` — expected: all clean.

- [ ] **Step 4: Commit** — `docs: document the security policy, its flags and
  exit code 9`
