use super::*;
use crate::error::NvmExitCode;

fn parsed(line: &str) -> Result<Options, CliError> {
    let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
    parse(&words)
}

#[test]
fn a_version_is_the_first_word_that_is_not_an_option() {
    let options = parsed("20").unwrap();
    assert_eq!(
        (options.version.as_str(), options.version_given),
        ("20", true)
    );
    assert_eq!(options.lts, None);
    assert_eq!(options.alias, None);
}

#[test]
fn only_an_lts_option_without_a_version_is_announced() {
    assert!(parsed("--lts").unwrap().announce_lts);
    assert!(parsed("--lts=iron").unwrap().announce_lts);
    assert!(!parsed("lts/iron").unwrap().announce_lts);
    assert!(!parsed("--lts 20").unwrap().announce_lts);
    assert!(!parsed("20").unwrap().announce_lts);
}

#[test]
fn the_npm_options_are_read_before_and_after_the_version() {
    let before = parsed("--latest-npm --skip-default-packages 20").unwrap();
    assert!(before.latest_npm && before.skip_default_packages);
    let after = parsed("20 --skip-default-packages").unwrap();
    assert!(!after.latest_npm && after.skip_default_packages);
    let neither = parsed("20").unwrap();
    assert!(!neither.latest_npm && !neither.skip_default_packages);
}

#[test]
fn nothing_at_all_is_no_version() {
    let options = parsed("").unwrap();
    assert_eq!(
        (options.version.as_str(), options.version_given),
        ("", false)
    );
}

#[test]
fn lts_options_and_lts_versions_give_the_lts_filter() {
    assert_eq!(parsed("--lts").unwrap().lts.as_deref(), Some("*"));
    assert_eq!(parsed("--lts=iron").unwrap().lts.as_deref(), Some("iron"));
    assert_eq!(parsed("lts/*").unwrap().lts.as_deref(), Some("*"));
    let named = parsed("--lts=gallium lts/iron").unwrap();
    assert_eq!(named.lts.as_deref(), Some("iron"));
    assert_eq!(named.version, "");
}

#[test]
fn default_and_alias_name_the_alias_to_make() {
    assert_eq!(
        parsed("--default 20").unwrap().alias.as_deref(),
        Some("default")
    );
    assert_eq!(
        parsed("--alias=work 20").unwrap().alias.as_deref(),
        Some("work")
    );
}

#[test]
fn default_and_alias_together_or_twice_are_status_6() {
    for line in [
        "--default --alias=x 20",
        "--alias=a --alias=b 20",
        "--default --default 20",
    ] {
        let error = parsed(line).unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::InvalidOptions, "{line}");
        assert_eq!(
            error.to_string(),
            "--default and --alias are mutually exclusive, and may not be provided more than once"
        );
    }
}

#[test]
fn harmless_options_are_accepted() {
    let options = parsed("-b --no-progress 20").unwrap();
    assert_eq!(options.version, "20");
}

#[test]
fn three_dashes_are_a_typo_with_status_55() {
    let error = parsed("---x").unwrap_err();
    assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption);
    assert_eq!(
        error.to_string(),
        "arguments with `---` are not supported - this is likely a typo"
    );
}

#[test]
fn options_that_need_a_source_build_or_npm_are_not_supported_yet() {
    for line in [
        "-s 20",
        "-j 4 20",
        "--offline 20",
        "--reinstall-packages-from=18 20",
        "20 --reinstall-packages-from=18",
        "20 --copy-packages-from=18",
        "20 --save",
    ] {
        let error = parsed(line).unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption, "{line}");
        assert!(
            error.to_string().ends_with("is not supported yet."),
            "{line}"
        );
    }
}

#[test]
fn an_unknown_option_is_taken_as_the_version_like_nvm_sh_does() {
    let options = parsed("--bogus").unwrap();
    assert_eq!(options.version, "--bogus");
}
