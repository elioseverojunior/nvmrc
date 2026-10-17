//! A bare `nvm` is `nvm --help`, as in nvm.sh.

use super::*;

#[test]
fn bare_nvm_prints_the_help_on_stdout_with_exit_0() {
    let (code, out, err) = run_cli(&["nvm"]);
    let (_, help, _) = run_cli(&["nvm", "--help"]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, help.as_str(), ""));
}

#[test]
fn upgrade_help_lists_the_flags() {
    let (code, out, err) = run_cli(&["nvm", "upgrade", "--help"]);
    assert_eq!((code, err.as_str()), (0, ""));
    for flag in ["--node", "--npm", "--yarn", "--pnpm", "--major"] {
        assert!(out.contains(flag), "{flag} missing from:\n{out}");
    }
}
