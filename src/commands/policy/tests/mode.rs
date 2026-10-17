use super::{context, policy_with_minimum};
use crate::commands::policy::{Mode, resolve_mode};
use crate::domain::policy::{NodePolicy, PolicyFlags, Source, extract};
use crate::error::{CliError, PolicyError};
use crate::fakes::{FakeEnv, FakeFileSystem};
use std::ffi::OsString;

fn flags(values: &[&str]) -> PolicyFlags {
    let args = std::iter::once("nvm").chain(values.iter().copied());
    extract(args.map(OsString::from).collect()).unwrap().0
}

fn enforced(mode: Mode) -> NodePolicy {
    match mode {
        Mode::Enforced(policy) => policy,
        Mode::Legacy => panic!("expected an enforced policy"),
    }
}

#[test]
fn legacy_mode_without_any_new_configuration() {
    let fs = FakeFileSystem::default().with_file("/n/min-version", "18\n");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("NVM_MIN_VERSION", "20")
        .with_current_dir("/w");
    let (mode, warnings) = resolve_mode(&context(&fs, &env)).unwrap();
    assert!(matches!(mode, Mode::Legacy));
    assert!(warnings.is_empty());
}

#[test]
fn the_min_node_flag_alone_enforces() {
    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
    let context = context(&fs, &env).with_policy_flags(flags(&["--min-node", "20"]));
    let policy = enforced(resolve_mode(&context).unwrap().0);
    assert_eq!(policy.minimum.unwrap().source, Source::Flag("--min-node"));
}

#[test]
fn a_repository_policy_is_found_above_the_working_directory_and_enforces() {
    let fs = FakeFileSystem::default()
        .with_file("/w/.nvmrc-policy.json", &policy_with_minimum("22"))
        .with_dir("/w/app/src");
    let env = FakeEnv::default()
        .with_var("PWD", "/w/app/src")
        .with_current_dir("/w/app/src");
    let policy = enforced(resolve_mode(&context(&fs, &env)).unwrap().0);
    assert_eq!(
        policy.minimum.unwrap().source,
        Source::RepositoryPolicy("/w/.nvmrc-policy.json".into())
    );
}

#[test]
fn an_invalid_central_policy_is_a_failure_to_know() {
    let fs = FakeFileSystem::default().with_file("/bad.json", "{ \"schemaVersion\": 1, }");
    let env = FakeEnv::default().with_var("NVMRC_POLICY", "/bad.json");
    let error = resolve_mode(&context(&fs, &env)).unwrap_err();
    assert!(matches!(error, PolicyError::Invalid { .. }));
    assert_eq!(CliError::from(error).exit_code().code(), 9);
}

#[test]
fn a_flag_or_variable_that_is_not_a_version_is_invalid() {
    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
    let flagged = context(&fs, &env).with_policy_flags(flags(&["--min-node", "banana"]));
    let error = resolve_mode(&flagged).unwrap_err();
    assert!(matches!(&error, PolicyError::Invalid { location, .. } if location == "--min-node"));

    let fs = FakeFileSystem::default().with_file("/p.json", &policy_with_minimum("20"));
    let env = FakeEnv::default()
        .with_var("NVMRC_POLICY", "/p.json")
        .with_var("NVM_MIN_VERSION", "banana");
    let error = resolve_mode(&context(&fs, &env)).unwrap_err();
    assert!(
        matches!(&error, PolicyError::Invalid { location, .. } if location == "NVM_MIN_VERSION")
    );
}

#[test]
fn every_source_takes_part_in_the_merge() {
    let fs = FakeFileSystem::default()
        .with_file("/central.json", &policy_with_minimum("20"))
        .with_file("/n/min-version", "18\n");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("NVMRC_POLICY", "/central.json")
        .with_var("NVM_MIN_VERSION", "22")
        .with_current_dir("/w");
    let context = context(&fs, &env).with_policy_flags(flags(&["--min-node", "21"]));
    let (mode, warnings) = resolve_mode(&context).unwrap();
    let minimum = enforced(mode).minimum.unwrap();
    assert_eq!(minimum.value.to_string(), "v22.0.0");
    assert_eq!(minimum.source, Source::Variable("NVM_MIN_VERSION"));
    assert_eq!(warnings.len(), 2);
}

#[test]
fn a_tie_is_attributed_to_the_central_policy() {
    let fs = FakeFileSystem::default().with_file("/central.json", &policy_with_minimum("20"));
    let env = FakeEnv::default().with_var("NVMRC_POLICY", "/central.json");
    let context = context(&fs, &env).with_policy_flags(flags(&["--min-node", "20"]));
    let policy = enforced(resolve_mode(&context).unwrap().0);
    assert!(matches!(
        policy.minimum.unwrap().source,
        Source::CentralPolicy(_)
    ));
}
