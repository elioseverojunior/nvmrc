use crate::commands::project::test_support::{
    INDEX_URL, MIN_NPM, MIN_PNPM, PNPM_PACKUMENT, PNPM_URL, file, init, project, served,
    served_bundling,
};
use crate::domain::fixtures::{advisories_json, index_text_dated};
use crate::domain::policy::DEFAULT_INDEX;
use crate::fakes::FakeHttp;

const NPM_PACKUMENT: &str = r#"{"versions": {"10.4.0": {}, "10.9.0": {}}}"#;

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
fn yarn_over_a_pnpm_pin_is_refused_before_any_request() {
    let yarn = "https://registry.npmjs.org/@yarnpkg%2fcli-dist";
    let http = served().with_body(yarn, r#"{"versions": {"4.5.0": {}}}"#);
    let (fs, env) = project(&[("/p/.nvmrc", "v22.11.0\npnpm=9.12.0\n")], MIN_PNPM);
    let output = init(&fs, &env, &http, &["--node", "--yarn"]);
    assert_eq!(output.status.code(), 3);
    assert_eq!(
        output.stderr,
        "nvm init: /p/.nvmrc pins pnpm; remove its pnpm= line to pin yarn."
    );
    assert_eq!(
        file(&fs, "/p/.nvmrc").as_deref(),
        Some("v22.11.0\npnpm=9.12.0\n")
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[test]
fn pnpm_over_a_yarn_pin_is_refused_before_any_request() {
    let package_json = "{ \"packageManager\": \"yarn@4.5.0\" }";
    let files = [
        ("/p/.nvmrc", "yarn=4.5.0\n"),
        ("/p/package.json", package_json),
    ];
    let (fs, env) = project(&files, "");
    let http = served();
    let output = init(&fs, &env, &http, &["--pnpm"]);
    assert_eq!(output.status.code(), 3);
    assert_eq!(
        output.stderr,
        "nvm init: /p/.nvmrc pins yarn; remove its yarn= line to pin pnpm."
    );
    assert_eq!(file(&fs, "/p/.nvmrc").as_deref(), Some("yarn=4.5.0\n"));
    assert_eq!(file(&fs, "/p/package.json").as_deref(), Some(package_json));
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[test]
fn a_malformed_pin_is_refused_before_any_request() {
    let (fs, env) = project(&[("/p/.nvmrc", "v22.11.0\npnpm=nine\n")], "");
    let http = served();
    let output = init(&fs, &env, &http, &["--node"]);
    assert_eq!(output.status.code(), 3, "{}", output.stderr);
    assert!(output.stderr.contains("/p/.nvmrc"), "{}", output.stderr);
    assert!(http.requests().is_empty(), "{:?}", http.requests());
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

#[test]
fn findings_below_the_limit_warn_for_the_chosen_node() {
    let (fs, env) = project(&[], r#"{ "schemaVersion": 1 }"#);
    let env = env.with_var("NVMRC_FAIL_ON_SEVERITY", "critical");
    let index = index_text_dated(&[("v22.20.0", "2025-09-24", "Jod")]);
    let http = FakeHttp::default()
        .with_body(INDEX_URL, &index)
        .with_body(DEFAULT_INDEX, advisories_json());
    let warning = "nvm: warning: node v22.20.0 has known vulnerabilities below the limit \
                   critical: CVE-2026-58043 (high)";
    for args in [["--node"], ["--node=22.20.0"]] {
        let output = init(&fs, &env, &http, &args);
        assert_eq!(output.status.code(), 0, "{}", output.stderr);
        assert_eq!(output.stderr, warning);
        assert_eq!(output.stdout, "Wrote /p/.nvmrc: node v22.20.0");
    }
}

#[test]
fn the_newest_node_whose_bundled_npm_is_refused_is_skipped() {
    let http = served_bundling(&[
        ("v22.11.0", "Jod", "10.4.0"),
        ("v20.18.0", "Iron", "10.8.2"),
    ]);
    let (fs, env) = project(&[], MIN_NPM);
    let output = init(&fs, &env, &http, &["--node"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "Wrote /p/.nvmrc: node v20.18.0");
}

#[test]
fn no_node_with_an_allowed_bundled_npm_writes_nothing() {
    let http = served_bundling(&[("v22.11.0", "Jod", "10.4.0"), ("v20.18.0", "Iron", "-")]);
    let (fs, env) = project(&[], MIN_NPM);
    let output = init(&fs, &env, &http, &["--node"]);
    assert_eq!(output.status.code(), 7);
    assert_eq!(output.stderr, "nvm init: no allowed node LTS release.");
    assert_eq!(file(&fs, "/p/.nvmrc"), None);
}

#[test]
fn an_npm_set_in_the_same_run_leaves_the_bundled_npm_out_of_the_choice() {
    let http = served_bundling(&[("v22.11.0", "Jod", "10.4.0")])
        .with_body("https://registry.npmjs.org/npm", NPM_PACKUMENT);
    let (fs, env) = project(&[], MIN_NPM);
    let output = init(&fs, &env, &http, &["--node", "--npm"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "Wrote /p/.nvmrc: node v22.11.0, npm 10.9.0");
}
