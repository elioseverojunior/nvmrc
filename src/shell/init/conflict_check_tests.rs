use super::*;

const POSIX: [Shell; 5] = [Shell::Bash, Shell::Zsh, Shell::Sh, Shell::Dash, Shell::Ksh];

const HELPERS_POSIX: &str = "    if type nvm_has >/dev/null 2>&1; then\n      \
                             command \\nvm __conflict helpers >&2 || true\n";
const WARN_FUNCTION_POSIX: &str = "command \\nvm __conflict function >&2 || true";

fn text(shell: Shell) -> String {
    snippet(shell, &InitOptions::default())
}

/// The position of `needle` in `haystack`, which must contain it.
fn position(haystack: &str, needle: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in {haystack}"))
}

#[test]
fn posix_shells_check_only_when_interactive_and_before_anything_else() {
    for shell in POSIX {
        let text = text(shell);
        let check = position(&text, "\ncase $- in\n  *i*)\n");
        assert!(
            check < position(&text, "\nexport NVMRC_SHELL=1\n"),
            "{text}"
        );
        assert!(check < position(&text, "\nunalias nvm"), "{text}");
        assert!(text.contains("\n    ;;\nesac\n"), "{text}");
    }
}

#[test]
fn posix_shells_warn_first_about_the_helpers_of_nvm_sh() {
    for shell in POSIX {
        let text = text(shell);
        assert!(text.contains(HELPERS_POSIX), "{text}");
        assert!(
            position(&text, HELPERS_POSIX) < position(&text, WARN_FUNCTION_POSIX),
            "{text}"
        );
        assert_eq!(text.matches("__conflict").count(), 2, "{text}");
    }
}

#[test]
fn bash_and_ksh_warn_about_a_function_that_is_not_theirs() {
    let cases = [
        (Shell::Bash, "declare -F nvm", "declare -f nvm"),
        (Shell::Ksh, "typeset -f nvm", "typeset -f nvm"),
    ];
    for (shell, defined, body) in cases {
        let text = text(shell);
        let expected = format!(
            "    elif {defined} >/dev/null 2>&1; then\n      \
             case \"$({body})\" in\n        *NVMRC_SCRIPT_FD*) ;;\n        \
             *) {WARN_FUNCTION_POSIX} ;;\n      esac\n"
        );
        assert!(text.contains(&expected), "{text}");
    }
}

#[test]
fn zsh_reads_its_functions_table() {
    let text = text(Shell::Zsh);
    let expected = format!(
        "    elif (( ${{+functions[nvm]}} )) && [[ ${{functions[nvm]}} != *NVMRC_SCRIPT_FD* ]]; then\n      \
         {WARN_FUNCTION_POSIX}\n"
    );
    assert!(text.contains(&expected), "{text}");
}

#[test]
fn sh_and_dash_cannot_tell_whose_function_so_skip_a_second_init() {
    for shell in [Shell::Sh, Shell::Dash] {
        let text = text(shell);
        let expected = format!(
            "    elif [ -z \"${{NVMRC_SHELL+set}}\" ] && [ \"$(command -v nvm)\" = nvm ]; then\n      \
             {WARN_FUNCTION_POSIX}\n"
        );
        assert!(text.contains(&expected), "{text}");
        assert!(!text.contains("declare"), "{text}");
        assert!(!text.contains("typeset"), "{text}");
    }
}

#[test]
fn fish_checks_only_when_interactive_and_before_anything_else() {
    let text = text(Shell::Fish);
    let expected = "\nif status is-interactive\n    if functions -q nvm_has\n        \
                    command nvm __conflict helpers >&2\n    \
                    else if functions -q nvm; and not functions nvm | string match -q '*NVMRC_SCRIPT_FD*'\n        \
                    command nvm __conflict function >&2\n    end\nend\n";
    assert!(text.contains(expected), "{text}");
    assert!(
        position(&text, expected) < position(&text, "\nset -gx NVMRC_SHELL 1\n"),
        "{text}"
    );
}

#[test]
fn the_check_does_not_depend_on_the_options() {
    let no_use = InitOptions {
        no_use: true,
        install: false,
    };
    for shell in SHELLS {
        let text = snippet(shell, &no_use);
        assert_eq!(text.matches("__conflict helpers").count(), 1, "{text}");
        assert_eq!(text.matches("__conflict function").count(), 1, "{text}");
    }
}
