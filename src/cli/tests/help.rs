//! A bare `nvm` is `nvm --help`, as in nvm.sh.

use super::*;

#[test]
fn bare_nvm_prints_the_help_on_stdout_with_exit_0() {
    let (code, out, err) = run_cli(&["nvm"]);
    let (_, help, _) = run_cli(&["nvm", "--help"]);
    assert_eq!((code, out.as_str(), err.as_str()), (0, help.as_str(), ""));
}
