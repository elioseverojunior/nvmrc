use super::{context, policy_with_minimum};
use crate::commands::policy::EnforcedPolicy;
use crate::commands::policy::policy_mode_requested;
use crate::commands::policy::{Mode, resolve_mode};
use crate::domain::policy::{DEFAULT_INDEX, PolicyFlags, Severity, Source, Sourced, extract};
use crate::domain::tool::Tool;
use crate::error::{CliError, PolicyError};
use crate::fakes::{FakeEnv, FakeFileSystem};
use std::ffi::OsString;

fn flags(values: &[&str]) -> PolicyFlags {
    let args = std::iter::once("nvm").chain(values.iter().copied());
    extract(args.map(OsString::from).collect()).unwrap().0
}

fn enforced(mode: Mode) -> EnforcedPolicy {
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
    let policy = enforced(resolve_mode(&context).unwrap().0).node;
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
    let policy = enforced(resolve_mode(&context(&fs, &env)).unwrap().0).node;
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
    let minimum = enforced(mode).node.minimum.unwrap();
    assert_eq!(minimum.value.to_string(), "v22.0.0");
    assert_eq!(minimum.source, Source::Variable("NVM_MIN_VERSION"));
    assert_eq!(warnings.len(), 2);
}

#[test]
fn a_tie_is_attributed_to_the_central_policy() {
    let fs = FakeFileSystem::default().with_file("/central.json", &policy_with_minimum("20"));
    let env = FakeEnv::default().with_var("NVMRC_POLICY", "/central.json");
    let context = context(&fs, &env).with_policy_flags(flags(&["--min-node", "20"]));
    let policy = enforced(resolve_mode(&context).unwrap().0).node;
    assert!(matches!(
        policy.minimum.unwrap().source,
        Source::CentralPolicy(_)
    ));
}

#[test]
fn a_min_version_file_that_cannot_be_read_is_an_error() {
    use crate::ports::FileSystem;
    let fs = FakeFileSystem::default();
    fs.write_bytes(std::path::Path::new("/n/min-version"), &[0xff, 0xfe])
        .unwrap();
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let context = context(&fs, &env).with_policy_flags(flags(&["--min-node", "20"]));
    let error = resolve_mode(&context).unwrap_err();
    assert!(
        matches!(&error, PolicyError::Unreadable { location, .. } if location == "/n/min-version")
    );
}

#[test]
fn a_missing_min_version_file_is_no_layer() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let context = context(&fs, &env).with_policy_flags(flags(&["--min-node", "20"]));
    let (mode, warnings) = resolve_mode(&context).unwrap();
    assert!(enforced(mode).node.minimum.is_some());
    assert!(warnings.is_empty());
}

#[test]
fn a_policy_flag_wins_over_the_policy_variable() {
    let fs = FakeFileSystem::default()
        .with_file("/variable.json", &policy_with_minimum("22"))
        .with_file("/flag.json", &policy_with_minimum("20"));
    let env = FakeEnv::default().with_var("NVMRC_POLICY", "/variable.json");
    let context = context(&fs, &env).with_policy_flags(flags(&["--policy", "/flag.json"]));
    let minimum = enforced(resolve_mode(&context).unwrap().0)
        .node
        .minimum
        .unwrap();
    assert_eq!(minimum.value.to_string(), "v20.0.0");
    assert_eq!(minimum.source, Source::CentralPolicy("/flag.json".into()));
}

fn cve_policy(severity: &str, source: Option<&str>) -> String {
    let source = source.map_or(String::new(), |value| format!(r#", "source": "{value}""#));
    format!(r#"{{ "schemaVersion": 1, "cve": {{ "failOnSeverity": "{severity}"{source} }} }}"#)
}

#[test]
fn the_severity_flag_and_variable_turn_policy_mode_on() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVMRC_FAIL_ON_SEVERITY", "low");
    assert!(policy_mode_requested(&context(&fs, &env)));
    let env = FakeEnv::default();
    let flagged = context(&fs, &env).with_policy_flags(flags(&["--fail-on-severity", "low"]));
    assert!(policy_mode_requested(&flagged));
    let env = FakeEnv::default().with_var("NVMRC_FAIL_ON_SEVERITY", " ");
    assert!(!policy_mode_requested(&context(&fs, &env)));
}

#[test]
fn an_invalid_severity_flag_or_variable_is_a_failure_to_know() {
    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
    let flagged = context(&fs, &env).with_policy_flags(flags(&["--fail-on-severity", "critial"]));
    let reason = "'critial' is not one of critical, high, medium, low, none".to_owned();
    assert_eq!(
        resolve_mode(&flagged).unwrap_err(),
        PolicyError::Invalid {
            location: "--fail-on-severity".into(),
            reason: reason.clone()
        }
    );
    let env = FakeEnv::default().with_var("NVMRC_FAIL_ON_SEVERITY", "critial");
    assert_eq!(
        resolve_mode(&context(&fs, &env)).unwrap_err(),
        PolicyError::Invalid {
            location: "NVMRC_FAIL_ON_SEVERITY".into(),
            reason
        }
    );
}

#[test]
fn the_cve_settings_of_both_policies_take_part() {
    let fs = FakeFileSystem::default()
        .with_file("/central.json", &cve_policy("medium", None))
        .with_file("/w/.nvmrc-policy.json", &cve_policy("low", Some("v.json")));
    let env = FakeEnv::default()
        .with_var("NVMRC_POLICY", "/central.json")
        .with_var("PWD", "/w")
        .with_current_dir("/w");
    let policy = enforced(resolve_mode(&context(&fs, &env)).unwrap().0);
    assert_eq!(policy.cve.limit.value, Severity::Low);
    assert_eq!(policy.cve.index.value, DEFAULT_INDEX);
}

#[test]
fn the_severity_flag_can_only_tighten() {
    let fs = FakeFileSystem::default().with_file("/central.json", &cve_policy("medium", None));
    let env = FakeEnv::default().with_var("NVMRC_POLICY", "/central.json");
    let flagged = context(&fs, &env).with_policy_flags(flags(&["--fail-on-severity", "critical"]));
    let (mode, warnings) = resolve_mode(&flagged).unwrap();
    assert_eq!(enforced(mode).cve.limit.value, Severity::Medium);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn a_relative_index_location_is_read_next_to_its_policy() {
    let fs = FakeFileSystem::default().with_file(
        "/w/.nvmrc-policy.json",
        &cve_policy("high", Some("sec/vuln.json")),
    );
    let env = FakeEnv::default()
        .with_var("PWD", "/w/app/src")
        .with_current_dir("/w/app/src");
    let fs = fs.with_dir("/w/app/src");
    let policy = enforced(resolve_mode(&context(&fs, &env)).unwrap().0);
    assert_eq!(policy.cve.index.value, "/w/sec/vuln.json");
}

#[test]
fn a_relative_index_in_a_central_file_is_read_next_to_it() {
    let fs = FakeFileSystem::default()
        .with_file("/etc/p/central.json", &cve_policy("high", Some("v.json")));
    let env = FakeEnv::default()
        .with_var("NVMRC_POLICY", "/etc/p/central.json")
        .with_current_dir("/w");
    let policy = enforced(resolve_mode(&context(&fs, &env)).unwrap().0);
    assert_eq!(policy.cve.index.value, "/etc/p/v.json");
}

#[test]
fn a_relative_index_in_a_remote_policy_is_invalid() {
    let env = FakeEnv::default().with_var("NVMRC_POLICY", "https://sec/p.json");
    let fs = FakeFileSystem::default();
    let http = crate::fakes::FakeHttp::default()
        .with_body("https://sec/p.json", &cve_policy("high", Some("vuln.json")));
    let context = context(&fs, &env).with_http(&http);
    assert!(matches!(
        resolve_mode(&context).unwrap_err(),
        PolicyError::Invalid { location, .. } if location == "https://sec/p.json"
    ));
}

#[test]
fn an_uppercase_https_central_policy_is_remote() {
    let env = FakeEnv::default().with_var("NVMRC_POLICY", "HTTPS://sec/p.json");
    let fs = FakeFileSystem::default();
    let http = crate::fakes::FakeHttp::default()
        .with_body("HTTPS://sec/p.json", &cve_policy("high", Some("vuln.json")));
    let context = context(&fs, &env).with_http(&http);
    assert!(matches!(
        resolve_mode(&context).unwrap_err(),
        PolicyError::Invalid { location, .. } if location == "HTTPS://sec/p.json"
    ));
}

#[test]
fn a_central_policy_without_cve_settings_still_decides_the_index() {
    let fs = FakeFileSystem::default()
        .with_file("/central.json", &policy_with_minimum("20"))
        .with_file("/w/.nvmrc-policy.json", &cve_policy("high", Some("e.json")));
    let env = FakeEnv::default()
        .with_var("NVMRC_POLICY", "/central.json")
        .with_var("PWD", "/w")
        .with_current_dir("/w");
    let policy = enforced(resolve_mode(&context(&fs, &env)).unwrap().0);
    assert_eq!(
        policy.cve.index,
        Sourced {
            value: DEFAULT_INDEX.to_owned(),
            source: Source::Default
        }
    );
}

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
