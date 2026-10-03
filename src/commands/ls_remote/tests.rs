use super::*;
use crate::domain::fixtures::index_text;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};
use crate::ports::FileSystem;

const NODE_INDEX: &str = "https://nodejs.org/dist/index.tab";
const IOJS_INDEX: &str = "https://iojs.org/dist/index.tab";

fn node_text() -> String {
    index_text(&[
        ("v21.2.0", "-"),
        ("v21.1.0", "-"),
        ("v20.10.0", "Iron"),
        ("v20.9.0", "Iron"),
        ("v18.19.0", "Hydrogen"),
        ("v18.18.0", "Hydrogen"),
        ("v16.20.2", "Gallium"),
        ("v14.21.3", "Fermium"),
        ("v4.9.1", "Argon"),
        ("v4.0.0", "-"),
        ("v0.12.18", "-"),
        ("v0.10.48", "-"),
    ])
}

fn iojs_text() -> String {
    index_text(&[
        ("v3.3.1", "-"),
        ("v3.0.0", "-"),
        ("v2.5.0", "-"),
        ("v1.0.0", "-"),
    ])
}

fn mirror() -> FakeHttp {
    FakeHttp::default()
        .with_body(NODE_INDEX, &node_text())
        .with_body(IOJS_INDEX, &iojs_text())
}

fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

fn run_with(fs: &FakeFileSystem, env: &FakeEnv, http: &FakeHttp, line: &str) -> Output {
    let context = Context::new(fs, env).with_http(http);
    run(&context, &words(line)).unwrap()
}

fn env() -> FakeEnv {
    FakeEnv::default().with_var("NVM_DIR", "/n")
}

fn lines(output: &Output) -> Vec<&str> {
    output.stdout.lines().collect()
}

/// Expected rows are what the real nvm.sh printed against the same mirrors.
#[test]
fn a_plain_listing_has_every_release_and_the_latest_marker() {
    let output = run_with(&FakeFileSystem::default(), &env(), &mirror(), "");
    assert_eq!(
        lines(&output),
        [
            "       v0.10.48",
            "       v0.12.18",
            "    iojs-v1.0.0",
            "    iojs-v2.5.0",
            "    iojs-v3.0.0",
            "    iojs-v3.3.1",
            "         v4.0.0",
            "         v4.9.1   (Latest LTS: Argon)",
            "       v14.21.3   (Latest LTS: Fermium)",
            "       v16.20.2   (Latest LTS: Gallium)",
            "       v18.18.0   (LTS: Hydrogen)",
            "       v18.19.0   (Latest LTS: Hydrogen)",
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0   (Latest LTS: Iron)",
            "        v21.1.0",
            "        v21.2.0                            (Latest: node)",
        ]
    );
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(output.stderr.is_empty());
}

#[test]
fn lts_lists_only_the_lts_releases_from_the_node_index() {
    let http = mirror();
    let output = run_with(&FakeFileSystem::default(), &env(), &http, "--lts");
    assert_eq!(
        lines(&output),
        [
            "         v4.9.1   (Latest LTS: Argon)",
            "       v14.21.3   (Latest LTS: Fermium)",
            "       v16.20.2   (Latest LTS: Gallium)",
            "       v18.18.0   (LTS: Hydrogen)",
            "       v18.19.0   (Latest LTS: Hydrogen)",
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0   (Latest LTS: Iron)",
        ]
    );
    assert_eq!(http.requests(), [NODE_INDEX]);
}

#[test]
fn lts_with_a_name_or_as_a_pattern_is_the_same_filter() {
    let fs = FakeFileSystem::default();
    let expected = [
        "       v18.18.0   (LTS: Hydrogen)",
        "       v18.19.0   (Latest LTS: Hydrogen)",
    ];
    let by_flag = run_with(&fs, &env(), &mirror(), "--lts=hydrogen");
    let by_pattern = run_with(&fs, &env(), &mirror(), "lts/hydrogen");
    assert_eq!(lines(&by_flag), expected);
    assert_eq!(lines(&by_pattern), expected);
}

#[test]
fn lts_minus_one_is_the_codename_before_the_last() {
    let output = run_with(&FakeFileSystem::default(), &env(), &mirror(), "--lts=-1");
    assert_eq!(
        lines(&output),
        [
            "       v18.18.0   (LTS: Hydrogen)",
            "       v18.19.0   (Latest LTS: Hydrogen)",
        ]
    );
}

#[test]
fn an_uppercase_lts_name_is_an_error_row_with_status_3() {
    let output = run_with(&FakeFileSystem::default(), &env(), &mirror(), "--lts=Iron");
    assert_eq!(output.stderr, "LTS names must be lowercase");
    assert_eq!(output.stdout, "            N/A");
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}

#[test]
fn going_back_too_far_is_an_error_row_with_status_3() {
    let output = run_with(&FakeFileSystem::default(), &env(), &mirror(), "--lts=-9");
    assert_eq!(output.stderr, "That many LTS releases do not exist yet.");
    assert_eq!(output.stdout, "            N/A");
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}

#[test]
fn a_pattern_that_matches_nothing_is_n_a_with_status_3() {
    let output = run_with(&FakeFileSystem::default(), &env(), &mirror(), "99");
    assert_eq!(output.stdout, "            N/A");
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}

#[test]
fn an_implicit_alias_is_an_error_without_any_download() {
    let http = mirror();
    let output = run_with(&FakeFileSystem::default(), &env(), &http, "stable");
    assert_eq!(
        output.stderr,
        "Implicit aliases are not supported in nvm_remote_versions."
    );
    assert_eq!(output.stdout, "            N/A");
    assert!(http.requests().is_empty());
}

#[test]
fn an_unsupported_option_is_status_55() {
    let error = parse_options(&words("--bogus")).unwrap_err();
    assert_eq!(error.to_string(), "Unsupported option \"--bogus\".");
    assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption);
}

#[test]
fn no_colors_and_the_separator_are_accepted() {
    let options = parse_options(&words("--no-colors -- 18")).unwrap();
    assert_eq!(options.pattern.as_deref(), Some("18"));
    assert_eq!(options.lts, None);
}

#[test]
fn only_the_first_word_is_the_pattern() {
    let options = parse_options(&words("18 20")).unwrap();
    assert_eq!(options.pattern.as_deref(), Some("18"));
}

#[test]
fn a_failed_iojs_download_still_lists_node_but_exits_3_without_markers() {
    let http = FakeHttp::default()
        .with_body(NODE_INDEX, &node_text())
        .with_status(IOJS_INDEX, 503);
    let output = run_with(&FakeFileSystem::default(), &env(), &http, "");
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert!(!output.stdout.contains("iojs-"));
    assert!(!output.stdout.contains("(Latest: node)"));
    assert!(output.stdout.contains("v21.2.0"));
}

#[test]
fn a_mirror_that_is_not_a_url_is_reported_and_leaves_that_flavor_out() {
    let env = env().with_var("NVM_NODEJS_ORG_MIRROR", "http://x y");
    let output = run_with(&FakeFileSystem::default(), &env, &mirror(), "");
    assert_eq!(
        output.stderr,
        "$NVM_NODEJS_ORG_MIRROR and $NVM_IOJS_ORG_MIRROR may only contain a URL"
    );
    assert!(output.stdout.contains("iojs-v3.3.1"));
    assert!(!output.stdout.contains("v21.2.0"));
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}

#[test]
fn nothing_downloadable_is_n_a_with_status_3() {
    let output = run_with(&FakeFileSystem::default(), &env(), &FakeHttp::default(), "");
    assert_eq!(output.stdout, "            N/A");
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
}

#[test]
fn installed_current_and_aliased_releases_are_marked() {
    let fs = FakeFileSystem::default()
        .with_file("/n/versions/node/v20.10.0/bin/node", "")
        .with_file("/n/alias/prod", "v20.10.0\n")
        .with_file("/n/alias/default", "node\n");
    let env = env().with_var("PATH", "/n/versions/node/v20.10.0/bin");
    let output = run_with(&fs, &env, &mirror(), "");
    let rows = lines(&output);
    assert!(
        rows.contains(
            &"->     v20.10.0 * (Latest LTS: Iron)                        (Aliases: prod)"
        )
    );
    assert!(
        rows.iter()
            .any(|row| row.ends_with("(Latest: node)   (Aliases: default)"))
    );
}

#[test]
fn a_pattern_listing_has_no_alias_or_latest_columns() {
    let fs = FakeFileSystem::default().with_file("/n/alias/prod", "v20.10.0\n");
    let output = run_with(&fs, &env(), &mirror(), "20");
    assert_eq!(lines(&output).len(), 2);
    assert!(!output.stdout.contains("Aliases"));
}

#[test]
fn listing_refreshes_the_lts_aliases() {
    let fs = FakeFileSystem::default();
    run_with(&fs, &env(), &mirror(), "");
    let read = |name: &str| fs.read_to_string(std::path::Path::new(name)).unwrap();
    assert_eq!(read("/n/alias/lts/*"), "lts/iron\n");
}

#[test]
fn an_alias_target_without_the_v_is_shown_on_its_release() {
    let fs = FakeFileSystem::default()
        .with_file("/n/alias/default", "20\n")
        .with_file("/n/alias/old", "0.12\n");
    let output = run_with(&fs, &env(), &mirror(), "");
    let rows = lines(&output);
    assert!(
        rows.iter()
            .any(|row| row.contains("v20.10.0") && row.ends_with("(Aliases: default)"))
    );
    assert!(
        rows.iter()
            .any(|row| row.contains("v0.12.18") && row.ends_with("(Aliases: old)"))
    );
}
