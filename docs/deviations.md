# Deviations from nvm.sh

Each item is pinned by a test; the contract scenarios that pin one name its
id.

## The contract (D1 to D17)

- D1: when stdout and stderr go to the same place (`2>&1`), nvmrc writes all
  of stderr first; nvm.sh interleaves (`Found '.nvmrc'` first). Separate
  streams are identical. (Scenarios N7, X7.)
- D2: `--help` prints clap's help, not nvm's text.
- D3: fixed in Plan 9: a bare `nvm` prints the help with status 0.
- D4: `--version` prints `nvm <nvmrc version>`, not nvm's version number.
- D5: there is no `nvm unload` (status 127); remove the init line instead.
  (S8)
- D6: an unknown command gets clap's error, not nvm's help dump; the status
  is 127 for both. (S9)
- D7: fixed in Plan 9: `nvm alias` creates `$NVM_DIR/alias/lts`; a failure to
  create it is silent (nvm.sh's `mkdir` complains and goes on).
- D8: fixed in Plan 9: `nvm ls v0.1.` lists v0.1.x.
- D9: `ls-remote` with nothing to list prints `N/A` without the star that
  nvm.sh prints after it. (I5, I11)
- D10: `install -b` of a version before v0.12 says the legacy layout is not
  supported (nvm.sh: "Binary download is not available"); status 3 for both.
- D11: `set-colors` with an invalid setting prints the message without the
  help dump. (S10)
- D12: fixed in Plan 9: the system node is the one `nvm deactivate` leaves on
  `PATH` (`$NVM_DIR/../sys` included).
- D13: a v0.x version placed under `versions/node` (nvm puts it in `$NVM_DIR`
  itself) is listed by `ls` and `alias` but not used by `use`.
- D14: there is no `nvm debug`, so exit 42 is never produced.
- D15: a command that `exec` or `run` cannot start prints
  `nvm: <cmd>: not found` (`nvm-exec: ...` for nvm-exec), not bash's `exec`
  message; status 127 for both. (X6)
- D16: fixed in Plan 9: the floor messages name the real `min-version` path.
- D17: when `$NVM_DIR` ends with `/`, nvm.sh warns on stderr that it should
  not have trailing slashes; nvmrc removes the slash and prints nothing.
  (S7)

Two more differences are visible in the scenarios without an id of their
own:

- Scenario L12: with an empty `versions/node`, `nvm ls` on nvm.sh prints
  `find: .../versions/node/*: No such file...` on stderr; nvmrc prints
  nothing. The scenario hides stderr, as nvm's own test does.
- `nvm exec` and `nvm run` wait for their command as a child process, so
  their status is passed on after it ends; only `nvm-exec` replaces itself
  with the command (on Unix).

## By area

### Listing

- `ls` shows no phantom blank row with only a system node.
- io.js and node sharing a number are both listed.
- `ls unstable` prints one `N/A` row.
- Versions are sorted by bytes.
- `ls` does not read `.nvmrc` as a pattern and does not see a system io.js.
- `unalias` does not create `alias/`.
- A bad alias name exits 1.
- Non-UTF-8 paths are shown lossy.

### Remote

- ureq replaces curl and wget; only `NVM_AUTH_HEADER` is carried.
- The index is fetched once.
- `version-remote --lts=<bad>` prints one message and `N/A`, status 3.
- Patterns are literal text, not regexes.
- Rows are sorted by version.
- A refused mirror is reported once.

### Install

- The archive type follows the version and the OS; there is no `.tar.gz`
  fallback and no external `xz`.
- There is no progress bar and no `Computing checksum` line.
- Archives are read into memory up to 1 GiB; xz uses a 256 MiB dictionary.
- A checksum mismatch fails and the archive is removed.
- There is no npm bootstrap for a node without npm.
- `--default` and `--alias` with `--lts` alias `lts/<name>`.
- `--save` writes `$PWD/.nvmrc` after a fresh install.
- No installs before v0.12, and none before v0.8.6 or without binary and
  source: status 3. A platform without source builds says so, status 1.
- `uninstall` does no permission check and removes aliases by literal match.
- Build jobs come from the CPU count; command output is shown at the end.
- Only binary installs are one atomic rename.
- Connect and header timeouts are 30 s.

### Shell

- `use` of a resolvable but missing version is status 3 (nvm.sh: 0).
- `-h` and `help` are ordinary arguments.
- `deactivate` with an unresolvable `NVM_DIR` fails with 1.
- A command that cannot be executed is 127; bash says 126.
- The function needs `nvm` on `PATH` and runs the first one there, a global
  npm package named `nvm` included.
- `install` activates only through the function.
- Only bash, zsh, sh, dash, ksh and fish are supported.
- `--delete-prefix` without npm warns.
- `hash -r` is always emitted.
- Non-UTF-8 arguments are replaced lossily.
- The function needs `/dev/fd`.

### Security policy

nvmrc additions with no nvm.sh counterpart; without a policy, a policy flag
or `NVMRC_POLICY`, nothing changes (pinned by
`legacy_mode_without_any_new_configuration` in
`src/commands/policy/tests/mode.rs`).

- `--policy` and `--min-node` are taken before the subcommand or anywhere
  after it up to a `--`. After `exec` and `run` they are taken only right
  after the subcommand; a later occurrence there belongs to the program and
  is left alone. An empty or blank value is `nvm: <flag> needs a value.`,
  status 127.
  (`flags_before_the_subcommand_are_taken`,
  `flags_anywhere_after_another_subcommand_are_taken`,
  `flags_after_a_double_dash_are_left_alone`,
  `flags_after_the_first_argument_of_exec_or_run_are_left_alone`,
  `an_empty_or_blank_value_is_an_error` in
  `src/domain/policy/flags_tests.rs`; `a_flag_error_is_a_usage_error`,
  `a_policy_flag_after_the_version_of_use_is_enforced`,
  `a_floor_flag_after_the_version_of_install_is_enforced`,
  `a_policy_flag_after_the_program_of_exec_reaches_the_program` in
  `src/cli/tests/policy.rs`)
- `--policy` replaces `NVMRC_POLICY`; the two are never merged.
  (`a_policy_flag_wins_over_the_policy_variable`)
- An unreadable system node version under a policy is
  `nvm: cannot check the policy: the version of the system node cannot be
  read.`, status 9.
  (`exec_system_with_an_unreadable_version_says_so_and_fails_to_know`)
- `.nvmrc-policy.json` is found upward from the working directory.
  (`a_repository_policy_is_found_above_the_working_directory_and_enforces`)
- In policy mode `install`, `use`, `exec` and `run` check the node rules;
  `__auto` warns and goes on.
  (`install_below_a_policy_minimum_stops_before_downloading`,
  `use_of_a_disallowed_installed_version_stops_without_switching`,
  `exec_with_a_disallowed_version_runs_nothing`,
  `run_goes_through_the_same_policy_gate`,
  `auto_install_of_a_disallowed_version_warns_and_goes_on`,
  `auto_use_with_an_unreadable_policy_warns_and_switches`)
- The violation message names the version, its origin and each rule with its
  source. (`a_violation_stops_with_7_and_names_everything`,
  `run_from_an_nvmrc_names_the_file_in_the_policy_message`)
- Sources only tighten; a looser one warns.
  (`the_highest_minimum_wins_and_names_its_source`,
  `a_lower_override_loses_with_a_warning`)
- Exit 9 means the policy could not be checked, and exit 7 also covers a
  version the policy does not allow.
  (`an_invalid_central_policy_is_a_failure_to_know`,
  `an_unreachable_index_is_a_failure_to_know`,
  `offline_with_a_release_rule_is_a_failure_to_know`,
  `a_min_version_file_that_cannot_be_read_is_an_error`)
- `nvm-exec` keeps the status of a policy refusal (7, or 9 when the policy
  cannot be checked); its other failures to select stay 127.
  (`a_policy_refusal_keeps_its_own_status` in
  `src/commands/nvm_exec/tests.rs`)
- `use system` is checked only in policy mode.
  (`use_system_without_a_policy_never_asks_the_system_node`)
- `http://` policy locations are rejected, and so is a redirect from an
  `https://` location to http. (`an_http_location_is_rejected`,
  `a_policy_fetch_refuses_a_redirect_away_from_https`,
  `a_secure_fetch_never_contacts_a_url_that_is_not_https`)
- On the automatic paths the policy and the index are fetched once, with a
  5-second bound, instead of three tries of up to 30 seconds.
  (`an_advisory_policy_fetch_goes_through_the_quick_client_once`,
  `an_advisory_index_fetch_goes_through_the_quick_client`)
- The shell function skips leading policy flags to find the subcommand.
  (`every_function_routes_on_the_first_argument_that_is_not_a_policy_flag`,
  `use_after_leading_policy_flags_switches_the_shell` in
  `tests/init_policy_cli.rs`)
- The CVE check reads the Node.js security-wg index: affected means in
  `vulnerable` and not in `patched`, filtered by `affectedEnvironments`;
  a missing or `unknown` severity is `high`, any other unknown value is a
  failure to know; an advisory without CVE ids is `advisory <id>`; io.js is
  skipped. (`a_vulnerable_unpatched_version_is_affected`,
  `a_patched_version_is_not_affected`,
  `a_platform_specific_advisory_counts_only_on_its_platform`,
  `an_advisory_without_cve_ids_is_named_by_id`,
  `unknown_and_missing_severities_count_as_high`,
  `anything_else_is_a_failure_to_know` in
  `src/domain/policy/advisories_tests.rs`;
  `iojs_versions_skip_the_cve_check` in
  `src/commands/policy/tests/advisories.rs`)
- `--fail-on-severity`, `NVMRC_FAIL_ON_SEVERITY` and `cve.failOnSeverity`
  tighten each other (default `high`, `none` the loosest, a looser one
  warns); the variable turns policy mode on.
  (`without_any_setting_the_limit_is_high_and_the_index_is_the_default`,
  `the_strictest_limit_wins_and_a_looser_override_warns`,
  `none_is_the_loosest_limit` in `src/domain/policy/cve_tests.rs`;
  `the_severity_flag_and_variable_turn_policy_mode_on`,
  `the_severity_flag_can_only_tighten` in
  `src/commands/policy/tests/mode.rs`)
- The index is the central policy's `cve.source`, else the default even when
  the repository names one; the repository's counts only without a central
  policy. A relative source is read next to its policy file and is invalid in
  an https policy. (`the_central_index_wins_and_the_repository_index_counts_only_alone`
  in `src/domain/policy/cve_tests.rs`;
  `a_relative_index_location_is_read_next_to_its_policy`,
  `a_relative_index_in_a_central_file_is_read_next_to_it`,
  `a_relative_index_in_a_remote_policy_is_invalid` in
  `src/commands/policy/tests/mode.rs`)
- A finding at or above the limit is status 7; below it, a warning only (all
  findings with `none`). (`a_finding_at_the_limit_stops_with_7`,
  `findings_below_the_limit_only_warn`, `limit_none_reports_and_passes`,
  `a_patched_version_passes_quietly`,
  `findings_below_the_limit_are_also_named_when_the_gate_stops` in
  `src/commands/policy/tests/advisories.rs`)
- An unreadable or invalid index is status 9, or a warning when node rules
  already stop with 7. `--offline` with an https index is 9 before any
  request; a local index works offline. Automatic paths warn and pass;
  legacy mode never fetches. (`an_unreachable_index_is_a_failure_to_know`,
  `an_invalid_index_names_its_location`,
  `offline_with_a_remote_index_is_a_failure_to_know`,
  `offline_with_a_local_index_is_checked`,
  `an_automatic_path_warns_on_a_finding_and_passes`,
  `legacy_mode_never_fetches_the_index` in
  `src/commands/policy/tests/advisories.rs`;
  `a_node_violation_still_stops_with_7_when_the_index_is_unreachable`,
  `a_node_violation_still_stops_with_7_offline_with_a_remote_index` in
  `src/commands/policy/tests/unknown_index.rs`)
- `use --save` in policy mode writes `.nvmrc` only after the policy allows
  the version; the legacy order is unchanged.
  (`a_refused_use_save_leaves_nvmrc_untouched`,
  `an_allowed_use_save_still_writes_nvmrc` in
  `src/commands/use_version/tests.rs`)

### npm, yarn and pnpm

nvmrc additions; nvm.sh ignores `npm=`, `yarn=` and `pnpm=` lines in
`.nvmrc`.

- The lines change what `install`, `use`, `exec`, `run` and `deactivate` do:
  `install` fetches the pinned tools; `use`, `exec` and `run` put them on
  `PATH` and drop other projects' entries; `deactivate` removes them.
- A pin error, a `packageManager` in the neighbouring `package.json` that
  disagrees with the pin, or a pinned tool that is not installed exits 3 in
  an explicit `use`, `exec` or `run`; the automatic paths only warn.
- The tools live in `$NVM_DIR/package-managers/<tool>/<version>/`, shared by
  all node versions, and their `bin` directories lead `PATH`.
- `NVM_AUTH_HEADER` is not sent to the npm registry, tarball hosts or the
  built-in vulnerability index; `NVMRC_NPM_AUTH_HEADER` goes to the registry
  host only.
- `install --save` and `use --save` (`-w`) replace only the bare version
  line of `.nvmrc` and keep every other line (pins, other keys, comments);
  nvm.sh rewrites the whole file. A file with nothing else is written as
  `<version>` alone, as nvm.sh does, and an `.nvmrc` that cannot be read is
  left alone (status 3). (`saving_keeps_the_pins_and_the_other_lines`,
  `saving_over_a_lone_version_writes_just_the_version`,
  `an_unreadable_nvmrc_is_left_alone` in `src/commands/nvmrc_file.rs`)
- An `.nvmrc` that cannot be read gives
  `nvm: warning: <path> cannot be read: <reason>` and no pins; the bundled
  npm is then checked when an npm rule exists.
  (`an_unreadable_nvmrc_is_a_warning_and_no_pins` in
  `src/commands/project_pins.rs`)

### Colors

- An invalid `NVM_COLORS` letter warns once; an unknown `TERM` is silent.
- There is no color legend in `--help`.
- `NVM_NO_COLORS` does not leak.
- Invalid `.nvmrc` lines and alias targets are printed raw, control bytes
  included.
- `NVM_COLORS` is read by character.
- The install `--alias`/`--default` row is never colored.
- `set-colors x --help` sets the colors.

### Conflicts

- There is no ripgrep and no `LineScanner`.
- The runtime check is in the init snippet, so a late `source nvm.sh` is only
  found by `doctor`.
- Lazy-loader files are never edited.
- `OmzPlugin` and `HelperCall` are hints.
- `source` must be in command position; a `zsh-defer` or `time` before it
  counts; `eval "$(cat nvm.sh)"` is not seen.
- Here-documents are not tracked.
- Relative source paths resolve from the sourcing file's directory.
- Only loader and completion lines are migrated; compound lines are manual,
  and so is a line inside a block or continued.
- An unterminated BEGIN marker is not a block.
- The load line calls `nvmrc`.
- The block's shell comes from the file name.
- The diff is part of the prompt.
- Backups keep the file's permissions but not its owner, extended attributes
  or ACLs, and a read-only profile is replaced.
- `--undo` backs up first, refuses an empty backup and prefers backups not
  dated in the future.
- An unreadable file is exit 1.
- A file the syntax checker already refuses is left alone; a missing checker
  writes with a warning.
- fish is verified in the Docker image and in CI (installed from apt).
- `doctor`, `migrate` and `__conflict` are additions.

## Platforms

v1 runs on Linux and macOS (x86_64 and arm64). Windows is not supported:
the shell channel is an inherited file descriptor, `nvm-exec` replaces
itself with `exec`, and installs set Unix permissions, so building for
Windows stops with a compile error naming this file. Not handled:

- AIX gets `.tar.xz` (nodejs.org publishes `.tar.gz` there).
- armv6 and Rosetta are not detected as nvm.sh does.
- An install lock left by Ctrl-C waits for its stale timeout.
- `.nvmrc` is searched from `$PWD` as the shell gives it.
