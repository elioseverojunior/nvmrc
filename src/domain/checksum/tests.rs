use super::*;

const SHASUMS: &str = "\
aaa111  node-v20.10.0-darwin-arm64.tar.gz
bbb222  node-v20.10.0-linux-x64.tar.gz
ccc333  node-v20.10.0-linux-x64.tar.xz
";

#[test]
fn the_checksum_of_a_listed_file_is_found_by_its_exact_name() {
    let found = expected_digest(SHASUMS, "node-v20.10.0-linux-x64.tar.gz");
    assert_eq!(found.as_deref(), Some("bbb222"));
    let other = expected_digest(SHASUMS, "node-v20.10.0-linux-x64.tar.xz");
    assert_eq!(other.as_deref(), Some("ccc333"));
}

#[test]
fn a_file_that_is_not_listed_has_no_checksum() {
    assert_eq!(expected_digest(SHASUMS, "node-v20.10.0-linux-x64"), None);
    assert_eq!(expected_digest("", "x"), None);
    assert_eq!(expected_digest("lonely\n", "x"), None);
}

#[test]
fn equal_digests_match_with_or_without_the_escape_marker() {
    assert_eq!(compare("abc", "abc"), Ok(()));
    assert_eq!(compare("\\abc", "abc"), Ok(()));
}

#[test]
fn different_digests_are_a_mismatch_with_the_message_of_nvm_sh() {
    let error = compare("abc", "def").unwrap_err();
    assert_eq!(
        error.to_string(),
        "Checksums do not match: 'abc' found, 'def' expected."
    );
}

#[test]
fn a_missing_expected_digest_is_an_error() {
    let error = compare("abc", "").unwrap_err();
    assert_eq!(error, ChecksumError::MissingExpected);
    assert_eq!(
        error.to_string(),
        "Provided checksum to compare to is empty."
    );
}
