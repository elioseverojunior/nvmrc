use super::*;
use crate::error::NvmExitCode;

fn parsed(args: &[&str]) -> Options {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    parse(&args).expect("valid options")
}

fn rejected(args: &[&str]) -> CliError {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    parse(&args).expect_err("rejected options")
}

#[test]
fn no_arguments_is_no_version_and_no_flags() {
    assert_eq!(parsed(&[]), Options::default());
}

#[test]
fn every_flag_is_taken_wherever_it_stands() {
    let options = parsed(&["18", "--silent", "--delete-prefix", "--save"]);
    let expected = Options {
        silent: true,
        delete_prefix: true,
        save: true,
        lts: None,
        version: Some("18".to_owned()),
    };
    assert_eq!(options, expected);
}

#[test]
fn dash_w_is_save() {
    assert!(parsed(&["-w", "20"]).save);
}

#[test]
fn lts_alone_is_every_lts_and_lts_equals_names_one() {
    assert_eq!(parsed(&["--lts"]).lts.as_deref(), Some("*"));
    assert_eq!(parsed(&["--lts=hydrogen"]).lts.as_deref(), Some("hydrogen"));
}

#[test]
fn an_empty_lts_name_is_no_lts_and_the_last_lts_flag_wins() {
    assert_eq!(parsed(&["--lts="]).lts, None);
    assert_eq!(parsed(&["--lts", "--lts="]).lts, None);
    assert_eq!(parsed(&["--lts=", "--lts"]).lts.as_deref(), Some("*"));
    assert_eq!(parsed(&["--lts=argon", "--lts"]).lts.as_deref(), Some("*"));
}

#[test]
fn the_last_non_empty_positional_wins() {
    assert_eq!(parsed(&["18", "20"]).version.as_deref(), Some("20"));
    assert_eq!(parsed(&["18", ""]).version.as_deref(), Some("18"));
    assert_eq!(parsed(&[""]).version, None);
}

#[test]
fn double_dash_and_unknown_long_flags_are_ignored() {
    assert_eq!(parsed(&["--", "18"]).version.as_deref(), Some("18"));
    assert_eq!(parsed(&["--bogus", "18"]).version.as_deref(), Some("18"));
    assert_eq!(parsed(&["18", "--"]), parsed(&["18"]));
}

#[test]
fn a_single_dash_or_a_short_flag_is_a_version_string() {
    assert_eq!(parsed(&["-"]).version.as_deref(), Some("-"));
    assert_eq!(parsed(&["-x"]).version.as_deref(), Some("-x"));
}

#[test]
fn save_given_twice_in_any_spelling_is_status_6() {
    for args in [["--save", "-w"], ["-w", "-w"], ["--save", "--save"]] {
        let error = rejected(&args);
        assert_eq!(error.to_string(), "--save and -w may only be provided once");
        assert_eq!(error.exit_code(), NvmExitCode::InvalidOptions);
    }
}
