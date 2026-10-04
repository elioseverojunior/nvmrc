# nvmrc Plan 7: Colors and Terminal Detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `nvm ls`, `nvm ls-remote` and `nvm alias` show colors exactly
where and as `nvm.sh` does: the five color roles of `NVM_COLORS`, the derived
colors (LTS, latest LTS, annotations), the italics, the `--no-colors` flag, the
detection of a terminal that can show them, and `nvm set-colors`.

**Architecture:** Same layering as Plans 1 to 6. `ports/` gains `Terminal`
(is stdout a terminal), with `StdTerminal`, `NoTerminal` and a test-only fake;
`domain/colors` holds the palette (letters, roles, derived colors) as pure
code; `commands/color_policy` decides, once per command, whether colors and
italics are on (`ColorPolicy`), asking `tput` through the existing `Process`
port as `nvm.sh` does (no terminfo parser, no new crate); the renderers are pure
functions that take the palette and a flag (`domain/listing/colored`,
`domain/alias_format/colored`, `domain/remote_format`), so every colored byte is
unit-tested without a pseudo-terminal. `set-colors` prints shell code through
the channel of Plan 6, so that the variable `NVM_COLORS` stays in the shell.

**Tech Stack:** Rust 2024 edition (MSRV 1.85), the dependencies of Plan 5. No
new crate.

**Spec:** `docs/superpowers/specs/2026-10-02-nvmrc-design.md` (sections 3 and
11, the colors that Plan 5 deferred). This plan starts from the final state of
`2026-10-03-nvmrc-plan-6-shell-integration.md` (the `main` branch at
`324c09f`).

**Reference behaviour.** Every byte below was read from `nvm.sh` and run
against it under a pseudo-terminal and under pipes (bash 5, zsh): the oracle is
the real script with a fixture of three node versions, one io.js, aliases,
LTS aliases and a system node. At the end of the plan the whole command set was
compared with `nvm.sh` in 3136 cases (4 versions in use, 15 environments, a
terminal or a pipe, 28 commands) plus 192 for the invalid `.nvmrc` message: all
the differences are explained by the deviations listed at the end.

**Revised roadmap:** Plan 8 `doctor` and `migrate`; Plan 9 compatibility
contract and CI.

**Not in this plan, on purpose:** the color legend of `nvm --help` (the help
is clap's), `nvm unload`, a terminfo parser, and any color in `current`,
`version`, `which`, `use`, `install`, `uninstall`, `unalias`, `deactivate`,
errors and warnings (`nvm.sh` colors none of them).

## Global Constraints

- Rust edition 2024, `rust-version = "1.85.0"`; no API newer than 1.85. Run
  cargo through the rustup proxies. `.cargo/config.toml` sets `-D warnings`. No
  new dependency and no change to `Cargo.toml`.
- Files under 300 lines (tests included), functions under 30 lines, cyclomatic
  complexity under 10, meaningful names without abbreviations. A module with
  tests is `foo/mod.rs` plus `foo/tests.rs`; a file that would pass 300 lines is
  split into a directory. `src/ports/mod.rs` is at 299 lines after Task 2: a
  task that adds to it must split it first.
- Errors use `thiserror` in the library. Exit codes do not change in this plan
  (`ls`, `ls-remote` and `alias` keep 0, 3 for no match, 55 for an unsupported
  option); `set-colors` is always 0, as in `nvm.sh`.
- Colors are on iff stdout is a terminal AND `tput -T "${TERM:-vt100}" colors`
  prints 8 or more AND the command was not given `--no-colors`. stderr is never
  consulted; `NO_COLOR` and an `NVM_NO_COLORS` in the environment are ignored by
  `ls`, `ls-remote` and `alias` (only the flag counts); `NVM_NO_COLORS` set to
  exactly `--no-colors` is honoured by `set-colors` only; an exported
  `NVM_HAS_COLORS=1` forces colors in `nvm alias <name> <target>` and in the
  invalid-`.nvmrc` message only.
- Every colored span is `\x1b[<code>` text `\x1b[0m` (a full reset, never
  nested), except the italic span `\x1b[3m` .. `\x1b[23m` inside an annotation.
  The arrow is always `\x1b[0;90m->\x1b[0m`.
- With colors off the output is byte-identical to Plan 3 to 6 (the `*` markers,
  `->` arrows and aligned columns): no task may change it.
- Commits: Conventional Commits, GPG-signed with `git commit -S`, and no AI
  attribution or `Co-Authored-By` trailer in the message.
- Tests never touch the real `~/.nvm` or the user's dotfiles; nothing needs a
  pseudo-terminal (the terminal is a port with a fake). Real-shell tests skip a
  shell that is not installed. Do not keep test scripts or test-output
  directories in the repository.

Each task below lists its steps in TDD order. Where a file already exists, the
code is shown as a unified diff against the previous task's final state; where
a file is new, the full code is shown. Apply diffs by hand (hunk headers are
informative, line numbers may drift), then run `cargo fmt`. A new module with
tests is created first as its `mod.rs` holding only the `#[cfg(test)] mod
tests;` declaration and its `tests.rs`; the implementation is then inserted
ABOVE that declaration, which must stay in the file.

---

### Task 1: The color palette

**Files:**

- Create: `src/domain/colors/mod.rs`, `src/domain/colors/tests.rs`
- Modify: `src/domain/mod.rs`

**Interfaces:**

- Produces: `domain::colors::{RESET, ARROW, ITALIC_ON, ITALIC_OFF, sgr, Role,
  Palette, wrap, is_valid_setting}`. `sgr(char) -> Option<&'static str>` is the
  letter table of `nvm_print_color_code` (`r` is `0;31m`, `R` `1;31m`, then g,
  b, c, m, y, k in the same way, `e` `0;37m`, `W` `1;37m`; `0`, which means "no
  color" in `nvm.sh`, and unknown letters are `None`). `Role` is `Installed`,
  `System`, `Current`, `NotInstalled`, `Default` (positions 1 to 5 of
  `NVM_COLORS`). `Palette::from_setting(Option<&str>) -> (Palette,
  Vec<Option<char>>)` (unset or empty is `bygre`; extra characters are ignored;
  a missing or invalid position has no color and is returned for the caller's
  single warning) with `code(role)`, `lts()`, `latest_lts()`, `annotation()` and
  `old_lts()`; `wrap(code, text)`; `is_valid_setting(&str)`.
- Behaviour (digest sections 2.1 to 2.4): the LTS color is the system color with
  EVERY `0` turned into `1` (nvm.sh's `tr '0;' '1;'`, so `k` becomes `1;31m`,
  kept on purpose); the latest-LTS color is the current color with the FIRST
  `0;` turned into `1;`; the annotation color is the current color with a
  LEADING `1;` turned into `0;`; the old-LTS color is the default role.
  `set-colors` accepts exactly five characters of `rRgGbBcCyYmMkKeW` (`0` is not
  one).
- [ ] **Step 1: Declare the new modules**

Apply to `src/domain/mod.rs`:

```diff
--- a/src/domain/mod.rs
+++ b/src/domain/mod.rs
@@ -1,6 +1,7 @@
 pub mod alias;
 pub mod alias_format;
 pub mod checksum;
+pub mod colors;
 pub mod compression;
 pub mod current;
 #[cfg(test)]
```

- [ ] **Step 2: Write the failing tests**

Create `src/domain/colors/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;

/// `\e[0m`, the reset that closes every colored span.
pub const RESET: &str = "\x1b[0m";
/// `\e[0;90m`, the fixed arrow color of the alias rows.
pub const ARROW: &str = "\x1b[0;90m";
/// `\e[3m`, italics on (ls-remote annotations).
pub const ITALIC_ON: &str = "\x1b[3m";
/// `\e[23m`, italics off.
pub const ITALIC_OFF: &str = "\x1b[23m";

/// The letters accepted by `nvm_set_colors`, in the order of its regex.
const SETTABLE_LETTERS: &str = "rRgGbBcCyYmMkKeW";
/// The setting used when `NVM_COLORS` is unset or empty.
const DEFAULT_SETTING: &str = "bygre";

/// Mirrors `nvm_print_color_code`: the SGR code of a letter, without the
/// escape prefix but with the trailing `m` (`r` gives `0;31m`).
///
/// `0` (the undocumented "no color") and every unknown letter give `None`.
#[must_use]
pub fn sgr(letter: char) -> Option<&'static str> {
    match letter {
        'r' => Some("0;31m"),
        'R' => Some("1;31m"),
        'g' => Some("0;32m"),
        'G' => Some("1;32m"),
        'b' => Some("0;34m"),
        'B' => Some("1;34m"),
        'c' => Some("0;36m"),
        'C' => Some("1;36m"),
        'm' => Some("0;35m"),
        'M' => Some("1;35m"),
        'y' => Some("0;33m"),
        'Y' => Some("1;33m"),
        'k' => Some("0;30m"),
        'K' => Some("1;30m"),
        'e' => Some("0;37m"),
        'W' => Some("1;37m"),
        _ => None,
    }
}

/// The five roles of `NVM_COLORS`, in the order of its positions 1 to 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Position 1: installed versions.
    Installed,
    /// Position 2: the system node (and, derived, the LTS aliases).
    System,
    /// Position 3: the current version.
    Current,
    /// Position 4: not installed (`N/A`, infinite loops).
    NotInstalled,
    /// Position 5: the `(default)` marker and old LTS annotations.
    Default,
}

impl Role {
    const fn index(self) -> usize {
        match self {
            Self::Installed => 0,
            Self::System => 1,
            Self::Current => 2,
            Self::NotInstalled => 3,
            Self::Default => 4,
        }
    }
}

/// The resolved colors of the five roles. A role that is missing or invalid
/// in the setting has no color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    codes: [Option<&'static str>; 5],
}

impl Palette {
    /// Mirrors `nvm_get_colors` over `${NVM_COLORS:-bygre}`: unset or empty
    /// means `bygre`, extra characters are ignored.
    ///
    /// Quirk fixed on purpose: nvm.sh warns on every lookup and splices
    /// malformed escapes; here an invalid or missing role is just uncolored
    /// and reported once in the second value, for the caller's single
    /// warning: `Some(letter)` for an invalid letter, `None` for a missing
    /// position (nvm.sh prints `Invalid color code: ` with nothing after it).
    /// The letter `0` means "no color" and is not reported. Positions are
    /// counted in characters.
    #[must_use]
    pub fn from_setting(setting: Option<&str>) -> (Self, Vec<Option<char>>) {
        let text = setting
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_SETTING);
        let mut letters = text.chars();
        let mut codes = [None; 5];
        let mut offending = Vec::new();
        for code in &mut codes {
            let letter = letters.next();
            *code = letter.and_then(sgr);
            if code.is_none() && letter != Some('0') {
                offending.push(letter);
            }
        }
        (Self { codes }, offending)
    }

    /// The code of a role (`nvm_get_colors` 1 to 5), `None` when it has none.
    #[must_use]
    pub fn code(&self, role: Role) -> Option<&str> {
        self.codes[role.index()]
    }

    /// The LTS color (`nvm_get_colors 6`): the system color piped through
    /// `tr '0;' '1;'`, so EVERY `0` becomes `1` (quirk kept: `k` gives
    /// `1;31m`).
    #[must_use]
    pub fn lts(&self) -> Option<String> {
        self.code(Role::System).map(|code| code.replace('0', "1"))
    }

    /// The ls-remote "Latest LTS" color: the current color with the FIRST
    /// `0;` turned into `1;` (awk `sub(/0;/,"1;")`).
    #[must_use]
    pub fn latest_lts(&self) -> Option<String> {
        self.code(Role::Current)
            .map(|code| code.replacen("0;", "1;", 1))
    }

    /// The ls-remote alias/latest annotation color: the current color with a
    /// LEADING `1;` turned into `0;`.
    #[must_use]
    pub fn annotation(&self) -> Option<String> {
        self.code(Role::Current)
            .map(|code| match code.strip_prefix("1;") {
                Some(rest) => format!("0;{rest}"),
                None => code.to_owned(),
            })
    }

    /// The ls-remote old `(LTS: x)` color: the default role.
    #[must_use]
    pub fn old_lts(&self) -> Option<&str> {
        self.code(Role::Default)
    }
}

/// Mirrors `nvm_wrap_with_color_code` once colors are known to be on:
/// `\e[<code><text>\e[0m`, or the plain text when the code is `None`.
#[must_use]
pub fn wrap(code: Option<&str>, text: &str) -> String {
    match code {
        Some(code) => format!("\x1b[{code}{text}{RESET}"),
        None => text.to_owned(),
    }
}

/// Mirrors the check of `nvm_set_colors`: exactly five characters, each one
/// of `rRgGbBcCyYmMkKeW` (`0` is not accepted here).
#[must_use]
pub fn is_valid_setting(setting: &str) -> bool {
    setting.chars().count() == 5
        && setting
            .chars()
            .all(|letter| SETTABLE_LETTERS.contains(letter))
}
```

Create `src/domain/colors/tests.rs`:

```rust
use super::*;

const TABLE: [(char, &str); 16] = [
    ('r', "0;31m"),
    ('R', "1;31m"),
    ('g', "0;32m"),
    ('G', "1;32m"),
    ('b', "0;34m"),
    ('B', "1;34m"),
    ('c', "0;36m"),
    ('C', "1;36m"),
    ('m', "0;35m"),
    ('M', "1;35m"),
    ('y', "0;33m"),
    ('Y', "1;33m"),
    ('k', "0;30m"),
    ('K', "1;30m"),
    ('e', "0;37m"),
    ('W', "1;37m"),
];

fn roles(palette: &Palette) -> Vec<Option<&str>> {
    [
        Role::Installed,
        Role::System,
        Role::Current,
        Role::NotInstalled,
        Role::Default,
    ]
    .into_iter()
    .map(|role| palette.code(role))
    .collect()
}

#[test]
fn every_letter_of_the_table_has_its_code() {
    for (letter, code) in TABLE {
        assert_eq!(sgr(letter), Some(code), "letter {letter}");
    }
}

#[test]
fn zero_and_unknown_letters_have_no_code() {
    for letter in ['0', 'x', 'E', 'w', ' ', 'z'] {
        assert_eq!(sgr(letter), None, "letter {letter}");
    }
}

#[test]
fn constants_are_the_exact_byte_sequences() {
    assert_eq!(RESET, "\x1b[0m");
    assert_eq!(ARROW, "\x1b[0;90m");
    assert_eq!(ITALIC_ON, "\x1b[3m");
    assert_eq!(ITALIC_OFF, "\x1b[23m");
}

#[test]
fn unset_and_empty_settings_use_bygre() {
    for setting in [None, Some("")] {
        let (palette, offending) = Palette::from_setting(setting);
        assert!(offending.is_empty());
        assert_eq!(
            roles(&palette),
            [
                Some("0;34m"),
                Some("0;33m"),
                Some("0;32m"),
                Some("0;31m"),
                Some("0;37m")
            ]
        );
    }
}

#[test]
fn custom_settings_map_by_position() {
    let (palette, offending) = Palette::from_setting(Some("rgbcm"));
    assert!(offending.is_empty());
    assert_eq!(
        roles(&palette),
        [
            Some("0;31m"),
            Some("0;32m"),
            Some("0;34m"),
            Some("0;36m"),
            Some("0;35m")
        ]
    );
    let (bold, _) = Palette::from_setting(Some("RGBCM"));
    assert_eq!(bold.code(Role::Installed), Some("1;31m"));
    assert_eq!(bold.code(Role::Default), Some("1;35m"));
}

#[test]
fn extra_characters_are_ignored() {
    let (palette, offending) = Palette::from_setting(Some("bygreXX"));
    assert!(offending.is_empty());
    assert_eq!(palette, Palette::from_setting(None).0);
}

#[test]
fn a_short_setting_leaves_missing_roles_without_color() {
    let (palette, offending) = Palette::from_setting(Some("rg"));
    assert_eq!(
        roles(&palette),
        [Some("0;31m"), Some("0;32m"), None, None, None]
    );
    assert_eq!(offending, [None, None, None]);
}

#[test]
fn invalid_letters_are_reported_and_uncolored() {
    let (palette, offending) = Palette::from_setting(Some("zzzzz"));
    assert_eq!(roles(&palette), [None; 5]);
    assert_eq!(offending, [Some('z'); 5]);
    assert_eq!(palette.lts(), None);
    assert_eq!(palette.latest_lts(), None);
    assert_eq!(palette.annotation(), None);
    assert_eq!(palette.old_lts(), None);
}

#[test]
fn zero_means_no_color_without_a_warning() {
    let (palette, offending) = Palette::from_setting(Some("b0gre"));
    assert_eq!(palette.code(Role::System), None);
    assert!(offending.is_empty());
}

#[test]
fn default_palette_derives_the_documented_colors() {
    let (palette, _) = Palette::from_setting(None);
    assert_eq!(palette.lts().as_deref(), Some("1;33m"));
    assert_eq!(palette.latest_lts().as_deref(), Some("1;32m"));
    assert_eq!(palette.annotation().as_deref(), Some("0;32m"));
    assert_eq!(palette.old_lts(), Some("0;37m"));
}

#[test]
fn black_lts_turns_bold_red_but_latest_lts_stays_black() {
    let (palette, _) = Palette::from_setting(Some("kKkKk"));
    assert_eq!(palette.lts().as_deref(), Some("1;31m"));
    assert_eq!(palette.latest_lts().as_deref(), Some("1;30m"));
    let (bold_black, _) = Palette::from_setting(Some("bKbbb"));
    assert_eq!(bold_black.lts().as_deref(), Some("1;31m"));
}

#[test]
fn bold_current_keeps_latest_lts_and_unbolds_the_annotation() {
    let (palette, _) = Palette::from_setting(Some("byGre"));
    assert_eq!(palette.latest_lts().as_deref(), Some("1;32m"));
    assert_eq!(palette.annotation().as_deref(), Some("0;32m"));
}

#[test]
fn wrap_surrounds_the_text_with_the_code_and_a_reset() {
    assert_eq!(wrap(Some("0;32m"), "v18"), "\x1b[0;32mv18\x1b[0m");
    assert_eq!(wrap(None, "v18"), "v18");
}

#[test]
fn set_colors_validation_accepts_exactly_five_settable_letters() {
    for setting in ["bygre", "rRgGb", "BcCyY", "mMkKe", "WWWWW", "rgbcm"] {
        assert!(is_valid_setting(setting), "{setting}");
    }
}

#[test]
fn set_colors_validation_rejects_zero_bad_letters_and_wrong_lengths() {
    for setting in [
        "b0gre", "00000", "bygrx", "byg", "bygrey", "", "bygr ", "bygrE",
    ] {
        assert!(!is_valid_setting(setting), "{setting:?}");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find module `colors` in
`domain`", "cannot find function `sgr`" and "cannot find struct `Palette`".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/domain/colors/mod.rs`:

```rust
//! The color palette of nvm.sh: letter table, the five `NVM_COLORS` roles and
//! the colors derived from them. Pure: no terminal detection happens here.

```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1029 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(colors): the color palette of nvm.sh"
```

---

### Task 2: The terminal port and the color policy

**Files:**

- Create: `src/adapters/no_terminal.rs`, `src/adapters/std_terminal.rs`,
  `src/commands/color_policy/mod.rs`, `src/commands/color_policy/tests.rs`,
  `src/fakes/terminal.rs`
- Modify: `src/adapters/mod.rs`, `src/cli/mod.rs`, `src/commands/mod.rs`,
  `src/context/mod.rs`, `src/fakes/mod.rs`, `src/fakes/process.rs`,
  `src/ports/mod.rs`

**Interfaces:**

- Produces: `ports::Terminal::stdout_is_terminal()`; `StdTerminal`
  (`std::io::IsTerminal`), `NoTerminal` (always false), test-only
  `FakeTerminal::{terminal, pipe}`; `Context::{with_terminal, terminal}`
  (default `NoTerminal`; the real context of `nvm` and `nvm-exec` uses
  `StdTerminal`); `FakeProcess::with_run(program, args, success, stdout)` (the
  fake `run` could not tell `tput -T x colors` from `tput -T x sitm`);
  `commands::color_policy::{ColorPolicy { enabled, italics }, detect,
  forced, detect_or_forced}`.
- Behaviour (`nvm_has_colors`, `nvm_has_italics`, digest section 1):
  `detect(context, no_colors_flag)` is off when the flag is given or stdout is
  not a terminal; an unset or empty `TERM` is `vt100`; `tput` is found on the
  `PATH` and run through `Process::run` with `-T <term> colors`; its output is
  read like `[ "${x:--1}" -ge 8 ]` (whitespace is accepted, empty, text and
  values below 8 are off, a missing or failing `tput` is off); italics need
  colors on and `tput -T <term> sitm` to succeed. `forced` is true iff
  `NVM_HAS_COLORS` is exactly `1`; `detect_or_forced` forces `enabled` but not
  `italics`. `NVM_NO_COLORS` and `NO_COLOR` change nothing in `detect`.
- [ ] **Step 1: Declare the new modules**

Apply to `src/adapters/mod.rs`:

```diff
--- a/src/adapters/mod.rs
+++ b/src/adapters/mod.rs
@@ -6,6 +6,7 @@
 pub mod no_http;
 pub mod no_process;
 pub mod no_sleeper;
+pub mod no_terminal;
 pub mod retrying_http;
 pub mod sha256_digest;
 pub mod std_cpu;
@@ -14,5 +15,6 @@
 pub mod std_process;
 pub mod std_sleeper;
 mod std_spawn;
+pub mod std_terminal;
 pub mod tar_archive;
 pub mod ureq_http;
```

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -2,6 +2,7 @@
 pub mod aliases;
 pub mod auto;
 pub mod cache;
+pub mod color_policy;
 pub mod current;
 pub mod deactivate;
 pub mod exec;
```

- [ ] **Step 2: Write the failing tests**

Create `src/adapters/no_terminal.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_never_a_terminal() {
        assert!(!NoTerminal.stdout_is_terminal());
    }
}
```

Create `src/adapters/std_terminal.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_without_panicking() {
        // Under `cargo test` stdout is captured, so the answer depends on the
        // harness; only the call itself is checked.
        let _answer = StdTerminal.stdout_is_terminal();
    }
}
```

Create `src/commands/color_policy/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;

/// Colors are shown from this many terminal colors up (`-ge 8`).
const MINIMUM_COLORS: i64 = 8;
/// What nvm.sh asks terminfo about when `TERM` is unset or empty.
const FALLBACK_TERM: &str = "vt100";

/// What a command may print: SGR colors, and italics on top of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorPolicy {
    pub enabled: bool,
    pub italics: bool,
}

impl ColorPolicy {
    #[must_use]
    pub fn off() -> Self {
        Self {
            enabled: false,
            italics: false,
        }
    }
}

/// `tput -T <term> <capability>` through the `Process` port; `None` when
/// `tput` is not on `PATH` (`nvm_has tput`) or cannot be started.
fn tput(
    context: &Context<'_>,
    term: &str,
    capability: &str,
) -> Option<crate::ports::ProcessOutput> {
    let path_variable = context.env.var_os("PATH").unwrap_or_default();
    let program = find_in_path(context.fs, &path_variable, "tput")?;
    run_tput(context, &program, &["-T", term, capability])
}

fn run_tput(
    context: &Context<'_>,
    program: &Path,
    args: &[&str],
) -> Option<crate::ports::ProcessOutput> {
    context.process().run(program, args).ok()
}

/// The `[ "${NVM_NUM_COLORS:--1}" -ge 8 ]` of nvm.sh: blank, failing or
/// non-integer output counts as -1. Like `[`, surrounding whitespace and a
/// sign are accepted.
fn has_enough_colors(output: Option<crate::ports::ProcessOutput>) -> bool {
    output
        .filter(|output| output.success)
        .and_then(|output| output.stdout.trim().parse::<i64>().ok())
        .is_some_and(|colors| colors >= MINIMUM_COLORS)
}

/// `nvm_has_colors` and `nvm_has_italics`. `no_colors_flag` is whether the
/// command was given `--no-colors`; `NVM_NO_COLORS` and `NO_COLOR` in the
/// environment are ignored, as the commands of nvm.sh shadow the first and
/// nothing reads the second.
#[must_use]
pub fn detect(context: &Context<'_>, no_colors_flag: bool) -> ColorPolicy {
    if no_colors_flag || !context.terminal().stdout_is_terminal() {
        return ColorPolicy::off();
    }
    let term = context
        .env
        .var("TERM")
        .filter(|term| !term.is_empty())
        .unwrap_or_else(|| FALLBACK_TERM.to_owned());
    if !has_enough_colors(tput(context, &term, "colors")) {
        return ColorPolicy::off();
    }
    let italics = tput(context, &term, "sitm").is_some_and(|output| output.success);
    ColorPolicy {
        enabled: true,
        italics,
    }
}

/// The exported `NVM_HAS_COLORS` is exactly `1`: the fork's override that
/// beats a pipe, `TERM=dumb` and `--no-colors`. Any other value is ignored.
#[must_use]
pub fn forced(context: &Context<'_>) -> bool {
    context.env.var("NVM_HAS_COLORS").as_deref() == Some("1")
}

/// [`detect`], except that [`forced`] turns the colors on. Italics stay what
/// detection says: `nvm_has_italics` runs the real `nvm_has_colors`, which the
/// override does not reach.
#[must_use]
pub fn detect_or_forced(context: &Context<'_>, no_colors_flag: bool) -> ColorPolicy {
    let detected = detect(context, no_colors_flag);
    ColorPolicy {
        enabled: detected.enabled || forced(context),
        italics: detected.italics,
    }
}
```

Create `src/commands/color_policy/tests.rs`:

```rust
use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess, FakeTerminal};

const TPUT: &str = "/usr/bin/tput";

fn tput_process(term: &str, colors: &str) -> FakeProcess {
    FakeProcess::default().with_run(TPUT, &format!("-T {term} colors"), true, colors)
}

fn italic_process(term: &str, colors: &str) -> FakeProcess {
    tput_process(term, colors).with_run(TPUT, &format!("-T {term} sitm"), true, "\x1b[3m")
}

fn policy(
    env: &FakeEnv,
    terminal: &FakeTerminal,
    process: &FakeProcess,
    flag: bool,
    has_tput: bool,
) -> ColorPolicy {
    let fs = if has_tput {
        FakeFileSystem::default().with_file(TPUT, "")
    } else {
        FakeFileSystem::default()
    };
    let context = Context::new(&fs, env)
        .with_terminal(terminal)
        .with_process(process);
    detect(&context, flag)
}

fn env_with(term: Option<&str>) -> FakeEnv {
    let env = FakeEnv::default().with_var("PATH", "/usr/bin");
    match term {
        Some(term) => env.with_var("TERM", term),
        None => env,
    }
}

fn on(term: &str, colors: &str) -> bool {
    let env = env_with(Some(term));
    policy(
        &env,
        &FakeTerminal::terminal(),
        &tput_process(term, colors),
        false,
        true,
    )
    .enabled
}

#[test]
fn a_terminal_with_256_colors_is_on() {
    assert!(on("xterm-256color", "256\n"));
}

#[test]
fn a_terminal_with_exactly_8_colors_is_on() {
    assert!(on("xterm", "8\n"));
    assert!(!on("xterm", "7\n"));
}

#[test]
fn dumb_and_vt100_have_no_colors() {
    assert!(!on("dumb", "-1\n"));
    assert!(!on("vt100", "-1\n"));
}

#[test]
fn an_unset_or_empty_term_asks_for_vt100() {
    for term in [None, Some("")] {
        let env = env_with(term);
        let process = tput_process("vt100", "256\n");
        let off = tput_process("xterm", "256\n");
        let terminal = FakeTerminal::terminal();
        assert!(policy(&env, &terminal, &process, false, true).enabled);
        assert!(!policy(&env, &terminal, &off, false, true).enabled);
    }
}

#[test]
fn a_pipe_is_off_even_with_colors() {
    let env = env_with(Some("xterm"));
    let process = tput_process("xterm", "256");
    assert_eq!(
        policy(&env, &FakeTerminal::pipe(), &process, false, true),
        ColorPolicy::off()
    );
}

#[test]
fn the_no_colors_flag_turns_it_off() {
    let env = env_with(Some("xterm"));
    let process = italic_process("xterm", "256");
    let terminal = FakeTerminal::terminal();
    assert_eq!(
        policy(&env, &terminal, &process, true, true),
        ColorPolicy::off()
    );
}

#[test]
fn a_missing_tput_is_off() {
    let env = env_with(Some("xterm"));
    let process = italic_process("xterm", "256");
    let terminal = FakeTerminal::terminal();
    assert_eq!(
        policy(&env, &terminal, &process, false, false),
        ColorPolicy::off()
    );
}

#[test]
fn a_tput_that_cannot_run_or_fails_is_off() {
    let env = env_with(Some("xterm"));
    let terminal = FakeTerminal::terminal();
    let unrunnable = FakeProcess::default();
    let failing = FakeProcess::default().with_run(TPUT, "-T xterm colors", false, "256");
    assert!(!policy(&env, &terminal, &unrunnable, false, true).enabled);
    assert!(!policy(&env, &terminal, &failing, false, true).enabled);
}

#[test]
fn tput_garbage_is_off() {
    for garbage in [
        "",
        "\n",
        "many",
        "8 9",
        "8\n9",
        "0x10",
        "-1",
        "99999999999999999999",
    ] {
        assert!(!on("xterm", garbage), "{garbage:?}");
    }
}

#[test]
fn tput_numbers_are_read_like_test_ge_does() {
    for number in [" 256 \n", "08", "+8", "\t16\n"] {
        assert!(on("xterm", number), "{number:?}");
    }
}

#[test]
fn italics_need_sitm_to_succeed() {
    let env = env_with(Some("xterm"));
    let terminal = FakeTerminal::terminal();
    let with = policy(&env, &terminal, &italic_process("xterm", "8"), false, true);
    let without = policy(&env, &terminal, &tput_process("xterm", "8"), false, true);
    assert_eq!(
        with,
        ColorPolicy {
            enabled: true,
            italics: true
        }
    );
    assert_eq!(
        without,
        ColorPolicy {
            enabled: true,
            italics: false
        }
    );
    let failing = tput_process("xterm", "8").with_run(TPUT, "-T xterm sitm", false, "");
    assert!(!policy(&env, &terminal, &failing, false, true).italics);
}

#[test]
fn italics_are_off_when_colors_are_off() {
    let env = env_with(Some("xterm"));
    let process = italic_process("xterm", "256");
    assert!(!policy(&env, &FakeTerminal::pipe(), &process, false, true).italics);
    assert!(!policy(&env, &FakeTerminal::terminal(), &process, true, true).italics);
}

#[test]
fn no_color_variables_change_nothing() {
    let terminal = FakeTerminal::terminal();
    let process = italic_process("xterm", "256");
    let fs = FakeFileSystem::default().with_file(TPUT, "");
    for (name, value) in [
        ("NVM_NO_COLORS", "1"),
        ("NVM_NO_COLORS", "--no-colors"),
        ("NO_COLOR", "1"),
    ] {
        let env = env_with(Some("xterm")).with_var(name, value);
        let context = Context::new(&fs, &env)
            .with_terminal(&terminal)
            .with_process(&process);
        let expected = ColorPolicy {
            enabled: true,
            italics: true,
        };
        assert_eq!(detect(&context, false), expected, "{name}={value}");
    }
}

fn forced_policy(value: Option<&str>, terminal: &FakeTerminal, flag: bool) -> (bool, ColorPolicy) {
    let mut env = env_with(Some("dumb"));
    if let Some(value) = value {
        env = env.with_var("NVM_HAS_COLORS", value);
    }
    let fs = FakeFileSystem::default().with_file(TPUT, "");
    let process = FakeProcess::default().with_run(TPUT, "-T dumb colors", true, "-1\n");
    let context = Context::new(&fs, &env)
        .with_terminal(terminal)
        .with_process(&process);
    (forced(&context), detect_or_forced(&context, flag))
}

#[test]
fn has_colors_one_forces_on_a_pipe_and_over_the_flag() {
    let forced_on = ColorPolicy {
        enabled: true,
        italics: false,
    };
    assert_eq!(
        forced_policy(Some("1"), &FakeTerminal::pipe(), false),
        (true, forced_on)
    );
    assert_eq!(
        forced_policy(Some("1"), &FakeTerminal::terminal(), true),
        (true, forced_on)
    );
}

#[test]
fn has_colors_other_values_do_not_force() {
    for value in [None, Some("0"), Some(""), Some("true"), Some("11")] {
        let (is_forced, policy) = forced_policy(value, &FakeTerminal::pipe(), false);
        assert!(!is_forced, "{value:?}");
        assert_eq!(policy, ColorPolicy::off());
    }
}

#[test]
fn forcing_does_not_add_italics_that_detection_would_not() {
    let env = env_with(Some("xterm")).with_var("NVM_HAS_COLORS", "1");
    let fs = FakeFileSystem::default().with_file(TPUT, "");
    let process = italic_process("xterm", "256");
    let context = Context::new(&fs, &env).with_process(&process);
    let piped = detect_or_forced(&context, false);
    assert_eq!(
        piped,
        ColorPolicy {
            enabled: true,
            italics: false
        }
    );
    let terminal = FakeTerminal::terminal();
    let context = context.with_terminal(&terminal);
    let real = detect_or_forced(&context, false);
    assert_eq!(
        real,
        ColorPolicy {
            enabled: true,
            italics: true
        }
    );
}
```

Create `src/fakes/terminal.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_what_it_was_made_as() {
        assert!(FakeTerminal::terminal().stdout_is_terminal());
        assert!(!FakeTerminal::pipe().stdout_is_terminal());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find trait `Terminal`",
"cannot find module `color_policy`" and "no method named `with_terminal`
found".

- [ ] **Step 4: Write the implementation**

Insert above the `#[cfg(test)]` line of `src/adapters/no_terminal.rs`:

```rust
use crate::ports::Terminal;

/// The `Terminal` of a `Context` that was not given one: output is a pipe.
pub struct NoTerminal;

impl Terminal for NoTerminal {
    fn stdout_is_terminal(&self) -> bool {
        false
    }
}

```

Insert above the `#[cfg(test)]` line of `src/adapters/std_terminal.rs`:

```rust
use std::io::IsTerminal;

use crate::ports::Terminal;

/// The real [`Terminal`]: asks the operating system about standard output.
pub struct StdTerminal;

impl Terminal for StdTerminal {
    fn stdout_is_terminal(&self) -> bool {
        std::io::stdout().is_terminal()
    }
}

```

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -20,6 +20,7 @@
 use crate::adapters::std_fs::StdFileSystem;
 use crate::adapters::std_process::StdProcess;
 use crate::adapters::std_sleeper::StdSleeper;
+use crate::adapters::std_terminal::StdTerminal;
 use crate::adapters::tar_archive::TarArchive;
 use crate::adapters::ureq_http::UreqHttp;
 use crate::commands::Output;
@@ -149,6 +150,7 @@
         .with_archive(&TarArchive)
         .with_sleeper(&StdSleeper)
         .with_cpu(&StdCpu)
+        .with_terminal(&StdTerminal)
         .with_platform(platform);
     match &channel {
         Some(channel) => body(&context.with_script_channel(channel)),
```

Insert above the `#[cfg(test)]` line of `src/commands/color_policy/mod.rs`:

```rust
//! Whether the colors of nvm.sh are on: `nvm_has_colors` and
//! `nvm_has_italics`, with the `NVM_HAS_COLORS=1` override of the fork.

use std::path::Path;

use crate::context::Context;
use crate::domain::path_search::find_in_path;

```

Apply to `src/context/mod.rs` (above the test module):

```diff
--- a/src/context/mod.rs
+++ b/src/context/mod.rs
@@ -9,11 +9,14 @@
 use crate::adapters::no_http::NoHttp;
 use crate::adapters::no_process::NoProcess;
 use crate::adapters::no_sleeper::NoSleeper;
+use crate::adapters::no_terminal::NoTerminal;
 use crate::domain::alias::AliasStore;
 use crate::domain::platform::{Os, Platform};
 use crate::domain::version::Version;
 use crate::error::CliError;
-use crate::ports::{Archive, Cpu, Digest, Env, FileSystem, Http, Process, ScriptChannel, Sleeper};
+use crate::ports::{
+    Archive, Cpu, Digest, Env, FileSystem, Http, Process, ScriptChannel, Sleeper, Terminal,
+};
 
 pub struct Context<'a> {
     pub fs: &'a dyn FileSystem,
@@ -24,6 +27,7 @@
     archive: &'a dyn Archive,
     sleeper: &'a dyn Sleeper,
     cpu: &'a dyn Cpu,
+    terminal: &'a dyn Terminal,
     platform: Option<Platform>,
     script_channel: Option<&'a dyn ScriptChannel>,
 }
@@ -42,6 +46,7 @@
             archive: &NoArchive,
             sleeper: &NoSleeper,
             cpu: &NoCpu,
+            terminal: &NoTerminal,
             platform: Some(Platform {
                 os: Os::Linux,
                 arch: "x64".to_owned(),
@@ -86,6 +91,19 @@
     #[must_use]
     pub fn cpu(&self) -> &dyn Cpu {
         self.cpu
+    }
+
+    /// The terminal behind standard output; a context starts out without one
+    /// (output is a pipe).
+    #[must_use]
+    pub fn with_terminal(mut self, terminal: &'a dyn Terminal) -> Self {
+        self.terminal = terminal;
+        self
+    }
+
+    #[must_use]
+    pub fn terminal(&self) -> &dyn Terminal {
+        self.terminal
     }
 
     /// The machine the binaries are for; `None` when it has no official ones.
```

Apply to `src/fakes/mod.rs` (above the test module):

```diff
--- a/src/fakes/mod.rs
+++ b/src/fakes/mod.rs
@@ -9,6 +9,7 @@
 mod http;
 mod process;
 mod sleeper;
+mod terminal;
 
 pub use archive::FakeArchive;
 pub use channel::FakeScriptChannel;
@@ -19,3 +20,4 @@
 pub use http::FakeHttp;
 pub use process::FakeProcess;
 pub use sleeper::FakeSleeper;
+pub use terminal::FakeTerminal;
```

Apply to `src/fakes/process.rs` (above the test module):

```diff
--- a/src/fakes/process.rs
+++ b/src/fakes/process.rs
@@ -9,6 +9,7 @@
 #[derive(Default)]
 pub struct FakeProcess {
     outputs: BTreeMap<PathBuf, ProcessOutput>,
+    runs: BTreeMap<(PathBuf, String), ProcessOutput>,
     executions: BTreeMap<(PathBuf, String), Completed>,
     effects: BTreeMap<(PathBuf, String), Box<dyn Fn()>>,
     executed: RefCell<Vec<Invocation>>,
@@ -39,6 +40,19 @@
 }
 
 impl FakeProcess {
+    /// What `run` answers to `program` run with exactly `args` (joined with
+    /// spaces), taking precedence over [`Self::with_output`].
+    #[must_use]
+    pub fn with_run(mut self, program: &str, args: &str, success: bool, stdout: &str) -> Self {
+        let output = ProcessOutput {
+            success,
+            stdout: stdout.to_owned(),
+        };
+        self.runs
+            .insert((PathBuf::from(program), args.to_owned()), output);
+        self
+    }
+
     /// What `execute` answers to `program` run with exactly `args` (joined
     /// with spaces); any other invocation is not found.
     #[must_use]
@@ -112,9 +126,11 @@
             .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
     }
 
-    fn run(&self, program: &Path, _args: &[&str]) -> io::Result<ProcessOutput> {
-        self.outputs
-            .get(program)
+    fn run(&self, program: &Path, args: &[&str]) -> io::Result<ProcessOutput> {
+        let key = (program.to_path_buf(), args.join(" "));
+        self.runs
+            .get(&key)
+            .or_else(|| self.outputs.get(program))
             .cloned()
             .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
     }
```

Insert above the `#[cfg(test)]` line of `src/fakes/terminal.rs`:

```rust
use crate::ports::Terminal;

/// Standard output as a terminal or as a pipe.
pub struct FakeTerminal(bool);

impl FakeTerminal {
    #[must_use]
    pub fn terminal() -> Self {
        Self(true)
    }

    #[must_use]
    pub fn pipe() -> Self {
        Self(false)
    }
}

impl Terminal for FakeTerminal {
    fn stdout_is_terminal(&self) -> bool {
        self.0
    }
}

```

Apply to `src/ports/mod.rs` (above the test module):

```diff
--- a/src/ports/mod.rs
+++ b/src/ports/mod.rs
@@ -256,6 +256,11 @@
     fn extract(&self, archive: &Path, destination: &Path) -> io::Result<()>;
 }
 
+pub trait Terminal {
+    /// Whether standard output is a terminal (`[ -t 1 ]`).
+    fn stdout_is_terminal(&self) -> bool;
+}
+
 pub trait Cpu {
     /// How many processors this machine has for a build to use, when known.
     fn cores(&self) -> Option<usize>;
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1048 unit tests plus the end-to-end tests.

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(terminal): detect when stdout can show colors like nvm_has_colors"
```

---

### Task 3: `nvm ls`: colored version rows

**Files:**

- Create: `src/commands/ls/colored_tests.rs`, `src/commands/ls/options.rs`,
  `src/domain/listing/colored.rs`, `src/domain/listing/mod.rs`
- Modify: `src/commands/color_policy/mod.rs`, `src/commands/ls/mod.rs`,
  `src/commands/ls/tests.rs`, `src/domain/colors/mod.rs`,
  `src/domain/colors/tests.rs`
- Delete: `src/domain/listing.rs`

**Interfaces:**

- Produces: `domain::listing::colored::{paint_row, paint_system_row}`
  (`src/domain/listing.rs` becomes a directory);
  `domain::colors::invalid_color_warning(&[Option<char>]) -> Option<String>`;
  `commands::color_policy::palette(&Context) -> (Palette, Option<String>)`
  (reads `NVM_COLORS`; the second value is the single warning line);
  `commands::ls::Options { pattern, no_alias, no_colors }`, `parse_options` in
  `commands/ls/options.rs`, and `commands::ls::run(context, pattern,
  no_colors)`.
- Behaviour (digest sections 3.1 and 4): with colors on, the current row is
  `\x1b[<current>->%13s\x1b[0m`, an installed row `\x1b[<installed>%15s\x1b[0m`,
  the system row `\x1b[<system>%15s\x1b[0m` followed by `(\x1b[<system>->
  v16.0.0\x1b[0m)` when the system node resolves (when system is the current row
  it takes the current color but its target keeps the system color), and the
  `N/A` row stays plain (exit 3). A role whose color is missing renders plain
  WITHOUT the `*` and without the `->` (as `nvm.sh`), and the command prints
  ONE `Invalid color code: <x>` line on stderr (nvm.sh prints dozens).
  `--no-colors` may be anywhere in the arguments and is forwarded to the alias
  section; any other unknown `--x` is still `Unsupported option "--x".` (55).
  The alias section of `ls` keeps its Plan 3 rows until Task 4.
- [ ] **Step 1: Write the failing tests**

Apply to the test module of `src/commands/color_policy/mod.rs`:

```diff
--- a/src/commands/color_policy/mod.rs
+++ b/src/commands/color_policy/mod.rs
@@ -95,3 +95,13 @@
         italics: detected.italics,
     }
 }
+
+/// The palette of `NVM_COLORS`, read once per command, with the single
+/// `Invalid color code: <x>` line the command prints on stderr when the
+/// setting has an invalid or missing role.
+#[must_use]
+pub fn palette(context: &Context<'_>) -> (Palette, Option<String>) {
+    let setting = context.env.var("NVM_COLORS");
+    let (palette, offending) = Palette::from_setting(setting.as_deref());
+    (palette, invalid_color_warning(&offending))
+}
```

Apply to `src/commands/ls/tests.rs`:

```diff
--- a/src/commands/ls/tests.rs
+++ b/src/commands/ls/tests.rs
@@ -21,7 +21,7 @@
         .with_var("NVM_DIR", "/n")
         .with_var("PATH", path);
     let context = Context::new(fs, &env).with_process(&process);
-    run(&context, pattern).unwrap()
+    run(&context, pattern, false).unwrap()
 }
 
 fn ls(pattern: Option<&str>) -> Output {
@@ -142,7 +142,8 @@
         parsed,
         Options {
             pattern: Some("20".to_owned()),
-            no_alias: false
+            no_alias: false,
+            no_colors: true,
         }
     );
     let no_alias = parse_options(&words(&["--no-alias"])).unwrap();
```

Apply to the test module of `src/domain/colors/mod.rs`:

```diff
--- a/src/domain/colors/mod.rs
+++ b/src/domain/colors/mod.rs
@@ -155,6 +155,17 @@
     }
 }
 
+/// The single stderr line a command prints for the offending roles reported
+/// by [`Palette::from_setting`]: `Invalid color code: <letter>` for the first
+/// one (nothing after the colon for a missing position), `None` when the
+/// setting is valid. nvm.sh prints it on every lookup; once is enough.
+#[must_use]
+pub fn invalid_color_warning(offending: &[Option<char>]) -> Option<String> {
+    let first = offending.first()?;
+    let letter = first.map(String::from).unwrap_or_default();
+    Some(format!("Invalid color code: {letter}"))
+}
+
 /// Mirrors the check of `nvm_set_colors`: exactly five characters, each one
 /// of `rRgGbBcCyYmMkKeW` (`0` is not accepted here).
 #[must_use]
```

Apply to `src/domain/colors/tests.rs`:

```diff
--- a/src/domain/colors/tests.rs
+++ b/src/domain/colors/tests.rs
@@ -120,6 +120,21 @@
 }
 
 #[test]
+fn the_warning_is_one_line_for_the_first_offending_role() {
+    let (_, invalid) = Palette::from_setting(Some("bzgxe"));
+    assert_eq!(
+        invalid_color_warning(&invalid).as_deref(),
+        Some("Invalid color code: z")
+    );
+    let (_, short) = Palette::from_setting(Some("rg"));
+    assert_eq!(
+        invalid_color_warning(&short).as_deref(),
+        Some("Invalid color code: ")
+    );
+    assert_eq!(invalid_color_warning(&[]), None);
+}
+
+#[test]
 fn zero_means_no_color_without_a_warning() {
     let (palette, offending) = Palette::from_setting(Some("b0gre"));
     assert_eq!(palette.code(Role::System), None);
```

Delete `src/domain/listing.rs` (`git rm src/domain/listing.rs`).

Create `src/domain/listing/colored.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn palette(setting: &str) -> Palette {
        Palette::from_setting(Some(setting)).0
    }

    #[test]
    fn colors_off_gives_the_plain_rows() {
        let colors = palette("bygre");
        assert_eq!(
            paint_row("v18.20.4", RowKind::Installed, &colors, false),
            "       v18.20.4 *"
        );
        assert_eq!(
            paint_row("v20.11.1", RowKind::Current, &colors, false),
            "->     v20.11.1 *"
        );
        assert_eq!(
            paint_system_row(RowKind::Current, Some("v16.0.0"), &colors, false),
            "->       system * (-> v16.0.0)"
        );
    }

    #[test]
    fn installed_and_current_rows_are_one_span_without_a_star() {
        let colors = palette("bygre");
        assert_eq!(
            paint_row("iojs-v3.3.1", RowKind::Installed, &colors, true),
            "\x1b[0;34m    iojs-v3.3.1\x1b[0m"
        );
        assert_eq!(
            paint_row("v20.11.1", RowKind::Current, &colors, true),
            "\x1b[0;32m->     v20.11.1\x1b[0m"
        );
    }

    #[test]
    fn a_plain_row_is_never_colored() {
        let colors = palette("bygre");
        assert_eq!(
            paint_row("N/A", RowKind::Plain, &colors, true),
            "            N/A"
        );
    }

    #[test]
    fn the_system_row_and_its_target_take_the_system_color() {
        let colors = palette("bygre");
        assert_eq!(
            paint_system_row(RowKind::Installed, Some("v16.0.0"), &colors, true),
            "\x1b[0;33m         system\x1b[0m (\x1b[0;33m-> v16.0.0\x1b[0m)"
        );
        assert_eq!(
            paint_system_row(RowKind::Installed, None, &colors, true),
            "\x1b[0;33m         system\x1b[0m"
        );
    }

    #[test]
    fn a_current_system_row_is_current_but_its_target_stays_system() {
        let colors = palette("bygre");
        assert_eq!(
            paint_system_row(RowKind::Current, Some("v16.0.0"), &colors, true),
            "\x1b[0;32m->       system\x1b[0m (\x1b[0;33m-> v16.0.0\x1b[0m)"
        );
    }

    #[test]
    fn a_role_without_color_is_plain_without_star_or_arrow() {
        let short = palette("rg");
        assert_eq!(
            paint_row("v20.11.1", RowKind::Current, &short, true),
            "       v20.11.1"
        );
        let invalid = palette("zzzzz");
        assert_eq!(
            paint_row("v18.20.4", RowKind::Installed, &invalid, true),
            "       v18.20.4"
        );
        assert_eq!(
            paint_system_row(RowKind::Installed, Some("v16.0.0"), &invalid, true),
            "         system (-> v16.0.0)"
        );
    }

    #[test]
    fn the_roles_follow_the_setting() {
        let colors = palette("rgbcm");
        assert_eq!(
            paint_row("v22.3.0", RowKind::Installed, &colors, true),
            "\x1b[0;31m        v22.3.0\x1b[0m"
        );
        assert_eq!(
            paint_system_row(RowKind::Installed, Some("v16.0.0"), &colors, true),
            "\x1b[0;32m         system\x1b[0m (\x1b[0;32m-> v16.0.0\x1b[0m)"
        );
    }
}
```

Create `src/domain/listing/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_versions_are_right_aligned_with_a_star() {
        assert_eq!(
            format_row("v18.9.0", RowKind::Installed),
            "        v18.9.0 *"
        );
        assert_eq!(
            format_row("iojs-v2.5.0", RowKind::Installed),
            "    iojs-v2.5.0 *"
        );
    }

    #[test]
    fn the_current_version_has_an_arrow_and_the_same_width() {
        let row = format_row("v20.1.0", RowKind::Current);
        assert_eq!(row, "->      v20.1.0 *");
        assert_eq!(row.len(), format_row("v20.1.0", RowKind::Installed).len());
    }

    #[test]
    fn a_plain_row_has_no_star() {
        assert_eq!(format_row("N/A", RowKind::Plain), "            N/A");
    }

    #[test]
    fn padding_counts_bytes_so_infinity_gets_twelve_spaces() {
        assert_eq!(format_row("∞", RowKind::Plain), "            ∞");
    }

    #[test]
    fn a_name_longer_than_the_column_is_not_cut() {
        let row = format_row("iojs-v100.100.100", RowKind::Installed);
        assert_eq!(row, "iojs-v100.100.100 *");
    }

    #[test]
    fn the_system_row_shows_the_version_of_the_system_node() {
        let installed = format_system_row(RowKind::Installed, Some("v22.1.0"));
        assert_eq!(installed, "         system * (-> v22.1.0)");
        let current = format_system_row(RowKind::Current, Some("v22.1.0"));
        assert_eq!(current, "->       system * (-> v22.1.0)");
        assert_eq!(
            format_system_row(RowKind::Installed, None),
            "         system *"
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "no field `no_colors` on type
`Options`", "cannot find function `paint_row`" and "this function takes 2
arguments but 3 were supplied" for `ls::run`.

- [ ] **Step 3: Write the implementation**

Apply to `src/commands/color_policy/mod.rs` (above the test module):

```diff
--- a/src/commands/color_policy/mod.rs
+++ b/src/commands/color_policy/mod.rs
@@ -4,5 +4,6 @@
 use std::path::Path;
 
 use crate::context::Context;
+use crate::domain::colors::{Palette, invalid_color_warning};
 use crate::domain::path_search::find_in_path;
 
```

Create `src/commands/ls/colored_tests.rs`:

```rust
//! The colored version rows, against the golden `nvm ls` of the digest
//! (fixture: node v18.20.4, v20.11.1, v22.3.0, io.js v3.3.1, a system node
//! v16.0.0).

use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess, FakeTerminal};

const TPUT: &str = "/usr/bin/tput";
const IN_USE: &str = "/n/versions/node/v20.11.1/bin";

fn versions_only() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/n/versions/node/v18.20.4/bin/node", "")
        .with_file("/n/versions/node/v20.11.1/bin/node", "")
        .with_file("/n/versions/node/v22.3.0/bin/node", "")
        .with_file("/n/versions/io.js/v3.3.1/bin/node", "")
        .with_file("/n/alias/default", "18")
        .with_file(TPUT, "")
}

fn fixture() -> FakeFileSystem {
    versions_only().with_file("/sys/node", "")
}

struct Setup<'a> {
    path: &'a str,
    term: &'a str,
    colors: Option<&'a str>,
    terminal: FakeTerminal,
}

impl<'a> Setup<'a> {
    fn tty() -> Self {
        Self {
            path: IN_USE,
            term: "xterm-256color",
            colors: None,
            terminal: FakeTerminal::terminal(),
        }
    }

    fn path(self, path: &'a str) -> Self {
        Self { path, ..self }
    }

    fn colors(self, colors: &'a str) -> Self {
        Self {
            colors: Some(colors),
            ..self
        }
    }

    fn run(&self, fs: &FakeFileSystem, args: &[&str]) -> Output {
        let process = FakeProcess::default()
            .with_output("/sys/node", "v16.0.0\n")
            .with_run(TPUT, "-T xterm-256color colors", true, "256\n")
            .with_run(TPUT, "-T dumb colors", true, "-1\n");
        let path = format!("{}:/sys:/usr/bin", self.path);
        let mut env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", &path)
            .with_var("TERM", self.term);
        if let Some(colors) = self.colors {
            env = env.with_var("NVM_COLORS", colors);
        }
        let context = Context::new(fs, &env)
            .with_process(&process)
            .with_terminal(&self.terminal);
        let args: Vec<String> = args.iter().map(ToString::to_string).collect();
        run_command(&context, &args).unwrap()
    }
}

fn rows(lines: &[&str]) -> String {
    lines.join("\n")
}

const PLAIN: [&str; 5] = [
    "    iojs-v3.3.1 *",
    "       v18.20.4 *",
    "->     v20.11.1 *",
    "        v22.3.0 *",
    "         system * (-> v16.0.0)",
];

#[test]
fn the_default_palette_colors_every_version_row() {
    let output = Setup::tty().run(&fixture(), &["--no-alias"]);
    let expected = rows(&[
        "\x1b[0;34m    iojs-v3.3.1\x1b[0m",
        "\x1b[0;34m       v18.20.4\x1b[0m",
        "\x1b[0;32m->     v20.11.1\x1b[0m",
        "\x1b[0;34m        v22.3.0\x1b[0m",
        "\x1b[0;33m         system\x1b[0m (\x1b[0;33m-> v16.0.0\x1b[0m)",
    ]);
    assert_eq!(output, Output::stdout(expected));
}

#[test]
fn nvm_colors_picks_the_role_colors() {
    let output = Setup::tty()
        .colors("rgbcm")
        .run(&fixture(), &["--no-alias"]);
    let expected = rows(&[
        "\x1b[0;31m    iojs-v3.3.1\x1b[0m",
        "\x1b[0;31m       v18.20.4\x1b[0m",
        "\x1b[0;34m->     v20.11.1\x1b[0m",
        "\x1b[0;31m        v22.3.0\x1b[0m",
        "\x1b[0;32m         system\x1b[0m (\x1b[0;32m-> v16.0.0\x1b[0m)",
    ]);
    assert_eq!(output, Output::stdout(expected));
}

#[test]
fn a_pipe_dumb_term_or_no_colors_flag_gives_the_plain_rows() {
    let pipe = Setup {
        terminal: FakeTerminal::pipe(),
        ..Setup::tty()
    };
    let dumb = Setup {
        term: "dumb",
        ..Setup::tty()
    };
    let expected = Output::stdout(rows(&PLAIN));
    assert_eq!(pipe.run(&fixture(), &["--no-alias"]), expected);
    assert_eq!(dumb.run(&fixture(), &["--no-alias"]), expected);
    let flag_last = Setup::tty().run(&fixture(), &["--no-alias", "--no-colors"]);
    assert_eq!(flag_last, expected);
    let flag_first = Setup::tty().run(&fixture(), &["--no-colors", "--no-alias"]);
    assert_eq!(flag_first, expected);
}

#[test]
fn nvm_no_colors_in_the_environment_is_ignored() {
    let fs = fixture();
    let process = FakeProcess::default()
        .with_output("/sys/node", "v16.0.0\n")
        .with_run(TPUT, "-T xterm-256color colors", true, "256\n");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/usr/bin")
        .with_var("TERM", "xterm-256color")
        .with_var("NVM_NO_COLORS", "--no-colors");
    let terminal = FakeTerminal::terminal();
    let context = Context::new(&fs, &env)
        .with_process(&process)
        .with_terminal(&terminal);
    let output = run(&context, Some("18"), false).unwrap();
    assert_eq!(output, Output::stdout("\x1b[0;34m       v18.20.4\x1b[0m"));
}

#[test]
fn the_current_system_row_is_current_with_a_system_target() {
    let output = Setup::tty().path("/sys").run(&fixture(), &["--no-alias"]);
    let last = output.stdout.lines().last().unwrap();
    assert_eq!(
        last,
        "\x1b[0;32m->       system\x1b[0m (\x1b[0;33m-> v16.0.0\x1b[0m)"
    );
    assert!(output.stdout.contains("\x1b[0;34m       v20.11.1\x1b[0m\n"));
}

#[test]
fn a_current_io_js_gets_the_current_color() {
    let in_use = "/n/versions/io.js/v3.3.1/bin";
    let output = Setup::tty().path(in_use).run(&fixture(), &["--no-alias"]);
    let first = output.stdout.lines().next().unwrap();
    assert_eq!(first, "\x1b[0;32m->  iojs-v3.3.1\x1b[0m");
}

#[test]
fn with_no_node_in_use_there_is_no_arrow_and_no_system_row() {
    let fs = versions_only();
    let output = Setup::tty().path("/nowhere").run(&fs, &["--no-alias"]);
    let expected = rows(&[
        "\x1b[0;34m    iojs-v3.3.1\x1b[0m",
        "\x1b[0;34m       v18.20.4\x1b[0m",
        "\x1b[0;34m       v20.11.1\x1b[0m",
        "\x1b[0;34m        v22.3.0\x1b[0m",
    ]);
    assert_eq!(output, Output::stdout(expected));
}

#[test]
fn the_not_available_row_stays_plain_with_status_3() {
    let output = Setup::tty().run(&fixture(), &["99"]);
    let expected = Output::stdout("            N/A").with_status(NvmExitCode::InvalidVersion);
    assert_eq!(output, expected);
}

#[test]
fn a_short_setting_loses_the_current_arrow_and_warns_once() {
    let output = Setup::tty().colors("rg").run(&fixture(), &["--no-alias"]);
    let expected = rows(&[
        "\x1b[0;31m    iojs-v3.3.1\x1b[0m",
        "\x1b[0;31m       v18.20.4\x1b[0m",
        "       v20.11.1",
        "\x1b[0;31m        v22.3.0\x1b[0m",
        "\x1b[0;32m         system\x1b[0m (\x1b[0;32m-> v16.0.0\x1b[0m)",
    ]);
    let warning = "Invalid color code: ";
    assert_eq!(output, Output::stdout(expected).with_stderr(warning));
}

#[test]
fn an_invalid_setting_gives_plain_rows_without_markers_and_warns_once() {
    let output = Setup::tty()
        .colors("zzzzz")
        .run(&fixture(), &["--no-alias"]);
    let expected = rows(&[
        "    iojs-v3.3.1",
        "       v18.20.4",
        "       v20.11.1",
        "        v22.3.0",
        "         system (-> v16.0.0)",
    ]);
    let warning = "Invalid color code: z";
    assert_eq!(output, Output::stdout(expected).with_stderr(warning));
}

#[test]
fn the_alias_rows_are_left_as_they_are() {
    let output = Setup::tty().run(&fixture(), &[]);
    let lines: Vec<&str> = output.stdout.lines().collect();
    assert_eq!(lines[1], "\x1b[0;34m       v18.20.4\x1b[0m");
    assert_eq!(lines[5], "default -> 18 (-> v18.20.4 *)");
    let plain = Setup::tty().run(&fixture(), &["--no-colors"]);
    assert_eq!(plain.stdout.lines().nth(5), Some(lines[5]));
    assert_eq!(plain.stdout.lines().nth(2), Some(PLAIN[2]));
}
```

Apply to `src/commands/ls/mod.rs` (above the test module):

```diff
--- a/src/commands/ls/mod.rs
+++ b/src/commands/ls/mod.rs
@@ -1,6 +1,7 @@
 //! `nvm ls [pattern]`: the installed versions, one row each.
 //!
-//! Output is always plain, as `nvm.sh` prints when stdout is not a terminal.
+//! The version rows are colored like `nvm_print_versions` when stdout can
+//! show colors and `--no-colors` is not given; the alias rows are plain.
 //! Unlike `nvm.sh` it does not print a blank row when only a system node
 //! exists, and it keeps both io.js and Node versions that share a number.
 //! For an alias that resolves to nothing (`ls lts/gallium`, `ls unstable`,
@@ -9,13 +10,19 @@
 
 use crate::commands::Output;
 use crate::commands::aliases;
+use crate::commands::color_policy;
 use crate::commands::current;
 use crate::commands::resolve::{Resolved, resolve_installed, system_node, system_version};
 use crate::context::Context;
 use crate::domain::alias::AliasStore;
-use crate::domain::listing::{RowKind, format_row, format_system_row};
+use crate::domain::colors::Palette;
+use crate::domain::listing::RowKind;
+use crate::domain::listing::colored::{paint_row, paint_system_row};
 use crate::domain::version::{Flavor, Version, VersionPattern};
 use crate::error::{CliError, NvmExitCode};
+
+mod options;
+pub use options::{Options, parse_options};
 
 enum Entry {
     Version(Version),
@@ -30,40 +37,6 @@
     entries: Vec<Entry>,
     /// Nothing matched: the status is 3, like `nvm.sh`.
     missing: bool,
-}
-
-/// The command line of `nvm ls`, as `nvm.sh` reads it: the first non-empty
-/// word is the pattern, `--no-colors` is accepted (output is always plain).
-#[derive(Debug, Default, PartialEq, Eq)]
-pub struct Options {
-    pub pattern: Option<String>,
-    pub no_alias: bool,
-}
-
-/// # Errors
-/// - [`CliError::Unsupported`] for an unknown `--option`, and for
-///   `--no-alias` together with a pattern.
-pub fn parse_options(args: &[String]) -> Result<Options, CliError> {
-    let mut options = Options::default();
-    for arg in args {
-        match arg.as_str() {
-            "--" | "--no-colors" => {}
-            "--no-alias" => options.no_alias = true,
-            option if option.starts_with("--") => {
-                let message = format!("Unsupported option \"{option}\".");
-                return Err(CliError::Unsupported(message));
-            }
-            word if options.pattern.is_none() && !word.is_empty() => {
-                options.pattern = Some(word.to_owned());
-            }
-            _ => {}
-        }
-    }
-    if options.pattern.is_some() && options.no_alias {
-        let message = "`--no-alias` is not supported when a pattern is provided.";
-        return Err(CliError::Unsupported(message.to_owned()));
-    }
-    Ok(options)
 }
 
 /// `nvm ls`: the versions, then (without a pattern or `--no-alias`) the
@@ -74,9 +47,14 @@
 /// cannot be found.
 pub fn run_command(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
     let options = parse_options(args)?;
-    let mut output = run(context, options.pattern.as_deref())?;
+    let mut output = run(context, options.pattern.as_deref(), options.no_colors)?;
     if options.pattern.is_none() && !options.no_alias {
-        let listed = aliases::list(context, None)?;
+        let alias_args = if options.no_colors {
+            vec!["--no-colors".to_owned()]
+        } else {
+            Vec::new()
+        };
+        let listed = aliases::run(context, &alias_args)?;
         if !listed.stdout.is_empty() {
             output.stdout = format!("{}\n{}", output.stdout, listed.stdout);
         }
@@ -84,21 +62,38 @@
     Ok(output)
 }
 
+/// `nvm ls [pattern]` without the aliases; `no_colors` is the `--no-colors`
+/// flag. An invalid `NVM_COLORS` adds its single warning on stderr.
+///
 /// # Errors
 /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
-pub fn run(context: &Context<'_>, pattern: Option<&str>) -> Result<Output, CliError> {
+pub fn run(
+    context: &Context<'_>,
+    pattern: Option<&str>,
+    no_colors: bool,
+) -> Result<Output, CliError> {
     let current = current::detect(context)?.to_string();
     let mut installed = context.installed_versions()?;
     installed.sort();
     let not_available = not_available_kind(context, &installed)?;
     let pattern = pattern.filter(|text| !text.is_empty());
     let selection = select(context, installed, pattern, &current)?;
+    let (palette, warning) = color_policy::palette(context);
+    let painter = Painter {
+        current,
+        not_available,
+        palette,
+        colors: color_policy::detect(context, no_colors).enabled,
+    };
     let rows: Vec<String> = selection
         .entries
         .iter()
-        .map(|entry| render(entry, &current, not_available))
+        .map(|entry| painter.render(entry))
         .collect();
-    let output = Output::stdout(rows.join("\n"));
+    let mut output = Output::stdout(rows.join("\n"));
+    if let Some(warning) = warning {
+        output = output.with_stderr(warning);
+    }
     Ok(if selection.missing {
         output.with_status(NvmExitCode::InvalidVersion)
     } else {
@@ -222,25 +217,45 @@
     }
 }
 
-fn kind_for(text: &str, current: &str) -> RowKind {
-    if text == current {
-        RowKind::Current
-    } else {
-        RowKind::Installed
-    }
-}
-
-fn render(entry: &Entry, current: &str, not_available: RowKind) -> String {
-    match entry {
-        Entry::Version(version) => {
-            let text = version.to_string();
-            format_row(&text, kind_for(&text, current))
+/// What the rows are rendered with: the version in use, how `N/A` shows,
+/// and the colors.
+struct Painter {
+    current: String,
+    not_available: RowKind,
+    palette: Palette,
+    colors: bool,
+}
+
+impl Painter {
+    fn kind_for(&self, text: &str) -> RowKind {
+        if text == self.current {
+            RowKind::Current
+        } else {
+            RowKind::Installed
         }
-        Entry::System(version) => {
-            format_system_row(kind_for("system", current), version.as_deref())
+    }
+
+    fn row(&self, text: &str, kind: RowKind) -> String {
+        paint_row(text, kind, &self.palette, self.colors)
+    }
+
+    fn render(&self, entry: &Entry) -> String {
+        match entry {
+            Entry::Version(version) => {
+                let text = version.to_string();
+                self.row(&text, self.kind_for(&text))
+            }
+            Entry::System(version) => paint_system_row(
+                self.kind_for("system"),
+                version.as_deref(),
+                &self.palette,
+                self.colors,
+            ),
+            Entry::NotAvailable => self.row("N/A", self.not_available),
+            Entry::Raw(text, kind) => self.row(text, *kind),
         }
-        Entry::NotAvailable => format_row("N/A", not_available),
-        Entry::Raw(text, kind) => format_row(text, *kind),
-    }
-}
-
+    }
+}
+
+#[cfg(test)]
+mod colored_tests;
```

Create `src/commands/ls/options.rs`:

```rust
//! The command line of `nvm ls`.

use crate::error::CliError;

/// The command line of `nvm ls`, as `nvm.sh` reads it: the first non-empty
/// word is the pattern, `--no-colors` turns the colors off.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub pattern: Option<String>,
    pub no_alias: bool,
    pub no_colors: bool,
}

/// # Errors
/// - [`CliError::Unsupported`] for an unknown `--option`, and for
///   `--no-alias` together with a pattern.
pub fn parse_options(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    for arg in args {
        match arg.as_str() {
            "--" => {}
            "--no-colors" => options.no_colors = true,
            "--no-alias" => options.no_alias = true,
            option if option.starts_with("--") => {
                let message = format!("Unsupported option \"{option}\".");
                return Err(CliError::Unsupported(message));
            }
            word if options.pattern.is_none() && !word.is_empty() => {
                options.pattern = Some(word.to_owned());
            }
            _ => {}
        }
    }
    if options.pattern.is_some() && options.no_alias {
        let message = "`--no-alias` is not supported when a pattern is provided.";
        return Err(CliError::Unsupported(message.to_owned()));
    }
    Ok(options)
}
```

Insert above the `#[cfg(test)]` line of `src/domain/listing/colored.rs`:

```rust
//! The rows of `nvm ls` as `nvm_print_versions` paints them: with colors off
//! they are the plain rows of [`format_row`]; with colors on each row is one
//! colored span and loses its ` *` marker.

use super::{RowKind, format_row, format_system_row, pad_left};
use crate::domain::colors::{Palette, Role, wrap};

/// A version row. `Installed` takes the installed color, `Current` the
/// current color with its `->`, `Plain` (`N/A`, `∞`) is never colored.
#[must_use]
pub fn paint_row(text: &str, kind: RowKind, palette: &Palette, colors: bool) -> String {
    if colors {
        paint(text, kind, Role::Installed, palette)
    } else {
        format_row(text, kind)
    }
}

/// The `fmt_installed` / `fmt_system` / `fmt_current` of the awk: a role
/// without a color falls back to `%15s`, so the current row loses its `->`.
fn paint(text: &str, kind: RowKind, role: Role, palette: &Palette) -> String {
    match kind {
        RowKind::Current => match palette.code(Role::Current) {
            Some(code) => wrap(Some(code), &format!("->{}", pad_left(text, 13))),
            None => pad_left(text, 15),
        },
        RowKind::Installed => wrap(palette.code(role), &pad_left(text, 15)),
        RowKind::Plain => pad_left(text, 15),
    }
}

/// The `system` row: the system color (the current color when it is the
/// version in use), then the target ` (-> v16.0.0)` in the system color.
#[must_use]
pub fn paint_system_row(
    kind: RowKind,
    version: Option<&str>,
    palette: &Palette,
    colors: bool,
) -> String {
    if !colors {
        return format_system_row(kind, version);
    }
    let row = paint("system", kind, Role::System, palette);
    match version {
        Some(version) => {
            let target = wrap(palette.code(Role::System), &format!("-> {version}"));
            format!("{row} ({target})")
        }
        None => row,
    }
}

```

Insert above the `#[cfg(test)]` line of `src/domain/listing/mod.rs`:

```rust
//! The rows `nvm ls` prints, as `nvm.sh` prints them when stdout is not a
//! terminal (no colors): the version right-aligned in 15 columns.

pub mod colored;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// An installed version: `        v18.9.0 *`.
    Installed,
    /// The version in use: `->      v20.1.0 *`.
    Current,
    /// Not installed, so no marker: `            N/A`.
    Plain,
}

/// Right-aligns by bytes, like the `awk` of `nvm.sh`: the 3-byte `∞` is
/// padded with 12 spaces, not 14.
fn pad_left(text: &str, width: usize) -> String {
    format!("{}{text}", " ".repeat(width.saturating_sub(text.len())))
}

#[must_use]
pub fn format_row(text: &str, kind: RowKind) -> String {
    match kind {
        RowKind::Installed => format!("{} *", pad_left(text, 15)),
        RowKind::Current => format!("->{} *", pad_left(text, 13)),
        RowKind::Plain => pad_left(text, 15),
    }
}

/// The `system` row, with the version of the system node when it is known.
#[must_use]
pub fn format_system_row(kind: RowKind, version: Option<&str>) -> String {
    let row = format_row("system", kind);
    match version {
        Some(version) => format!("{row} (-> {version})"),
        None => row,
    }
}

```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1067 unit tests plus the end-to-end tests.

- [ ] **Step 5: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(ls): color the version rows like nvm_print_versions"
```

---

### Task 4: Colored aliases

**Files:**

- Create: `src/commands/alias/colored_tests.rs`, `src/commands/alias/mod.rs`,
  `src/commands/alias/tests.rs`, `src/commands/aliases/colored_tests.rs`,
  `src/commands/aliases/fixture.rs`, `src/commands/aliases/painter.rs`,
  `src/commands/ls/alias_section_tests.rs`,
  `src/domain/alias_format/colored.rs`,
  `src/domain/alias_format/colored_tests.rs`, `src/domain/alias_format/mod.rs`
- Modify: `src/commands/aliases/mod.rs`, `src/commands/aliases/tests.rs`,
  `src/commands/ls/colored_tests.rs`, `src/commands/ls/mod.rs`
- Delete: `src/commands/alias.rs`, `src/domain/alias_format.rs`

**Interfaces:**

- Produces: `domain::alias_format::colored::{VersionState, AliasKind, AliasRow,
  paint_line}` (`alias_format.rs` becomes a directory);
  `commands::aliases::{list(context, prefix, no_colors), AliasPainter}`;
  `commands::alias::run_with_colors(context, name, target, no_colors)`
  (`alias.rs` becomes a directory; `alias::run` keeps its signature, uncolored,
  for `install`).
- Behaviour (digest sections 3.3 to 3.5): a row is `ALIAS ARROW VERSION` when
  the destination equals the version, otherwise `ALIAS ARROW DEST (ARROW
  VERSION)`. With colors on, alias, destination and version take: the current
  color when the version is the one in use, else the installed color when it is
  installed, else the not-installed color for `N/A` and `∞`, else no color
  (`system`); an alias of the `lts/` directory takes the LTS color for its name,
  a destination that starts with `lts/` takes it too; the `(default)` marker
  (the default-role color) is only for the built-in `node`, `stable`, `unstable`
  and `iojs` that no file overrides. With colors off the version gets `*`
  unless it is `N/A` or `∞`. In color mode each block of the list is sorted on
  the RENDERED bytes (so rows group by color code first: `myalias` before
  `default`, `unstable` first), in plain mode by name; `nvm ls` prints the same
  alias section and the invalid-color warning once for the whole command. `nvm
  alias <name> <target>` prints one row (the alias name is never LTS-colored)
  with `detect_or_forced`: an exported `NVM_HAS_COLORS=1` colors it even on a
  pipe and over `--no-colors`; the warning `! WARNING: Version '99' does not
  exist.` comes first, then the color warning.
- [ ] **Step 1: Write the failing tests**

Delete `src/commands/alias.rs` (`git rm src/commands/alias.rs`).

Create `src/commands/alias/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;
```

Create `src/commands/alias/tests.rs`:

```rust
use std::path::Path;

use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem};
use crate::ports::FileSystem;

fn installed() -> FakeFileSystem {
    FakeFileSystem::default().with_file("/n/versions/node/v20.1.0/bin/node", "")
}

fn alias(fs: &FakeFileSystem, name: &str, target: &str) -> Result<Output, CliError> {
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    run(&Context::new(fs, &env), name, target)
}

fn alias_with_system_node(name: &str, target: &str) -> Result<Output, CliError> {
    let fs = installed()
        .with_file("/usr/bin/node", "")
        .with_file("/n/alias/default", "system");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/usr/bin");
    run(&Context::new(&fs, &env), name, target)
}

fn stored(fs: &FakeFileSystem, name: &str) -> Option<String> {
    fs.read_to_string(&Path::new("/n/alias").join(name)).ok()
}

#[test]
fn writes_the_target_as_the_alias_file() {
    let fs = installed();
    alias(&fs, "test", "v20.1.0").unwrap();
    assert_eq!(stored(&fs, "test"), Some("v20.1.0\n".to_owned()));
}

#[test]
fn an_exact_installed_target_prints_one_arrow() {
    let output = alias(&installed(), "test", "v20.1.0").unwrap();
    assert_eq!(output, Output::stdout("test -> v20.1.0 *"));
}

#[test]
fn a_pattern_target_shows_what_it_resolves_to() {
    let output = alias(&installed(), "work", "20").unwrap();
    assert_eq!(output, Output::stdout("work -> 20 (-> v20.1.0 *)"));
}

#[test]
fn a_target_that_is_not_installed_warns_but_still_creates_the_alias() {
    let fs = FakeFileSystem::default();
    let output = alias(&fs, "test", "v0.1.2").unwrap();
    assert_eq!(output.stdout, "test -> v0.1.2 (-> N/A)");
    assert_eq!(output.stderr, "! WARNING: Version 'v0.1.2' does not exist.");
    assert_eq!(stored(&fs, "test"), Some("v0.1.2\n".to_owned()));
}

#[test]
fn system_with_a_system_node_is_a_valid_target_without_a_warning() {
    let output = alias_with_system_node("default", "system").unwrap();
    assert_eq!(output, Output::stdout("default -> system *"));
}

#[test]
fn an_alias_to_an_alias_of_system_shows_system() {
    let output = alias_with_system_node("foo", "default").unwrap();
    assert_eq!(output, Output::stdout("foo -> default (-> system *)"));
}

#[test]
fn system_without_a_system_node_warns_like_any_missing_target() {
    let output = alias(&FakeFileSystem::default(), "default", "system").unwrap();
    assert_eq!(output.stdout, "default -> system (-> N/A)");
    assert_eq!(output.stderr, "! WARNING: Version 'system' does not exist.");
}

#[test]
fn a_target_whose_chain_loops_shows_infinity_without_a_warning() {
    let fs = installed()
        .with_file("/n/alias/x", "y")
        .with_file("/n/alias/y", "x");
    let output = alias(&fs, "z", "x").unwrap();
    assert_eq!(output, Output::stdout("z -> x (-> ∞)"));
}

#[test]
fn an_existing_alias_is_overwritten() {
    let fs = installed().with_file("/n/alias/work", "v18\n");
    alias(&fs, "work", "20").unwrap();
    assert_eq!(stored(&fs, "work"), Some("20\n".to_owned()));
}

#[test]
fn an_empty_target_deletes_the_alias() {
    let fs = installed().with_file("/n/alias/work", "v18\n");
    let output = alias(&fs, "work", "").unwrap();
    assert!(output.stdout.starts_with("Deleted alias work"));
    assert_eq!(stored(&fs, "work"), None);
}

#[test]
fn unsupported_names_are_rejected_and_nothing_is_written() {
    let cases = [
        (
            "a#b",
            "Aliases with a comment delimiter (#) are not supported.",
        ),
        ("lts/iron", "Aliases in subdirectories are not supported."),
        ("..", "invalid alias name: .."),
        (".", "invalid alias name: ."),
        ("", "invalid alias name: "),
    ];
    for (name, message) in cases {
        let fs = installed();
        let error = alias(&fs, name, "v20.1.0").unwrap_err();
        assert_eq!(error.to_string(), message);
        assert_eq!(fs.read_dir(Path::new("/n/alias")).ok(), None);
    }
}
```

Apply to `src/commands/aliases/tests.rs`:

```diff
--- a/src/commands/aliases/tests.rs
+++ b/src/commands/aliases/tests.rs
@@ -20,7 +20,7 @@
     let env = FakeEnv::default()
         .with_var("NVM_DIR", "/n")
         .with_var("PATH", path);
-    list(&Context::new(fs, &env), prefix).unwrap()
+    list(&Context::new(fs, &env), prefix, false).unwrap()
 }
 
 fn list_with(fs: &FakeFileSystem, prefix: Option<&str>) -> Output {
```

Delete `src/domain/alias_format.rs` (`git rm src/domain/alias_format.rs`).

Create `src/domain/alias_format/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_that_is_the_version_prints_one_arrow() {
        let line = format_line("lts/iron", "v20.10.0", "v20.10.0", true, false);
        assert_eq!(line, "lts/iron -> v20.10.0 *");
    }

    #[test]
    fn a_target_that_resolves_shows_the_version_after_a_second_arrow() {
        let line = format_line("work", "v18", "v18.9.0", true, false);
        assert_eq!(line, "work -> v18 (-> v18.9.0 *)");
    }

    #[test]
    fn unresolved_targets_carry_no_star() {
        assert_eq!(
            format_line("old", "v16", "N/A", false, false),
            "old -> v16 (-> N/A)"
        );
        assert_eq!(format_line("a", "b", "∞", false, false), "a -> b (-> ∞)");
    }

    #[test]
    fn implicit_aliases_end_with_default() {
        let line = format_line("node", "stable", "v20.10.0", true, true);
        assert_eq!(line, "node -> stable (-> v20.10.0 *) (default)");
        let missing = format_line("unstable", "N/A", "N/A", false, true);
        assert_eq!(missing, "unstable -> N/A (default)");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find struct `AliasRow`",
"cannot find function `paint_line`" and "cannot find struct `AliasPainter`".

- [ ] **Step 3: Write the implementation**

Create `src/commands/alias/colored_tests.rs`:

```rust
//! `nvm alias <name> <target>` against the examples of the digest (3.5, 4.1,
//! 4.3, 4.4): one colored row, the alias name never in the LTS color.

use crate::commands::Output;
use crate::commands::aliases::fixture::{GOLDEN, RGBCM, Setup, fixture};

const WARNING: &str = "! WARNING: Version '99' does not exist.";

#[test]
fn creating_myalias_prints_its_golden_row() {
    assert_eq!(
        Setup::tty().alias(&["myalias", "20"]),
        Output::stdout(GOLDEN[0])
    );
    let rgbcm = Setup::tty().var("NVM_COLORS", "rgbcm");
    assert_eq!(rgbcm.alias(&["myalias", "20"]), Output::stdout(RGBCM[1]));
}

#[test]
fn an_unknown_target_is_not_installed_and_warns() {
    let output = Setup::tty().alias(&["bar", "99"]);
    let row = "\x1b[0;31mbar\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31m99\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;31mN/A\x1b[0m)";
    assert_eq!(output, Output::stdout(row).with_stderr(WARNING));
    let plain = Setup::tty().alias(&["bar", "99", "--no-colors"]);
    assert_eq!(
        plain,
        Output::stdout("bar -> 99 (-> N/A)").with_stderr(WARNING)
    );
}

#[test]
fn a_system_target_not_in_use_is_plain_but_for_the_arrow() {
    let output = Setup::tty().alias(&["sys", "system"]);
    assert_eq!(output, Output::stdout("sys \x1b[0;90m->\x1b[0m system"));
    let plain = Setup::pipe().alias(&["sys", "system"]);
    assert_eq!(plain, Output::stdout("sys -> system *"));
}

#[test]
fn a_system_target_in_use_is_current() {
    let output = Setup::tty().in_use("/sys").alias(&["sys", "system"]);
    let row = "\x1b[0;32msys\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;32msystem\x1b[0m";
    assert_eq!(output, Output::stdout(row));
}

#[test]
fn an_lts_target_takes_the_lts_color_and_the_name_does_not() {
    let output = Setup::tty().alias(&["baz", "lts/iron"]);
    let row = "\x1b[0;32mbaz\x1b[0m \x1b[0;90m->\x1b[0m \x1b[1;33mlts/iron\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;32mv20.11.1\x1b[0m)";
    assert_eq!(output, Output::stdout(row));
}

#[test]
fn a_circular_target_is_infinite() {
    let fs = fixture()
        .with_file("/n/alias/a", "b\n")
        .with_file("/n/alias/b", "a\n");
    let output = Setup::tty()
        .run(&fs, |context| {
            super::run_with_colors(context, "a", "b", false)
        })
        .unwrap();
    let row = "\x1b[0;31ma\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31mb\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;31m∞\x1b[0m)";
    assert_eq!(output, Output::stdout(row));
}

#[test]
fn an_exported_nvm_has_colors_forces_the_colors() {
    let forced = Setup::pipe().var("NVM_HAS_COLORS", "1");
    assert_eq!(forced.alias(&["myalias", "20"]), Output::stdout(GOLDEN[0]));
    assert_eq!(
        forced.alias(&["--no-colors", "myalias", "20"]),
        Output::stdout(GOLDEN[0])
    );
    let other = Setup::pipe().var("NVM_HAS_COLORS", "0");
    assert_eq!(
        other.alias(&["myalias", "20"]),
        Output::stdout("myalias -> 20 (-> v20.11.1 *)")
    );
}

#[test]
fn an_invalid_setting_warns_once_after_the_missing_version_warning() {
    let output = Setup::tty()
        .var("NVM_COLORS", "zzzzz")
        .alias(&["bar", "99"]);
    let row = "bar \x1b[0;90m->\x1b[0m 99 (\x1b[0;90m->\x1b[0m N/A)";
    let stderr = format!("{WARNING}\nInvalid color code: z");
    assert_eq!(output, Output::stdout(row).with_stderr(stderr));
}
```

Insert above the `#[cfg(test)]` line of `src/commands/alias/mod.rs`:

```rust
//! `nvm alias <name> <target>`: create an alias file.
//!
//! Listing aliases (`nvm alias` without a target) lives in
//! [`crate::commands::aliases`], which shares its painter.

use crate::commands::aliases::AliasPainter;
use crate::commands::color_policy::{self, ColorPolicy};
use crate::commands::resolve::{Shown, shown};
use crate::commands::{Output, unalias};
use crate::context::Context;
use crate::domain::alias_format::colored::AliasKind;
use crate::error::CliError;

/// `nvm alias <name> <target>` from the command line: the row is colored
/// unless `no_colors` (`--no-colors`) or stdout cannot show colors, and an
/// exported `NVM_HAS_COLORS=1` colors it whatever the rest says.
///
/// # Errors
/// As [`run`].
pub fn run_with_colors(
    context: &Context<'_>,
    name: &str,
    target: &str,
    no_colors: bool,
) -> Result<Output, CliError> {
    let policy = color_policy::detect_or_forced(context, no_colors);
    create(context, name, target, policy)
}

/// The plain `nvm alias <name> <target>` that `nvm install` runs.
///
/// # Errors
/// - [`CliError::InvalidArgument`] for a name that is empty, `.`, `..`, or
///   contains `#` or `/`.
/// - [`CliError::Io`] when the alias directory cannot be created or the alias
///   file cannot be written.
/// - An empty target deletes the alias instead, with the errors of
///   [`unalias::run`].
///
/// A target that is not installed is only a warning on stderr.
pub fn run(context: &Context<'_>, name: &str, target: &str) -> Result<Output, CliError> {
    create(context, name, target, ColorPolicy::off())
}

/// The version is resolved before the file is written, so `nvm alias a a`
/// shows `N/A`, not a loop. The `! WARNING` comes before the
/// `Invalid color code` line, as nvm.sh prints them.
fn create(
    context: &Context<'_>,
    name: &str,
    target: &str,
    policy: ColorPolicy,
) -> Result<Output, CliError> {
    if target.is_empty() {
        return unalias::run(context, &[name.to_owned()]);
    }
    validate_name(name)?;
    let version = shown(context, target)?;
    let alias_dir = context.alias_dir()?;
    context
        .fs
        .create_dir_all(&alias_dir)
        .map_err(|source| CliError::io(&alias_dir, source))?;
    let alias_file = alias_dir.join(name);
    context
        .fs
        .write_file(&alias_file, &format!("{target}\n"))
        .map_err(|source| CliError::io(&alias_file, source))?;
    let painter = AliasPainter::new(context, policy)?;
    let line = painter.line(name, target, &version, AliasKind::File);
    let mut warnings = Vec::new();
    if version == Shown::NotAvailable {
        warnings.push(format!("! WARNING: Version '{target}' does not exist."));
    }
    warnings.extend(painter.warning().map(str::to_owned));
    Ok(Output::stdout(line).with_stderr(warnings.join("\n")))
}

fn validate_name(name: &str) -> Result<(), CliError> {
    let message = if name.contains('#') {
        "Aliases with a comment delimiter (#) are not supported.".to_owned()
    } else if name.contains('/') {
        "Aliases in subdirectories are not supported.".to_owned()
    } else if matches!(name, "" | "." | "..") {
        format!("invalid alias name: {name}")
    } else {
        return Ok(());
    };
    Err(CliError::InvalidArgument(message))
}

#[cfg(test)]
mod colored_tests;
```

Create `src/commands/aliases/colored_tests.rs`:

```rust
//! `nvm alias` (list) against the alias blocks of the golden captures of the
//! digest: 4.1 (pty, default colors), 4.2 (plain), 4.3 (`rgbcm`), 4.4.

use super::fixture::{GOLDEN, PLAIN, RGBCM, Setup, fixture};
use super::*;

#[test]
fn a_terminal_gets_the_golden_rows_sorted_on_their_color_codes() {
    let output = Setup::tty().alias(&[]);
    assert_eq!(output, Output::stdout(GOLDEN.join("\n")));
}

#[test]
fn a_pipe_or_no_colors_anywhere_gives_the_plain_rows_sorted_by_name() {
    let expected = Output::stdout(PLAIN.join("\n"));
    assert_eq!(Setup::pipe().alias(&[]), expected);
    assert_eq!(Setup::tty().alias(&["--no-colors"]), expected);
    assert_eq!(Setup::tty().alias(&["--", "--no-colors"]), expected);
}

#[test]
fn nvm_colors_picks_the_role_colors() {
    let output = Setup::tty().var("NVM_COLORS", "rgbcm").alias(&[]);
    assert_eq!(output, Output::stdout(RGBCM.join("\n")));
}

#[test]
fn nvm_no_colors_and_nvm_has_colors_in_the_environment_do_not_change_the_list() {
    let shadowed = Setup::tty().var("NVM_NO_COLORS", "--no-colors").alias(&[]);
    assert_eq!(shadowed, Output::stdout(GOLDEN.join("\n")));
    let forced = Setup::pipe().var("NVM_HAS_COLORS", "1").alias(&[]);
    assert_eq!(forced, Output::stdout(PLAIN.join("\n")));
}

#[test]
fn a_current_io_js_colors_the_iojs_row_as_current() {
    let output = Setup::tty()
        .in_use("/n/versions/io.js/v3.3.1/bin")
        .alias(&["iojs"]);
    let expected = "\x1b[0;32miojs\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;32miojs-v3.3\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;32miojs-v3.3.1\x1b[0m) \x1b[0;37m(default)\x1b[0m";
    assert_eq!(output, Output::stdout(expected));
}

#[test]
fn black_turns_the_lts_names_bold_red() {
    let output = Setup::tty().var("NVM_COLORS", "kKkKk").alias(&[]);
    let lines: Vec<&str> = output.stdout.lines().collect();
    assert!(lines[2].ends_with(" \x1b[0;30m(default)\x1b[0m"));
    assert!(lines[8].starts_with("\x1b[1;31mlts/iron\x1b[0m "));
}

#[test]
fn a_prefix_is_colored_too() {
    let output = Setup::tty().alias(&["my"]);
    assert_eq!(output, Output::stdout(GOLDEN[0]));
}

#[test]
fn an_lts_name_prints_the_raw_target_without_color() {
    assert_eq!(
        Setup::tty().alias(&["lts/iron"]),
        Output::stdout("v20.11.1")
    );
}

#[test]
fn no_match_prints_nothing_not_even_the_invalid_color_warning() {
    let output = Setup::tty().var("NVM_COLORS", "zzzzz").alias(&["nope"]);
    assert_eq!(output, Output::default());
}

#[test]
fn an_invalid_setting_leaves_the_rows_plain_with_gray_arrows_and_warns_once() {
    let output = Setup::tty().var("NVM_COLORS", "zzzzz").alias(&["default"]);
    let row = "default \x1b[0;90m->\x1b[0m 18 (\x1b[0;90m->\x1b[0m v18.20.4)";
    let expected = Output::stdout(row).with_stderr("Invalid color code: z");
    assert_eq!(output, expected);
    let plain = Setup::pipe().var("NVM_COLORS", "zzzzz").alias(&[]);
    assert_eq!(plain.stdout, PLAIN.join("\n"));
    assert_eq!(plain.stderr, "Invalid color code: z");
}

#[test]
fn other_options_are_still_exit_55() {
    let error = Setup::tty()
        .run(&fixture(), |context| {
            run(context, &["--no-colors".to_owned(), "--x".to_owned()])
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "Unsupported option \"--x\".");
    assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption);
}
```

Create `src/commands/aliases/fixture.rs`:

```rust
//! Test-only: the fixture of the golden captures of the digest (section 4):
//! node v18.20.4, v20.11.1 (in use), v22.3.0, io.js v3.3.1, a system node
//! v16.0.0, aliases `default -> 18`, `myalias -> 20`, `lts/* -> lts/iron`,
//! `lts/hydrogen -> v18.20.4`, `lts/iron -> v20.11.1`, and a `tput` that
//! knows `xterm-256color`.

use crate::commands::Output;
use crate::context::Context;
use crate::error::CliError;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess, FakeTerminal};

const TPUT: &str = "/usr/bin/tput";
pub const IN_USE: &str = "/n/versions/node/v20.11.1/bin";

pub fn fixture() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/n/versions/node/v18.20.4/bin/node", "")
        .with_file("/n/versions/node/v20.11.1/bin/node", "")
        .with_file("/n/versions/node/v22.3.0/bin/node", "")
        .with_file("/n/versions/io.js/v3.3.1/bin/node", "")
        .with_file("/n/alias/default", "18\n")
        .with_file("/n/alias/myalias", "20\n")
        .with_file("/n/alias/lts/*", "lts/iron\n")
        .with_file("/n/alias/lts/hydrogen", "v18.20.4\n")
        .with_file("/n/alias/lts/iron", "v20.11.1\n")
        .with_file("/sys/node", "")
        .with_file(TPUT, "")
}

/// Where stdout goes, which `node` is in use and the extra variables.
pub struct Setup {
    pub in_use: &'static str,
    pub terminal: bool,
    pub variables: Vec<(&'static str, &'static str)>,
}

impl Setup {
    pub fn tty() -> Self {
        Self {
            in_use: IN_USE,
            terminal: true,
            variables: Vec::new(),
        }
    }

    pub fn pipe() -> Self {
        Self {
            terminal: false,
            ..Self::tty()
        }
    }

    pub fn in_use(self, in_use: &'static str) -> Self {
        Self { in_use, ..self }
    }

    pub fn var(mut self, key: &'static str, value: &'static str) -> Self {
        self.variables.push((key, value));
        self
    }

    pub fn run(
        &self,
        fs: &FakeFileSystem,
        command: impl FnOnce(&Context<'_>) -> Result<Output, CliError>,
    ) -> Result<Output, CliError> {
        let process = FakeProcess::default()
            .with_output("/sys/node", "v16.0.0\n")
            .with_run(TPUT, "-T xterm-256color colors", true, "256\n")
            .with_run(TPUT, "-T xterm-256color sitm", true, "");
        let path = format!("{}:/sys:/usr/bin", self.in_use);
        let mut env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", &path)
            .with_var("TERM", "xterm-256color");
        for (key, value) in &self.variables {
            env = env.with_var(key, value);
        }
        let terminal = if self.terminal {
            FakeTerminal::terminal()
        } else {
            FakeTerminal::pipe()
        };
        let context = Context::new(fs, &env)
            .with_process(&process)
            .with_terminal(&terminal);
        command(&context)
    }

    /// `nvm alias <args>` on the fixture.
    pub fn alias(&self, args: &[&str]) -> Output {
        let args: Vec<String> = args.iter().map(ToString::to_string).collect();
        self.run(&fixture(), |context| super::run(context, &args))
            .unwrap()
    }
}

/// The alias block of 4.1, byte for byte.
pub const GOLDEN: [&str; 9] = [
    "\x1b[0;32mmyalias\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;32m20\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;32mv20.11.1\x1b[0m)",
    "\x1b[0;34mdefault\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34m18\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;34mv18.20.4\x1b[0m)",
    "\x1b[0;31munstable\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31mN/A\x1b[0m \x1b[0;37m(default)\x1b[0m",
    "\x1b[0;34miojs\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34miojs-v3.3\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;34miojs-v3.3.1\x1b[0m) \x1b[0;37m(default)\x1b[0m",
    "\x1b[0;34mnode\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34mstable\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;34mv22.3.0\x1b[0m) \x1b[0;37m(default)\x1b[0m",
    "\x1b[0;34mstable\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34m22.3\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;34mv22.3.0\x1b[0m) \x1b[0;37m(default)\x1b[0m",
    "\x1b[1;33mlts/*\x1b[0m \x1b[0;90m->\x1b[0m \x1b[1;33mlts/iron\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;32mv20.11.1\x1b[0m)",
    "\x1b[1;33mlts/hydrogen\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34mv18.20.4\x1b[0m",
    "\x1b[1;33mlts/iron\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;32mv20.11.1\x1b[0m",
];

/// The alias block of 4.2 (pipe, `--no-colors`), byte for byte.
pub const PLAIN: [&str; 9] = [
    "default -> 18 (-> v18.20.4 *)",
    "myalias -> 20 (-> v20.11.1 *)",
    "iojs -> iojs-v3.3 (-> iojs-v3.3.1 *) (default)",
    "node -> stable (-> v22.3.0 *) (default)",
    "stable -> 22.3 (-> v22.3.0 *) (default)",
    "unstable -> N/A (default)",
    "lts/* -> lts/iron (-> v20.11.1 *)",
    "lts/hydrogen -> v18.20.4 *",
    "lts/iron -> v20.11.1 *",
];

/// The alias block of 4.3 (`NVM_COLORS=rgbcm`), byte for byte.
pub const RGBCM: [&str; 9] = [
    "\x1b[0;31mdefault\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31m18\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;31mv18.20.4\x1b[0m)",
    "\x1b[0;34mmyalias\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34m20\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;34mv20.11.1\x1b[0m)",
    "\x1b[0;31miojs\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31miojs-v3.3\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;31miojs-v3.3.1\x1b[0m) \x1b[0;35m(default)\x1b[0m",
    "\x1b[0;31mnode\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31mstable\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;31mv22.3.0\x1b[0m) \x1b[0;35m(default)\x1b[0m",
    "\x1b[0;31mstable\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31m22.3\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;31mv22.3.0\x1b[0m) \x1b[0;35m(default)\x1b[0m",
    "\x1b[0;36munstable\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;36mN/A\x1b[0m \x1b[0;35m(default)\x1b[0m",
    "\x1b[1;32mlts/*\x1b[0m \x1b[0;90m->\x1b[0m \x1b[1;32mlts/iron\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;34mv20.11.1\x1b[0m)",
    "\x1b[1;32mlts/hydrogen\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31mv18.20.4\x1b[0m",
    "\x1b[1;32mlts/iron\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;34mv20.11.1\x1b[0m",
];
```

Apply to `src/commands/aliases/mod.rs` (above the test module):

```diff
--- a/src/commands/aliases/mod.rs
+++ b/src/commands/aliases/mod.rs
@@ -1,23 +1,30 @@
 //! `nvm alias` and `nvm alias <prefix>`: list aliases.
 //!
-//! Three groups, each sorted: the alias files, the implicit aliases that have
-//! no file (`iojs`, `node`, `stable`, `unstable`), and the `lts/*` files.
-//! Output is always plain, as `nvm.sh` prints when stdout is not a terminal.
+//! Three groups, each sorted on the bytes it prints like `command sort`: the
+//! alias files, the implicit aliases that have no file (`iojs`, `node`,
+//! `stable`, `unstable`), and the `lts/*` files. With colors on, the rows of a
+//! group therefore come in the order of their leading color code first
+//! (nvm.sh quirk, kept).
 
 use std::path::Path;
 
+use crate::commands::color_policy;
 use crate::commands::resolve::shown;
 use crate::commands::{Output, alias, unalias};
 use crate::context::Context;
 use crate::domain::alias::AliasStore;
-use crate::domain::alias_format::format_line;
+use crate::domain::alias_format::colored::AliasKind;
 use crate::domain::implicit::{IMPLICIT_ALIASES, destination};
 use crate::error::{CliError, NvmExitCode};
+
+mod painter;
+pub use painter::AliasPainter;
 
 #[derive(Default)]
 struct Words {
     name: Option<String>,
     target: Option<String>,
+    no_colors: bool,
 }
 
 /// `nvm alias [--no-colors] [name [target]]`: the first two words are the
@@ -26,7 +33,8 @@
     let mut words = Words::default();
     for arg in args {
         match arg.as_str() {
-            "--" | "--no-colors" => {}
+            "--" => {}
+            "--no-colors" => words.no_colors = true,
             option if option.starts_with("--") => {
                 let message = format!("Unsupported option \"{option}\".");
                 return Err(CliError::Unsupported(message));
@@ -47,36 +55,64 @@
 /// - [`CliError::Unsupported`] for an unknown `--option`.
 /// - Whatever the listing, creation or deletion fails with.
 pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
-    let Words { name, target } = parse_words(args)?;
+    let Words {
+        name,
+        target,
+        no_colors,
+    } = parse_words(args)?;
     match (name, target) {
         (Some(name), Some(target)) if target.is_empty() => unalias::run(context, &[name]),
         (Some(name), _) if name.contains('#') => {
             let message = "Aliases with a comment delimiter (#) are not supported.";
             Err(CliError::InvalidArgument(message.to_owned()))
         }
-        (Some(name), Some(target)) => alias::run(context, &name, &target),
-        (Some(name), None) => list(context, Some(&name)),
-        (None, _) => list(context, None),
+        (Some(name), Some(target)) => alias::run_with_colors(context, &name, &target, no_colors),
+        (Some(name), None) => list(context, Some(&name), no_colors),
+        (None, _) => list(context, None, no_colors),
     }
 }
 
+/// `nvm alias [prefix]`, colored unless `no_colors` (`--no-colors`) or
+/// stdout cannot show colors. An invalid `NVM_COLORS` adds its single
+/// warning on stderr when a row is printed.
+///
 /// # Errors
 /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
-pub fn list(context: &Context<'_>, prefix: Option<&str>) -> Result<Output, CliError> {
+pub fn list(
+    context: &Context<'_>,
+    prefix: Option<&str>,
+    no_colors: bool,
+) -> Result<Output, CliError> {
     let prefix = prefix.unwrap_or_default();
     if prefix.starts_with("lts/") {
         return lts_target(context, prefix);
     }
+    let painter = AliasPainter::new(context, color_policy::detect(context, no_colors))?;
+    let lines = rows(context, &painter, prefix)?;
+    let output = Output::stdout(lines.join("\n"));
+    Ok(match painter.warning() {
+        Some(warning) if !lines.is_empty() => output.with_stderr(warning),
+        _ => output,
+    })
+}
+
+/// The three groups of rows.
+fn rows(
+    context: &Context<'_>,
+    painter: &AliasPainter,
+    prefix: &str,
+) -> Result<Vec<String>, CliError> {
     let alias_dir = context.alias_dir()?;
-    let mut lines = directory_lines(context, &alias_dir, "", prefix)?;
-    lines.extend(implicit_lines(context, &alias_dir, prefix)?);
+    let mut lines = directory_lines(context, painter, &alias_dir, AliasKind::File, prefix)?;
+    lines.extend(implicit_lines(context, painter, &alias_dir, prefix)?);
     lines.extend(directory_lines(
         context,
+        painter,
         &alias_dir.join("lts"),
-        "lts/",
+        AliasKind::Lts,
         prefix,
     )?);
-    Ok(Output::stdout(lines.join("\n")))
+    Ok(lines)
 }
 
 /// `nvm alias lts/iron` prints the alias file's target as is.
@@ -91,30 +127,27 @@
 
 fn line(
     context: &Context<'_>,
+    painter: &AliasPainter,
     name: &str,
     target: &str,
-    default: bool,
+    kind: AliasKind,
 ) -> Result<String, CliError> {
     let version = shown(context, target)?;
-    let text = version.to_string();
-    Ok(format_line(
-        name,
-        target,
-        &text,
-        version.is_available(),
-        default,
-    ))
+    Ok(painter.line(name, target, &version, kind))
 }
 
 /// The alias files directly inside `directory` whose name starts with
-/// `prefix`, named `label` + the file name. Hidden files only match a hidden
-/// prefix, like a shell glob.
+/// `prefix`; those of `alias/lts` ([`AliasKind::Lts`]) are named `lts/` +
+/// the file name. Hidden
+/// files only match a hidden prefix, like a shell glob.
 fn directory_lines(
     context: &Context<'_>,
+    painter: &AliasPainter,
     directory: &Path,
-    label: &str,
+    kind: AliasKind,
     prefix: &str,
 ) -> Result<Vec<String>, CliError> {
+    let label = if kind == AliasKind::Lts { "lts/" } else { "" };
     let store = context.alias_store()?;
     let mut lines = Vec::new();
     for entry in context.fs.read_dir(directory).unwrap_or_default() {
@@ -124,7 +157,7 @@
         }
         let name = format!("{label}{}", entry.name);
         if let Some(target) = store.target(&name) {
-            lines.push(line(context, &name, &target, false)?);
+            lines.push(line(context, painter, &name, &target, kind)?);
         }
     }
     lines.sort();
@@ -135,6 +168,7 @@
 /// prefix exactly (a prefix does not select among them).
 fn implicit_lines(
     context: &Context<'_>,
+    painter: &AliasPainter,
     alias_dir: &Path,
     prefix: &str,
 ) -> Result<Vec<String>, CliError> {
@@ -146,10 +180,14 @@
             continue;
         }
         if let Some(target) = destination(&installed, name) {
-            lines.push(line(context, name, &target, true)?);
+            lines.push(line(context, painter, name, &target, AliasKind::Implicit)?);
         }
     }
     lines.sort();
     Ok(lines)
 }
 
+#[cfg(test)]
+mod colored_tests;
+#[cfg(test)]
+pub mod fixture;
```

Create `src/commands/aliases/painter.rs`:

```rust
//! What the alias rows of `nvm alias` and `nvm ls` are painted with: the
//! version in use, the palette of `NVM_COLORS` and whether colors are on.

use crate::commands::color_policy::{self, ColorPolicy};
use crate::commands::current;
use crate::commands::resolve::Shown;
use crate::context::Context;
use crate::domain::alias_format::colored::{AliasKind, AliasRow, VersionState, paint_line};
use crate::domain::colors::Palette;
use crate::error::CliError;

/// Built once per command: `nvm_ls_current` and `nvm_get_colors` are the
/// same for every row.
#[derive(Debug, Clone)]
pub struct AliasPainter {
    current: String,
    palette: Palette,
    colors: bool,
    warning: Option<String>,
}

impl AliasPainter {
    /// # Errors
    /// Returns [`CliError::NvmDirUnresolved`] when `$NVM_DIR` cannot be found.
    pub fn new(context: &Context<'_>, policy: ColorPolicy) -> Result<Self, CliError> {
        let (palette, warning) = color_policy::palette(context);
        Ok(Self {
            current: current::detect(context)?.to_string(),
            palette,
            colors: policy.enabled,
            warning,
        })
    }

    /// The row of the alias `name` whose `target` resolves to `shown`.
    #[must_use]
    pub fn line(&self, name: &str, target: &str, shown: &Shown, kind: AliasKind) -> String {
        let version = shown.to_string();
        let row = AliasRow {
            alias: name,
            target,
            version: &version,
            state: self.state(shown, &version),
            kind,
        };
        paint_line(&row, &self.palette, self.colors)
    }

    /// The single `Invalid color code: <x>` line, when `NVM_COLORS` has an
    /// invalid or missing role.
    #[must_use]
    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    /// The version in use first (`system` too), then installed, then `N/A`
    /// or `∞`; `system` not in use is plain (`nvm_is_version_installed`
    /// fails for it).
    fn state(&self, shown: &Shown, version: &str) -> VersionState {
        if version == self.current {
            return VersionState::Current;
        }
        match shown {
            Shown::Version(_) => VersionState::Installed,
            Shown::System => VersionState::Other,
            Shown::NotAvailable | Shown::Infinite => VersionState::Missing,
        }
    }
}
```

Create `src/commands/ls/alias_section_tests.rs`:

```rust
//! `nvm ls` with its alias section, against the golden captures of the
//! digest (4.1, 4.2, 4.3): the same colors as `nvm alias`, one warning.

use super::*;
use crate::commands::aliases::fixture::{GOLDEN, PLAIN, RGBCM, Setup, fixture};

fn ls(setup: &Setup, args: &[&str]) -> Output {
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    setup
        .run(&fixture(), |context| run_command(context, &args))
        .unwrap()
}

fn joined(versions: &[&str], aliases: &[&str]) -> String {
    [versions, aliases].concat().join("\n")
}

#[test]
fn a_terminal_gets_the_golden_listing() {
    let versions = [
        "\x1b[0;34m    iojs-v3.3.1\x1b[0m",
        "\x1b[0;34m       v18.20.4\x1b[0m",
        "\x1b[0;32m->     v20.11.1\x1b[0m",
        "\x1b[0;34m        v22.3.0\x1b[0m",
        "\x1b[0;33m         system\x1b[0m (\x1b[0;33m-> v16.0.0\x1b[0m)",
    ];
    let expected = Output::stdout(joined(&versions, &GOLDEN));
    assert_eq!(ls(&Setup::tty(), &[]), expected);
}

#[test]
fn a_pipe_or_no_colors_gives_the_plain_listing() {
    let versions = [
        "    iojs-v3.3.1 *",
        "       v18.20.4 *",
        "->     v20.11.1 *",
        "        v22.3.0 *",
        "         system * (-> v16.0.0)",
    ];
    let expected = Output::stdout(joined(&versions, &PLAIN));
    assert_eq!(ls(&Setup::pipe(), &[]), expected);
    assert_eq!(ls(&Setup::tty(), &["--no-colors"]), expected);
    let forced = Setup::pipe().var("NVM_HAS_COLORS", "1");
    assert_eq!(ls(&forced, &[]), expected);
}

#[test]
fn nvm_colors_colors_both_parts() {
    let versions = [
        "\x1b[0;31m    iojs-v3.3.1\x1b[0m",
        "\x1b[0;31m       v18.20.4\x1b[0m",
        "\x1b[0;34m->     v20.11.1\x1b[0m",
        "\x1b[0;31m        v22.3.0\x1b[0m",
        "\x1b[0;32m         system\x1b[0m (\x1b[0;32m-> v16.0.0\x1b[0m)",
    ];
    let output = ls(&Setup::tty().var("NVM_COLORS", "rgbcm"), &[]);
    assert_eq!(output, Output::stdout(joined(&versions, &RGBCM)));
}

#[test]
fn an_invalid_setting_warns_once_for_the_whole_command() {
    let output = ls(&Setup::tty().var("NVM_COLORS", "zzzzz"), &[]);
    assert_eq!(output.stderr, "Invalid color code: z");
    let lines: Vec<&str> = output.stdout.lines().collect();
    assert_eq!(lines[2], "       v20.11.1");
    assert_eq!(
        lines[5],
        "default \x1b[0;90m->\x1b[0m 18 (\x1b[0;90m->\x1b[0m v18.20.4)"
    );
    let plain = ls(&Setup::pipe().var("NVM_COLORS", "rg"), &[]);
    assert_eq!(plain.stderr, "Invalid color code: ");
}
```

Apply to `src/commands/ls/colored_tests.rs` (above the test module):

```diff
--- a/src/commands/ls/colored_tests.rs
+++ b/src/commands/ls/colored_tests.rs
@@ -217,14 +217,3 @@
     let warning = "Invalid color code: z";
     assert_eq!(output, Output::stdout(expected).with_stderr(warning));
 }
-
-#[test]
-fn the_alias_rows_are_left_as_they_are() {
-    let output = Setup::tty().run(&fixture(), &[]);
-    let lines: Vec<&str> = output.stdout.lines().collect();
-    assert_eq!(lines[1], "\x1b[0;34m       v18.20.4\x1b[0m");
-    assert_eq!(lines[5], "default -> 18 (-> v18.20.4 *)");
-    let plain = Setup::tty().run(&fixture(), &["--no-colors"]);
-    assert_eq!(plain.stdout.lines().nth(5), Some(lines[5]));
-    assert_eq!(plain.stdout.lines().nth(2), Some(PLAIN[2]));
-}
```

Apply to `src/commands/ls/mod.rs` (above the test module):

```diff
--- a/src/commands/ls/mod.rs
+++ b/src/commands/ls/mod.rs
@@ -1,7 +1,8 @@
 //! `nvm ls [pattern]`: the installed versions, one row each.
 //!
-//! The version rows are colored like `nvm_print_versions` when stdout can
-//! show colors and `--no-colors` is not given; the alias rows are plain.
+//! The version rows are colored like `nvm_print_versions` and the alias rows
+//! like `nvm_print_formatted_alias` when stdout can show colors and
+//! `--no-colors` is not given.
 //! Unlike `nvm.sh` it does not print a blank row when only a system node
 //! exists, and it keeps both io.js and Node versions that share a number.
 //! For an alias that resolves to nothing (`ls lts/gallium`, `ls unstable`,
@@ -49,12 +50,8 @@
     let options = parse_options(args)?;
     let mut output = run(context, options.pattern.as_deref(), options.no_colors)?;
     if options.pattern.is_none() && !options.no_alias {
-        let alias_args = if options.no_colors {
-            vec!["--no-colors".to_owned()]
-        } else {
-            Vec::new()
-        };
-        let listed = aliases::run(context, &alias_args)?;
+        // The versions part already printed the `Invalid color code` line.
+        let listed = aliases::list(context, None, options.no_colors)?;
         if !listed.stdout.is_empty() {
             output.stdout = format!("{}\n{}", output.stdout, listed.stdout);
         }
@@ -85,12 +82,7 @@
         palette,
         colors: color_policy::detect(context, no_colors).enabled,
     };
-    let rows: Vec<String> = selection
-        .entries
-        .iter()
-        .map(|entry| painter.render(entry))
-        .collect();
-    let mut output = Output::stdout(rows.join("\n"));
+    let mut output = Output::stdout(painter.render_all(&selection.entries));
     if let Some(warning) = warning {
         output = output.with_stderr(warning);
     }
@@ -239,6 +231,11 @@
         paint_row(text, kind, &self.palette, self.colors)
     }
 
+    fn render_all(&self, entries: &[Entry]) -> String {
+        let rows: Vec<String> = entries.iter().map(|entry| self.render(entry)).collect();
+        rows.join("\n")
+    }
+
     fn render(&self, entry: &Entry) -> String {
         match entry {
             Entry::Version(version) => {
@@ -258,4 +255,6 @@
 }
 
 #[cfg(test)]
+mod alias_section_tests;
+#[cfg(test)]
 mod colored_tests;
```

Create `src/domain/alias_format/colored.rs`:

```rust
//! The alias rows as `nvm_print_formatted_alias` paints them: with colors off
//! they are the plain lines of [`format_line`]; with colors on the arrows are
//! gray and the alias, its target and its version take the color of what the
//! version is.

use super::format_line;
use crate::domain::colors::{ARROW, Palette, RESET, Role, wrap};

/// What the resolved version of an alias is, in the order nvm.sh checks it:
/// the version in use, an installed version, nothing (`N/A`) or a loop (`∞`),
/// or anything else (`system` when it is not in use).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionState {
    Current,
    Installed,
    Missing,
    Other,
}

/// Where an alias comes from: a file of the alias directory, one of the
/// implicit aliases without a file (`node`, `stable`, `unstable`, `iojs`,
/// marked `(default)`), or a file of `alias/lts` (its name in the LTS color).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasKind {
    File,
    Implicit,
    Lts,
}

/// One alias row: `alias -> target (-> version)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AliasRow<'a> {
    pub alias: &'a str,
    pub target: &'a str,
    pub version: &'a str,
    pub state: VersionState,
    pub kind: AliasKind,
}

/// The row with colors on or off. A role without a color (an invalid
/// `NVM_COLORS`) leaves its text plain; nvm.sh splices a malformed `\e[`
/// there instead (deviation on purpose).
#[must_use]
pub fn paint_line(row: &AliasRow<'_>, palette: &Palette, colors: bool) -> String {
    if !colors {
        let available = row.state != VersionState::Missing;
        let implicit = row.kind == AliasKind::Implicit;
        return format_line(row.alias, row.target, row.version, available, implicit);
    }
    let line = colored_line(row, palette);
    if row.kind == AliasKind::Implicit {
        format!("{line} {}", wrap(palette.code(Role::Default), "(default)"))
    } else {
        line
    }
}

/// `ALIAS ARROW VERSION` or `ALIAS ARROW DEST (ARROW VERSION)`, colored.
fn colored_line(row: &AliasRow<'_>, palette: &Palette) -> String {
    let role = role_of(row.state).and_then(|role| palette.code(role));
    let lts = palette.lts();
    let lts_or_role = |is_lts: bool| if is_lts { lts.as_deref() } else { role };
    let arrow = format!("{ARROW}->{RESET}");
    let alias = wrap(lts_or_role(row.kind == AliasKind::Lts), row.alias);
    let version = wrap(role, row.version);
    if row.target == row.version {
        return format!("{alias} {arrow} {version}");
    }
    let target = wrap(lts_or_role(is_lts_target(row.target)), row.target);
    format!("{alias} {arrow} {target} ({arrow} {version})")
}

/// The role of the first matching rule; `Other` is plain.
fn role_of(state: VersionState) -> Option<Role> {
    match state {
        VersionState::Current => Some(Role::Current),
        VersionState::Installed => Some(Role::Installed),
        VersionState::Missing => Some(Role::NotInstalled),
        VersionState::Other => None,
    }
}

/// The `[ "_${DEST%/*}" = "_lts" ]` of nvm.sh: what precedes the last `/` is
/// `lts` (a bare `lts` counts too, `%/*` leaves it as is).
fn is_lts_target(target: &str) -> bool {
    let parent = target.rfind('/').map_or(target, |slash| &target[..slash]);
    parent == "lts"
}
```

Create `src/domain/alias_format/colored_tests.rs`:

```rust
//! The alias rows of the golden captures of the digest (4.1, 4.3, 4.4) and
//! the examples of `nvm alias <name> <target>` (3.5).

use super::colored::{AliasKind, AliasRow, VersionState, paint_line};
use crate::domain::colors::Palette;

fn palette(setting: &str) -> Palette {
    Palette::from_setting(Some(setting)).0
}

fn row<'a>(alias: &'a str, target: &'a str, version: &'a str, state: VersionState) -> AliasRow<'a> {
    AliasRow {
        alias,
        target,
        version,
        state,
        kind: AliasKind::File,
    }
}

fn implicit(row: AliasRow<'_>) -> AliasRow<'_> {
    AliasRow {
        kind: AliasKind::Implicit,
        ..row
    }
}

fn lts(row: AliasRow<'_>) -> AliasRow<'_> {
    AliasRow {
        kind: AliasKind::Lts,
        ..row
    }
}

fn on(row: &AliasRow<'_>, setting: &str) -> String {
    paint_line(row, &palette(setting), true)
}

fn off(row: &AliasRow<'_>) -> String {
    paint_line(row, &palette("bygre"), false)
}

const ARROW: &str = "\x1b[0;90m->\x1b[0m";

#[test]
fn the_current_version_colors_alias_target_and_version_green() {
    let myalias = row("myalias", "20", "v20.11.1", VersionState::Current);
    let expected = format!(
        "\x1b[0;32mmyalias\x1b[0m {ARROW} \x1b[0;32m20\x1b[0m ({ARROW} \x1b[0;32mv20.11.1\x1b[0m)"
    );
    assert_eq!(on(&myalias, "bygre"), expected);
}

#[test]
fn an_installed_version_colors_the_row_blue() {
    let default = row("default", "18", "v18.20.4", VersionState::Installed);
    let expected = format!(
        "\x1b[0;34mdefault\x1b[0m {ARROW} \x1b[0;34m18\x1b[0m ({ARROW} \x1b[0;34mv18.20.4\x1b[0m)"
    );
    assert_eq!(on(&default, "bygre"), expected);
}

#[test]
fn implicit_aliases_end_with_the_default_marker() {
    let unstable = implicit(row("unstable", "N/A", "N/A", VersionState::Missing));
    let expected = format!(
        "\x1b[0;31munstable\x1b[0m {ARROW} \x1b[0;31mN/A\x1b[0m \x1b[0;37m(default)\x1b[0m"
    );
    assert_eq!(on(&unstable, "bygre"), expected);
    let iojs = implicit(row(
        "iojs",
        "iojs-v3.3",
        "iojs-v3.3.1",
        VersionState::Installed,
    ));
    let expected = format!(
        "\x1b[0;34miojs\x1b[0m {ARROW} \x1b[0;34miojs-v3.3\x1b[0m ({ARROW} \x1b[0;34miojs-v3.3.1\x1b[0m) \x1b[0;37m(default)\x1b[0m"
    );
    assert_eq!(on(&iojs, "bygre"), expected);
}

#[test]
fn lts_names_and_lts_targets_take_the_lts_color_but_the_version_keeps_its_own() {
    let star = lts(row("lts/*", "lts/iron", "v20.11.1", VersionState::Current));
    let expected = format!(
        "\x1b[1;33mlts/*\x1b[0m {ARROW} \x1b[1;33mlts/iron\x1b[0m ({ARROW} \x1b[0;32mv20.11.1\x1b[0m)"
    );
    assert_eq!(on(&star, "bygre"), expected);
    let hydrogen = lts(row(
        "lts/hydrogen",
        "v18.20.4",
        "v18.20.4",
        VersionState::Installed,
    ));
    let expected = format!("\x1b[1;33mlts/hydrogen\x1b[0m {ARROW} \x1b[0;34mv18.20.4\x1b[0m");
    assert_eq!(on(&hydrogen, "bygre"), expected);
}

#[test]
fn the_roles_follow_the_setting() {
    let default = row("default", "18", "v18.20.4", VersionState::Installed);
    let expected = format!(
        "\x1b[0;31mdefault\x1b[0m {ARROW} \x1b[0;31m18\x1b[0m ({ARROW} \x1b[0;31mv18.20.4\x1b[0m)"
    );
    assert_eq!(on(&default, "rgbcm"), expected);
    let unstable = implicit(row("unstable", "N/A", "N/A", VersionState::Missing));
    let expected = format!(
        "\x1b[0;36munstable\x1b[0m {ARROW} \x1b[0;36mN/A\x1b[0m \x1b[0;35m(default)\x1b[0m"
    );
    assert_eq!(on(&unstable, "rgbcm"), expected);
    let iron = lts(row(
        "lts/iron",
        "v20.11.1",
        "v20.11.1",
        VersionState::Current,
    ));
    let expected = format!("\x1b[1;32mlts/iron\x1b[0m {ARROW} \x1b[0;34mv20.11.1\x1b[0m");
    assert_eq!(on(&iron, "rgbcm"), expected);
}

#[test]
fn black_lts_turns_bold_red_like_the_tr_quirk() {
    let iron = lts(row("lts/iron", "v20.11.1", "v20.11.1", VersionState::Other));
    assert_eq!(
        on(&iron, "kKkKk"),
        format!("\x1b[1;31mlts/iron\x1b[0m {ARROW} v20.11.1")
    );
    let node = implicit(row("node", "stable", "v22.3.0", VersionState::Other));
    assert!(on(&node, "kKkKk").ends_with(" \x1b[0;30m(default)\x1b[0m"));
}

#[test]
fn the_examples_of_creating_an_alias() {
    let unknown = row("bar", "99", "N/A", VersionState::Missing);
    let expected =
        format!("\x1b[0;31mbar\x1b[0m {ARROW} \x1b[0;31m99\x1b[0m ({ARROW} \x1b[0;31mN/A\x1b[0m)");
    assert_eq!(on(&unknown, "bygre"), expected);
    assert_eq!(off(&unknown), "bar -> 99 (-> N/A)");
    let system = row("sys", "system", "system", VersionState::Other);
    assert_eq!(on(&system, "bygre"), format!("sys {ARROW} system"));
    assert_eq!(off(&system), "sys -> system *");
    let baz = row("baz", "lts/iron", "v20.11.1", VersionState::Current);
    let expected = format!(
        "\x1b[0;32mbaz\x1b[0m {ARROW} \x1b[1;33mlts/iron\x1b[0m ({ARROW} \x1b[0;32mv20.11.1\x1b[0m)"
    );
    assert_eq!(on(&baz, "bygre"), expected);
    let circular = row("a", "b", "∞", VersionState::Missing);
    let expected =
        format!("\x1b[0;31ma\x1b[0m {ARROW} \x1b[0;31mb\x1b[0m ({ARROW} \x1b[0;31m∞\x1b[0m)");
    assert_eq!(on(&circular, "bygre"), expected);
    assert_eq!(off(&circular), "a -> b (-> ∞)");
}

#[test]
fn colors_off_gives_the_plain_lines_with_stars() {
    let node = implicit(row("node", "stable", "v22.3.0", VersionState::Installed));
    assert_eq!(off(&node), "node -> stable (-> v22.3.0 *) (default)");
    let iron = lts(row(
        "lts/iron",
        "v20.11.1",
        "v20.11.1",
        VersionState::Current,
    ));
    assert_eq!(off(&iron), "lts/iron -> v20.11.1 *");
    let unstable = implicit(row("unstable", "N/A", "N/A", VersionState::Missing));
    assert_eq!(off(&unstable), "unstable -> N/A (default)");
}

#[test]
fn only_a_target_whose_parent_is_lts_is_lts_colored() {
    let bare = row("x", "lts", "N/A", VersionState::Missing);
    assert!(on(&bare, "bygre").contains(" \x1b[1;33mlts\x1b[0m ("));
    let deep = row("x", "lts/a/b", "N/A", VersionState::Missing);
    assert!(on(&deep, "bygre").contains(" \x1b[0;31mlts/a/b\x1b[0m ("));
}

#[test]
fn a_role_without_a_color_is_plain() {
    let myalias = row("myalias", "20", "v20.11.1", VersionState::Current);
    assert_eq!(
        on(&myalias, "rg"),
        format!("myalias {ARROW} 20 ({ARROW} v20.11.1)")
    );
    let node = implicit(row("node", "stable", "v22.3.0", VersionState::Installed));
    let expected = format!("node {ARROW} stable ({ARROW} v22.3.0) (default)");
    assert_eq!(on(&node, "zzzzz"), expected);
    let iron = lts(row(
        "lts/iron",
        "v20.11.1",
        "v20.11.1",
        VersionState::Installed,
    ));
    assert_eq!(
        on(&iron, "bzgre"),
        format!("lts/iron {ARROW} \x1b[0;34mv20.11.1\x1b[0m")
    );
}
```

Insert above the `#[cfg(test)]` line of `src/domain/alias_format/mod.rs`:

```rust
//! The line `nvm alias` prints for one alias, as `nvm.sh` prints it when stdout
//! is not a terminal (no colors); [`colored`] paints it.

pub mod colored;
#[cfg(test)]
mod colored_tests;

/// `name -> target *` when the target already is the version, else
/// `name -> target (-> version *)`. The `*` marks a version that resolved; the
/// `(default)` suffix marks the implicit aliases.
#[must_use]
pub fn format_line(
    alias: &str,
    target: &str,
    version: &str,
    available: bool,
    default: bool,
) -> String {
    let marker = if available { " *" } else { "" };
    let line = if target == version {
        format!("{alias} -> {version}{marker}")
    } else {
        format!("{alias} -> {target} (-> {version}{marker})")
    };
    if default {
        format!("{line} (default)")
    } else {
        line
    }
}

```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1099 unit tests plus the end-to-end tests.

- [ ] **Step 5: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(alias): color the alias rows like nvm_print_formatted_alias"
```

---

### Task 5: Colored `nvm ls-remote`

**Files:**

- Create: `src/commands/ls_remote/colored_tests.rs`,
  `src/commands/ls_remote/options.rs`, `src/domain/remote_format/cells.rs`,
  `src/domain/remote_format/colored_tests.rs`,
  `src/domain/remote_format/latest.rs`
- Modify: `src/commands/ls_remote/mod.rs`, `src/commands/ls_remote/tests.rs`,
  `src/domain/remote_format/mod.rs`, `src/domain/remote_format/tests.rs`

**Interfaces:**

- Produces: `domain::remote_format::{RemoteColors { palette, italics },
  paint_remote_rows(&FormatInput, Option<RemoteColors>), format_remote_rows}`
  (`format_remote_rows` keeps its signature and is the colors-off path; the
  module is split into `cells` and `latest`); `commands::ls_remote::Options {
  pattern, lts, no_colors }` and `parse_options` in `options.rs`.
- Behaviour (digest section 3.2): version rows as in `ls` (a version that is not
  installed is plain `%15s` in both modes). The annotation columns are `(LTS:
  x)` in the default-role color, `(Latest LTS: x)` in the latest-LTS color, and
  `(Latest: node)` and `(Aliases: a, b)` in the annotation color with the name
  (each alias name separately) in italics `\x1b[3m` .. `\x1b[23m` only when the
  terminal has italics and the latest or aliases data exist (never for `--lts`
  or a pattern). The visible layout (padding, column widths, gaps) is IDENTICAL
  with colors on and off: widths count the visible text only, escapes stripped;
  the padding is two spaces except on an installed row with colors off, where the
  asterisk takes those two columns. The invalid-color warning is printed once on
  every path that prints rows, after any download or LTS warning; an unsupported
  option prints none.
- [ ] **Step 1: Write the failing tests**

Apply to `src/commands/ls_remote/tests.rs`:

```diff
--- a/src/commands/ls_remote/tests.rs
+++ b/src/commands/ls_remote/tests.rs
@@ -175,6 +175,8 @@
     let options = parse_options(&words("--no-colors -- 18")).unwrap();
     assert_eq!(options.pattern.as_deref(), Some("18"));
     assert_eq!(options.lts, None);
+    assert!(options.no_colors);
+    assert!(!parse_options(&words("18")).unwrap().no_colors);
 }
 
 #[test]
```

Apply to `src/domain/remote_format/tests.rs`:

```diff
--- a/src/domain/remote_format/tests.rs
+++ b/src/domain/remote_format/tests.rs
@@ -1,3 +1,4 @@
+use super::latest::is_old_unstable;
 use super::*;
 use crate::domain::fixtures::{iojs_releases, node_releases};
 use crate::domain::remote::{Query, list};
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find struct
`RemoteColors`", "cannot find function `paint_remote_rows`" and "no field
`no_colors` on type `Options`".

- [ ] **Step 3: Write the implementation**

Create `src/commands/ls_remote/colored_tests.rs`:

```rust
//! The colored listing, against the golden `nvm ls-remote` blocks of the
//! digest (fixture: node v18.20.4, v20.11.1, v22.3.0 and io.js v3.3.1
//! installed, `default -> 18`, `myalias -> 20`, the digest mirror).

use super::*;
use crate::domain::fixtures::index_text;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp, FakeProcess, FakeTerminal};

const TPUT: &str = "/usr/bin/tput";
const IN_USE: &str = "/n/versions/node/v20.11.1/bin";

fn mirror() -> FakeHttp {
    let node = index_text(&[
        ("v22.3.0", "-"),
        ("v22.2.0", "-"),
        ("v20.11.1", "Iron"),
        ("v20.11.0", "Iron"),
        ("v18.20.4", "Hydrogen"),
        ("v4.0.0", "-"),
        ("v0.12.18", "-"),
        ("v0.11.16", "-"),
    ]);
    let iojs = index_text(&[("v3.3.1", "-"), ("v3.3.0", "-")]);
    FakeHttp::default()
        .with_body("https://nodejs.org/dist/index.tab", &node)
        .with_body("https://iojs.org/dist/index.tab", &iojs)
}

fn fixture() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/n/versions/node/v18.20.4/bin/node", "")
        .with_file("/n/versions/node/v20.11.1/bin/node", "")
        .with_file("/n/versions/node/v22.3.0/bin/node", "")
        .with_file("/n/versions/io.js/v3.3.1/bin/node", "")
        .with_file("/n/alias/default", "18\n")
        .with_file("/n/alias/myalias", "20\n")
        .with_file("/sys/node", "")
        .with_file(TPUT, "")
}

struct Setup<'a> {
    path: &'a str,
    term: &'a str,
    variables: Vec<(&'a str, &'a str)>,
    terminal: FakeTerminal,
}

impl<'a> Setup<'a> {
    fn tty() -> Self {
        Self {
            path: IN_USE,
            term: "xterm-256color",
            variables: Vec::new(),
            terminal: FakeTerminal::terminal(),
        }
    }

    fn var(mut self, name: &'a str, value: &'a str) -> Self {
        self.variables.push((name, value));
        self
    }

    fn process() -> FakeProcess {
        FakeProcess::default()
            .with_output("/sys/node", "v16.0.0\n")
            .with_run(TPUT, "-T xterm-256color colors", true, "256\n")
            .with_run(TPUT, "-T xterm-256color sitm", true, "")
            .with_run(TPUT, "-T screen colors", true, "8\n")
            .with_run(TPUT, "-T screen sitm", false, "")
    }

    fn run(&self, line: &str) -> Output {
        let (fs, http, process) = (fixture(), mirror(), Self::process());
        let path = format!("{}:/sys:/usr/bin", self.path);
        let mut env = FakeEnv::default()
            .with_var("NVM_DIR", "/n")
            .with_var("PATH", &path)
            .with_var("TERM", self.term);
        for (name, value) in &self.variables {
            env = env.with_var(name, value);
        }
        let context = Context::new(&fs, &env)
            .with_http(&http)
            .with_process(&process)
            .with_terminal(&self.terminal);
        let args: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        run(&context, &args).unwrap()
    }
}

fn joined(lines: &[&str]) -> String {
    lines.join("\n")
}

const GOLDEN_COLORED: [&str; 10] = [
    "       v0.11.16",
    "       v0.12.18",
    "    iojs-v3.3.0",
    "\x1b[0;34m    iojs-v3.3.1\x1b[0m",
    "         v4.0.0",
    "\x1b[0;34m       v18.20.4\x1b[0m  \x1b[1;32m (Latest LTS: Hydrogen)\x1b[0m                   \x1b[0;32m (Aliases: \x1b[3mdefault\x1b[23m)\x1b[0m",
    "       v20.11.0  \x1b[0;37m (LTS: Iron)\x1b[0m",
    "\x1b[0;32m->     v20.11.1\x1b[0m  \x1b[1;32m (Latest LTS: Iron)\x1b[0m                       \x1b[0;32m (Aliases: \x1b[3mmyalias\x1b[23m)\x1b[0m",
    "        v22.2.0",
    "\x1b[0;34m        v22.3.0\x1b[0m                           \x1b[0;32m (Latest: \x1b[3mnode\x1b[23m)\x1b[0m",
];

const GOLDEN_PLAIN: [&str; 10] = [
    "       v0.11.16",
    "       v0.12.18",
    "    iojs-v3.3.0",
    "    iojs-v3.3.1 *",
    "         v4.0.0",
    "       v18.20.4 * (Latest LTS: Hydrogen)                    (Aliases: default)",
    "       v20.11.0   (LTS: Iron)",
    "->     v20.11.1 * (Latest LTS: Iron)                        (Aliases: myalias)",
    "        v22.2.0",
    "        v22.3.0 *                          (Latest: node)",
];

const LTS_COLORED: [&str; 3] = [
    "\x1b[0;34m       v18.20.4\x1b[0m  \x1b[1;32m (Latest LTS: Hydrogen)\x1b[0m",
    "       v20.11.0  \x1b[0;37m (LTS: Iron)\x1b[0m",
    "\x1b[0;32m->     v20.11.1\x1b[0m  \x1b[1;32m (Latest LTS: Iron)\x1b[0m",
];

#[test]
fn a_terminal_with_italics_gets_the_golden_colored_listing() {
    let output = Setup::tty().run("");
    assert_eq!(output, Output::stdout(joined(&GOLDEN_COLORED)));
}

#[test]
fn a_terminal_without_italics_gets_no_italic_names() {
    let output = Setup {
        term: "screen",
        ..Setup::tty()
    }
    .run("");
    let expected = joined(&GOLDEN_COLORED)
        .replace("\x1b[3m", "")
        .replace("\x1b[23m", "");
    assert_eq!(output, Output::stdout(expected));
}

#[test]
fn a_pipe_or_the_no_colors_flag_anywhere_gives_the_plain_listing() {
    let plain = Output::stdout(joined(&GOLDEN_PLAIN));
    let pipe = Setup {
        terminal: FakeTerminal::pipe(),
        ..Setup::tty()
    };
    assert_eq!(pipe.run(""), plain);
    assert_eq!(Setup::tty().run("--no-colors"), plain);
    let lts = Setup::tty().run("--lts --no-colors");
    assert_eq!(
        lts.stdout,
        joined(&[
            "       v18.20.4 * (Latest LTS: Hydrogen)",
            "       v20.11.0   (LTS: Iron)",
            "->     v20.11.1 * (Latest LTS: Iron)",
        ])
    );
    let pattern = Setup::tty().run("--no-colors 18");
    assert_eq!(pattern.stdout, "       v18.20.4 * (Latest LTS: Hydrogen)");
}

#[test]
fn nvm_no_colors_in_the_environment_is_ignored() {
    let output = Setup::tty().var("NVM_NO_COLORS", "--no-colors").run("");
    assert_eq!(output, Output::stdout(joined(&GOLDEN_COLORED)));
}

#[test]
fn rgbcm_moves_every_role_of_the_listing() {
    let output = Setup::tty().var("NVM_COLORS", "rgbcm").run("--lts");
    let expected = joined(&LTS_COLORED)
        .replace("0;34m", "0;31m")
        .replace("0;32m", "0;34m")
        .replace("1;32m", "1;34m")
        .replace("0;37m", "0;35m");
    assert_eq!(output, Output::stdout(expected));
}

#[test]
fn lts_listings_are_colored_without_italics() {
    assert_eq!(
        Setup::tty().run("--lts"),
        Output::stdout(joined(&LTS_COLORED))
    );
    assert_eq!(
        Setup::tty().run("--lts=iron"),
        Output::stdout(joined(&LTS_COLORED[1..]))
    );
    assert_eq!(
        Setup::tty().run("lts/hydrogen"),
        Output::stdout(LTS_COLORED[0])
    );
}

#[test]
fn a_pattern_listing_is_colored_and_keeps_its_status() {
    let output = Setup::tty().run("20");
    assert_eq!(output.stdout, joined(&LTS_COLORED[1..]));
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}

#[test]
fn no_results_is_the_plain_n_a_row_with_status_3() {
    let output = Setup::tty().run("99");
    let expected = Output::stdout("            N/A").with_status(NvmExitCode::InvalidVersion);
    assert_eq!(output, expected);
}

#[test]
fn an_invalid_setting_is_plain_without_markers_and_warns_once() {
    let output = Setup::tty().var("NVM_COLORS", "zzzzz").run("--lts");
    let expected = joined(&[
        "       v18.20.4   (Latest LTS: Hydrogen)",
        "       v20.11.0   (LTS: Iron)",
        "       v20.11.1   (Latest LTS: Iron)",
    ]);
    let warning = "Invalid color code: z";
    assert_eq!(output, Output::stdout(expected).with_stderr(warning));
}

#[test]
fn the_invalid_setting_warning_is_printed_on_a_pipe_and_for_n_a() {
    let pipe = Setup {
        terminal: FakeTerminal::pipe(),
        ..Setup::tty()
    };
    let output = pipe.var("NVM_COLORS", "zzzzz").run("99");
    assert_eq!(output.stdout, "            N/A");
    assert_eq!(output.stderr, "Invalid color code: z");
}

#[test]
fn the_system_node_in_use_leaves_no_arrow() {
    let output = Setup {
        path: "/sys",
        ..Setup::tty()
    }
    .run("--lts");
    assert_eq!(
        output.stdout.lines().last(),
        Some("\x1b[0;34m       v20.11.1\x1b[0m  \x1b[1;32m (Latest LTS: Iron)\x1b[0m")
    );
}

#[test]
fn an_io_js_in_use_gets_the_current_color() {
    let output = Setup {
        path: "/n/versions/io.js/v3.3.1/bin",
        ..Setup::tty()
    }
    .run("");
    assert!(
        output
            .stdout
            .contains("\n\x1b[0;32m->  iojs-v3.3.1\x1b[0m\n")
    );
    assert!(
        output
            .stdout
            .contains("\n\x1b[0;34m       v20.11.1\x1b[0m  ")
    );
}

#[test]
fn no_node_in_use_has_no_arrow() {
    let output = Setup {
        path: "/nowhere",
        ..Setup::tty()
    }
    .run("--lts");
    assert!(!output.stdout.contains("->"));
}
```

Apply to `src/commands/ls_remote/mod.rs` (above the test module):

```diff
--- a/src/commands/ls_remote/mod.rs
+++ b/src/commands/ls_remote/mod.rs
@@ -1,67 +1,63 @@
 //! `nvm ls-remote [pattern]`: the releases a mirror offers, one row each.
 //!
-//! Output is always plain, as `nvm.sh` prints when stdout is not a terminal.
+//! The rows and their annotations are colored like `nvm_print_versions` when
+//! stdout can show colors and `--no-colors` is not given; `NVM_NO_COLORS` in
+//! the environment is ignored, as `nvm.sh` shadows it.
 //! Every flavor that is listed is downloaded again each time, and the `lts/*`
 //! aliases are refreshed from the node index, as in `nvm.sh`.
 
 mod named_aliases;
+mod options;
+
+pub use options::{Options, parse_options};
 
 use crate::commands::Output;
+use crate::commands::color_policy::{self, ColorPolicy};
 use crate::commands::current;
 use crate::commands::remote_index::{fetch_if, lts_filter};
 use crate::context::Context;
+use crate::domain::colors::Palette;
 use crate::domain::listing::{RowKind, format_row};
 use crate::domain::remote::{Query, RemoteRow, list, scope};
-use crate::domain::remote_format::{FormatInput, format_remote_rows};
+use crate::domain::remote_format::{FormatInput, RemoteColors, paint_remote_rows};
 use crate::domain::version::Flavor;
 use crate::error::{CliError, NvmExitCode};
 
-/// The command line of `nvm ls-remote`, as `nvm.sh` reads it: the first
-/// non-empty word is the pattern, `lts/*` and `lts/<name>` mean the LTS
-/// filter, and `--no-colors` is accepted (output is always plain).
-#[derive(Debug, Default, PartialEq, Eq)]
-pub struct Options {
-    pub pattern: Option<String>,
-    pub lts: Option<String>,
+/// The colors of this run, decided once: the policy, the palette and the
+/// single `Invalid color code` warning of an invalid `NVM_COLORS`.
+struct Colors {
+    policy: ColorPolicy,
+    palette: Palette,
+    warning: Option<String>,
 }
 
-/// # Errors
-/// [`CliError::Unsupported`] for an unknown `--option`.
-pub fn parse_options(args: &[String]) -> Result<Options, CliError> {
-    let mut options = Options::default();
-    for arg in args {
-        match arg.as_str() {
-            "--" | "--no-colors" => {}
-            "--lts" => options.lts = Some("*".to_owned()),
-            option if option.starts_with("--lts=") => {
-                options.lts = Some(option["--lts=".len()..].to_owned());
-            }
-            option if option.starts_with("--") => {
-                let message = format!("Unsupported option \"{option}\".");
-                return Err(CliError::Unsupported(message));
-            }
-            word if options.pattern.is_none() && !word.is_empty() => {
-                options.pattern = Some(word.to_owned());
-                options.take_lts_pattern();
-            }
-            _ => {}
+impl Colors {
+    fn detect(context: &Context<'_>, no_colors: bool) -> Self {
+        let (palette, warning) = color_policy::palette(context);
+        Self {
+            policy: color_policy::detect(context, no_colors),
+            palette,
+            warning,
         }
     }
-    Ok(options)
-}
 
-impl Options {
-    /// `lts/*` and `lts/<name>` given as the pattern are the LTS filter,
-    /// unless `--lts` was already given.
-    fn take_lts_pattern(&mut self) {
-        if self.lts.as_deref().is_some_and(|lts| !lts.is_empty()) {
-            return;
+    fn remote(&self) -> Option<RemoteColors<'_>> {
+        self.policy.enabled.then_some(RemoteColors {
+            palette: &self.palette,
+            italics: self.policy.italics,
+        })
+    }
+
+    /// `nvm_print_versions` reads the palette for every listing, the `N/A`
+    /// one included, so the warning follows whatever else went to stderr.
+    fn warn(&self, mut output: Output) -> Output {
+        if let Some(warning) = &self.warning {
+            if !output.stderr.is_empty() {
+                output.stderr.push('\n');
+            }
+            output.stderr.push_str(warning);
         }
-        let Some(name) = self.pattern.as_deref().and_then(|p| p.strip_prefix("lts/")) else {
-            return;
-        };
-        self.lts = Some(name.to_owned());
-        self.pattern = Some(String::new());
+        output
     }
 }
 
@@ -70,6 +66,12 @@
 /// cannot be found.
 pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
     let options = parse_options(args)?;
+    let colors = Colors::detect(context, options.no_colors);
+    let output = listing(context, &options, &colors)?;
+    Ok(colors.warn(output))
+}
+
+fn listing(context: &Context<'_>, options: &Options, colors: &Colors) -> Result<Output, CliError> {
     let mut query = Query {
         pattern: options.pattern.clone().filter(|text| !text.is_empty()),
         lts: options.lts.clone().filter(|text| !text.is_empty()),
@@ -91,7 +93,7 @@
         return Ok(not_available(warnings));
     }
     let plain = listing.missing || query.lts.is_some() || options.pattern.is_some();
-    let lines = format_rows(context, &listing.rows, plain)?;
+    let lines = format_rows(context, &listing.rows, plain, colors)?;
     Ok(printed(lines, &warnings, listing.missing))
 }
 
@@ -128,6 +130,7 @@
     context: &Context<'_>,
     rows: &[RemoteRow],
     plain: bool,
+    colors: &Colors,
 ) -> Result<Vec<String>, CliError> {
     let installed = context.installed_versions()?;
     let current = current::detect(context)?.to_string();
@@ -136,12 +139,15 @@
     } else {
         named_aliases::collect(context)?
     };
-    Ok(format_remote_rows(&FormatInput {
+    let input = FormatInput {
         rows,
         installed: &installed,
         current: &current,
         latest_alias: (!plain).then_some("node"),
         named_aliases: &named,
-    }))
+    };
+    Ok(paint_remote_rows(&input, colors.remote()))
 }
 
+#[cfg(test)]
+mod colored_tests;
```

Create `src/commands/ls_remote/options.rs`:

```rust
//! The command line of `nvm ls-remote`.

use crate::error::CliError;

/// The command line of `nvm ls-remote`, as `nvm.sh` reads it: the first
/// non-empty word is the pattern, `lts/*` and `lts/<name>` mean the LTS
/// filter, and `--no-colors` (anywhere) turns the colors off.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub pattern: Option<String>,
    pub lts: Option<String>,
    pub no_colors: bool,
}

/// # Errors
/// [`CliError::Unsupported`] for an unknown `--option`.
pub fn parse_options(args: &[String]) -> Result<Options, CliError> {
    let mut options = Options::default();
    for arg in args {
        match arg.as_str() {
            "--" => {}
            "--no-colors" => options.no_colors = true,
            "--lts" => options.lts = Some("*".to_owned()),
            option if option.starts_with("--lts=") => {
                options.lts = Some(option["--lts=".len()..].to_owned());
            }
            option if option.starts_with("--") => {
                let message = format!("Unsupported option \"{option}\".");
                return Err(CliError::Unsupported(message));
            }
            word if options.pattern.is_none() && !word.is_empty() => {
                options.pattern = Some(word.to_owned());
                options.take_lts_pattern();
            }
            _ => {}
        }
    }
    Ok(options)
}

impl Options {
    /// `lts/*` and `lts/<name>` given as the pattern are the LTS filter,
    /// unless `--lts` was already given.
    fn take_lts_pattern(&mut self) {
        if self.lts.as_deref().is_some_and(|lts| !lts.is_empty()) {
            return;
        }
        let Some(name) = self.pattern.as_deref().and_then(|p| p.strip_prefix("lts/")) else {
            return;
        };
        self.lts = Some(name.to_owned());
        self.pattern = Some(String::new());
    }
}
```

Create `src/domain/remote_format/cells.rs`:

```rust
//! One row of `nvm ls-remote` before alignment: the version cell and the
//! three annotation columns, each as printed (maybe colored) and as seen.

use std::collections::HashMap;

use super::latest::Latest;
use super::{FormatInput, RemoteColors};
use crate::domain::colors::{ITALIC_OFF, ITALIC_ON, Palette, wrap};
use crate::domain::listing::colored::paint_row;
use crate::domain::listing::{RowKind, format_row};
use crate::domain::remote::RemoteRow;

/// How the cells are painted: the colors (`None` when off) and whether the
/// alias names are italic (`has_italics` of the awk).
pub(super) struct Style<'a> {
    pub(super) colors: Option<RemoteColors<'a>>,
    pub(super) italics: bool,
}

impl Style<'_> {
    /// The `paint` of the awk: the colored span when colors are on and the
    /// role has a color, otherwise the text.
    fn paint(&self, code: impl FnOnce(&Palette) -> Option<String>, text: &str) -> String {
        match self.colors {
            Some(colors) => wrap(code(colors.palette).as_deref(), text),
            None => text.to_owned(),
        }
    }

    fn italicize(&self, name: &str) -> String {
        if self.italics {
            format!("{ITALIC_ON}{name}{ITALIC_OFF}")
        } else {
            name.to_owned()
        }
    }

    fn version(&self, text: &str, kind: RowKind) -> String {
        match self.colors {
            Some(colors) => paint_row(text, kind, colors.palette, true),
            None => format_row(text, kind),
        }
    }
}

/// An annotation column: what is printed and what the terminal shows.
#[derive(Default)]
pub(super) struct Annotation {
    pub(super) shown: String,
    pub(super) printed: String,
}

pub(super) struct Cells {
    pub(super) version: String,
    pub(super) padding: &'static str,
    /// The visible width of the version and its padding.
    pub(super) width: usize,
    pub(super) columns: [Annotation; 3],
}

/// The length the terminal shows: `\e[...m` sequences take no room. Bytes
/// are counted, like the `length` of the awk.
fn visible_length(text: &str) -> usize {
    let mut length = 0;
    let mut rest = text;
    while let Some(start) = rest.find('\x1b') {
        length += start;
        let end = rest[start..]
            .find('m')
            .map_or(rest.len(), |at| start + at + 1);
        rest = &rest[end..];
    }
    length + rest.len()
}

fn row_kind(text: &str, current: &str, installed: bool) -> RowKind {
    if text == current {
        RowKind::Current
    } else if installed {
        RowKind::Installed
    } else {
        RowKind::Plain
    }
}

fn lts_column(row: &RemoteRow, style: &Style<'_>) -> Annotation {
    let Some(name) = row.lts.as_deref() else {
        return Annotation::default();
    };
    if row.latest_lts {
        let shown = format!(" (Latest LTS: {name})");
        let printed = style.paint(Palette::latest_lts, &shown);
        Annotation { shown, printed }
    } else {
        let shown = format!(" (LTS: {name})");
        let printed = style.paint(|palette| palette.old_lts().map(str::to_owned), &shown);
        Annotation { shown, printed }
    }
}

fn latest_column(alias: &str, style: &Style<'_>) -> Annotation {
    let printed = format!(" (Latest: {})", style.italicize(alias));
    Annotation {
        shown: format!(" (Latest: {alias})"),
        printed: style.paint(Palette::annotation, &printed),
    }
}

fn aliases_column(names: &[String], style: &Style<'_>) -> Annotation {
    let italic: Vec<String> = names.iter().map(|name| style.italicize(name)).collect();
    let printed = format!(" (Aliases: {})", italic.join(", "));
    Annotation {
        shown: format!(" (Aliases: {})", names.join(", ")),
        printed: style.paint(Palette::annotation, &printed),
    }
}

/// The three annotation columns of a row: LTS, latest, aliases.
fn columns(
    row: &RemoteRow,
    input: &FormatInput<'_>,
    latest: &Latest,
    named: &HashMap<String, Vec<String>>,
    style: &Style<'_>,
) -> [Annotation; 3] {
    let text = row.version.to_string();
    let newest = input
        .latest_alias
        .filter(|_| latest.stable.as_deref() == Some(text.as_str()));
    [
        lts_column(row, style),
        newest.map_or_else(Annotation::default, |alias| latest_column(alias, style)),
        named
            .get(&text)
            .map_or_else(Annotation::default, |names| aliases_column(names, style)),
    ]
}

/// The cells of a row. The padding is two spaces, except for an installed
/// row with colors off, whose ` *` takes those two columns.
pub(super) fn cells_for(
    row: &RemoteRow,
    input: &FormatInput<'_>,
    latest: &Latest,
    named: &HashMap<String, Vec<String>>,
    style: &Style<'_>,
) -> Cells {
    let text = row.version.to_string();
    let installed = input.installed.contains(&row.version);
    let version = style.version(&text, row_kind(&text, input.current, installed));
    let padding = if installed && style.colors.is_none() {
        ""
    } else {
        "  "
    };
    Cells {
        width: visible_length(&version) + padding.len(),
        version,
        padding,
        columns: columns(row, input, latest, named, style),
    }
}
```

Create `src/domain/remote_format/colored_tests.rs`:

```rust
//! The colored rows, against the golden `nvm ls-remote` blocks of the digest
//! (mirror: node v22.3.0, v22.2.0, v20.11.1 Iron, v20.11.0 Iron, v18.20.4
//! Hydrogen, v4.0.0, v0.12.18, v0.11.16; io.js v3.3.1, v3.3.0; installed:
//! v18.20.4, v20.11.1 (current), v22.3.0, iojs-v3.3.1; aliases
//! `default -> 18`, `myalias -> 20`).

use super::*;
use crate::domain::colors::Palette;

fn row(version: &str, lts: Option<&str>, latest_lts: bool) -> RemoteRow {
    RemoteRow {
        version: version.parse().unwrap(),
        lts: lts.map(str::to_owned),
        latest_lts,
    }
}

fn mirror_rows() -> Vec<RemoteRow> {
    vec![
        row("v0.11.16", None, false),
        row("v0.12.18", None, false),
        row("iojs-v3.3.0", None, false),
        row("iojs-v3.3.1", None, false),
        row("v4.0.0", None, false),
        row("v18.20.4", Some("Hydrogen"), true),
        row("v20.11.0", Some("Iron"), false),
        row("v20.11.1", Some("Iron"), true),
        row("v22.2.0", None, false),
        row("v22.3.0", None, false),
    ]
}

fn lts_rows() -> Vec<RemoteRow> {
    vec![
        row("v18.20.4", Some("Hydrogen"), true),
        row("v20.11.0", Some("Iron"), false),
        row("v20.11.1", Some("Iron"), true),
    ]
}

fn installed() -> Vec<Version> {
    ["iojs-v3.3.1", "v18.20.4", "v20.11.1", "v22.3.0"]
        .iter()
        .map(|name| name.parse().unwrap())
        .collect()
}

fn aliases() -> Vec<(String, String)> {
    vec![
        ("v18".to_owned(), "default".to_owned()),
        ("v20".to_owned(), "myalias".to_owned()),
    ]
}

fn palette(setting: &str) -> Palette {
    Palette::from_setting(Some(setting)).0
}

/// The full listing (`latest_alias` and aliases) or a filtered one.
fn paint(rows: &[RemoteRow], full: bool, colors: Option<RemoteColors<'_>>) -> Vec<String> {
    let installed = installed();
    let named = if full { aliases() } else { Vec::new() };
    let input = FormatInput {
        rows,
        installed: &installed,
        current: "v20.11.1",
        latest_alias: full.then_some("node"),
        named_aliases: &named,
    };
    paint_remote_rows(&input, colors)
}

fn on(palette: &Palette, italics: bool) -> Option<RemoteColors<'_>> {
    Some(RemoteColors { palette, italics })
}

/// Removes every `\e[...m`, leaving what the terminal shows.
fn visible(line: &str) -> String {
    let mut shown = String::new();
    let mut rest = line;
    while let Some(start) = rest.find('\x1b') {
        shown.push_str(&rest[..start]);
        let end = rest[start..]
            .find('m')
            .map_or(rest.len(), |at| start + at + 1);
        rest = &rest[end..];
    }
    shown.push_str(rest);
    shown
}

const GOLDEN_COLORED: [&str; 10] = [
    "       v0.11.16",
    "       v0.12.18",
    "    iojs-v3.3.0",
    "\x1b[0;34m    iojs-v3.3.1\x1b[0m",
    "         v4.0.0",
    "\x1b[0;34m       v18.20.4\x1b[0m  \x1b[1;32m (Latest LTS: Hydrogen)\x1b[0m                   \x1b[0;32m (Aliases: \x1b[3mdefault\x1b[23m)\x1b[0m",
    "       v20.11.0  \x1b[0;37m (LTS: Iron)\x1b[0m",
    "\x1b[0;32m->     v20.11.1\x1b[0m  \x1b[1;32m (Latest LTS: Iron)\x1b[0m                       \x1b[0;32m (Aliases: \x1b[3mmyalias\x1b[23m)\x1b[0m",
    "        v22.2.0",
    "\x1b[0;34m        v22.3.0\x1b[0m                           \x1b[0;32m (Latest: \x1b[3mnode\x1b[23m)\x1b[0m",
];

const GOLDEN_PLAIN: [&str; 10] = [
    "       v0.11.16",
    "       v0.12.18",
    "    iojs-v3.3.0",
    "    iojs-v3.3.1 *",
    "         v4.0.0",
    "       v18.20.4 * (Latest LTS: Hydrogen)                    (Aliases: default)",
    "       v20.11.0   (LTS: Iron)",
    "->     v20.11.1 * (Latest LTS: Iron)                        (Aliases: myalias)",
    "        v22.2.0",
    "        v22.3.0 *                          (Latest: node)",
];

#[test]
fn the_default_palette_with_italics_is_the_golden_block() {
    let colors = palette("bygre");
    assert_eq!(
        paint(&mirror_rows(), true, on(&colors, true)),
        GOLDEN_COLORED
    );
}

#[test]
fn without_italics_the_names_are_not_wrapped() {
    let colors = palette("bygre");
    let expected: Vec<String> = GOLDEN_COLORED
        .iter()
        .map(|line| line.replace("\x1b[3m", "").replace("\x1b[23m", ""))
        .collect();
    assert_eq!(paint(&mirror_rows(), true, on(&colors, false)), expected);
}

#[test]
fn colors_off_is_the_plain_listing() {
    assert_eq!(paint(&mirror_rows(), true, None), GOLDEN_PLAIN);
    let rows = mirror_rows();
    let installed = installed();
    let named = aliases();
    let input = FormatInput {
        rows: &rows,
        installed: &installed,
        current: "v20.11.1",
        latest_alias: Some("node"),
        named_aliases: &named,
    };
    assert_eq!(format_remote_rows(&input), GOLDEN_PLAIN);
}

#[test]
fn the_visible_layout_is_the_same_with_and_without_colors() {
    let colors = palette("bygre");
    for italics in [true, false] {
        let shown: Vec<String> = paint(&mirror_rows(), true, on(&colors, italics))
            .iter()
            .map(|line| visible(line))
            .collect();
        let plain: Vec<String> = GOLDEN_PLAIN
            .iter()
            .map(|line| line.replacen(" *", "  ", 1).trim_end().to_owned())
            .collect();
        assert_eq!(shown, plain);
    }
}

#[test]
fn rgbcm_moves_every_role() {
    let colors = palette("rgbcm");
    assert_eq!(
        paint(&mirror_rows(), true, on(&colors, true)),
        [
            "       v0.11.16",
            "       v0.12.18",
            "    iojs-v3.3.0",
            "\x1b[0;31m    iojs-v3.3.1\x1b[0m",
            "         v4.0.0",
            "\x1b[0;31m       v18.20.4\x1b[0m  \x1b[1;34m (Latest LTS: Hydrogen)\x1b[0m                   \x1b[0;34m (Aliases: \x1b[3mdefault\x1b[23m)\x1b[0m",
            "       v20.11.0  \x1b[0;35m (LTS: Iron)\x1b[0m",
            "\x1b[0;34m->     v20.11.1\x1b[0m  \x1b[1;34m (Latest LTS: Iron)\x1b[0m                       \x1b[0;34m (Aliases: \x1b[3mmyalias\x1b[23m)\x1b[0m",
            "        v22.2.0",
            "\x1b[0;31m        v22.3.0\x1b[0m                           \x1b[0;34m (Latest: \x1b[3mnode\x1b[23m)\x1b[0m",
        ]
    );
}

#[test]
fn an_lts_listing_never_has_italics() {
    let colors = palette("bygre");
    assert_eq!(
        paint(&lts_rows(), false, on(&colors, true)),
        [
            "\x1b[0;34m       v18.20.4\x1b[0m  \x1b[1;32m (Latest LTS: Hydrogen)\x1b[0m",
            "       v20.11.0  \x1b[0;37m (LTS: Iron)\x1b[0m",
            "\x1b[0;32m->     v20.11.1\x1b[0m  \x1b[1;32m (Latest LTS: Iron)\x1b[0m",
        ]
    );
}

/// What nvm.sh printed with `NVM_COLORS=rg`: no current or default color, so
/// the current row loses its arrow, the LTS columns and the annotations are
/// plain, yet the alias names keep their italics.
#[test]
fn a_role_without_color_is_plain_and_keeps_the_layout() {
    let colors = palette("rg");
    assert_eq!(
        paint(&mirror_rows(), true, on(&colors, true)),
        [
            "       v0.11.16",
            "       v0.12.18",
            "    iojs-v3.3.0",
            "\x1b[0;31m    iojs-v3.3.1\x1b[0m",
            "         v4.0.0",
            "\x1b[0;31m       v18.20.4\x1b[0m   (Latest LTS: Hydrogen)                    (Aliases: \x1b[3mdefault\x1b[23m)",
            "       v20.11.0   (LTS: Iron)",
            "       v20.11.1   (Latest LTS: Iron)                        (Aliases: \x1b[3mmyalias\x1b[23m)",
            "        v22.2.0",
            "\x1b[0;31m        v22.3.0\x1b[0m                            (Latest: \x1b[3mnode\x1b[23m)",
        ]
    );
}

#[test]
fn several_aliases_on_a_release_are_each_italic_and_joined_by_commas() {
    let rows = vec![row("v18.20.4", Some("Hydrogen"), true)];
    let installed = installed();
    let named = vec![
        ("v18".to_owned(), "default".to_owned()),
        ("v18.20.4".to_owned(), "work".to_owned()),
    ];
    let input = FormatInput {
        rows: &rows,
        installed: &installed,
        current: "none",
        latest_alias: Some("node"),
        named_aliases: &named,
    };
    let colors = palette("bygre");
    assert_eq!(
        paint_remote_rows(&input, on(&colors, true)),
        [
            "\x1b[0;34m       v18.20.4\x1b[0m  \x1b[1;32m (Latest LTS: Hydrogen)\x1b[0m  \x1b[0;32m (Latest: \x1b[3mnode\x1b[23m)\x1b[0m  \x1b[0;32m (Aliases: \x1b[3mdefault\x1b[23m, \x1b[3mwork\x1b[23m)\x1b[0m"
        ]
    );
}
```

Create `src/domain/remote_format/latest.rs`:

```rust
//! Which release each `(Latest: …)` and `(Aliases: …)` annotation lands on,
//! as the `BEGIN` block of the `awk` in `nvm_print_versions` works it out.

use std::collections::HashMap;

use super::FormatInput;
use crate::domain::remote::RemoteRow;

/// The newest release of each kind in the listing.
#[derive(Default)]
pub(super) struct Latest {
    pub(super) stable: Option<String>,
    unstable: Option<String>,
    iojs: Option<String>,
}

/// `^v0\.[0-9]*[13579]\.`: an odd minor of the old 0.x scheme.
pub(super) fn is_old_unstable(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("v0.") else {
        return false;
    };
    let minor: String = rest.chars().take_while(char::is_ascii_digit).collect();
    rest[minor.len()..].starts_with('.')
        && minor
            .chars()
            .last()
            .is_some_and(|digit| "13579".contains(digit))
}

pub(super) fn latest_of_each_kind(rows: &[RemoteRow]) -> Latest {
    let mut latest = Latest::default();
    for row in rows {
        let text = row.version.to_string();
        if text.starts_with("iojs-") {
            latest.iojs = Some(text);
        } else if is_old_unstable(&text) {
            latest.unstable = Some(text);
        } else if text.starts_with('v') {
            latest.stable = Some(text);
        }
    }
    latest
}

/// The release each alias target stands for, and the alias names on each
/// release, in the order the aliases were given.
pub(super) fn aliases_by_version(
    input: &FormatInput<'_>,
    latest: &Latest,
) -> HashMap<String, Vec<String>> {
    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    for (target, name) in input.named_aliases {
        let version = match target.as_str() {
            "node" | "stable" => latest.stable.clone(),
            "unstable" => latest.unstable.clone(),
            "iojs" | "iojs-" => latest.iojs.clone(),
            _ => input
                .rows
                .iter()
                .map(|row| row.version.to_string())
                .rfind(|text| text == target || text.starts_with(&format!("{target}."))),
        };
        if let Some(version) = version {
            named.entry(version).or_default().push(name.clone());
        }
    }
    named
}
```

Apply to `src/domain/remote_format/mod.rs` (above the test module):

```diff
--- a/src/domain/remote_format/mod.rs
+++ b/src/domain/remote_format/mod.rs
@@ -1,17 +1,22 @@
 //! The rows `nvm ls-remote` prints, as the `awk` of `nvm_print_versions`
-//! prints them when stdout is not a terminal (no colors): the version
-//! right-aligned in 15 columns, then up to three annotation columns that each
-//! start at the same place on every row.
+//! prints them: the version right-aligned in 15 columns, then up to three
+//! annotation columns that each start at the same place on every row. With
+//! colors on, the rows and annotations are colored and the alignment counts
+//! only what the terminal shows, so the layout is the same either way.
 
-use std::collections::HashMap;
+mod cells;
+mod latest;
 
-use crate::domain::listing::{RowKind, format_row};
+use cells::{Cells, Style, cells_for};
+use latest::{aliases_by_version, latest_of_each_kind};
+
+use crate::domain::colors::Palette;
 use crate::domain::remote::RemoteRow;
 use crate::domain::version::Version;
 
 pub struct FormatInput<'a> {
     pub rows: &'a [RemoteRow],
-    /// Everything installed: those rows get a `*`.
+    /// Everything installed: those rows get a `*` (or the installed color).
     pub installed: &'a [Version],
     /// What `nvm current` prints: that row gets an arrow.
     pub current: &'a str,
@@ -23,140 +28,43 @@
     pub named_aliases: &'a [(String, String)],
 }
 
-#[derive(Default)]
-struct Latest {
-    stable: Option<String>,
-    unstable: Option<String>,
-    iojs: Option<String>,
+/// The colors of a colored listing: the palette, and whether the terminal
+/// shows italics (`nvm_has_italics`).
+#[derive(Debug, Clone, Copy)]
+pub struct RemoteColors<'a> {
+    pub palette: &'a Palette,
+    pub italics: bool,
 }
 
-/// `^v0\.[0-9]*[13579]\.`: an odd minor of the old 0.x scheme.
-fn is_old_unstable(text: &str) -> bool {
-    let Some(rest) = text.strip_prefix("v0.") else {
-        return false;
-    };
-    let minor: String = rest.chars().take_while(char::is_ascii_digit).collect();
-    rest[minor.len()..].starts_with('.')
-        && minor
-            .chars()
-            .last()
-            .is_some_and(|digit| "13579".contains(digit))
+/// The rows with colors off, as nvm.sh prints them when stdout is not a
+/// terminal.
+#[must_use]
+pub fn format_remote_rows(input: &FormatInput<'_>) -> Vec<String> {
+    paint_remote_rows(input, None)
 }
 
-fn latest_of_each_kind(rows: &[RemoteRow]) -> Latest {
-    let mut latest = Latest::default();
-    for row in rows {
-        let text = row.version.to_string();
-        if text.starts_with("iojs-") {
-            latest.iojs = Some(text);
-        } else if is_old_unstable(&text) {
-            latest.unstable = Some(text);
-        } else if text.starts_with('v') {
-            latest.stable = Some(text);
-        }
-    }
-    latest
-}
-
-/// The release each alias target stands for, and the aliases on each release.
-fn aliases_by_version(input: &FormatInput<'_>, latest: &Latest) -> HashMap<String, Vec<String>> {
-    let mut named: HashMap<String, Vec<String>> = HashMap::new();
-    for (target, name) in input.named_aliases {
-        let version = match target.as_str() {
-            "node" | "stable" => latest.stable.clone(),
-            "unstable" => latest.unstable.clone(),
-            "iojs" | "iojs-" => latest.iojs.clone(),
-            _ => input
-                .rows
-                .iter()
-                .map(|row| row.version.to_string())
-                .rfind(|text| text == target || text.starts_with(&format!("{target}."))),
-        };
-        if let Some(version) = version {
-            named.entry(version).or_default().push(name.clone());
-        }
-    }
-    named
-}
-
-struct Cells {
-    version: String,
-    padding: &'static str,
-    width: usize,
-    text: [String; 3],
-}
-
-fn row_kind(text: &str, current: &str, installed: bool) -> RowKind {
-    if text == current {
-        RowKind::Current
-    } else if installed {
-        RowKind::Installed
-    } else {
-        RowKind::Plain
-    }
-}
-
-/// The three annotation columns of a row: LTS, latest, aliases.
-fn annotations(
-    row: &RemoteRow,
-    input: &FormatInput<'_>,
-    latest: &Latest,
-    named: &HashMap<String, Vec<String>>,
-) -> [String; 3] {
-    let text = row.version.to_string();
-    let lts = row.lts.as_deref().map(|name| {
-        if row.latest_lts {
-            format!(" (Latest LTS: {name})")
-        } else {
-            format!(" (LTS: {name})")
-        }
-    });
-    let newest = input
-        .latest_alias
-        .filter(|_| latest.stable.as_deref() == Some(text.as_str()))
-        .map(|alias| format!(" (Latest: {alias})"));
-    let aliases = named
-        .get(&text)
-        .map(|names| format!(" (Aliases: {})", names.join(", ")));
-    [lts, newest, aliases].map(Option::unwrap_or_default)
-}
-
-fn cells_for(
-    row: &RemoteRow,
-    input: &FormatInput<'_>,
-    latest: &Latest,
-    named: &HashMap<String, Vec<String>>,
-) -> Cells {
-    let text = row.version.to_string();
-    let installed = input.installed.contains(&row.version);
-    let version = format_row(&text, row_kind(&text, input.current, installed));
-    let padding = if installed { "" } else { "  " };
-    Cells {
-        width: version.len() + padding.len(),
-        version,
-        padding,
-        text: annotations(row, input, latest, named),
-    }
-}
-
-fn spaces(count: usize) -> String {
-    " ".repeat(count)
-}
-
+/// The rows with the given colors (`None`: colors off). The alias names and
+/// `node` are italic only when the terminal shows italics and the listing
+/// has the latest or alias annotations.
 #[must_use]
-pub fn format_remote_rows(input: &FormatInput<'_>) -> Vec<String> {
+pub fn paint_remote_rows(input: &FormatInput<'_>, colors: Option<RemoteColors<'_>>) -> Vec<String> {
+    let annotated = input.latest_alias.is_some() || !input.named_aliases.is_empty();
+    let style = Style {
+        colors,
+        italics: annotated && colors.is_some_and(|colors| colors.italics),
+    };
     let latest = latest_of_each_kind(input.rows);
     let named = aliases_by_version(input, &latest);
     let cells: Vec<Cells> = input
         .rows
         .iter()
-        .map(|row| cells_for(row, input, &latest, &named))
+        .map(|row| cells_for(row, input, &latest, &named, &style))
         .collect();
     let widest_version = cells.iter().map(|cell| cell.width).max().unwrap_or(0);
     let widths: [usize; 3] = std::array::from_fn(|column| {
         cells
             .iter()
-            .map(|cell| cell.text[column].len())
+            .map(|cell| cell.columns[column].shown.len())
             .max()
             .unwrap_or(0)
     });
@@ -166,10 +74,18 @@
         .collect()
 }
 
+fn spaces(count: usize) -> String {
+    " ".repeat(count)
+}
+
 /// One row: the version, then each annotation column that any row has, padded
 /// so that the columns line up; nothing is added after the last annotation.
 fn assemble(cell: &Cells, widest_version: usize, widths: &[usize; 3]) -> String {
-    let Some(last) = cell.text.iter().rposition(|text| !text.is_empty()) else {
+    let Some(last) = cell
+        .columns
+        .iter()
+        .rposition(|column| !column.shown.is_empty())
+    else {
         return cell.version.clone();
     };
     let mut row = format!("{}{}", cell.version, cell.padding);
@@ -177,13 +93,15 @@
         row.push_str(&spaces(widest_version - cell.width));
     }
     let mut gap = String::new();
-    for (text, &width) in cell.text.iter().zip(widths).take(last + 1) {
+    for (column, &width) in cell.columns.iter().zip(widths).take(last + 1) {
         if width > 0 {
             row.push_str(&gap);
-            row.push_str(text);
-            gap = format!("{}  ", spaces(width - text.len()));
+            row.push_str(&column.printed);
+            gap = format!("{}  ", spaces(width - column.shown.len()));
         }
     }
     row
 }
 
+#[cfg(test)]
+mod colored_tests;
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1120 unit tests plus the end-to-end tests.

- [ ] **Step 5: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(ls-remote): color the versions and their annotations"
```

---

### Task 6: `nvm set-colors`

**Files:**

- Create: `src/cli/tests/set_colors.rs`, `src/commands/set_colors/mod.rs`,
  `src/commands/set_colors/tests.rs`
- Modify: `src/cli/channel.rs`, `src/cli/commands.rs`, `src/cli/mod.rs`,
  `src/cli/tests/mod.rs`, `src/commands/auto/mod.rs`, `src/commands/mod.rs`,
  `src/shell/init/fish.rs`, `src/shell/init/mod.rs`,
  `src/shell/init/posix.rs`, `src/shell/init/tests.rs`, `tests/init_cli.rs`

**Interfaces:**

- Produces: `commands::set_colors::run(&Context, &[String])`; the `set-colors`
  subcommand; `Output::blank_stdout` with `with_blank_stdout()` (the CLI prints
  `"\n"` when stdout is otherwise empty: the invalid case needs a blank line);
  `set-colors` joins the commands that keep a leading `--` (`KEEP_DOUBLE_DASH`
  with `exec` and `run`) and the commands whose code the init snippets capture
  (`use | deactivate | install | i | set-colors | __auto`, POSIX and fish).
- Behaviour (digest section 2.5 and the real `nvm_set_colors`): only the FIRST
  argument counts (`set-colors rgbcm extra` is accepted). A valid setting (five
  characters of `rRgGbBcCyYmMkKeW`) emits the shell code `export
  NVM_COLORS='<letters>'` and prints on stdout `Setting colors to:` followed by
  each letter in its own color with no separators when colors are on (`detect`
  with the ENVIRONMENT `NVM_NO_COLORS` equal to `--no-colors` as the flag; an
  exported `NVM_HAS_COLORS` does not force here), or `Setting colors to: r g b c
  m` and the line `WARNING: Colors may not display because they are not
  supported in this shell.` when they are off. An invalid or missing setting
  prints a blank line on stdout and on stderr, always with forced colors,
  `\x1b[1;37mPlease pass in five \x1b[1;31mvalid color codes\x1b[1;37m. Choose
  from: rRgGbBcCyYmMkKeW\x1b[0m`, emits no code, and exits 0 (as nvm.sh).
  Deliberate deviation: nvm.sh also dumps `nvm --help` on stderr before that
  line.
- The end-to-end tests run in every installed shell (bash, zsh, sh, dash, ksh,
  and fish where installed): after `nvm set-colors rgbcm` the shell's
  `NVM_COLORS` is `rgbcm`; an invalid call leaves it unchanged.
- [ ] **Step 1: Declare the new modules**

Apply to `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -20,6 +20,7 @@
 pub mod resolve;
 pub mod run;
 pub mod sanitize;
+pub mod set_colors;
 pub mod transcript;
 pub mod unalias;
 pub mod uninstall;
```

- [ ] **Step 2: Write the failing tests**

Apply to `src/cli/tests/mod.rs`:

```diff
--- a/src/cli/tests/mod.rs
+++ b/src/cli/tests/mod.rs
@@ -294,4 +294,5 @@
 mod install;
 mod nvm_exec;
 mod script;
+mod set_colors;
 mod spawn;
```

Create `src/cli/tests/set_colors.rs`:

```rust
use super::*;
use crate::fakes::FakeScriptChannel;

const PLAIN: &str = "Setting colors to: r g b c m\n\
WARNING: Colors may not display because they are not supported in this shell.\n";
const INVALID: &str = "\x1b[1;37mPlease pass in five \x1b[1;31mvalid color codes\x1b[1;37m. \
Choose from: rRgGbBcCyYmMkKeW\x1b[0m\n";

fn in_the_function(args: &[&str]) -> (u8, String, String, String) {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default();
    let channel = FakeScriptChannel::default();
    let context = Context::new(&fs, &env).with_script_channel(&channel);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(args, &context, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
        channel.sent(),
    )
}

#[test]
fn set_colors_hands_the_export_to_the_function_and_confirms_on_stdout() {
    let result = in_the_function(&["nvm", "set-colors", "rgbcm"]);
    let expected = (
        0,
        PLAIN.into(),
        String::new(),
        "export NVM_COLORS='rgbcm'\n".into(),
    );
    assert_eq!(result, expected);
}

#[test]
fn standalone_set_colors_prints_the_export_and_moves_the_text_to_stderr() {
    let result = run_cli(&["nvm", "set-colors", "rgbcm"]);
    assert_eq!(
        result,
        (0, "export NVM_COLORS='rgbcm'\n".into(), PLAIN.into())
    );
}

#[test]
fn an_invalid_set_colors_prints_a_blank_line_and_the_error_with_status_0() {
    for args in [
        &["nvm", "set-colors"][..],
        &["nvm", "set-colors", "rgb"],
        &["nvm", "set-colors", "--no-colors"],
    ] {
        let result = in_the_function(args);
        let expected = (0, "\n".into(), INVALID.into(), String::new());
        assert_eq!(result, expected, "{args:?}");
        assert_eq!(run_cli(args), (0, "\n".into(), INVALID.into()), "{args:?}");
    }
}

#[test]
fn a_double_dash_is_the_setting_as_in_nvm_sh_and_so_invalid() {
    let result = in_the_function(&["nvm", "set-colors", "--", "rgbcm"]);
    assert_eq!(result, (0, "\n".into(), INVALID.into(), String::new()));
}

#[test]
fn a_blank_stdout_line_is_printed_only_when_the_text_is_empty() {
    let blank = Output::default().with_blank_stdout();
    assert_eq!(deliver(&blank), (0, "\n".into(), String::new()));
    let text = Output::stdout("o").with_blank_stdout();
    assert_eq!(deliver(&text), (0, "o\n".into(), String::new()));
}

fn deliver(output: &Output) -> (u8, String, String) {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = finish("nvm", output, &Context::new(&fs, &env), &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}
```

Apply to the test module of `src/commands/mod.rs`:

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -16,6 +16,12 @@
         let script = Script::new().unset("NVM_BIN").unwrap();
         let output = Output::stdout("x").with_script(script);
         assert_eq!(output.script.render(), "unset NVM_BIN\n");
+    }
+
+    #[test]
+    fn the_blank_stdout_line_is_off_by_default_and_can_be_asked_for() {
+        assert!(!Output::default().blank_stdout);
+        assert!(Output::default().with_blank_stdout().blank_stdout);
     }
 
     #[test]
```

Create `src/commands/set_colors/mod.rs` containing only the test module:

```rust
#[cfg(test)]
mod tests;

/// The variable the setting is exported to.
const VARIABLE: &str = "NVM_COLORS";
/// The only value of `NVM_NO_COLORS` that turns the colors off here.
const NO_COLORS_FLAG: &str = "--no-colors";
/// The confirmation's prefix.
const CONFIRMATION: &str = "Setting colors to: ";
/// The line after the uncolored confirmation.
const NO_COLORS_WARNING: &str =
    "WARNING: Colors may not display because they are not supported in this shell.";
/// The error of an invalid setting, colored whether or not colors are on.
const INVALID_SETTING: &str = "\x1b[1;37mPlease pass in five \x1b[1;31mvalid color codes\
\x1b[1;37m. Choose from: rRgGbBcCyYmMkKeW\x1b[0m";

/// `nvm set-colors [<setting>]`: only the first argument counts, as in
/// nvm.sh (`nvm_set_colors "${1-}"`), so `nvm set-colors rgbcm extra` is
/// valid and `nvm set-colors -- rgbcm` is not.
///
/// A valid setting (exactly five of `rRgGbBcCyYmMkKeW`) is exported and
/// confirmed on stdout, each letter in its own color when the colors are on
/// (`nvm_has_colors`, which reads an `NVM_NO_COLORS` of exactly
/// `--no-colors`, unlike `ls`, `ls-remote` and `alias`; `NVM_HAS_COLORS`
/// does not force them here). An invalid one prints an empty line on stdout
/// and the error on stderr, and the status is 0, as nvm.sh does.
///
/// DELIBERATE DEVIATION: on an invalid setting nvm.sh also prints its whole
/// `nvm --help` on stderr before the error; the port does not (its help is
/// clap's).
///
/// # Errors
/// [`CliError::Shell`] only if the script cannot be built, which the fixed
/// variable name makes a programming error.
pub fn run(context: &Context<'_>, args: &[String]) -> Result<Output, CliError> {
    let setting = args.first().map_or("", String::as_str);
    if !is_valid_setting(setting) {
        return Ok(Output::default()
            .with_blank_stdout()
            .with_stderr(INVALID_SETTING));
    }
    let no_colors = context.env.var("NVM_NO_COLORS").as_deref() == Some(NO_COLORS_FLAG);
    let colors = color_policy::detect(context, no_colors).enabled;
    let script = Script::new().export(VARIABLE, setting)?;
    Ok(Output::stdout(confirmation(setting, colors)).with_script(script))
}

/// The confirmation of `nvm_set_colors`: the letters each in their own
/// color and unseparated, or spaced and followed by the warning.
fn confirmation(setting: &str, colors: bool) -> String {
    if colors {
        let letters: String = setting
            .chars()
            .map(|letter| wrap(sgr(letter), &letter.to_string()))
            .collect();
        return format!("{CONFIRMATION}{letters}");
    }
    let letters: Vec<String> = setting.chars().map(String::from).collect();
    format!("{CONFIRMATION}{}\n{NO_COLORS_WARNING}", letters.join(" "))
}
```

Create `src/commands/set_colors/tests.rs`:

```rust
use super::*;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess, FakeTerminal};

const TPUT: &str = "/usr/bin/tput";
const COLORED: &str = "Setting colors to: \x1b[0;31mr\x1b[0m\x1b[0;32mg\x1b[0m\
\x1b[0;34mb\x1b[0m\x1b[0;36mc\x1b[0m\x1b[0;35mm\x1b[0m";
const PLAIN: &str = "Setting colors to: r g b c m\n\
WARNING: Colors may not display because they are not supported in this shell.";
const INVALID: &str = "\x1b[1;37mPlease pass in five \x1b[1;31mvalid color codes\x1b[1;37m. \
Choose from: rRgGbBcCyYmMkKeW\x1b[0m";

/// A machine whose stdout is `terminal`, with a `tput` reporting 256 colors
/// for `xterm-256color`, and `variables` in the environment.
fn set_colors(terminal: &FakeTerminal, variables: &[(&str, &str)], args: &[&str]) -> Output {
    let fs = FakeFileSystem::default().with_file(TPUT, "");
    let env = variables.iter().fold(
        FakeEnv::default()
            .with_var("PATH", "/usr/bin")
            .with_var("TERM", "xterm-256color"),
        |env, (name, value)| env.with_var(name, value),
    );
    let process = FakeProcess::default().with_run(TPUT, "-T xterm-256color colors", true, "256\n");
    let context = Context::new(&fs, &env)
        .with_terminal(terminal)
        .with_process(&process);
    let args: Vec<String> = args.iter().map(|&argument| argument.to_owned()).collect();
    run(&context, &args).unwrap()
}

fn on_a_terminal(args: &[&str]) -> Output {
    set_colors(&FakeTerminal::terminal(), &[], args)
}

fn exported(setting: &str) -> String {
    format!("export NVM_COLORS='{setting}'\n")
}

#[test]
fn a_valid_setting_on_a_terminal_shows_each_letter_in_its_own_color() {
    let output = on_a_terminal(&["rgbcm"]);
    assert_eq!(output.stdout, COLORED);
    assert_eq!(output.stderr, "");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(!output.blank_stdout);
}

#[test]
fn a_valid_setting_exports_nvm_colors_for_the_shell() {
    assert_eq!(on_a_terminal(&["rgbcm"]).script.render(), exported("rgbcm"));
    let pipe = set_colors(&FakeTerminal::pipe(), &[], &["RGBCM"]);
    assert_eq!(pipe.script.render(), exported("RGBCM"));
}

#[test]
fn bold_letters_and_the_whites_take_their_own_codes() {
    let output = on_a_terminal(&["YkKeW"]);
    let expected = "Setting colors to: \x1b[1;33mY\x1b[0m\x1b[0;30mk\x1b[0m\
\x1b[1;30mK\x1b[0m\x1b[0;37me\x1b[0m\x1b[1;37mW\x1b[0m";
    assert_eq!(output.stdout, expected);
}

#[test]
fn without_colors_the_letters_are_spaced_and_a_warning_follows() {
    let output = set_colors(&FakeTerminal::pipe(), &[], &["rgbcm"]);
    assert_eq!(output.stdout, PLAIN);
    assert_eq!(output.stderr, "");
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn nvm_no_colors_set_to_the_flag_turns_the_colors_off() {
    let output = set_colors(
        &FakeTerminal::terminal(),
        &[("NVM_NO_COLORS", "--no-colors")],
        &["rgbcm"],
    );
    assert_eq!(output.stdout, PLAIN);
    assert_eq!(output.script.render(), exported("rgbcm"));
}

#[test]
fn other_values_of_nvm_no_colors_are_ignored() {
    for value in ["1", "", "--no-color", "true"] {
        let output = set_colors(
            &FakeTerminal::terminal(),
            &[("NVM_NO_COLORS", value)],
            &["rgbcm"],
        );
        assert_eq!(output.stdout, COLORED, "{value:?}");
    }
}

#[test]
fn nvm_has_colors_does_not_force_the_colors() {
    let output = set_colors(
        &FakeTerminal::pipe(),
        &[("NVM_HAS_COLORS", "1")],
        &["rgbcm"],
    );
    assert_eq!(output.stdout, PLAIN);
}

#[test]
fn arguments_after_the_first_are_ignored_as_in_nvm_sh() {
    let output = on_a_terminal(&["rgbcm", "zzz"]);
    assert_eq!(output.stdout, COLORED);
    assert_eq!(output.script.render(), exported("rgbcm"));
}

fn assert_invalid(output: &Output, shape: &str) {
    assert_eq!(output.stdout, "", "{shape}");
    assert!(output.blank_stdout, "{shape}");
    assert_eq!(output.stderr, INVALID, "{shape}");
    assert_eq!(output.status, NvmExitCode::Success, "{shape}");
    assert!(output.script.is_empty(), "{shape}");
}

#[test]
fn a_missing_setting_is_invalid_with_status_0_and_no_shell_code() {
    assert_invalid(&on_a_terminal(&[]), "missing");
}

#[test]
fn every_invalid_shape_prints_the_forced_color_error_and_exports_nothing() {
    let shapes: [&[&str]; 9] = [
        &[""],
        &["rgbc"],
        &["rgbcmy"],
        &["rgbc0"],
        &["00000"],
        &["rgbcz"],
        &["rgbcé"],
        &["--no-colors"],
        &["x", "rgbcm"],
    ];
    for shape in shapes {
        let label = format!("{shape:?}");
        assert_invalid(&on_a_terminal(shape), &label);
        assert_invalid(&set_colors(&FakeTerminal::pipe(), &[], shape), &label);
    }
}

#[test]
fn the_error_is_colored_even_when_colors_are_off() {
    let output = set_colors(
        &FakeTerminal::pipe(),
        &[("NVM_NO_COLORS", "--no-colors")],
        &["nope"],
    );
    assert_invalid(&output, "nope");
}
```

Apply to `src/shell/init/tests.rs`:

```diff
--- a/src/shell/init/tests.rs
+++ b/src/shell/init/tests.rs
@@ -78,7 +78,7 @@
     let zsh = snippet(Shell::Zsh, &InitOptions::default());
     assert!(zsh.contains("\nfunction nvm {\n"), "{zsh}");
     assert!(text.contains("NVMRC_SCRIPT_FD=3 command \\nvm \"$@\" 3>&1 1>&4 4>&-"));
-    assert!(text.contains("use | deactivate | install | i | __auto)"));
+    assert!(text.contains("use | deactivate | install | i | set-colors | __auto)"));
     assert!(!text.contains("command nvm"), "{text}");
     assert!(text.contains("eval \"$__nvmrc_code\""));
 }
@@ -155,7 +155,9 @@
     let text = snippet(Shell::Fish, &InitOptions::default());
     assert!(text.contains("\nfunction nvm "), "{text}");
     assert!(
-        text.contains("if not contains -- \"$argv[1]\" use deactivate install i __auto\n"),
+        text.contains(
+            "if not contains -- \"$argv[1]\" use deactivate install i set-colors __auto\n"
+        ),
         "{text}"
     );
     let capture = "NVMRC_SCRIPT_FD=3 NVMRC_SHELL_KIND=fish command nvm $argv 3>&1 1>&4 4>&- \
```

Apply to `tests/init_cli.rs`:

```diff
--- a/tests/init_cli.rs
+++ b/tests/init_cli.rs
@@ -234,3 +234,35 @@
     let expected = format!("BIN={bin}\nPATH={bin}:{}\n", lab.base_path());
     assert_eq!(run.stdout, expected, "{}", run.stderr);
 }
+
+const SET_COLORS_PLAIN: &str = "Setting colors to: r g b c m\n\
+WARNING: Colors may not display because they are not supported in this shell.\n";
+
+#[test]
+fn set_colors_keeps_nvm_colors_in_the_shell() {
+    let lab = Lab::new();
+    each_shell(|name, shell| {
+        let commands = "nvm set-colors rgbcm\n/usr/bin/printenv NVM_COLORS\necho \"$NVM_COLORS\"";
+        let run = lab.run(shell, &with_function(name, commands));
+        let expected = format!("{SET_COLORS_PLAIN}rgbcm\nrgbcm\n");
+        assert_eq!(
+            (run.status, run.stdout.as_str(), run.stderr.as_str()),
+            (0, expected.as_str(), ""),
+            "{name}"
+        );
+    });
+}
+
+#[test]
+fn an_invalid_set_colors_leaves_nvm_colors_unchanged() {
+    let lab = Lab::new();
+    each_shell(|name, shell| {
+        let commands = format!(
+            "nvm set-colors rgbcm >/dev/null\nnvm set-colors rgbc0 2>/dev/null\necho \"status={}\"\n\
+/usr/bin/printenv NVM_COLORS",
+            in_dialect(name, "$?", "$status")
+        );
+        let run = lab.run(shell, &with_function(name, &commands));
+        assert_eq!(run.stdout, "\nstatus=0\nrgbcm\n", "{name}: {}", run.stderr);
+    });
+}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL to compile with errors such as "cannot find module
`set_colors`", "no field `blank_stdout` on type `Output`" and "no variant
named `SetColors` found".

- [ ] **Step 4: Write the implementation**

Apply to `src/cli/channel.rs` (above the test module):

```diff
--- a/src/cli/channel.rs
+++ b/src/cli/channel.rs
@@ -31,9 +31,18 @@
     }
 }
 
+/// The stdout text as a line, or the empty line the command asked for.
+fn stdout_of(output: &Output) -> String {
+    if output.stdout.is_empty() && output.blank_stdout {
+        "\n".to_owned()
+    } else {
+        line(&output.stdout)
+    }
+}
+
 pub(super) fn deliver(output: &Output, context: &Context<'_>) -> Delivery {
     let mut delivery = Delivery {
-        stdout: line(&output.stdout),
+        stdout: stdout_of(output),
         stderr: line(&output.stderr),
         status: output.status,
     };
```

Apply to `src/cli/commands.rs` (above the test module):

```diff
--- a/src/cli/commands.rs
+++ b/src/cli/commands.rs
@@ -95,6 +95,14 @@
         #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
         args: Vec<String>,
     },
+    /// Set the five colors of `ls`, `ls-remote` and `alias` (`NVM_COLORS`),
+    /// from `rRgGbBcCyYmMkKeW`: installed, system, current, not installed,
+    /// default.
+    #[command(name = "set-colors")]
+    SetColors {
+        #[arg(allow_hyphen_values = true, trailing_var_arg = true)]
+        args: Vec<String>,
+    },
     // DELIBERATE DEVIATION: unlike nvm.sh, `-h`, `help` and `--help` after
     // `exec` and `run` are not taken for a request for nvm's help: every
     // argument after the subcommand is passed through to the command.
@@ -147,6 +155,7 @@
         Command::Unalias { names } => commands::unalias::run(context, names),
         Command::Use { args } => commands::use_version::run(context, args),
         Command::Deactivate { args } => commands::deactivate::run(context, args),
+        Command::SetColors { args } => commands::set_colors::run(context, args),
         Command::Exec { args } => commands::exec::run(context, args),
         Command::Run { args } => commands::run::run(context, args),
         Command::Init { args } => commands::init::run(args),
```

Apply to `src/cli/mod.rs` (above the test module):

```diff
--- a/src/cli/mod.rs
+++ b/src/cli/mod.rs
@@ -74,13 +74,17 @@
     finish("nvm", &output, context, out, err)
 }
 
+/// The commands that see a `--` right after their name.
+const KEEP_DOUBLE_DASH: [&str; 3] = ["exec", "run", "set-colors"];
+
 /// clap takes a `--` right after a subcommand for the end of the options
-/// and drops it, but nvm.sh's `exec` stops its own options there and `run`
-/// hands it to the script; doubling it makes clap pass the user's one on.
+/// and drops it, but nvm.sh's `exec` stops its own options there, `run`
+/// hands it to the script and `set-colors` takes it for the (invalid)
+/// setting; doubling it makes clap pass the user's one on.
 fn keep_leading_double_dash(mut args: Vec<OsString>) -> Vec<OsString> {
     let passes_it_on = args
         .get(1)
-        .is_some_and(|command| command == "exec" || command == "run");
+        .is_some_and(|command| KEEP_DOUBLE_DASH.iter().any(|name| command == name));
     if passes_it_on && args.get(2).is_some_and(|argument| argument == "--") {
         args.insert(2, OsString::from("--"));
     }
```

Apply to `src/commands/auto/mod.rs` (above the test module):

```diff
--- a/src/commands/auto/mod.rs
+++ b/src/commands/auto/mod.rs
@@ -95,6 +95,7 @@
     match result {
         Ok(output) => Output {
             stdout: String::new(),
+            blank_stdout: false,
             ..output
         },
         Err(CliError::NotInstalled) => {
```

Apply to `src/commands/mod.rs` (above the test module):

```diff
--- a/src/commands/mod.rs
+++ b/src/commands/mod.rs
@@ -39,6 +39,9 @@
 pub struct Output {
     pub stdout: String,
     pub stderr: String,
+    /// With an empty [`Self::stdout`], print an empty line there (a bare
+    /// `nvm_echo`) instead of nothing.
+    pub blank_stdout: bool,
     pub status: NvmExitCode,
     /// Shell code the generated `nvm` function must `eval`; empty for a
     /// command that changes nothing in the calling shell.
@@ -60,6 +63,13 @@
     #[must_use]
     pub fn with_stderr(mut self, text: impl Into<String>) -> Self {
         self.stderr = text.into();
+        self
+    }
+
+    /// An empty line on stdout when the stdout text is empty.
+    #[must_use]
+    pub fn with_blank_stdout(mut self) -> Self {
+        self.blank_stdout = true;
         self
     }
 
```

Insert above the `#[cfg(test)]` line of `src/commands/set_colors/mod.rs`:

```rust
//! `nvm set-colors <five letters>`: `nvm_set_colors` and its case branch.
//!
//! The binary cannot change its parent shell, so the `export NVM_COLORS`
//! of nvm.sh is returned as a [`Script`] for the `nvm` function to eval
//! (the init snippets capture the code of `set-colors`, as of `use`).

use crate::commands::Output;
use crate::commands::color_policy;
use crate::context::Context;
use crate::domain::colors::{is_valid_setting, sgr, wrap};
use crate::error::CliError;
use crate::shell::Script;

```

Apply to `src/shell/init/fish.rs` (above the test module):

```diff
--- a/src/shell/init/fish.rs
+++ b/src/shell/init/fish.rs
@@ -23,7 +23,7 @@
 "#;
 
 const FUNCTION: &str = r#"function nvm --description 'Node Version Manager (nvmrc)'
-    if not contains -- "$argv[1]" use deactivate install i __auto
+    if not contains -- "$argv[1]" use deactivate install i set-colors __auto
         begin
             @EXPORTS@
             command nvm $argv
```

Apply to `src/shell/init/mod.rs` (above the test module):

```diff
--- a/src/shell/init/mod.rs
+++ b/src/shell/init/mod.rs
@@ -3,10 +3,10 @@
 //! one POSIX function for bash, zsh, sh, dash and ksh (`posix.rs`), and one
 //! fish function for fish 3.4 or newer (`fish.rs`).
 //!
-//! The function runs `use`, `deactivate`, `install` (and `i`) and `__auto`
-//! with `NVMRC_SCRIPT_FD=3`: the binary writes the shell code for the
-//! calling shell to descriptor 3, which the function captures and
-//! evaluates, while stdout and stderr pass straight through (so `nvm use 18
+//! The function runs `use`, `deactivate`, `install` (and `i`),
+//! `set-colors` and `__auto` with `NVMRC_SCRIPT_FD=3`: the binary writes
+//! the shell code for the calling shell to descriptor 3, which the function
+//! captures and evaluates, while stdout and stderr pass straight through (so `nvm use 18
 //! >/dev/null` silences only the messages); the function returns the
 //! binary's status. Every other command is the binary, untouched, without
 //! descriptor 3. Both export the variables the binary reads that a shell
```

Apply to `src/shell/init/posix.rs` (above the test module):

```diff
--- a/src/shell/init/posix.rs
+++ b/src/shell/init/posix.rs
@@ -15,7 +15,7 @@
 
 const BODY: &str = r#"
   case "${1-}" in
-    use | deactivate | install | i | __auto) ;;
+    use | deactivate | install | i | set-colors | __auto) ;;
     *)
       (
         @EXPORTS@
```

Then run `cargo fmt`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test`
Expected: PASS: 1137 unit tests plus the end-to-end tests (the fish cases skip
when `fish` is not installed).

- [ ] **Step 6: Run the quality gate and commit**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings and the crate still builds on
MSRV 1.85.

```bash
git add src tests
git commit -S -m "feat(colors): add set-colors"
```

---

### Task 7: Forced `.nvmrc` message, `NVM_COLORS` in `init`, and the gate

**Files:**

- Create: `tests/init_colors_cli.rs`
- Modify: `src/commands/install/lock/mod.rs`,
  `src/commands/npm/packages/tests.rs`, `src/commands/rc_version/mod.rs`,
  `src/commands/rc_version/tests.rs`, `src/domain/nvmrc/mod.rs`,
  `src/domain/nvmrc/tests.rs`, `src/domain/remote/mod.rs`,
  `src/domain/remote_format/tests.rs`, `src/shell/init/mod.rs`,
  `src/shell/init/tests.rs`

**Interfaces:**

- Produces: `domain::nvmrc::colored_invalid_message(&[String]) -> String`; the
  invalid `.nvmrc` message of `rc_version` is colored (red block, blank line,
  yellow parsed block) ONLY when `NVM_HAS_COLORS` is exactly `1` (digest 3.8);
  the variables that the init function (POSIX and fish) passes to the binary
  only when the shell has them set are now `MANPATH`, `NODE_PATH`, `NVM_COLORS`,
  `NVM_HAS_COLORS`, `NVM_NO_COLORS`, `NVM_SYMLINK_CURRENT` and `PREFIX`
  (`PASSED` in `shell/init/mod.rs`), so an UNEXPORTED `NVM_COLORS` of the user's
  shell reaches the binary, as it reaches `nvm.sh`; a new end-to-end file
  `tests/init_colors_cli.rs` proves it in every installed shell. Five functions
  that were over 30 lines are split (`install/lock::wait_for_lock`,
  `domain/remote::flavor_rows` and `marked_rows`, and two test helpers); six
  that are exactly 30 lines are left, as `clippy::too_many_lines` measures.

- [ ] **Step 1: Write the failing tests**

Apply to `src/commands/npm/packages/tests.rs`:

```diff
--- a/src/commands/npm/packages/tests.rs
+++ b/src/commands/npm/packages/tests.rs
@@ -46,11 +46,8 @@
         .collect()
 }
 
-/// The expectations are what `nvm install --reinstall-packages-from` of the
-/// real `nvm.sh` printed and ran, with a fake `npm`.
-#[test]
-fn the_packages_are_installed_in_one_command_and_the_links_are_linked() {
-    let process = FakeProcess::default()
+fn npm_that_installs_and_links() -> FakeProcess {
+    FakeProcess::default()
         .with_success(OLD_NPM, "list -g --depth=0", LISTING)
         .with_success(
             NEW_NPM,
@@ -62,7 +59,14 @@
             "root -g",
             "/n/versions/node/v20.10.0/lib/node_modules\n",
         )
-        .with_success(NEW_NPM, "link", "linked\n");
+        .with_success(NEW_NPM, "link", "linked\n")
+}
+
+/// The expectations are what `nvm install --reinstall-packages-from` of the
+/// real `nvm.sh` printed and ran, with a fake `npm`.
+#[test]
+fn the_packages_are_installed_in_one_command_and_the_links_are_linked() {
+    let process = npm_that_installs_and_links();
     let (status, stdout, stderr) = run_reinstall(&Source::Version(version("v18.19.0")), &process);
     assert_eq!(status, NvmExitCode::Success);
     assert_eq!(
```

Apply to `src/commands/rc_version/tests.rs`:

```diff
--- a/src/commands/rc_version/tests.rs
+++ b/src/commands/rc_version/tests.rs
@@ -97,6 +97,36 @@
     assert!(output.stdout.is_empty());
 }
 
+fn lookup_with_has_colors(value: &str) -> Output {
+    let fs = project("foo=bar\n");
+    let env = FakeEnv::default()
+        .with_var("PWD", "/proj")
+        .with_var("NVM_HAS_COLORS", value);
+    let mut transcript = Transcript::default();
+    rc_version(&Context::new(&fs, &env), false, &mut transcript);
+    transcript.finish(NvmExitCode::Success)
+}
+
+#[test]
+fn an_exported_nvm_has_colors_of_one_colors_the_invalid_message() {
+    let output = lookup_with_has_colors("1");
+    assert!(output.stderr.starts_with("\x1b[0;31minvalid .nvmrc!\n"));
+    assert!(
+        output
+            .stderr
+            .ends_with("\x1b[0;33mnon-commented content parsed:\nfoo=bar\x1b[0m")
+    );
+}
+
+#[test]
+fn any_other_nvm_has_colors_value_keeps_the_invalid_message_plain() {
+    for value in ["0", "true", "", " 1"] {
+        let output = lookup_with_has_colors(value);
+        assert!(output.stderr.starts_with("invalid .nvmrc!\n"), "{value:?}");
+        assert!(!output.stderr.contains('\x1b'), "{value:?}");
+    }
+}
+
 #[test]
 fn an_empty_parse_ends_right_after_the_header_line() {
     let (_, output) = lookup(&project("# only\n"), Some("/proj"), false);
```

Apply to `src/domain/nvmrc/tests.rs`:

```diff
--- a/src/domain/nvmrc/tests.rs
+++ b/src/domain/nvmrc/tests.rs
@@ -96,6 +96,33 @@
     );
 }
 
+const COLORED_ERROR_BLOCK: &str = "\x1b[0;31minvalid .nvmrc!\n\
+all non-commented content (anything after # is a comment) must be either:\n\
+\x20 - a single bare nvm-recognized version-ish\n\
+\x20 - or, multiple distinct key-value pairs, each key/value separated by a single equals sign (=)\n\
+\n\
+additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).\x1b[0m\n\
+\n";
+
+#[test]
+fn the_colored_invalid_message_wraps_the_error_red_and_the_parsed_block_yellow() {
+    let expected =
+        format!("{COLORED_ERROR_BLOCK}\x1b[0;33mnon-commented content parsed:\nfoo=bar\x1b[0m");
+    assert_eq!(colored_invalid_message(&["foo=bar".to_owned()]), expected);
+}
+
+#[test]
+fn the_colored_invalid_message_joins_the_parsed_lines() {
+    let message = colored_invalid_message(&["18".to_owned(), "20".to_owned()]);
+    assert!(message.ends_with("\x1b[0;33mnon-commented content parsed:\n18\n20\x1b[0m"));
+}
+
+#[test]
+fn the_colored_invalid_message_keeps_the_newline_before_the_reset_when_nothing_was_parsed() {
+    let expected = format!("{COLORED_ERROR_BLOCK}\x1b[0;33mnon-commented content parsed:\n\x1b[0m");
+    assert_eq!(colored_invalid_message(&[]), expected);
+}
+
 #[test]
 fn please_see_is_the_exact_hint() {
     assert_eq!(
```

Apply to `src/domain/remote_format/tests.rs`:

```diff
--- a/src/domain/remote_format/tests.rs
+++ b/src/domain/remote_format/tests.rs
@@ -22,8 +22,27 @@
         .collect()
 }
 
-/// Every expected line below is what the real nvm.sh printed for the same
-/// mirror, installed versions and aliases.
+/// What the real nvm.sh printed, plain, for the same mirror with nothing
+/// installed and no named alias.
+const PLAIN_LISTING: &[&str] = &[
+    "       v0.10.48",
+    "       v0.12.18",
+    "    iojs-v1.0.0",
+    "    iojs-v2.5.0",
+    "    iojs-v3.0.0",
+    "    iojs-v3.3.1",
+    "         v4.0.0",
+    "         v4.9.1   (Latest LTS: Argon)",
+    "       v14.21.3   (Latest LTS: Fermium)",
+    "       v16.20.2   (Latest LTS: Gallium)",
+    "       v18.18.0   (LTS: Hydrogen)",
+    "       v18.19.0   (Latest LTS: Hydrogen)",
+    "        v20.9.0   (LTS: Iron)",
+    "       v20.10.0   (Latest LTS: Iron)",
+    "        v21.1.0",
+    "        v21.2.0                            (Latest: node)",
+];
+
 #[test]
 fn a_plain_listing_matches_nvm_sh() {
     let rows = rows(None);
@@ -34,27 +53,7 @@
         latest_alias: Some("node"),
         named_aliases: &[],
     };
-    assert_eq!(
-        format_remote_rows(&input),
-        [
-            "       v0.10.48",
-            "       v0.12.18",
-            "    iojs-v1.0.0",
-            "    iojs-v2.5.0",
-            "    iojs-v3.0.0",
-            "    iojs-v3.3.1",
-            "         v4.0.0",
-            "         v4.9.1   (Latest LTS: Argon)",
-            "       v14.21.3   (Latest LTS: Fermium)",
-            "       v16.20.2   (Latest LTS: Gallium)",
-            "       v18.18.0   (LTS: Hydrogen)",
-            "       v18.19.0   (Latest LTS: Hydrogen)",
-            "        v20.9.0   (LTS: Iron)",
-            "       v20.10.0   (Latest LTS: Iron)",
-            "        v21.1.0",
-            "        v21.2.0                            (Latest: node)",
-        ]
-    );
+    assert_eq!(format_remote_rows(&input), PLAIN_LISTING);
 }
 
 /// Every release with its installed mark and its aliases.
```

Apply to `src/shell/init/tests.rs`:

```diff
--- a/src/shell/init/tests.rs
+++ b/src/shell/init/tests.rs
@@ -1,5 +1,17 @@
 use super::*;
 use crate::error::NvmExitCode;
+
+/// What the binary reads that a shell may hold unexported: nvm.sh sees the
+/// shell's own variables, the color settings included.
+const PASSED_VARIABLES: [&str; 7] = [
+    "MANPATH",
+    "NODE_PATH",
+    "NVM_COLORS",
+    "NVM_HAS_COLORS",
+    "NVM_NO_COLORS",
+    "NVM_SYMLINK_CURRENT",
+    "PREFIX",
+];
 
 const ALL: [(&str, Shell); 6] = [
     ("bash", Shell::Bash),
@@ -86,7 +98,7 @@
 #[test]
 fn every_command_gets_the_passed_variables_exported_only_when_set() {
     let text = snippet(Shell::Ksh, &InitOptions::default());
-    for passed in ["MANPATH", "NODE_PATH", "NVM_SYMLINK_CURRENT", "PREFIX"] {
+    for passed in PASSED_VARIABLES {
         let export = format!("if [ -n \"${{{passed}+set}}\" ]; then export {passed}; fi");
         assert_eq!(text.matches(&export).count(), 2, "{passed}: {text}");
         assert!(!text.contains(&format!("{passed}=")), "{passed}: {text}");
@@ -185,7 +197,7 @@
 #[test]
 fn fish_passes_the_variables_to_the_binary_only_when_set() {
     let text = snippet(Shell::Fish, &InitOptions::default());
-    for passed in ["MANPATH", "NODE_PATH", "NVM_SYMLINK_CURRENT", "PREFIX"] {
+    for passed in PASSED_VARIABLES {
         let export = format!("set -q {passed}; and set -lx {passed} ${passed}\n");
         assert_eq!(text.matches(&export).count(), 2, "{passed}: {text}");
         assert!(!text.contains(&format!("{passed}=")), "{passed}: {text}");
```

Create `tests/init_colors_cli.rs`:

```rust
//! End-to-end: the color settings a shell holds without exporting them
//! (`NVM_COLORS`, `NVM_HAS_COLORS`) reach the binary through the `nvm`
//! function, as nvm.sh (a shell function) sees them, and unset ones stay
//! unset for the programs the binary starts.
#![cfg(unix)]

mod init_lab;

use init_lab::{Lab, each_shell, in_dialect, with_function};

const ALIAS_UNEXPORTED: &str = "NVM_COLORS=rgbcm\nNVM_HAS_COLORS=1\nnvm alias foo 20";
const ALIAS_UNEXPORTED_FISH: &str = "set NVM_COLORS rgbcm\nset NVM_HAS_COLORS 1\nnvm alias foo 20";
/// `v20.11.1` is installed and not current: the installed role, `r` here.
const ALIAS_IN_RED: &str = "\x1b[0;31mfoo\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31m20\x1b[0m \
(\x1b[0;90m->\x1b[0m \x1b[0;31mv20.11.1\x1b[0m)\n";

#[test]
fn unexported_color_settings_reach_alias() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let commands = in_dialect(name, ALIAS_UNEXPORTED, ALIAS_UNEXPORTED_FISH);
        let run = lab.run(shell, &with_function(name, commands));
        assert_eq!(run.stdout, ALIAS_IN_RED, "{name}: {}", run.stderr);
    });
}

#[test]
fn an_unexported_color_setting_reaches_ls() {
    let lab = Lab::new();
    let ls = "nvm ls >/dev/null";
    let invalid = format!("NVM_COLORS=rgbzm\n{ls}");
    let invalid_fish = format!("set NVM_COLORS rgbzm\n{ls}");
    each_shell(|name, shell| {
        let cases = [
            (ls, ""),
            (
                in_dialect(name, &invalid, &invalid_fish),
                "Invalid color code: z\n",
            ),
        ];
        for (commands, expected) in cases {
            let run = lab.run(shell, &with_function(name, commands));
            assert_eq!(run.stderr, expected, "{name}: {commands}");
        }
    });
}

#[test]
fn unset_color_settings_stay_unset_for_children() {
    let lab = Lab::new();
    let child = "echo \"${NVM_COLORS-unset} ${NVM_HAS_COLORS-unset} ${NVM_NO_COLORS-unset}\"";
    let commands = format!("nvm exec --silent 18 /bin/sh -c '{child}'");
    each_shell(|name, shell| {
        let run = lab.run(shell, &with_function(name, &commands));
        assert_eq!(run.stdout, "unset unset unset\n", "{name}: {}", run.stderr);
    });
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test`
Expected: FAIL by assertion: the invalid `.nvmrc` message is plain even with
`NVM_HAS_COLORS=1`, an unexported `NVM_COLORS` does not reach the binary
through the init function, and the five functions that grew past 30 lines are
still over it.

- [ ] **Step 3: Apply the changes**

Apply to `src/commands/install/lock/mod.rs` (above the test module):

```diff
--- a/src/commands/install/lock/mod.rs
+++ b/src/commands/install/lock/mod.rs
@@ -82,6 +82,18 @@
         return Ok(None);
     }
     let path = request.root.join(lock_name(request.version));
+    wait_for_lock(fs, sleeper, request, path, notes)
+}
+
+/// Retries the lock directory once a second, stealing a stale one, until it
+/// is created or `timeout_seconds` have passed.
+fn wait_for_lock<'a>(
+    fs: &'a dyn FileSystem,
+    sleeper: &dyn Sleeper,
+    request: &LockRequest<'_>,
+    path: PathBuf,
+    notes: &mut Vec<String>,
+) -> Result<Option<InstallLock<'a>>, CliError> {
     let mut waited = 0;
     loop {
         match fs.create_dir(&path) {
```

Apply to `src/commands/rc_version/mod.rs` (above the test module):

```diff
--- a/src/commands/rc_version/mod.rs
+++ b/src/commands/rc_version/mod.rs
@@ -3,10 +3,11 @@
 
 use std::path::PathBuf;
 
+use crate::commands::color_policy;
 use crate::commands::transcript::Transcript;
 use crate::context::Context;
 use crate::domain::nvmrc::{
-    NvmrcContent, PLEASE_SEE, find_nvmrc, invalid_message, process_content,
+    NvmrcContent, PLEASE_SEE, colored_invalid_message, find_nvmrc, invalid_message, process_content,
 };
 
 const MISSING_MESSAGE: &str = "No version provided and no .nvmrc file found";
@@ -54,10 +55,20 @@
             RcVersion::Found { path, version }
         }
         NvmrcContent::Invalid { parsed } => {
-            transcript.err(invalid_message(&parsed).trim_end_matches('\n'));
+            transcript.err(invalid_report(context, &parsed));
             RcVersion::Invalid
         }
     }
+}
+
+/// The invalid-file message: colored only when the exported
+/// `NVM_HAS_COLORS=1` forces it, since nvm.sh wraps it inside a command
+/// substitution where the terminal check always fails.
+fn invalid_report(context: &Context<'_>, parsed: &[String]) -> String {
+    if color_policy::forced(context) {
+        return colored_invalid_message(parsed);
+    }
+    invalid_message(parsed).trim_end_matches('\n').to_owned()
 }
 
 /// The closing stderr line `use` and `install` add after an unusable
```

Apply to `src/domain/nvmrc/mod.rs` (above the test module):

```diff
--- a/src/domain/nvmrc/mod.rs
+++ b/src/domain/nvmrc/mod.rs
@@ -3,6 +3,7 @@
 
 use std::path::{Path, PathBuf};
 
+use crate::domain::colors::{sgr, wrap};
 use crate::ports::FileSystem;
 
 pub const NVMRC_FILE_NAME: &str = ".nvmrc";
@@ -11,15 +12,16 @@
 pub const PLEASE_SEE: &str =
     "Please see `nvm --help` or https://github.com/nvm-sh/nvm#nvmrc for more information.";
 
-const INVALID_HEADER: &str = "invalid .nvmrc!
+/// The red block of `nvm_nvmrc_invalid_msg` (its `error_text`).
+const INVALID_ERROR_TEXT: &str = "invalid .nvmrc!
 all non-commented content (anything after # is a comment) must be either:
   - a single bare nvm-recognized version-ish
   - or, multiple distinct key-value pairs, each key/value separated by a single equals sign (=)
 
-additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).
+additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).";
 
-non-commented content parsed:
-";
+/// The first line of the yellow block (its `warn_text`).
+const PARSED_HEADING: &str = "non-commented content parsed:";
 
 /// The outcome of applying the `.nvmrc` content rules.
 #[derive(Debug, Clone, PartialEq, Eq)]
@@ -101,7 +103,7 @@
 /// newline, without the closing [`PLEASE_SEE`] line (the caller adds it).
 #[must_use]
 pub fn invalid_message(parsed: &[String]) -> String {
-    let mut message = String::from(INVALID_HEADER);
+    let mut message = format!("{INVALID_ERROR_TEXT}\n\n{PARSED_HEADING}\n");
     for line in parsed {
         message.push_str(line);
         message.push('\n');
@@ -109,3 +111,18 @@
     message
 }
 
+/// [`invalid_message`] as nvm.sh prints it when `NVM_HAS_COLORS=1` forces
+/// `nvm_wrap_with_color_code`: the error block in red, the parsed block in
+/// yellow (fixed letters, whatever `NVM_COLORS` says), without a closing
+/// newline. The parsed block keeps its newline before the reset when nothing
+/// was parsed, as the reset follows `${1}` directly.
+#[must_use]
+pub fn colored_invalid_message(parsed: &[String]) -> String {
+    let parsed_block = format!("{PARSED_HEADING}\n{}", parsed.join("\n"));
+    format!(
+        "{}\n\n{}",
+        wrap(sgr('r'), INVALID_ERROR_TEXT),
+        wrap(sgr('y'), &parsed_block)
+    )
+}
+
```

Apply to `src/domain/remote/mod.rs` (above the test module):

```diff
--- a/src/domain/remote/mod.rs
+++ b/src/domain/remote/mod.rs
@@ -62,26 +62,32 @@
         iojs_runs,
     } = scope(query)?;
 
-    let mut missing = false;
-    let mut node_rows = Vec::new();
-    let mut iojs_rows = Vec::new();
-    if node_runs {
-        node_rows = node
-            .map(|releases| select(releases, pattern, query.lts.as_deref(), false))
-            .unwrap_or_default();
-        missing |= node_rows.is_empty();
-    }
-    if iojs_runs {
-        iojs_rows = iojs
-            .map(|releases| select(releases, pattern, None, true))
-            .unwrap_or_default();
-        missing |= iojs_rows.is_empty();
-    }
+    let (node_rows, node_missing) = flavor_rows(node, node_runs, |releases| {
+        select(releases, pattern, query.lts.as_deref(), false)
+    });
+    let (iojs_rows, iojs_missing) = flavor_rows(iojs, iojs_runs, |releases| {
+        select(releases, pattern, None, true)
+    });
     let rows = merge(node_rows, iojs_rows);
     Ok(Listing {
-        missing: missing || rows.is_empty(),
+        missing: node_missing || iojs_missing || rows.is_empty(),
         rows,
     })
+}
+
+/// The rows of one flavor when the query reads it, and whether it came up
+/// empty (an unreadable index included); nothing and not missing otherwise.
+fn flavor_rows(
+    releases: Option<&[Release]>,
+    runs: bool,
+    pick: impl Fn(&[Release]) -> Vec<RemoteRow>,
+) -> (Vec<RemoteRow>, bool) {
+    if !runs {
+        return (Vec::new(), false);
+    }
+    let rows = releases.map(pick).unwrap_or_default();
+    let missing = rows.is_empty();
+    (rows, missing)
 }
 
 /// Which indexes a query reads, and the pattern left once a flavor word
@@ -137,12 +143,22 @@
     let pattern = pattern
         .map(|text| normalize_pattern(text, iojs))
         .filter(|text| !text.is_empty());
+    let mut rows = marked_rows(releases, lts);
+    rows.retain(|row| {
+        pattern
+            .as_deref()
+            .is_none_or(|text| matches_word(row, text))
+    });
+    rows.sort_by_key(|row| row.version);
+    rows
+}
+
+/// The releases that fit the LTS filter, in index order (newest first), the
+/// first of each codename marked as its latest.
+fn marked_rows(releases: &[Release], lts: Option<&str>) -> Vec<RemoteRow> {
     let mut previous: Option<&str> = None;
     let mut rows = Vec::new();
-    for release in releases {
-        if !fits_lts(release, lts) {
-            continue;
-        }
+    for release in releases.iter().filter(|release| fits_lts(release, lts)) {
         let name = release.lts.as_deref();
         rows.push(RemoteRow {
             version: release.version,
@@ -151,12 +167,6 @@
         });
         previous = name;
     }
-    rows.retain(|row| {
-        pattern
-            .as_deref()
-            .is_none_or(|text| matches_word(row, text))
-    });
-    rows.sort_by_key(|row| row.version);
     rows
 }
 
```

Apply to `src/shell/init/mod.rs` (above the test module):

```diff
--- a/src/shell/init/mod.rs
+++ b/src/shell/init/mod.rs
@@ -10,11 +10,13 @@
 //! >/dev/null` silences only the messages); the function returns the
 //! binary's status. Every other command is the binary, untouched, without
 //! descriptor 3. Both export the variables the binary reads that a shell
-//! may hold unexported (`MANPATH`, `NODE_PATH`, `NVM_SYMLINK_CURRENT`,
-//! `PREFIX`) only when they are set, so `nvm exec` links `current` and
-//! checks the prefix as `use` does, and an unset one never reaches a program
-//! the binary starts as an empty variable (a `make` run by `nvm install`
-//! seeing `PREFIX=`).
+//! may hold unexported (`MANPATH`, `NODE_PATH`, `NVM_COLORS`,
+//! `NVM_HAS_COLORS`, `NVM_NO_COLORS`, `NVM_SYMLINK_CURRENT`, `PREFIX`) only
+//! when they are set, so `nvm exec` links `current` and checks the prefix as
+//! `use` does, the colors follow the shell's settings as nvm.sh (a shell
+//! function) sees them, and an unset one never reaches a program the binary
+//! starts as an empty variable (a `make` run by `nvm install` seeing
+//! `PREFIX=`).
 //!
 //! As sourcing `nvm.sh` does, the snippet ends with the automatic `use` (or
 //! `install`), whose status is the status of the whole snippet: `eval
@@ -118,7 +120,15 @@
 }
 
 /// The variables the binary reads that a shell may hold unexported.
-const PASSED: [&str; 4] = ["MANPATH", "NODE_PATH", "NVM_SYMLINK_CURRENT", "PREFIX"];
+const PASSED: [&str; 7] = [
+    "MANPATH",
+    "NODE_PATH",
+    "NVM_COLORS",
+    "NVM_HAS_COLORS",
+    "NVM_NO_COLORS",
+    "NVM_SYMLINK_CURRENT",
+    "PREFIX",
+];
 
 /// `template` with each `@EXPORTS@` line replaced by `export(name)` for
 /// every [`PASSED`] variable, at the same indentation.
```

Then run `cargo fmt`.

- [ ] **Step 4: Run the tests**

Run: `cargo test`
Expected: PASS: 1142 unit tests plus 102 end-to-end tests on macOS.

- [ ] **Step 5: Run the full quality gate**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo audit
rustup run 1.85 cargo check --all-targets
```

Expected: formatting clean, zero clippy warnings, every test passing, no
advisories, and the crate still builds on MSRV 1.85.

Also check that no file is over 300 lines and that no function is over 30
(temporarily add a `clippy.toml` with `too-many-lines-threshold = 30`, run
`cargo clippy --all-targets -- -W clippy::too_many_lines`, and delete it):

```bash
find src tests -name '*.rs' | xargs wc -l | sort -n | tail -3
```

- [ ] **Step 6: Run the tests on Linux**

Run:

```bash
docker buildx build -f Dockerfile -t nvmrc:p7 .
docker run --rm nvmrc:p7
docker rmi -f nvmrc:p7
```

Expected: 1245 tests passing with every shell of the image (bash, zsh, ksh,
dash, fish) installed; cargo stops at the first failing test binary, so count
the `test result` lines against the number of test files.

- [ ] **Step 7: Compare with the real `nvm.sh` (optional, manual)**

Give `nvm.sh` and the binary a pseudo-terminal (`python3` with `pty.openpty`,
`TERM=xterm-256color`) and a pipe, run each command of the plan with each
environment (the default, `--no-colors`, `NVM_NO_COLORS`, `NO_COLOR`, `TERM`
dumb, screen, unset and unknown, `NVM_COLORS` valid and invalid,
`NVM_HAS_COLORS=1`) and with each node in use (a version, system, io.js,
none), and compare the bytes of stdout, stderr and the exit status. 3136 cases
plus 192 for the invalid `.nvmrc` message were compared: 2528 plus 192
identical and 608 explained by the deviations listed at the end of this plan.
Do not keep the harness in the repository.

- [ ] **Step 8: Commit**

```bash
git add src tests
git commit -S -m "refactor: color the invalid .nvmrc message when forced, pass NVM_COLORS to the binary, and split the functions longer than 30 lines"
```

---

## Environment variables

Every variable below was run against `nvm.sh`; "makes sense" is a verdict on
whether the port should honour it as the script does.

- `NVM_COLORS`: the five roles (installed, system, current, not installed,
  default; default `bygre`; unset or empty is the default; extra characters
  ignored). Honoured by `ls`, `ls-remote` and `alias`; `set-colors` exports it
  through the shell channel; the init function passes an unexported one in.
  Makes sense.
- `TERM`: `tput -T "${TERM:-vt100}" colors` must be 8 or more (unset, empty,
  `dumb`, `vt100` and unknown terminals are off) and `sitm` decides italics.
  Makes sense.
- `NVM_NO_COLORS`: only the exact value `--no-colors` counts and only for
  `set-colors`; `ls`, `ls-remote` and `alias` ignore it (nvm.sh shadows it
  there). Makes little sense but is nvm.sh's behaviour and is kept; the flag is
  the supported way.
- `NO_COLOR`: ignored, as in `nvm.sh`.
- `NVM_HAS_COLORS`: exactly `1` forces colors in `nvm alias <name> <target>` and
  in the invalid-`.nvmrc` message (a fork internal that leaks from the
  environment in `nvm.sh`); `0` does not force them off. Kept for parity.
- Not read: `CLICOLOR`, `FORCE_COLOR`, `COLORTERM` (nvm.sh does not either).

## Deliberate deviations from nvm.sh

Each is pinned by a test and is for the Plan 9 compatibility contract.

- **An invalid or missing letter in `NVM_COLORS` prints one warning and leaves
  that role uncolored.** nvm.sh prints `Invalid color code: x` on every lookup
  (dozens of times per command, even through a pipe) and splices malformed
  `\x1b[<text>\x1b[0m` sequences into alias rows.
- **An unknown `TERM` is silent.** nvm.sh lets `tput` print `tput: unknown
  terminal` on every detection, even through a pipe; the port captures it and
  only runs `tput` when stdout is a terminal and `--no-colors` was not given.
  The colors are the same: off.
- **`nvm set-colors` with an invalid argument does not dump `nvm --help`** on
  stderr first; the blank line, the error line (same bytes) and exit 0 are the
  same.
- **No color legend in `--help`, and no `NVM_NO_COLORS` leak into the shell**
  from it (an upstream bug of `nvm --help`).
- **The forced colored invalid-`.nvmrc` message prints the parsed lines as they
  are**; nvm.sh sends them through `printf %b`, which expands backslash escapes
  found in the file (`\t`, `\c`).
- **The variables the `nvm` function passes in also reach the programs it
  starts** (`NVM_COLORS`, `NVM_HAS_COLORS`, `NVM_NO_COLORS`, `MANPATH`,
  `NODE_PATH`, `NVM_SYMLINK_CURRENT`, `PREFIX`): the binary cannot tell an
  exported variable from an unexported one.
- **`NVM_COLORS` is read by character, not by byte.** nvm.sh reads it with
  `awk substr`, which counts bytes; a multibyte letter (`éygre`) is therefore
  one role in the port and two positions in nvm.sh. Only invalid settings are
  affected.
- **The alias row printed by `nvm install --alias=<name>` and `--default` is
  not colored** (`ColorPolicy::off`); nvm.sh runs `nvm alias`, which colors it
  on a terminal.
- **`nvm set-colors rgbcm --help` sets the colors**; nvm.sh shows its help (the
  port's help hijack rule of Plan 6: arguments after a subcommand are
  ordinary).
- **The bash 3.2 alias colors are not reproduced** (a bug of that shell:
  `command awk` ends a background job); the port follows bash 5 and zsh.

## Self-review against the spec

- **Spec coverage:** section 3 layering (`ports/Terminal`, `domain/colors`,
  `commands/color_policy`), section 11 (the colors deferred by Plan 5 and 6),
  and the environment variables of the global requirement (`NVM_COLORS`,
  `NVM_NO_COLORS`, `NVM_HAS_COLORS`, `TERM`) with a verdict each.
- **Placeholder scan:** none.
- **Type consistency:** names match across tasks (`Palette`, `Role`,
  `ColorPolicy`, `Options`, `RemoteColors`, `AliasRow`, `AliasPainter`,
  `Output::blank_stdout`).
