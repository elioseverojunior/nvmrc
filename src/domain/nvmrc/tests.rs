use std::path::{Path, PathBuf};

use super::*;
use crate::fakes::FakeFileSystem;

fn version(contents: &str) -> NvmrcContent {
    process_content(contents)
}

fn bare(text: &str) -> NvmrcContent {
    NvmrcContent::Version(text.to_owned())
}

fn invalid(parsed: &[&str]) -> NvmrcContent {
    NvmrcContent::Invalid {
        parsed: parsed.iter().map(|line| (*line).to_owned()).collect(),
    }
}

#[test]
fn plain_and_padded_versions_are_bare() {
    assert_eq!(version("18\n"), bare("18"));
    assert_eq!(version("  v20.11  \r\n"), bare("v20.11"));
    assert_eq!(version("lts/*"), bare("lts/*"));
    assert_eq!(version("node"), bare("node"));
}

#[test]
fn comments_are_stripped() {
    assert_eq!(version("# comment\n\n  18 # trailing\n"), bare("18"));
}

#[test]
fn key_value_pairs_are_ignored_beside_a_bare_line() {
    assert_eq!(version("18\nfoo=bar\n baz = qux \n"), bare("18"));
}

#[test]
fn a_line_starting_with_equals_is_a_bare_version() {
    assert_eq!(version("=x"), bare("=x"));
}

#[test]
fn empty_comment_only_and_blank_content_is_invalid_with_nothing_parsed() {
    assert_eq!(version(""), invalid(&[]));
    assert_eq!(version("# only comment"), invalid(&[]));
    assert_eq!(version("  \r\n\t\n"), invalid(&[]));
}

#[test]
fn a_second_bare_line_is_invalid() {
    assert_eq!(version("18\n20\n"), invalid(&["18", "20"]));
    assert_eq!(version("=x\n18"), invalid(&["=x", "18"]));
}

#[test]
fn the_key_node_is_invalid() {
    assert_eq!(version("node=18"), invalid(&["node=18"]));
    assert_eq!(version("18\n node = 1"), invalid(&["18", "node = 1"]));
}

#[test]
fn duplicate_keys_are_invalid() {
    assert_eq!(version("a=1\na=2\n18"), invalid(&["a=1", "a=2", "18"]));
    assert_eq!(version("a=1\n a =2\n18"), invalid(&["a=1", "a =2", "18"]));
}

#[test]
fn pairs_without_a_bare_line_are_invalid() {
    assert_eq!(version("foo=bar"), invalid(&["foo=bar"]));
}

#[test]
fn the_invalid_message_lists_the_parsed_lines() {
    let expected = "invalid .nvmrc!\n\
all non-commented content (anything after # is a comment) must be either:\n\
\x20 - a single bare nvm-recognized version-ish\n\
\x20 - or, multiple distinct key-value pairs, each key/value separated by a single equals sign (=)\n\
\n\
additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).\n\
\n\
non-commented content parsed:\n\
18\n\
20\n";
    assert_eq!(
        invalid_message(&["18".to_owned(), "20".to_owned()]),
        expected
    );
}

#[test]
fn the_invalid_message_for_empty_content_ends_after_the_heading() {
    assert!(
        invalid_message(&[])
            .ends_with("(after stripping comments).\n\nnon-commented content parsed:\n")
    );
}

const COLORED_ERROR_BLOCK: &str = "\x1b[0;31minvalid .nvmrc!\n\
all non-commented content (anything after # is a comment) must be either:\n\
\x20 - a single bare nvm-recognized version-ish\n\
\x20 - or, multiple distinct key-value pairs, each key/value separated by a single equals sign (=)\n\
\n\
additionally, a single bare nvm-recognized version-ish must be present (after stripping comments).\x1b[0m\n\
\n";

#[test]
fn the_colored_invalid_message_wraps_the_error_red_and_the_parsed_block_yellow() {
    let expected =
        format!("{COLORED_ERROR_BLOCK}\x1b[0;33mnon-commented content parsed:\nfoo=bar\x1b[0m");
    assert_eq!(colored_invalid_message(&["foo=bar".to_owned()]), expected);
}

#[test]
fn the_colored_invalid_message_joins_the_parsed_lines() {
    let message = colored_invalid_message(&["18".to_owned(), "20".to_owned()]);
    assert!(message.ends_with("\x1b[0;33mnon-commented content parsed:\n18\n20\x1b[0m"));
}

#[test]
fn the_colored_invalid_message_keeps_the_newline_before_the_reset_when_nothing_was_parsed() {
    let expected = format!("{COLORED_ERROR_BLOCK}\x1b[0;33mnon-commented content parsed:\n\x1b[0m");
    assert_eq!(colored_invalid_message(&[]), expected);
}

#[test]
fn please_see_is_the_exact_hint() {
    assert_eq!(
        PLEASE_SEE,
        "Please see `nvm --help` or https://github.com/nvm-sh/nvm#nvmrc for more information."
    );
}

#[test]
fn find_nvmrc_walks_up_to_a_parent() {
    let fs = FakeFileSystem::default().with_file("/work/proj/.nvmrc", "20");
    let found = find_nvmrc(&fs, Path::new("/work/proj/src/deep"));
    assert_eq!(found, Some(PathBuf::from("/work/proj/.nvmrc")));
}

#[test]
fn find_nvmrc_prefers_the_nearest_file() {
    let fs = FakeFileSystem::default()
        .with_file("/work/.nvmrc", "18")
        .with_file("/work/proj/.nvmrc", "20");
    let found = find_nvmrc(&fs, Path::new("/work/proj"));
    assert_eq!(found, Some(PathBuf::from("/work/proj/.nvmrc")));
}

#[test]
fn a_directory_named_nvmrc_is_not_a_file() {
    let fs = FakeFileSystem::default()
        .with_file("/work/.nvmrc", "18")
        .with_file("/work/proj/.nvmrc/inner", "x");
    let found = find_nvmrc(&fs, Path::new("/work/proj"));
    assert_eq!(found, Some(PathBuf::from("/work/.nvmrc")));
}

#[test]
fn find_nvmrc_returns_none_when_absent() {
    let fs = FakeFileSystem::default();
    assert_eq!(find_nvmrc(&fs, Path::new("/work/proj")), None);
}

#[test]
fn the_root_nvmrc_is_found_through_the_exists_retry() {
    let fs = FakeFileSystem::default().with_file("/.nvmrc", "18");
    let found = find_nvmrc(&fs, Path::new("/work/proj"));
    assert_eq!(found, Some(PathBuf::from("/.nvmrc")));
}

#[test]
fn a_root_nvmrc_that_is_a_directory_counts_for_the_retry() {
    let fs = FakeFileSystem::default().with_file("/.nvmrc/inner", "x");
    let found = find_nvmrc(&fs, Path::new("/work"));
    assert_eq!(found, Some(PathBuf::from("/.nvmrc")));
}

#[test]
fn starting_at_the_root_checks_the_root_file() {
    let fs = FakeFileSystem::default().with_file("/.nvmrc", "18");
    assert_eq!(
        find_nvmrc(&fs, Path::new("/")),
        Some(PathBuf::from("/.nvmrc"))
    );
}
