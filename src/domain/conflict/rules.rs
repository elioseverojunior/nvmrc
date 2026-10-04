//! The per-line rules of digest section 7, hand-written (no regex engine),
//! one function per row of the table. They read a line without its comment
//! and know nothing of the lines around it (see the scanner in `mod.rs`).
//!
//! Deviations from the regexes, all stricter or documented:
//!
//! - Loader and Completion read the parsed source path ([`source_targets`]),
//!   so `"$NVM_DIR"/nvm.sh` is a loader and `nvm.sh.bak` is not; a bare
//!   `. nvm.sh` is a loader.
//! - A `bass source .../nvm.sh` line is Bass only, not also a Loader.
//! - `unset` and `unfunction` must start a word; the zstyle quote is optional.

use super::kind::Kind;
use super::lexer::{boundary_before, has_word, has_word_start, is_word_character};
use super::source_path::source_targets;

type Rule = fn(&str) -> bool;

/// Every rule, in the order the kinds of one line are reported.
const RULES: [(Kind, Rule); 9] = [
    (Kind::LazyStub, defines_stub),
    (Kind::Unset, unsets_nvm),
    (Kind::NvmDirExport, exports_nvm_dir),
    (Kind::Bass, bass_sources_nvm_sh),
    (Kind::Loader, sources_nvm_sh),
    (Kind::Completion, sources_completion),
    (Kind::OmzPlugin, configures_omz_plugin),
    (Kind::ZshNvm, mentions_zsh_nvm),
    (Kind::HelperCall, calls_helper),
];

/// The commands a lazy stub is named after.
const STUB_NAMES: [&str; 8] = [
    "nvm", "node", "npm", "npx", "yarn", "pnpm", "pnpx", "corepack",
];

/// The nvm.sh internals whose callers break once nvm.sh is gone.
const HELPERS: [&str; 7] = [
    "nvm_find_nvmrc",
    "nvm_find_up",
    "nvm_ls",
    "nvm_version",
    "nvm_has",
    "nvm_echo",
    "nvm_rc_version",
];

/// The kinds `code` (a line without its comment) matches, in table order.
pub(super) fn line_kinds(code: &str) -> Vec<Kind> {
    RULES
        .iter()
        .filter(|(_, rule)| rule(code))
        .map(|(kind, _)| *kind)
        .collect()
}

/// `name() {`, `name () {`, `function name {`, `function name() {`.
fn defines_stub(code: &str) -> bool {
    let line = code.trim_start();
    let (rest, keyword) = match after_word(line, "function") {
        Some(after) => (after, true),
        None => (line, false),
    };
    STUB_NAMES
        .iter()
        .find_map(|name| rest.strip_prefix(name))
        .and_then(|after| skip_parentheses(after.trim_start(), keyword))
        .is_some_and(|body| body.trim_start().starts_with('{'))
}

/// `text` after `()` (blanks allowed inside); the parentheses are optional
/// after the `function` keyword.
fn skip_parentheses(text: &str, keyword: bool) -> Option<&str> {
    match text.strip_prefix('(') {
        Some(inside) => inside.trim_start().strip_prefix(')'),
        None => keyword.then_some(text),
    }
}

/// `unset -f ... nvm ...` or `unfunction ... nvm ...`.
fn unsets_nvm(code: &str) -> bool {
    let unset =
        word_starts(code, "unset").filter_map(|rest| after_blanks(rest)?.strip_prefix("-f"));
    word_starts(code, "unfunction")
        .chain(unset)
        .any(|rest| !rest.starts_with(is_word_character) && has_word(rest, "nvm"))
}

/// `NVM_DIR=...` or `export NVM_DIR=...` at the start of the line.
fn exports_nvm_dir(code: &str) -> bool {
    let line = code.trim_start();
    after_word(line, "export")
        .unwrap_or(line)
        .starts_with("NVM_DIR=")
}

/// `bass source <path holding nvm.sh>`.
fn bass_sources_nvm_sh(code: &str) -> bool {
    word_starts(code, "bass")
        .filter_map(|rest| after_word(after_blanks(rest)?, "source"))
        .any(|arguments| {
            arguments
                .split_whitespace()
                .next()
                .is_some_and(|path| path.contains("nvm.sh"))
        })
}

fn sources_nvm_sh(code: &str) -> bool {
    !bass_sources_nvm_sh(code) && source_targets(code).iter().any(|path| is_nvm_sh(path))
}

/// A path whose file is `nvm.sh` (after `/`, after `}`, or alone).
fn is_nvm_sh(target: &str) -> bool {
    let path = target.trim_matches(['"', '\'']);
    path.strip_suffix("nvm.sh")
        .is_some_and(|head| head.is_empty() || head.ends_with(['/', '}']))
}

fn sources_completion(code: &str) -> bool {
    const ENDINGS: [&str; 4] = [
        "/bash_completion.d/nvm",
        "nvm/bash_completion",
        "NVM_DIR/bash_completion",
        "NVM_DIR}/bash_completion",
    ];
    source_targets(code).iter().any(|target| {
        let path = target.trim_matches(['"', '\'']);
        ENDINGS.iter().any(|ending| path.ends_with(ending))
    })
}

/// `zstyle ':omz:plugins:nvm' ...`.
fn configures_omz_plugin(code: &str) -> bool {
    word_starts(code, "zstyle")
        .filter_map(after_blanks)
        .any(|rest| {
            rest.strip_prefix(['\'', '"'])
                .unwrap_or(rest)
                .starts_with(":omz:plugins:nvm")
        })
}

/// `zsh-nvm` as a word (`lukechilds/zsh-nvm` included) or `NVM_LAZY_LOAD=`.
fn mentions_zsh_nvm(code: &str) -> bool {
    has_word(code, "zsh-nvm") || has_word_start(code, "NVM_LAZY_LOAD=")
}

fn calls_helper(code: &str) -> bool {
    HELPERS.iter().any(|helper| has_word(code, helper))
}

/// What follows each occurrence of `word` that starts a word in `code`.
fn word_starts<'a>(code: &'a str, word: &'a str) -> impl Iterator<Item = &'a str> {
    code.match_indices(word)
        .filter(|(start, _)| boundary_before(code, *start))
        .map(move |(start, _)| &code[start + word.len()..])
}

/// `text` after `word` and at least one blank, when it starts with them.
fn after_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    after_blanks(text.strip_prefix(word)?)
}

/// `text` without its leading blanks, when it has at least one.
fn after_blanks(text: &str) -> Option<&str> {
    text.starts_with(char::is_whitespace)
        .then(|| text.trim_start())
}
