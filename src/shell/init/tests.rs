use super::*;
use crate::error::NvmExitCode;

/// What the binary reads that a shell may hold unexported: nvm.sh sees the
/// shell's own variables, the color settings included.
const PASSED_VARIABLES: [&str; 7] = [
    "MANPATH",
    "NODE_PATH",
    "NVM_COLORS",
    "NVM_HAS_COLORS",
    "NVM_NO_COLORS",
    "NVM_SYMLINK_CURRENT",
    "PREFIX",
];

const ALL: [(&str, Shell); 6] = [
    ("bash", Shell::Bash),
    ("zsh", Shell::Zsh),
    ("sh", Shell::Sh),
    ("dash", Shell::Dash),
    ("ksh", Shell::Ksh),
    ("fish", Shell::Fish),
];

fn lines_of(shell: Shell, options: &InitOptions) -> Vec<String> {
    snippet(shell, options).lines().map(str::to_owned).collect()
}

/// The last statement before the closing marker.
fn last_statement(shell: Shell, options: &InitOptions) -> String {
    let lines = lines_of(shell, options);
    lines[lines.len() - 2].clone()
}

#[test]
fn every_supported_name_parses_and_prints_back() {
    for (name, shell) in ALL {
        assert_eq!(name.parse::<Shell>().unwrap(), shell);
        assert_eq!(shell.name(), name);
    }
}

#[test]
fn other_names_are_a_usage_error_naming_the_supported_shells() {
    for name in ["Fish", "powershell", "Bash", "ZSH", "", "tcsh"] {
        let error = name.parse::<Shell>().expect_err(name);
        assert!(matches!(error, CliError::Usage(_)), "{name}");
        assert_eq!(error.exit_code(), NvmExitCode::NotFound);
        let message = error.to_string();
        assert!(
            message.contains("bash, zsh, sh, dash, ksh, fish)"),
            "{message}"
        );
        assert!(message.contains(USAGE), "{message}");
    }
}

#[test]
fn the_snippet_sits_between_the_markers() {
    for (_, shell) in ALL {
        let lines = lines_of(shell, &InitOptions::default());
        assert_eq!(lines.first().map(String::as_str), Some(BEGIN_MARKER));
        assert_eq!(lines.last().map(String::as_str), Some(END_MARKER));
        assert!(snippet(shell, &InitOptions::default()).ends_with('\n'));
    }
}

#[test]
fn the_snippet_names_its_shell_and_is_plain_ascii() {
    for (name, shell) in ALL {
        let text = snippet(shell, &InitOptions::default());
        assert!(text.is_ascii());
        assert!(text.contains(&format!("`{}`", shell.load_line())), "{text}");
        assert!(text.contains(&format!("nvmrc init {name}")), "{text}");
    }
    let fish = snippet(Shell::Fish, &InitOptions::default());
    assert!(fish.contains("`nvmrc init fish | source`"), "{fish}");
}

#[test]
fn the_snippet_exports_the_shell_marker_and_the_nvm_dir_default() {
    let text = snippet(Shell::Zsh, &InitOptions::default());
    assert!(text.contains("\nexport NVMRC_SHELL=1\n"));
    assert!(text.contains("\nexport NVM_DIR=\"${NVM_DIR:-$HOME/.nvm}\"\n"));
}

#[test]
fn the_function_runs_the_binary_with_the_code_on_descriptor_3() {
    let text = snippet(Shell::Sh, &InitOptions::default());
    assert!(text.contains("\nnvm() {\n"), "{text}");
    let zsh = snippet(Shell::Zsh, &InitOptions::default());
    assert!(zsh.contains("\nfunction nvm {\n"), "{zsh}");
    assert!(text.contains("NVMRC_SCRIPT_FD=3 command \\nvm \"$@\" 3>&1 1>&4 4>&-"));
    assert!(text.contains("use | deactivate | install | i | set-colors | __auto)"));
    assert!(!text.contains("command nvm"), "{text}");
    assert!(text.contains("eval \"$__nvmrc_code\""));
}

#[test]
fn every_command_gets_the_passed_variables_exported_only_when_set() {
    let text = snippet(Shell::Ksh, &InitOptions::default());
    for passed in PASSED_VARIABLES {
        let export = format!("if [ -n \"${{{passed}+set}}\" ]; then export {passed}; fi");
        assert_eq!(text.matches(&export).count(), 2, "{passed}: {text}");
        assert!(!text.contains(&format!("{passed}=")), "{passed}: {text}");
    }
    assert!(!text.contains("@EXPORTS@"), "{text}");
    assert!(
        text.contains("\n        command \\nvm \"$@\"\n      )\n"),
        "{text}"
    );
}

#[test]
fn the_snippet_ends_with_the_auto_use_by_default() {
    assert_eq!(
        last_statement(Shell::Bash, &InitOptions::default()),
        "\\nvm __auto use"
    );
    assert_eq!(
        last_statement(Shell::Fish, &InitOptions::default()),
        "nvm __auto use"
    );
}

#[test]
fn install_makes_the_auto_step_install() {
    let options = InitOptions {
        install: true,
        ..InitOptions::default()
    };
    assert_eq!(
        last_statement(Shell::Bash, &options),
        "\\nvm __auto install"
    );
    assert_eq!(last_statement(Shell::Fish, &options), "nvm __auto install");
}

#[test]
fn no_use_leaves_the_auto_step_out() {
    for install in [false, true] {
        let options = InitOptions {
            no_use: true,
            install,
        };
        for shell in [Shell::Bash, Shell::Fish] {
            let text = snippet(shell, &options);
            assert!(!text.contains("nvm __auto "), "{text}");
        }
    }
}

#[test]
fn the_fish_snippet_sets_the_shell_marker_and_the_nvm_dir_default() {
    let text = snippet(Shell::Fish, &InitOptions::default());
    assert!(text.contains("\nset -gx NVMRC_SHELL 1\n"), "{text}");
    assert!(
        text.contains(
            "\ntest -n \"$NVM_DIR\"; or set -g NVM_DIR $HOME/.nvm\nset -gx NVM_DIR $NVM_DIR\n"
        ),
        "{text}"
    );
    assert!(!text.contains("export "), "{text}");
}

#[test]
fn the_fish_function_captures_descriptor_3_and_names_its_dialect() {
    let text = snippet(Shell::Fish, &InitOptions::default());
    assert!(text.contains("\nfunction nvm "), "{text}");
    assert!(
        text.contains(
            "if not contains -- \"$argv[1]\" use deactivate install i set-colors __auto\n"
        ),
        "{text}"
    );
    let capture = "NVMRC_SCRIPT_FD=3 NVMRC_SHELL_KIND=fish command nvm $argv 3>&1 1>&4 4>&- \
                   | read -z nvmrc_code\n";
    assert!(text.contains(capture), "{text}");
    assert!(text.contains("set nvmrc_status $pipestatus[1]\n"), "{text}");
    assert!(
        text.contains("    end 4>&1\n    eval $nvmrc_code\n"),
        "{text}"
    );
    assert!(text.contains("return $nvmrc_status\n"), "{text}");
    assert_eq!(text.matches("NVMRC_SHELL_KIND").count(), 1, "{text}");
}

#[test]
fn other_fish_commands_run_the_binary_untouched() {
    let text = snippet(Shell::Fish, &InitOptions::default());
    assert!(
        text.contains("            command nvm $argv\n        end\n        return $status\n"),
        "{text}"
    );
    assert_eq!(text.matches("command nvm $argv").count(), 2, "{text}");
}

#[test]
fn fish_passes_the_variables_to_the_binary_only_when_set() {
    let text = snippet(Shell::Fish, &InitOptions::default());
    for passed in PASSED_VARIABLES {
        let export = format!("set -q {passed}; and set -lx {passed} ${passed}\n");
        assert_eq!(text.matches(&export).count(), 2, "{passed}: {text}");
        assert!(!text.contains(&format!("{passed}=")), "{passed}: {text}");
    }
    assert!(!text.contains("@EXPORTS@"), "{text}");
}
