//! `nvm upgrade --node` with an npm rule and no `npm=` pin: the npm a node
//! release bundles must be allowed too, or the install would refuse it.

use super::tests::upgrade_in;
use crate::commands::project::test_support::{
    MIN_NPM, NODE_22_NPM_AGED, NPM_UNCHECKED, file, project, served_bundling,
};
use crate::fakes::FakeHttp;

/// Major 20 with its newest release bundling an npm below [`MIN_NPM`].
fn index() -> FakeHttp {
    served_bundling(&[
        ("v22.11.0", "Jod", "10.9.0"),
        ("v20.18.0", "Iron", "10.4.0"),
        ("v20.17.0", "Iron", "10.8.2"),
        ("v20.11.1", "Iron", "10.2.4"),
    ])
}

#[test]
fn a_node_whose_bundled_npm_is_refused_is_skipped() {
    let (fs, env) = project(&[("/p/.nvmrc", "v20.11.1\n")], MIN_NPM);
    let (output, _) = upgrade_in(&fs, &env, &index(), &["--node"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "node v20.11.1 -> v20.17.0");
    assert_eq!(file(&fs, "/p/.nvmrc").as_deref(), Some("v20.17.0\n"));
}

#[test]
fn a_node_whose_bundled_npm_is_unknown_is_skipped() {
    let http = served_bundling(&[("v20.18.0", "Iron", "-"), ("v20.17.0", "Iron", "10.8.2")]);
    let (fs, env) = project(&[("/p/.nvmrc", "v20.11.1\n")], MIN_NPM);
    let (output, _) = upgrade_in(&fs, &env, &http, &["--node"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "node v20.11.1 -> v20.17.0");
}

#[test]
fn an_npm_pin_leaves_the_bundled_npm_out_of_the_choice() {
    let nvmrc = "v20.11.1\nnpm=10.8.2\n";
    let (fs, env) = project(&[("/p/.nvmrc", nvmrc)], MIN_NPM);
    let (output, _) = upgrade_in(&fs, &env, &index(), &["--node"]);
    assert_eq!(output.status.code(), 0, "{}", output.stderr);
    assert_eq!(output.stdout, "node v20.11.1 -> v20.18.0");
}

#[test]
fn no_node_with_an_allowed_bundled_npm_writes_nothing() {
    let policy = r#"{ "schemaVersion": 1, "tools": { "npm": { "minimum": "11.0.0" } } }"#;
    let (fs, env) = project(&[("/p/.nvmrc", "v20.11.1\n")], policy);
    let (output, _) = upgrade_in(&fs, &env, &index(), &["--node"]);
    assert_eq!(output.status.code(), 7, "{}", output.stderr);
    assert_eq!(output.stdout, "");
    assert_eq!(
        output.stderr,
        "nvm upgrade: no allowed node release in major 20.\n\
         nvm: node v20.18.0 (the newest in major 20) is not allowed by policy:\n  \
         - the npm it bundles, npm 10.4.0, is below the minimum 11.0.0 (from policy /policy.json)\n\
         No major has an allowed node release."
    );
    assert_eq!(file(&fs, "/p/.nvmrc").as_deref(), Some("v20.11.1\n"));
}

#[test]
fn a_no_target_block_with_node_violations_survives_an_npm_that_cannot_be_checked() {
    let http = served_bundling(&[
        ("v20.18.0", "Iron", "10.4.0"),
        ("v20.11.1", "Iron", "10.2.4"),
    ]);
    let (fs, env) = project(&[("/p/.nvmrc", "v20.11.1\n")], NODE_22_NPM_AGED);
    let (output, _) = upgrade_in(&fs, &env, &http, &["--node"]);
    assert_eq!(output.status.code(), 7, "{}", output.stderr);
    assert_eq!(
        output.stderr,
        format!(
            "nvm upgrade: no allowed node release in major 20.\n\
             nvm: node v20.18.0 (the newest in major 20) is not allowed by policy:\n  \
             - below the minimum v22.0.0 (from policy /policy.json)\n\
             No major has an allowed node release.\n{NPM_UNCHECKED}"
        )
    );
    assert_eq!(file(&fs, "/p/.nvmrc").as_deref(), Some("v20.11.1\n"));
}
