use super::*;
use crate::commands::Output;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem};

fn lookup(fs: &FakeFileSystem, pwd: Option<&str>, silent: bool) -> (RcVersion, Output) {
    let env = pwd.map_or_else(FakeEnv::default, |pwd| {
        FakeEnv::default().with_var("PWD", pwd)
    });
    let mut transcript = Transcript::default();
    let found = rc_version(&Context::new(fs, &env), silent, &mut transcript);
    (found, transcript.finish(NvmExitCode::Success))
}

fn project(content: &str) -> FakeFileSystem {
    FakeFileSystem::default()
        .with_dir("/proj/sub/deep")
        .with_file("/proj/.nvmrc", content)
}

#[test]
fn it_finds_the_nearest_ancestor_file_and_announces_it() {
    let (found, output) = lookup(&project("18\n"), Some("/proj/sub/deep"), false);
    let expected = RcVersion::Found {
        path: PathBuf::from("/proj/.nvmrc"),
        version: "18".to_owned(),
    };
    assert_eq!(found, expected);
    assert_eq!(output.stdout, "Found '/proj/.nvmrc' with version <18>");
    assert!(output.stderr.is_empty());
}

#[test]
fn the_nearest_file_wins() {
    let fs = project("18\n").with_file("/proj/sub/.nvmrc", "20\n");
    let (found, _) = lookup(&fs, Some("/proj/sub/deep"), true);
    assert!(matches!(found, RcVersion::Found { version, .. } if version == "20"));
}

#[test]
fn silent_prints_nothing_when_found() {
    let (found, output) = lookup(&project("18\n"), Some("/proj/sub"), true);
    assert!(matches!(found, RcVersion::Found { .. }));
    assert_eq!((output.stdout.as_str(), output.stderr.as_str()), ("", ""));
}

#[test]
fn crlf_and_comments_are_cleaned() {
    let fs = project("# pin\r\n  v20.11  \r\n");
    let (found, output) = lookup(&fs, Some("/proj"), false);
    assert!(matches!(found, RcVersion::Found { version, .. } if version == "v20.11"));
    assert_eq!(output.stdout, "Found '/proj/.nvmrc' with version <v20.11>");
}

#[test]
fn a_missing_file_is_reported_on_stderr() {
    let fs = FakeFileSystem::default().with_dir("/empty");
    let (found, output) = lookup(&fs, Some("/empty"), false);
    assert_eq!(found, RcVersion::Missing);
    assert_eq!(output.stderr, MISSING_MESSAGE);
    assert!(output.stdout.is_empty());
}

#[test]
fn a_missing_file_is_quiet_when_silent() {
    let fs = FakeFileSystem::default().with_dir("/empty");
    let (found, output) = lookup(&fs, Some("/empty"), true);
    assert_eq!(found, RcVersion::Missing);
    assert_eq!((output.stdout.as_str(), output.stderr.as_str()), ("", ""));
}

#[test]
fn an_unset_pwd_finds_nothing() {
    let (found, output) = lookup(&project("18\n"), None, false);
    assert_eq!(found, RcVersion::Missing);
    assert_eq!(output.stderr, MISSING_MESSAGE);
}

#[test]
fn an_unreadable_file_counts_as_missing() {
    let fs = FakeFileSystem::default().with_dir("/proj/.nvmrc");
    let (found, output) = lookup(&fs, Some("/proj"), false);
    assert_eq!(found, RcVersion::Missing);
    assert_eq!(output.stderr, MISSING_MESSAGE);
}

#[test]
fn invalid_content_is_reported_even_when_silent() {
    let (found, output) = lookup(&project("18\n20\n"), Some("/proj"), true);
    assert_eq!(found, RcVersion::Invalid);
    assert!(output.stderr.starts_with("invalid .nvmrc!\n"));
    assert!(
        output
            .stderr
            .ends_with("non-commented content parsed:\n18\n20")
    );
    assert!(output.stdout.is_empty());
}

fn lookup_with_has_colors(value: &str) -> Output {
    let fs = project("foo=bar\n");
    let env = FakeEnv::default()
        .with_var("PWD", "/proj")
        .with_var("NVM_HAS_COLORS", value);
    let mut transcript = Transcript::default();
    rc_version(&Context::new(&fs, &env), false, &mut transcript);
    transcript.finish(NvmExitCode::Success)
}

#[test]
fn an_exported_nvm_has_colors_of_one_colors_the_invalid_message() {
    let output = lookup_with_has_colors("1");
    assert!(output.stderr.starts_with("\x1b[0;31minvalid .nvmrc!\n"));
    assert!(
        output
            .stderr
            .ends_with("\x1b[0;33mnon-commented content parsed:\nfoo=bar\x1b[0m")
    );
}

#[test]
fn any_other_nvm_has_colors_value_keeps_the_invalid_message_plain() {
    for value in ["0", "true", "", " 1"] {
        let output = lookup_with_has_colors(value);
        assert!(output.stderr.starts_with("invalid .nvmrc!\n"), "{value:?}");
        assert!(!output.stderr.contains('\x1b'), "{value:?}");
    }
}

#[test]
fn an_empty_parse_ends_right_after_the_header_line() {
    let (_, output) = lookup(&project("# only\n"), Some("/proj"), false);
    assert!(output.stderr.ends_with("non-commented content parsed:"));
}

#[test]
fn please_see_adds_the_hint_line() {
    let mut transcript = Transcript::default();
    please_see(&mut transcript);
    let output = transcript.finish(NvmExitCode::Success);
    assert_eq!(output.stderr, PLEASE_SEE);
}
