use super::*;

#[test]
fn the_npm_commands_have_their_usages_with_exit_127() {
    let (code, _, err) = run_cli(&["nvm", "install-latest-npm", "x"]);
    assert_eq!(code, 127);
    assert_eq!(
        err,
        "Usage: nvm install-latest-npm\n  Run `nvm --help` for full help.\n"
    );
    for command in ["reinstall-packages", "copy-packages"] {
        let (code, _, err) = run_cli(&["nvm", command]);
        assert_eq!(code, 127);
        assert_eq!(
            err,
            format!("Usage: nvm {command} <version>\n  Run `nvm --help` for full help.\n")
        );
    }
}

#[test]
fn conflicting_install_options_exit_6_and_the_message_comes_on_stderr() {
    let (code, out, err) = run_cli(&["nvm", "install", "-s", "-b", "20"]);
    assert_eq!((code, out.as_str()), (6, ""));
    assert!(err.starts_with("-s and -b cannot be set together"));
}
