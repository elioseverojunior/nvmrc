use super::*;
use crate::error::AliasError;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess};

fn resolve_with(fs: &FakeFileSystem, name: &str) -> Result<Resolved, CliError> {
    resolve_on_path(fs, "/nonexistent", name)
}

fn resolve_on_path(fs: &FakeFileSystem, path: &str, name: &str) -> Result<Resolved, CliError> {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", path);
    resolve_installed(&Context::new(fs, &env), name)
}

fn installed() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file("/n/versions/node/v20.1.0/bin/node", "")
        .with_file("/n/versions/node/v20.10.0/bin/node", "")
        .with_file("/n/versions/node/v18.9.0/bin/node", "")
}

fn version(text: &str) -> Version {
    text.parse().expect("valid version")
}

#[test]
fn a_pattern_resolves_to_the_highest_installed_match() {
    let resolved = resolve_with(&installed(), "20").unwrap();
    assert_eq!(resolved, Resolved::Installed(version("v20.10.0")));
}

#[test]
fn node_is_the_latest_installed_node_ignoring_iojs() {
    let fs = installed().with_file("/n/versions/io.js/v3.0.0/bin/node", "");
    let resolved = resolve_with(&fs, "node").unwrap();
    assert_eq!(resolved, Resolved::Installed(version("v20.10.0")));
}

#[test]
fn iojs_is_the_latest_installed_iojs() {
    let fs = installed()
        .with_file("/n/versions/io.js/v3.0.0/bin/node", "")
        .with_file("/n/versions/io.js/v2.5.0/bin/node", "");
    let resolved = resolve_with(&fs, "iojs").unwrap();
    assert_eq!(resolved, Resolved::Installed(version("iojs-v3.0.0")));
}

fn version_of_system_node(process: &FakeProcess) -> Option<String> {
    let fs = FakeFileSystem::default().with_file("/usr/bin/node", "");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/usr/bin");
    let context = Context::new(&fs, &env).with_process(process);
    system_version(&context).unwrap()
}

#[test]
fn the_system_version_is_what_node_prints() {
    let process = FakeProcess::default().with_output("/usr/bin/node", "v22.1.0\n");
    assert_eq!(version_of_system_node(&process), Some("v22.1.0".to_owned()));
}

#[test]
fn the_system_version_is_none_when_node_fails_or_cannot_run() {
    let failing = FakeProcess::default().with_failure("/usr/bin/node");
    assert_eq!(version_of_system_node(&failing), None);
    assert_eq!(version_of_system_node(&FakeProcess::default()), None);
}

#[test]
fn without_a_system_node_there_is_no_system_version() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/usr/bin");
    let context = Context::new(&fs, &env);
    assert_eq!(system_version(&context).unwrap(), None);
}

#[test]
fn stable_is_the_highest_patch_of_the_highest_release_line() {
    let resolved = resolve_with(&installed(), "stable").unwrap();
    assert_eq!(resolved, Resolved::Installed(version("v20.10.0")));
}

#[test]
fn unstable_needs_an_old_odd_release_line() {
    let modern = resolve_with(&installed(), "unstable").unwrap();
    assert_eq!(
        modern,
        Resolved::Missing {
            resolved: "unstable".into()
        }
    );
    let old = FakeFileSystem::default()
        .with_file("/n/versions/node/v0.10.48/bin/node", "")
        .with_file("/n/versions/node/v0.11.16/bin/node", "")
        .with_file("/n/versions/node/v4.2.0/bin/node", "");
    assert_eq!(
        resolve_with(&old, "unstable").unwrap(),
        Resolved::Installed(version("v0.11.16"))
    );
    assert_eq!(
        resolve_with(&old, "node").unwrap(),
        Resolved::Installed(version("v4.2.0"))
    );
}

#[test]
fn shown_tells_versions_system_missing_and_loops_apart() {
    let fs = installed()
        .with_file("/n/alias/loop", "loop")
        .with_file("/usr/bin/node", "");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/usr/bin");
    let context = Context::new(&fs, &env);
    let texts: Vec<String> = ["20", "system", "16", "loop"]
        .iter()
        .map(|name| shown(&context, name).unwrap().to_string())
        .collect();
    assert_eq!(texts, ["v20.10.0", "system", "N/A", "∞"]);
    assert!(shown(&context, "20").unwrap().is_available());
    assert!(!shown(&context, "loop").unwrap().is_available());
    assert!(!shown(&context, "16").unwrap().is_available());
}

#[test]
fn a_built_in_alias_can_be_the_target_of_another_alias() {
    let fs = installed().with_file("/n/alias/default", "node");
    let resolved = resolve_with(&fs, "default").unwrap();
    assert_eq!(resolved, Resolved::Installed(version("v20.10.0")));
}

#[test]
fn a_built_in_alias_without_a_matching_install_is_missing() {
    let fs = FakeFileSystem::default().with_dir("/n/versions/node");
    let resolved = resolve_with(&fs, "node").unwrap();
    assert_eq!(
        resolved,
        Resolved::Missing {
            resolved: "node".into()
        }
    );
}

#[test]
fn an_alias_resolves_through_its_chain() {
    let fs = installed().with_file("/n/alias/default", "v18");
    let resolved = resolve_with(&fs, "default").unwrap();
    assert_eq!(resolved, Resolved::Installed(version("v18.9.0")));
}

#[test]
fn a_missing_version_reports_where_the_chain_ended() {
    let fs = installed().with_file("/n/alias/old", "v16");
    let resolved = resolve_with(&fs, "old").unwrap();
    assert_eq!(
        resolved,
        Resolved::Missing {
            resolved: "v16".into()
        }
    );
}

#[test]
fn a_name_that_is_not_a_version_is_missing_not_an_error() {
    let resolved = resolve_with(&installed(), "foo").unwrap();
    assert_eq!(
        resolved,
        Resolved::Missing {
            resolved: "foo".into()
        }
    );
}

#[test]
fn an_alias_loop_is_an_error() {
    let fs = installed()
        .with_file("/n/alias/a", "b")
        .with_file("/n/alias/b", "a");
    let error = resolve_with(&fs, "a").unwrap_err();
    assert!(matches!(error, CliError::Alias(AliasError::Loop(_))));
}

#[test]
fn system_is_resolved_when_a_node_exists_outside_nvm_dir() {
    let fs = installed().with_file("/usr/bin/node", "");
    let path = "/n/versions/node/v20.1.0/bin:/usr/bin";
    assert_eq!(
        resolve_on_path(&fs, path, "system").unwrap(),
        Resolved::System
    );
}

#[test]
fn an_alias_chain_ending_at_system_is_system() {
    let fs = installed()
        .with_file("/usr/bin/node", "")
        .with_file("/n/alias/default", "system");
    let resolved = resolve_on_path(&fs, "/usr/bin", "default").unwrap();
    assert_eq!(resolved, Resolved::System);
}

#[test]
fn system_without_a_system_node_is_missing() {
    let path = "/n/versions/node/v20.1.0/bin";
    let resolved = resolve_on_path(&installed(), path, "system").unwrap();
    assert_eq!(
        resolved,
        Resolved::Missing {
            resolved: "system".into()
        }
    );
}

#[test]
fn system_node_is_the_first_node_outside_nvm_dir() {
    let fs = installed().with_file("/usr/bin/node", "");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/n/versions/node/v20.1.0/bin:/usr/bin");
    let found = system_node(&Context::new(&fs, &env)).unwrap();
    assert_eq!(found, Some(PathBuf::from("/usr/bin/node")));
}

// nvm.sh never reads an alias file for the bare names `node` and `iojs`.
#[test]
fn node_ignores_its_own_alias_file_but_chains_through_it_follow_it() {
    let fs = installed()
        .with_file("/n/alias/node", "18")
        .with_file("/n/alias/default", "node");
    let node = resolve_with(&fs, "node").unwrap();
    assert_eq!(node, Resolved::Installed(version("v20.10.0")));
    let default = resolve_with(&fs, "default").unwrap();
    assert_eq!(default, Resolved::Installed(version("v18.9.0")));
}

#[test]
fn node_and_chains_ending_at_node_follow_a_stable_alias_file() {
    let fs = installed()
        .with_file("/n/alias/stable", "18")
        .with_file("/n/alias/default", "node");
    for name in ["node", "default", "stable"] {
        let resolved = resolve_with(&fs, name).unwrap();
        assert_eq!(resolved, Resolved::Installed(version("v18.9.0")), "{name}");
    }
}

#[test]
fn iojs_is_the_newest_iojs_whatever_its_alias_file_says() {
    let iojs = || installed().with_file("/n/versions/io.js/v3.0.0/bin/node", "");
    for target in ["iojs", "18"] {
        let fs = iojs().with_file("/n/alias/iojs", target);
        let resolved = resolve_with(&fs, "iojs").unwrap();
        assert_eq!(resolved, Resolved::Installed(version("iojs-v3.0.0")));
    }
    let chained = iojs()
        .with_file("/n/alias/iojs", "18")
        .with_file("/n/alias/y", "iojs");
    let resolved = resolve_with(&chained, "y").unwrap();
    assert_eq!(resolved, Resolved::Installed(version("v18.9.0")));
}

#[test]
fn a_missing_iojs_with_a_looping_alias_file_reports_the_loop() {
    let fs = installed().with_file("/n/alias/iojs", "iojs");
    let resolved = resolve_with(&fs, "iojs").unwrap();
    let infinite = "∞".to_owned();
    assert_eq!(resolved, Resolved::Missing { resolved: infinite });
}
