use super::lexer::{Token, has_word, has_word_start, indentation, strip_comment, tokens};

fn word(value: &str) -> Token {
    Token::Word {
        value: value.to_string(),
        quoted: false,
    }
}

fn quoted(value: &str) -> Token {
    Token::Word {
        value: value.to_string(),
        quoted: true,
    }
}

#[test]
fn strip_comment_cuts_a_hash_that_starts_a_word_outside_quotes() {
    let rows = [
        ("# [ -s \"$NVM_DIR/nvm.sh\" ]", ""),
        ("   # indented comment", "   "),
        ("foo # . ~/.nvm/nvm.sh", "foo "),
        ("a;# c", "a;"),
        ("echo \"a # b\"", "echo \"a # b\""),
        ("echo 'a # b'", "echo 'a # b'"),
        ("echo ${#array[@]} $#", "echo ${#array[@]} $#"),
        ("echo a#b", "echo a#b"),
        ("echo \\# not a comment", "echo \\# not a comment"),
        ("echo \"it\\\"s\" # c", "echo \"it\\\"s\" "),
        ("plain line", "plain line"),
    ];
    for (line, code) in rows {
        assert_eq!(strip_comment(line), code, "{line:?}");
    }
}

#[test]
fn tokens_split_words_and_operators_and_unquote() {
    assert_eq!(
        tokens("nvm() {"),
        [
            word("nvm"),
            Token::Operator('('),
            Token::Operator(')'),
            word("{")
        ]
    );
    assert_eq!(
        tokens("if [ -s \"$x\" ]; then"),
        [
            word("if"),
            word("["),
            word("-s"),
            quoted("$x"),
            word("]"),
            Token::Operator(';'),
            word("then"),
        ]
    );
    assert_eq!(tokens("  'nvm' \"node\""), [quoted("nvm"), quoted("node")]);
    assert_eq!(
        tokens("a&&b|c"),
        [
            word("a"),
            Token::Operator('&'),
            Token::Operator('&'),
            word("b"),
            Token::Operator('|'),
            word("c"),
        ]
    );
    assert_eq!(tokens("\\{ x"), [quoted("{"), word("x")]);
    assert_eq!(
        tokens("echo \"a \\\" b\""),
        [word("echo"), quoted("a \" b")]
    );
    assert_eq!(tokens(""), []);
}

#[test]
fn indentation_counts_leading_blanks() {
    assert_eq!(indentation("x"), 0);
    assert_eq!(indentation("  x"), 2);
    assert_eq!(indentation("\tx"), 1);
    assert_eq!(indentation(""), 0);
}

#[test]
fn has_word_needs_a_boundary_on_both_sides() {
    let rows = [
        ("unset -f nvm", "nvm", true),
        ("unset -f nvm node", "nvm", true),
        ("unset -f pnvm", "nvm", false),
        ("unset -f nvmx", "nvm", false),
        ("nvm_ls", "nvm", false),
        ("$NVM_DIR/nvm.sh", "nvm", true),
        ("antigen bundle zsh-nvm", "zsh-nvm", true),
        ("foo-zsh-nvm-bar", "zsh-nvm", true),
        ("zsh-nvmx", "zsh-nvm", false),
        ("nvm_ls_remote", "nvm_ls", false),
        ("", "nvm", false),
    ];
    for (text, needle, expected) in rows {
        assert_eq!(has_word(text, needle), expected, "{text:?} {needle:?}");
    }
}

#[test]
fn has_word_start_needs_a_boundary_before_only() {
    assert!(has_word_start(
        "export NVM_LAZY_LOAD=true",
        "NVM_LAZY_LOAD="
    ));
    assert!(!has_word_start("MY_NVM_LAZY_LOAD=1", "NVM_LAZY_LOAD="));
}
