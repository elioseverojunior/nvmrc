//! The rules of npm, yarn and pnpm: which sources set them and how they
//! merge.

use super::{context, enforced, flags};
use crate::commands::policy::{policy_mode_requested, resolve_mode};
use crate::domain::policy::Source;
use crate::domain::tool::Tool;
use crate::error::{CliError, PolicyError};
use crate::fakes::{FakeEnv, FakeFileSystem};

fn package_policy_file(tool: &str, minimum: &str) -> String {
    format!(r#"{{ "schemaVersion": 1, "tools": {{ "{tool}": {{ "minimum": "{minimum}" }} }} }}"#)
}

#[test]
fn npm_rules_merge_from_the_policy_and_the_variable() {
    let fs = FakeFileSystem::default().with_file("/c.json", &package_policy_file("npm", "10.2.4"));
    let env = FakeEnv::default()
        .with_var("NVMRC_POLICY", "/c.json")
        .with_var("NVMRC_MIN_NPM", "10.8.0");
    let (mode, warnings) = resolve_mode(&context(&fs, &env)).unwrap();
    let policy = enforced(mode);
    let minimum = policy.tool(Tool::Npm).minimum.clone().unwrap();
    assert_eq!(minimum.value.to_string(), "v10.8.0");
    assert_eq!(minimum.source, Source::Variable("NVMRC_MIN_NPM"));
    assert!(warnings.is_empty());
    assert!(policy.yarn.is_unrestricted() && policy.pnpm.is_unrestricted());
    assert!(policy.node.is_unrestricted());
}

#[test]
fn a_min_pnpm_flag_alone_turns_policy_mode_on() {
    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
    let context = context(&fs, &env).with_policy_flags(flags(&["--min-pnpm", "9"]));
    assert!(policy_mode_requested(&context));
    let policy = enforced(resolve_mode(&context).unwrap().0);
    let minimum = policy.tool(Tool::Pnpm).minimum.clone().unwrap();
    assert_eq!(minimum.value.to_string(), "v9.0.0");
    assert_eq!(minimum.source, Source::Flag("--min-pnpm"));
}

#[test]
fn a_blank_min_yarn_variable_is_unset() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVMRC_MIN_YARN", "  ");
    assert!(!policy_mode_requested(&context(&fs, &env)));
}

#[test]
fn an_invalid_min_npm_flag_is_a_failure_to_know() {
    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
    let flagged = context(&fs, &env).with_policy_flags(flags(&["--min-npm=ten"]));
    let error = resolve_mode(&flagged).unwrap_err();
    assert!(matches!(&error, PolicyError::Invalid { location, .. } if location == "--min-npm"));
    assert_eq!(CliError::from(error).exit_code().code(), 9);
}

#[test]
fn yarn_rules_from_the_repository_policy_apply() {
    let fs = FakeFileSystem::default()
        .with_file("/w/.nvmrc-policy.json", &package_policy_file("yarn", "4"))
        .with_dir("/w");
    let env = FakeEnv::default()
        .with_var("PWD", "/w")
        .with_current_dir("/w");
    let policy = enforced(resolve_mode(&context(&fs, &env)).unwrap().0);
    let minimum = policy.tool(Tool::Yarn).minimum.clone().unwrap();
    assert_eq!(minimum.value.to_string(), "v4.0.0");
    assert_eq!(
        minimum.source,
        Source::RepositoryPolicy("/w/.nvmrc-policy.json".into())
    );
}
