use super::{POLICY, context};
use crate::commands::policy::{policy_mode_requested, read_location};
use crate::error::PolicyError;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};

#[test]
fn an_http_location_is_rejected() {
    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
    assert_eq!(
        read_location(&context(&fs, &env), "http://sec/p.json"),
        Err(PolicyError::InsecureUrl("http://sec/p.json".into()))
    );
}

#[test]
fn an_https_location_is_fetched_and_a_failure_is_unreadable() {
    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
    let http = FakeHttp::default().with_body("https://sec/p.json", POLICY);
    let with_http = context(&fs, &env).with_http(&http);
    assert_eq!(
        read_location(&with_http, "https://sec/p.json").unwrap(),
        POLICY
    );
    assert!(matches!(
        read_location(&context(&fs, &env), "https://sec/missing.json"),
        Err(PolicyError::Unreadable { .. })
    ));
}

#[test]
fn a_relative_location_is_read_from_the_working_directory() {
    let fs = FakeFileSystem::default().with_file("/w/app/sec/p.json", POLICY);
    let env = FakeEnv::default().with_current_dir("/w/app");
    assert_eq!(
        read_location(&context(&fs, &env), "sec/p.json").unwrap(),
        POLICY
    );
}

#[test]
fn an_absolute_location_is_read_as_it_is() {
    let fs = FakeFileSystem::default().with_file("/etc/p.json", POLICY);
    let env = FakeEnv::default();
    assert_eq!(
        read_location(&context(&fs, &env), "/etc/p.json").unwrap(),
        POLICY
    );
    assert!(matches!(
        read_location(&context(&fs, &env), "/etc/none.json"),
        Err(PolicyError::Unreadable { .. })
    ));
}

#[test]
fn an_empty_policy_variable_is_unset() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVMRC_POLICY", "");
    assert!(!policy_mode_requested(&context(&fs, &env)));
    let env = FakeEnv::default().with_var("NVMRC_POLICY", "/p.json");
    assert!(policy_mode_requested(&context(&fs, &env)));
}

#[test]
fn an_advisory_policy_fetch_goes_through_the_quick_client_once() {
    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
    let (regular, quick) = (FakeHttp::default(), FakeHttp::default());
    let advisory = context(&fs, &env)
        .with_http(&regular)
        .with_quick_http(&quick)
        .advisory();
    assert!(matches!(
        read_location(&advisory, "https://sec/p.json"),
        Err(PolicyError::Unreadable { .. })
    ));
    assert_eq!(quick.requests(), ["https://sec/p.json"]);
    assert!(regular.requests().is_empty());
}

#[test]
fn an_explicit_policy_fetch_keeps_the_regular_client() {
    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
    let regular = FakeHttp::default().with_body("https://sec/p.json", POLICY);
    let quick = FakeHttp::default();
    let explicit = context(&fs, &env)
        .with_http(&regular)
        .with_quick_http(&quick);
    assert_eq!(
        read_location(&explicit, "https://sec/p.json").unwrap(),
        POLICY
    );
    assert!(quick.requests().is_empty());
}

#[test]
fn a_policy_fetch_refuses_a_redirect_away_from_https() {
    let (fs, env) = (FakeFileSystem::default(), FakeEnv::default());
    let http = FakeHttp::default()
        .with_redirect("https://sec/p.json", "http://sec/p.json")
        .with_body("http://sec/p.json", POLICY);
    let with_http = context(&fs, &env).with_http(&http);
    assert!(matches!(
        read_location(&with_http, "https://sec/p.json"),
        Err(PolicyError::Unreadable { .. })
    ));
}
