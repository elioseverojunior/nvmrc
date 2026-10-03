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
fn reinstall_and_copy_packages_from_name_the_version_to_take_packages_from() {
    for line in [
        "--reinstall-packages-from=18 20",
        "20 --reinstall-packages-from=18",
        "--copy-packages-from=18 20",
        "20 --copy-packages-from=18",
    ] {
        let options = parsed(line).unwrap();
        assert_eq!(options.reinstall_from.as_deref(), Some("18"), "{line}");
        assert_eq!(options.version, "20", "{line}");
    }
}

/// The reinstall options nvm.sh refuses, with the message the real
/// `nvm.sh` printed for each.
const REFUSED_REINSTALL_OPTIONS: &[(&str, &str)] = &[
    (
        "--reinstall-packages-from=18 --reinstall-packages-from=18 20",
        "--reinstall-packages-from may not be provided more than once",
    ),
    (
        "--copy-packages-from=18 --reinstall-packages-from=18 20",
        "--reinstall-packages-from may not be provided more than once",
    ),
    (
        "--reinstall-packages-from=18 --copy-packages-from=18 20",
        "--reinstall-packages-from may not be provided more than once, or combined with `--copy-packages-from`",
    ),
    (
        "--reinstall-packages-from= 20",
        "If --reinstall-packages-from is provided, it must point to an installed version of node.",
    ),
    (
        "--copy-packages-from= 20",
        "If --copy-packages-from is provided, it must point to an installed version of node.",
    ),
    (
        "--reinstall-packages-from 20",
        "If --reinstall-packages-from is provided, it must point to an installed version of node using `=`.",
    ),
    (
        "20 --copy-packages-from",
        "If --copy-packages-from is provided, it must point to an installed version of node using `=`.",
    ),
];

#[test]
fn a_reinstall_option_given_twice_or_without_a_version_is_status_6() {
    for &(line, message) in REFUSED_REINSTALL_OPTIONS {
        let error = parsed(line).unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::InvalidOptions, "{line}");
        assert_eq!(error.to_string(), message, "{line}");
    }
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
fn offline_and_save_are_read_before_the_version() {
    let options = parsed("--offline --save 20").unwrap();
    assert!(options.offline && options.save);
    assert!(parsed("-w 20").unwrap().save);
    let neither = parsed("20").unwrap();
    assert!(!neither.offline && !neither.save);
}

#[test]
fn save_after_the_version_is_not_an_option_like_in_nvm_sh() {
    let options = parsed("20 --save -w").unwrap();
    assert!(!options.save);
    assert_eq!(options.extra, ["--save", "-w"]);
    assert_eq!(options.version, "20");
}

#[test]
fn save_twice_is_status_6() {
    for line in ["--save --save 20", "-w --save 20", "--save -w 20"] {
        let error = parsed(line).unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::InvalidOptions, "{line}");
        assert_eq!(error.to_string(), "--save and -w may only be provided once");
    }
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
    let options = parsed("--no-progress 20").unwrap();
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
fn s_b_and_j_choose_how_to_build() {
    assert!(parsed("-s 20").unwrap().no_binary);
    assert!(parsed("-b 20").unwrap().no_source);
    let jobs = parsed("-j 4 20").unwrap();
    assert_eq!(
        (jobs.make_jobs.as_deref(), jobs.version.as_str()),
        (Some("4"), "20")
    );
    assert_eq!(parsed("-j").unwrap().make_jobs.as_deref(), Some(""));
}

#[test]
fn s_and_b_together_are_status_6() {
    for line in ["-s -b 20", "-b -s 20"] {
        let error = parsed(line).unwrap_err();
        assert_eq!(error.exit_code(), NvmExitCode::InvalidOptions, "{line}");
        assert_eq!(
            error.to_string(),
            "-s and -b cannot be set together since they would skip install from both binary and source"
        );
    }
}

#[test]
fn the_words_after_the_version_that_no_option_takes_go_to_configure() {
    let options = parsed("-s 20 --with-intl=full-icu --save --ninja").unwrap();
    assert_eq!(options.extra, ["--with-intl=full-icu", "--save", "--ninja"]);
    assert!(!options.save);
    assert!(parsed("20").unwrap().extra.is_empty());
}

#[test]
fn an_unknown_option_is_taken_as_the_version_like_nvm_sh_does() {
    let options = parsed("--bogus").unwrap();
    assert_eq!(options.version, "--bogus");
}
