//! `nvm init --node=<version>` with an npm rule and no npm pin: the npm the
//! release bundles must be allowed too, or the install would refuse it.

use crate::commands::project::test_support::{MIN_NPM, file, init, project, served_bundling};

#[test]
fn an_exact_node_whose_bundled_npm_is_refused_writes_nothing() {
    let http = served_bundling(&[("v20.18.0", "Iron", "10.4.0")]);
    let (fs, env) = project(&[], MIN_NPM);
    let output = init(&fs, &env, &http, &["--node=20.18.0"]);
    assert_eq!(output.status.code(), 7, "{}", output.stderr);
    assert_eq!(
        output.stderr,
        "nvm: node v20.18.0 (from the command line) is not allowed by policy:\n  \
         - the npm it bundles, npm 10.4.0, is below the minimum 10.5.0 (from policy /policy.json)\n\
         Use an allowed version."
    );
    assert_eq!(file(&fs, "/p/.nvmrc"), None);
}

#[test]
fn an_exact_node_whose_bundled_npm_is_unknown_writes_nothing() {
    let http = served_bundling(&[("v20.18.0", "Iron", "-")]);
    let (fs, env) = project(&[], MIN_NPM);
    let output = init(&fs, &env, &http, &["--node=20.18.0"]);
    assert_eq!(output.status.code(), 7, "{}", output.stderr);
    assert_eq!(
        output.stderr,
        "nvm: node v20.18.0 (from the command line) is not allowed by policy:\n  \
         - the npm it bundles is unknown, and npm has rules\n\
         Use an allowed version."
    );
    assert_eq!(file(&fs, "/p/.nvmrc"), None);
}

#[test]
fn an_exact_node_with_npm_set_in_the_same_run_is_allowed() {
    let packument = r#"{"versions": {"10.4.0": {}, "10.9.0": {}}}"#;
    let http = served_bundling(&[("v20.18.0", "Iron", "10.4.0")])
        .with_body("https://registry.npmjs.org/npm", packument);
    let (fs, env) = project(&[], MIN_NPM);
    let output = init(&fs, &env, &http, &["--node=20.18.0", "--npm=10.9.0"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "Wrote /p/.nvmrc: node v20.18.0, npm 10.9.0");
}
