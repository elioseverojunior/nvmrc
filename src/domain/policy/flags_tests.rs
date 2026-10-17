use super::*;
use std::ffi::OsString;

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn flags_before_the_subcommand_are_taken() {
    let (flags, rest) = extract(args(&["nvm", "--policy", "/p.json", "use", "20"])).unwrap();
    assert_eq!(flags.get("--policy"), Some("/p.json"));
    assert_eq!(rest, args(&["nvm", "use", "20"]));
}

#[test]
fn flags_right_after_the_subcommand_are_taken_in_both_forms() {
    let (flags, rest) = extract(args(&[
        "nvm",
        "use",
        "--min-node=20",
        "--policy",
        "p",
        "20",
    ]))
    .unwrap();
    assert_eq!(flags.get("--min-node"), Some("20"));
    assert_eq!(flags.get("--policy"), Some("p"));
    assert_eq!(rest, args(&["nvm", "use", "20"]));
}

#[test]
fn flags_after_the_first_argument_of_exec_or_run_are_left_alone() {
    for input in [
        &["nvm", "exec", "20", "node", "--policy", "x.js"][..],
        &["nvm", "run", "--silent", "20", "--min-node=9", "x.js"][..],
    ] {
        let (flags, rest) = extract(args(input)).unwrap();
        assert!(flags.is_empty());
        assert_eq!(rest, args(input));
    }
}

#[test]
fn flags_anywhere_after_another_subcommand_are_taken() {
    let (flags, rest) = extract(args(&[
        "nvm",
        "install",
        "--offline",
        "20",
        "--min-node=22",
        "--policy",
        "p",
    ]))
    .unwrap();
    assert_eq!(flags.get("--min-node"), Some("22"));
    assert_eq!(flags.get("--policy"), Some("p"));
    assert_eq!(rest, args(&["nvm", "install", "--offline", "20"]));
}

#[test]
fn flags_after_a_double_dash_are_left_alone() {
    let input = args(&["nvm", "use", "20", "--", "--policy", "p"]);
    let (flags, rest) = extract(input.clone()).unwrap();
    assert!(flags.is_empty());
    assert_eq!(rest, input);
}

#[test]
fn a_late_flag_without_a_value_is_an_error() {
    assert_eq!(
        extract(args(&["nvm", "use", "20", "--policy"]))
            .unwrap_err()
            .to_string(),
        "nvm: --policy needs a value."
    );
}

#[test]
fn a_missing_or_repeated_value_is_an_error() {
    assert_eq!(
        extract(args(&["nvm", "--policy"])).unwrap_err().to_string(),
        "nvm: --policy needs a value."
    );
    assert_eq!(
        extract(args(&["nvm", "--policy=a", "--policy", "b", "ls"]))
            .unwrap_err()
            .to_string(),
        "nvm: --policy is given twice."
    );
}

#[test]
fn an_empty_or_blank_value_is_an_error() {
    let cases = [
        (&["nvm", "--policy="][..], "nvm: --policy needs a value."),
        (
            &["nvm", "--min-node", ""][..],
            "nvm: --min-node needs a value.",
        ),
        (
            &["nvm", "--policy", " "][..],
            "nvm: --policy needs a value.",
        ),
        (
            &["nvm", "--min-node=  "][..],
            "nvm: --min-node needs a value.",
        ),
    ];
    for (input, message) in cases {
        assert_eq!(extract(args(input)).unwrap_err().to_string(), message);
    }
}

#[cfg(unix)]
#[test]
fn a_value_that_is_not_utf8_is_an_error() {
    use std::os::unix::ffi::OsStringExt;
    let bad = OsString::from_vec(vec![0xff]);
    let input = vec![OsString::from("nvm"), OsString::from("--policy"), bad];
    assert_eq!(
        extract(input).unwrap_err().to_string(),
        "nvm: the value of --policy is not valid UTF-8."
    );
}

#[test]
fn the_severity_flag_is_taken_like_the_others() {
    let (flags, rest) = extract(args(&["nvm", "--fail-on-severity", "low", "use", "20"])).unwrap();
    assert_eq!(flags.get("--fail-on-severity"), Some("low"));
    assert_eq!(rest, args(&["nvm", "use", "20"]));
    let (flags, rest) = extract(args(&["nvm", "use", "20", "--fail-on-severity=high"])).unwrap();
    assert_eq!(flags.get("--fail-on-severity"), Some("high"));
    assert_eq!(rest, args(&["nvm", "use", "20"]));
}
