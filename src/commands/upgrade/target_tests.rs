use super::*;
use crate::commands::transcript::Transcript;
use crate::domain::fixtures::index_text_dated;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};

const INDEX_URL: &str = "https://nodejs.org/dist/index.tab";
const PNPM_URL: &str = "https://registry.npmjs.org/pnpm";
const YARN_URL: &str = "https://registry.npmjs.org/yarn";
const CLI_DIST_URL: &str = "https://registry.npmjs.org/@yarnpkg%2fcli-dist";

fn version(text: &str) -> Version {
    format!("v{text}").parse().unwrap()
}

fn node_index() -> String {
    index_text_dated(&[
        ("v23.0.0", "2024-05-01", "-"),
        ("v22.11.0", "2024-10-29", "Jod"),
        ("v20.18.0", "2024-10-03", "Iron"),
        ("v20.11.1", "2024-02-14", "Iron"),
    ])
}

fn files(policy: &str) -> (FakeFileSystem, FakeEnv) {
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    if policy.is_empty() {
        return (FakeFileSystem::default(), env);
    }
    let fs = FakeFileSystem::default().with_file("/p.json", policy);
    (fs, env.with_var("NVMRC_POLICY", "/p.json"))
}

fn served() -> FakeHttp {
    FakeHttp::default()
        .with_body(INDEX_URL, &node_index())
        .with_body(
            PNPM_URL,
            r#"{"versions": {"8.15.0": {}, "9.1.0": {}, "9.12.0": {}}}"#,
        )
        .with_empty_advisories()
}

fn pick(
    http: &FakeHttp,
    policy: &str,
    tool: Tool,
    current: &str,
    major: Major,
) -> Result<Target, CliError> {
    let (fs, env) = files(policy);
    let context = Context::new(&fs, &env)
        .with_http(http)
        .with_third_party_http(http, http);
    let mut judge = Judge::load(&context, &mut Transcript::default()).unwrap();
    target(&context, &mut judge, tool, &version(current), major)
}

fn node(policy: &str, current: &str, major: Major) -> Target {
    pick(&served(), policy, Tool::Node, current, major).unwrap()
}

#[test]
fn moves_to_the_newest_allowed_in_the_same_major() {
    let found = node("", "20.11.1", Major::Same);
    assert_eq!(found, Target::Move(version("20.18.0")));
}

#[test]
fn major_any_crosses_majors() {
    let policy = r#"{ "schemaVersion": 1, "tools": { "node": { "ltsOnly": true } } }"#;
    let found = node(policy, "20.11.1", Major::Any);
    assert_eq!(found, Target::Move(version("22.11.0")));
}

#[test]
fn major_n_picks_that_major() {
    let found = node("", "20.11.1", Major::Exactly(22));
    assert_eq!(found, Target::Move(version("22.11.0")));
}

#[test]
fn already_newest_is_up_to_date() {
    let found = node("", "20.18.0", Major::Same);
    assert_eq!(found, Target::UpToDate);
}

#[test]
fn a_denied_current_version_moves_to_the_newest_allowed() {
    let policy = r#"{ "schemaVersion": 1, "tools": { "node":
        { "deny": ["20.18.0"] } } }"#;
    let found = node(policy, "20.18.0", Major::Same);
    assert_eq!(found, Target::Move(version("20.11.1")));
}

#[test]
fn no_allowed_release_in_the_major_names_the_newest_and_the_lowest_major() {
    let policy = r#"{ "schemaVersion": 1, "tools": { "node":
        { "deny": ["20.18.0", "20.11.1", "23.0.0"] } } }"#;
    let found = node(policy, "20.11.1", Major::Same);
    let Target::NoneInMajor {
        major,
        newest: Some((newest, violations)),
        lowest,
    } = found
    else {
        panic!("expected NoneInMajor, got {found:?}");
    };
    assert_eq!((major, newest, lowest), (20, version("20.18.0"), Some(22)));
    assert!(matches!(violations[..], [Violation::Denied(_)]));
}

#[test]
fn no_major_with_an_allowed_release_has_no_lowest() {
    let policy = r#"{ "schemaVersion": 1, "tools": { "pnpm": { "minimum": "10.0.0" } } }"#;
    let found = pick(&served(), policy, Tool::Pnpm, "9.1.0", Major::Same).unwrap();
    assert!(matches!(
        found,
        Target::NoneInMajor {
            major: 9,
            lowest: None,
            ..
        }
    ));
}

#[test]
fn yarn_one_stays_on_yarn_and_major_moves_to_cli_dist() {
    let http = FakeHttp::default()
        .with_body(YARN_URL, r#"{"versions": {"1.22.21": {}, "1.22.22": {}}}"#)
        .with_body(CLI_DIST_URL, r#"{"versions": {"4.5.0": {}}}"#)
        .with_empty_advisories();
    let same = pick(&http, "", Tool::Yarn, "1.22.21", Major::Same).unwrap();
    assert_eq!(same, Target::Move(version("1.22.22")));
    assert!(http.requests().iter().all(|url| url == YARN_URL));
    let any = pick(&http, "", Tool::Yarn, "1.22.21", Major::Any).unwrap();
    assert_eq!(any, Target::Move(version("4.5.0")));
    assert_eq!(
        http.requests().last().map(String::as_str),
        Some(CLI_DIST_URL)
    );
}

#[test]
fn node_without_lts_only_may_pick_a_non_lts_release() {
    let found = node("", "20.11.1", Major::Any);
    assert_eq!(found, Target::Move(version("23.0.0")));
}
