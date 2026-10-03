use super::*;

#[test]
fn init_prints_the_snippet_on_stdout_with_exit_0() {
    for shell in ["bash", "zsh", "sh", "dash", "ksh"] {
        let (code, out, err) = run_cli(&["nvm", "init", shell]);
        assert_eq!((code, err.as_str()), (0, ""), "{shell}");
        assert!(out.starts_with("# >>> nvmrc init >>>\n"), "{out}");
        assert!(
            out.ends_with("\\nvm __auto use\n# <<< nvmrc init <<<\n"),
            "{out}"
        );
    }
}

#[test]
fn init_fish_prints_the_fish_snippet_ending_with_the_auto_use() {
    let (code, out, err) = run_cli(&["nvm", "init", "fish"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(out.starts_with("# >>> nvmrc init >>>\n"), "{out}");
    assert!(out.contains("\nfunction nvm "), "{out}");
    assert!(
        out.ends_with("\nend\nnvm __auto use\n# <<< nvmrc init <<<\n"),
        "{out}"
    );
}

#[test]
fn init_options_pick_the_auto_step_and_the_last_one_wins() {
    let ends = |args: &[&str], tail: &str| {
        let (code, out, _) = run_cli(args);
        assert_eq!(code, 0);
        assert!(out.ends_with(tail), "{args:?}: {out}");
    };
    let end = "# <<< nvmrc init <<<\n";
    ends(
        &["nvm", "init", "bash", "--install"],
        &format!("\\nvm __auto install\n{end}"),
    );
    ends(
        &["nvm", "init", "--install", "zsh", "--no-use"],
        &format!("}}\n{end}"),
    );
    ends(
        &["nvm", "init", "sh", "--no-use", "--install"],
        &format!("\\nvm __auto install\n{end}"),
    );
}

#[test]
fn init_of_an_unsupported_shell_names_the_supported_ones_with_exit_127() {
    for shell in ["tcsh", "powershell"] {
        let (code, out, err) = run_cli(&["nvm", "init", shell]);
        assert_eq!((code, out.as_str()), (127, ""));
        assert!(err.contains("bash, zsh, sh, dash, ksh, fish"), "{err}");
    }
}

#[test]
fn init_without_a_shell_is_a_usage_error_with_exit_127() {
    let (code, out, err) = run_cli(&["nvm", "init"]);
    assert_eq!((code, out.as_str()), (127, ""));
    assert!(err.contains("Usage: nvm init <shell>"), "{err}");
    let (code, _, _) = run_cli(&["nvm", "init", "--no-use"]);
    assert_eq!(code, 127);
}

#[test]
fn init_with_an_unknown_option_or_a_second_shell_exits_55() {
    for args in [
        &["nvm", "init", "bash", "--bogus"][..],
        &["nvm", "init", "bash", "zsh"],
    ] {
        let (code, out, err) = run_cli(args);
        assert_eq!((code, out.as_str()), (55, ""), "{args:?}");
        assert!(!err.is_empty());
    }
}

#[test]
fn auto_is_a_hidden_command_and_rejects_other_modes_with_exit_1() {
    let (_, help, _) = run_cli(&["nvm", "--help"]);
    assert!(help.contains("init"), "{help}");
    assert!(!help.contains("__auto"), "{help}");
    let (code, out, err) = run_cli(&["nvm", "__auto", "bogus"]);
    assert_eq!((code, out.as_str()), (1, ""));
    assert_eq!(err, "Invalid auto mode supplied.\n");
    assert_eq!(run_cli(&["nvm", "__auto"]).0, 1);
    assert_eq!(
        run_cli(&["nvm", "__auto", "none"]),
        (0, String::new(), String::new())
    );
}
