use super::source_path::{source_target, source_targets};

/// Digest section 6: the real syntaxes, and the token after `source`, `.` or
/// `\.` with its quotes removed.
const PATH_TOKEN_ROWS: &[(&str, &str)] = &[
    ("source $ZSH/oh-my-zsh.sh", "$ZSH/oh-my-zsh.sh"),
    (
        "source ${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh",
        "${ZSH_CONFIG_SCRIPTS}/lazy-functions.zsh",
    ),
    (
        "[[ -r \"${ZDOTDIR:-$HOME}/.zshenv\" ]] && source \"${ZDOTDIR:-$HOME}/.zshenv\"",
        "${ZDOTDIR:-$HOME}/.zshenv",
    ),
    ("  . \"$HOME/.swiftly/env.sh\"", "$HOME/.swiftly/env.sh"),
    (
        "[ -s \"$HOME/.bun/_bun\" ] && source \"$HOME/.bun/_bun\"",
        "$HOME/.bun/_bun",
    ),
    (
        "[ -f \"${HOME}/.openclaw/completions/openclaw.zsh\" ] && source \"${HOME}/.openclaw/completions/openclaw.zsh\"",
        "${HOME}/.openclaw/completions/openclaw.zsh",
    ),
    (
        "[[ -r $NVM_DIR/bash_completion ]] && \\. $NVM_DIR/bash_completion",
        "$NVM_DIR/bash_completion",
    ),
    ("source ~/.nvm/nvm.sh", "~/.nvm/nvm.sh"),
    (
        "source ${0:A:h}/zsh-syntax-highlighting.zsh",
        "${0:A:h}/zsh-syntax-highlighting.zsh",
    ),
    (
        "for f in \"${_deferred_sources[@]}\"; do source \"$f\"; done",
        "$f",
    ),
    (
        "source $(brew --prefix nvm)/nvm.sh",
        "$(brew --prefix nvm)/nvm.sh",
    ),
    (
        ". \"$(brew --prefix nvm)/nvm.sh\"",
        "$(brew --prefix nvm)/nvm.sh",
    ),
    ("[ -f p ] && . p", "p"),
    ("[[ -r p ]] && source p", "p"),
    ("source 'a b'/c", "a b/c"),
    ("source x;echo y", "x"),
    ("(source x)", "x"),
    ("source x&", "x"),
    ("source x|cat", "x"),
    ("\tsource x", "x"),
    ("source \"$NVM_DIR\"/nvm.sh", "$NVM_DIR/nvm.sh"),
    ("source a\\ b", "a b"),
    (
        "source `brew --prefix nvm`/nvm.sh",
        "`brew --prefix nvm`/nvm.sh",
    ),
    ("source x # . y", "x"),
];

#[test]
fn source_target_reads_the_path_token() {
    for &(line, expected) in PATH_TOKEN_ROWS {
        assert_eq!(source_target(line).as_deref(), Some(expected), "{line:?}");
    }
}

#[test]
fn source_target_rejects_comments_and_non_source_lines() {
    let rows = [
        "# source ~/.nvm/nvm.sh",
        "  # . ~/.nvm/nvm.sh",
        "echo hi # source x",
        "defer_source file.zsh",
        "defer_eval 'eval \"$(direnv hook zsh)\"'",
        "./script.sh",
        "../script.sh",
        "source",
        "source   ",
        ". ;",
        "echo dnvm.sh",
        "resource x",
        "",
    ];
    for line in rows {
        assert_eq!(source_target(line), None, "{line:?}");
    }
}

#[test]
fn source_targets_lists_every_sourced_path_of_a_line() {
    assert_eq!(
        source_targets(". a; source \"b c\" && \\. d"),
        ["a", "b c", "d"]
    );
    assert_eq!(
        source_targets(
            "      [[ -f \\\"\\$NVM_DIR/nvm.sh\\\" ]] && source \\\"\\$NVM_DIR/nvm.sh\\\""
        ),
        ["\"$NVM_DIR/nvm.sh\""]
    );
    assert!(source_targets("echo nothing").is_empty());
}

/// A command word after a keyword, assignments, an operator or a quote that
/// starts a command, and the path it reads.
const COMMAND_POSITION_ROWS: &[(&str, &str)] = &[
    ("if [ -s x ]; then . x; fi", "x"),
    ("for f in a; do source \"$f\"; done", "$f"),
    ("if true; then :; else . y; fi", "y"),
    ("{ . x; }", "x"),
    (
        "ZSH_VERSION= source \"$_nvm_completion\"",
        "$_nvm_completion",
    ),
    ("A=1 B=\"c d\" . x", "x"),
    ("builtin source x", "x"),
    ("  bash) source x ;;", "x"),
    ("eval \"cd a; source x\"", "x"),
    ("alias l='cd a && . x'", "x"),
];

/// A `.` that is an argument (the user's `scripts/macos.zsh:119-122,201`) is
/// not a command word; one after a keyword, an assignment or an operator is.
#[test]
fn source_target_needs_the_command_position() {
    let arguments = [
        "alias qfind=\"find . -name \"                 # qfind:    Quickly search for file",
        "ff () { /usr/bin/find . -name \"$@\" ; }      # ff:       Find file under the current directory",
        "ffs () { /usr/bin/find . -name \"$@\"'*' ; }  # ffs:      Find file whose name starts with a given string",
        "alias cleanupDS=\"find . -type f -name '*.DS_Store' -ls -delete\"",
        "echo \"a\" . b",
        "cp x . ",
    ];
    for line in arguments {
        assert_eq!(source_target(line), None, "{line:?}");
    }
    for &(line, expected) in COMMAND_POSITION_ROWS {
        assert_eq!(source_target(line).as_deref(), Some(expected), "{line:?}");
    }
}

#[test]
fn a_wrapper_command_may_stand_before_the_loader() {
    assert_eq!(
        source_target("zsh-defer source ~/.nvm/nvm.sh").as_deref(),
        Some("~/.nvm/nvm.sh")
    );
    assert_eq!(
        source_target("time . ~/.nvm/nvm.sh").as_deref(),
        Some("~/.nvm/nvm.sh")
    );
}
