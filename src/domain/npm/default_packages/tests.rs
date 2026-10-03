use super::*;

const FILE: &str = "/n/default-packages";

fn parsed(contents: &str) -> Result<String, DefaultPackagesError> {
    parse(contents, FILE)
}

/// Every expectation is what `nvm_get_default_packages` of the real `nvm.sh`
/// printed for the same file.
#[test]
fn one_package_per_line_are_joined_by_spaces() {
    assert_eq!(parsed("a\nb\n").unwrap(), "a b");
    assert_eq!(parsed("a\n#c\nb").unwrap(), "a b");
}

#[test]
fn a_package_that_is_not_on_the_first_line_gets_a_space_in_front_like_nvm_sh() {
    assert_eq!(parsed("# c\na\n\nb\n").unwrap(), " a b");
    assert_eq!(parsed("\n\na\n").unwrap(), " a");
}

#[test]
fn comments_and_blank_lines_alone_are_no_packages() {
    assert_eq!(parsed("#x\n#y\n").unwrap(), "");
    assert_eq!(parsed("").unwrap(), "");
}

#[test]
fn two_values_on_a_line_or_any_blank_in_it_is_an_error_naming_the_file() {
    for contents in ["a b\n", "  a\n", "a\tb\n"] {
        let error = parsed(contents).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Only one package per line is allowed in `/n/default-packages`. Please remove any lines with multiple space-separated values."
        );
    }
}
