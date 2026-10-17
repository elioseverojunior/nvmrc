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
fn flags_after_the_first_argument_are_left_alone() {
    let (flags, rest) = extract(args(&["nvm", "exec", "20", "node", "--policy", "x.js"])).unwrap();
    assert!(flags.is_empty());
    assert_eq!(
        rest,
        args(&["nvm", "exec", "20", "node", "--policy", "x.js"])
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
