use super::source_path::{source_target, source_targets};

/// Digest section 6: the real syntaxes, and the token after `source`, `.` or
/// `\.` with its quotes removed.
#[test]
fn source_target_reads_the_path_token() {
    let rows = [
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
    for (line, expected) in rows {
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
