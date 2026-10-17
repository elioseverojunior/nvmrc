use std::path::Path;

use crate::commands::Output;
use crate::commands::init::run;
use crate::context::Context;
use crate::domain::fixtures::index_text_dated;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};
use crate::ports::FileSystem;

const INDEX_URL: &str = "https://nodejs.org/dist/index.tab";
const PNPM_URL: &str = "https://registry.npmjs.org/pnpm";
const PNPM_PACKUMENT: &str = r#"{"versions": {"9.1.0": {}, "9.12.0": {}, "10.0.0-rc.1": {}}}"#;
const MIN_PNPM: &str = r#"{ "schemaVersion": 1, "tools": { "pnpm": { "minimum": "9.12.0" } } }"#;

fn node_index() -> String {
    index_text_dated(&[
        ("v23.0.0", "2024-05-01", "-"),
        ("v22.11.0", "2024-10-29", "Jod"),
        ("v20.18.0", "2024-10-03", "Iron"),
    ])
}

/// A project in `/p` with `files` in it and, unless empty, a policy.
fn project(files: &[(&str, &str)], policy: &str) -> (FakeFileSystem, FakeEnv) {
    let mut fs = FakeFileSystem::default().with_dir("/p");
    for (path, contents) in files {
        fs = fs.with_file(path, contents);
    }
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PWD", "/p")
        .with_current_dir("/p");
    if policy.is_empty() {
        return (fs, env);
    }
    let fs = fs.with_file("/policy.json", policy);
    (fs, env.with_var("NVMRC_POLICY", "/policy.json"))
}

fn served() -> FakeHttp {
    FakeHttp::default()
        .with_body(INDEX_URL, &node_index())
        .with_body(PNPM_URL, PNPM_PACKUMENT)
        .with_empty_advisories()
}

fn init(fs: &FakeFileSystem, env: &FakeEnv, http: &FakeHttp, args: &[&str]) -> Output {
    let context = Context::new(fs, env)
        .with_http(http)
        .with_third_party_http(http, http);
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    run(&context, &args).unwrap_or_else(|error| panic!("{error}"))
}

fn file(fs: &FakeFileSystem, path: &str) -> Option<String> {
    fs.read_to_string(Path::new(path)).ok()
}

#[test]
fn newest_node_and_pnpm_are_written_with_package_manager() {
    let (fs, env) = project(&[("/p/package.json", "{\n  \"name\": \"app\"\n}\n")], "");
    let output = init(&fs, &env, &served(), &["--node", "--pnpm"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(
        output.stdout,
        "Wrote /p/.nvmrc: node v22.11.0, pnpm 9.12.0\n\
         Set packageManager to \"pnpm@9.12.0\" in /p/package.json"
    );
    assert_eq!(
        file(&fs, "/p/.nvmrc").as_deref(),
        Some("v22.11.0\npnpm=9.12.0\n")
    );
    assert_eq!(
        file(&fs, "/p/package.json").as_deref(),
        Some("{\n  \"name\": \"app\",\n  \"packageManager\": \"pnpm@9.12.0\"\n}\n")
    );
}

#[test]
fn an_existing_nvmrc_keeps_its_other_lines() {
    let nvmrc = "# the app\n20\nnpm=10.8.0 # bundled\n";
    let (fs, env) = project(&[("/p/.nvmrc", nvmrc)], "");
    let output = init(&fs, &env, &served(), &["--node=20.18.0"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "Wrote /p/.nvmrc: node v20.18.0");
    assert_eq!(
        file(&fs, "/p/.nvmrc").as_deref(),
        Some("# the app\nv20.18.0\nnpm=10.8.0 # bundled\n")
    );
}

#[test]
fn an_exact_version_that_is_not_allowed_writes_nothing() {
    let (fs, env) = project(&[("/p/package.json", "{}")], MIN_PNPM);
    let output = init(&fs, &env, &served(), &["--node", "--pnpm=9.1.0"]);
    assert_eq!(output.status.code(), 7);
    assert_eq!(
        output.stderr,
        "nvm: pnpm 9.1.0 (from the command line) is not allowed by policy:\n  \
         - below the minimum 9.12.0 (from policy /policy.json)\n\
         Use an allowed version."
    );
    assert_eq!(file(&fs, "/p/.nvmrc"), None);
    assert_eq!(file(&fs, "/p/package.json").as_deref(), Some("{}"));
}

#[test]
fn an_unpublished_exact_version_writes_nothing() {
    let (fs, env) = project(&[], "");
    let output = init(&fs, &env, &served(), &["--pnpm=9.99.0"]);
    assert_eq!(output.status.code(), 3);
    assert_eq!(
        output.stderr,
        "nvm init: pnpm 9.99.0 is not a published release."
    );
    let output = init(&fs, &env, &served(), &["--node=v20.99.0"]);
    assert_eq!(output.status.code(), 3);
    assert_eq!(
        output.stderr,
        "nvm init: node v20.99.0 is not a node release."
    );
    assert_eq!(file(&fs, "/p/.nvmrc"), None);
}

#[test]
fn no_allowed_release_writes_nothing() {
    let policy = r#"{ "schemaVersion": 1, "tools": { "node": { "minimum": "24.0.0" } } }"#;
    let (fs, env) = project(&[], policy);
    let output = init(&fs, &env, &served(), &["--node"]);
    assert_eq!(output.status.code(), 7);
    assert_eq!(output.stderr, "nvm init: no allowed node LTS release.");
    let policy = r#"{ "schemaVersion": 1, "tools": { "pnpm": { "minimum": "11.0.0" } } }"#;
    let (fs, env) = project(&[], policy);
    let output = init(&fs, &env, &served(), &["--pnpm"]);
    assert_eq!(output.status.code(), 7);
    assert_eq!(output.stderr, "nvm init: no allowed pnpm release.");
    assert_eq!(file(&fs, "/p/.nvmrc"), None);
}

#[test]
fn yarn_over_a_pnpm_pin_is_refused() {
    let yarn = "https://registry.npmjs.org/@yarnpkg%2fcli-dist";
    let http = served().with_body(yarn, r#"{"versions": {"4.5.0": {}}}"#);
    let (fs, env) = project(&[("/p/.nvmrc", "v22.11.0\npnpm=9.12.0\n")], "");
    let output = init(&fs, &env, &http, &["--yarn"]);
    assert_eq!(output.status.code(), 3);
    assert_eq!(
        output.stderr,
        "nvm init: /p/.nvmrc pins pnpm; remove its pnpm= line to pin yarn."
    );
    assert_eq!(
        file(&fs, "/p/.nvmrc").as_deref(),
        Some("v22.11.0\npnpm=9.12.0\n")
    );
}

#[test]
fn no_package_json_is_left_absent() {
    let (fs, env) = project(&[], "");
    let output = init(&fs, &env, &served(), &["--pnpm=9.12.0"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "Wrote /p/.nvmrc: pnpm 9.12.0");
    assert_eq!(file(&fs, "/p/.nvmrc").as_deref(), Some("pnpm=9.12.0\n"));
    assert_eq!(file(&fs, "/p/package.json"), None);
}

#[test]
fn a_malformed_package_json_writes_nothing() {
    let (fs, env) = project(&[("/p/package.json", "{")], "");
    let output = init(&fs, &env, &served(), &["--pnpm"]);
    assert_eq!(output.status.code(), 3);
    assert!(
        output
            .stderr
            .starts_with("nvm: /p/package.json cannot be read as JSON: "),
        "{}",
        output.stderr
    );
    assert_eq!(file(&fs, "/p/.nvmrc"), None);
}

#[test]
fn a_failure_to_know_writes_nothing() {
    let (fs, env) = project(&[("/p/package.json", "{}")], "");
    let http = FakeHttp::default()
        .with_status(INDEX_URL, 503)
        .with_body(PNPM_URL, PNPM_PACKUMENT);
    let output = init(&fs, &env, &http, &["--pnpm", "--node"]);
    assert_eq!(output.status.code(), 9);
    assert_eq!(
        output.stderr,
        format!(
            "nvm: cannot check the policy: the release information is unavailable: {INDEX_URL}"
        )
    );
    assert_eq!(file(&fs, "/p/.nvmrc"), None);
    assert_eq!(file(&fs, "/p/package.json").as_deref(), Some("{}"));
}
