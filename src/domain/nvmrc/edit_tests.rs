use super::{with_pin, with_version};
use crate::domain::tool::Tool;
use crate::domain::version::Version;

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

#[test]
fn with_pin_replaces_the_existing_line() {
    assert_eq!(
        with_pin(" pnpm = 9.1.0 # old\n", Tool::Pnpm, &version("9.12.0")),
        "pnpm=9.12.0 # old\n"
    );
}

#[test]
fn with_pin_appends_a_new_line() {
    assert_eq!(
        with_pin("20\n# tools\n\n\n", Tool::Yarn, &version("1.22.22")),
        "20\n# tools\nyarn=1.22.22\n\n\n"
    );
    assert_eq!(
        with_pin("20", Tool::Npm, &version("10.9.0")),
        "20\nnpm=10.9.0\n"
    );
    assert_eq!(with_pin("", Tool::Npm, &version("10.9.0")), "npm=10.9.0\n");
}

#[test]
fn editing_keeps_comments_keys_and_line_endings() {
    let contents = "# project\r\n20 # lts\r\nnpm=10.8.0\r\nlts=iron\r\n";
    let edited = with_version(contents, "v22.11.0");
    let edited = with_pin(&edited, Tool::Npm, &version("10.9.0"));
    assert_eq!(
        edited,
        "# project\r\nv22.11.0\r\nnpm=10.9.0\r\nlts=iron\r\n"
    );
}

#[test]
fn appending_uses_the_line_ending_of_the_file() {
    assert_eq!(
        with_pin("20\r\nlts=iron", Tool::Npm, &version("10.9.0")),
        "20\r\nlts=iron\r\nnpm=10.9.0\r\n"
    );
}

#[test]
fn with_version_on_an_empty_file_is_just_the_version() {
    assert_eq!(with_version("", "v22.11.0"), "v22.11.0\n");
}
