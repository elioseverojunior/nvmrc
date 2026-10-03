use super::*;
use crate::domain::fixtures::index_text;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};

const NODE_INDEX: &str = "https://nodejs.org/dist/index.tab";
const IOJS_INDEX: &str = "https://iojs.org/dist/index.tab";

fn mirror() -> FakeHttp {
    let node = index_text(&[
        ("v21.2.0", "-"),
        ("v20.10.0", "Iron"),
        ("v18.19.0", "Hydrogen"),
        ("v4.0.0", "-"),
    ]);
    let iojs = index_text(&[("v3.3.1", "-"), ("v2.5.0", "-")]);
    FakeHttp::default()
        .with_body(NODE_INDEX, &node)
        .with_body(IOJS_INDEX, &iojs)
}

fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

fn run_with(http: &FakeHttp, line: &str) -> Output {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    run(&Context::new(&fs, &env).with_http(http), &words(line)).unwrap()
}

fn printed(line: &str) -> (String, NvmExitCode) {
    let output = run_with(&mirror(), line);
    (output.stdout, output.status)
}

fn found(version: &str) -> (String, NvmExitCode) {
    (version.to_owned(), NvmExitCode::Success)
}

fn missing() -> (String, NvmExitCode) {
    ("N/A".to_owned(), NvmExitCode::InvalidVersion)
}

#[test]
fn no_pattern_is_the_newest_node_release_from_the_node_index_only() {
    let http = mirror();
    let output = run_with(&http, "");
    assert_eq!(output.stdout, "v21.2.0");
    assert_eq!(http.requests(), [NODE_INDEX]);
}

#[test]
fn patterns_resolve_as_the_real_nvm_does() {
    assert_eq!(printed("20"), found("v20.10.0"));
    assert_eq!(printed("--lts"), found("v20.10.0"));
    assert_eq!(printed("lts/iron"), found("v20.10.0"));
    assert_eq!(printed("lts/*"), found("v20.10.0"));
    assert_eq!(printed("--lts=-1"), found("v18.19.0"));
    assert_eq!(printed("iojs"), found("iojs-v3.3.1"));
    assert_eq!(printed("2"), found("iojs-v2.5.0"));
    assert_eq!(printed("stable"), found("v21.2.0"));
}

#[test]
fn what_matches_nothing_is_n_a_with_status_3() {
    assert_eq!(printed("99"), missing());
    assert_eq!(printed("unstable"), missing());
    assert_eq!(printed("lts/foo"), missing());
    assert_eq!(printed("--lts iojs"), missing());
}

#[test]
fn a_bad_lts_name_prints_its_message_once_and_n_a() {
    let output = run_with(&mirror(), "--lts=Iron");
    assert_eq!(output.stderr, "LTS names must be lowercase");
    assert_eq!(
        (output.stdout.as_str(), output.status),
        ("N/A", NvmExitCode::InvalidVersion)
    );
    let output = run_with(&mirror(), "--lts=-9");
    assert_eq!(output.stderr, "That many LTS releases do not exist yet.");
}

#[test]
fn the_lts_pattern_replaces_the_flag_and_the_first_word_is_the_pattern() {
    let options = parse_options(&words("--lts=gallium lts/iron 18")).unwrap();
    assert_eq!(options.pattern, None);
    assert_eq!(options.lts.as_deref(), Some("iron"));
    assert_eq!(
        parse_options(&words("18 20")).unwrap().pattern.as_deref(),
        Some("18")
    );
}

#[test]
fn options_other_than_lts_are_unsupported_even_no_colors() {
    for option in ["--bogus", "--no-colors"] {
        let error = parse_options(&words(option)).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("Unsupported option \"{option}\".")
        );
        assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption);
    }
}

#[test]
fn an_unreachable_mirror_is_n_a() {
    let output = run_with(&FakeHttp::default(), "");
    assert_eq!(output.stdout, "N/A");
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}
