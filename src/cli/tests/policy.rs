use super::*;

#[test]
fn a_flag_error_is_a_usage_error() {
    let (code, _, err) = run_cli(&["nvm", "--policy"]);
    assert_eq!(code, 127);
    assert_eq!(err.trim_end(), "nvm: --policy needs a value.");
}

#[test]
fn stripped_flags_do_not_reach_clap() {
    let (code, out, _) = run_cli(&["nvm", "--min-node", "18", "version", "20"]);
    assert_eq!((code, out.trim_end()), (0, "v20.1.0"));
}
