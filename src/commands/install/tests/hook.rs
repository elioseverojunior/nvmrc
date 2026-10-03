use super::built::*;
use super::*;

#[test]
fn a_hook_installs_in_place_of_nvm_and_is_told_how() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let hook_args = "v20.10.0 node std binary /n/versions/node/v20.10.0";
    let process = FakeProcess::default()
        .with_success("/opt/hook", hook_args, "")
        .with_effect("/opt/hook", hook_args, world.node_maker());
    let context = Context::new(world.fs.as_ref(), &world.env)
        .with_http(&world.http)
        .with_process(&process);
    let output = super::super::run(&context, &["20".to_owned()]).unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        output.stderr,
        "** $NVM_INSTALL_THIRD_PARTY_HOOK env var set; dispatching to third-party installation method **"
    );
    assert!(
        !world
            .http
            .requests()
            .iter()
            .any(|url| url.contains(".tar.xz"))
    );
}

#[test]
fn a_hook_that_fails_or_installs_nothing_ends_the_install() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let run_with = |process: &FakeProcess| {
        let context = Context::new(world.fs.as_ref(), &world.env)
            .with_http(&world.http)
            .with_process(process);
        super::super::run(&context, &["20".to_owned()]).unwrap()
    };
    let missing = run_with(&FakeProcess::default());
    assert_eq!(missing.status, NvmExitCode::Failure);
    assert!(
        missing.stderr.ends_with(
            "*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var failed to install! ***"
        )
    );
    let silent = FakeProcess::default().with_success(
        "/opt/hook",
        "v20.10.0 node std binary /n/versions/node/v20.10.0",
        "",
    );
    let claimed = run_with(&silent);
    assert_eq!(claimed.status, NvmExitCode::HookClaimedSuccess);
    assert!(claimed.stderr.ends_with("*** Third-party $NVM_INSTALL_THIRD_PARTY_HOOK env var claimed to succeed, but failed to install! ***"));
}

#[test]
fn the_hook_is_told_source_when_s_is_given() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let process = FakeProcess::default();
    let context = Context::new(world.fs.as_ref(), &world.env)
        .with_http(&world.http)
        .with_process(&process);
    super::super::run(&context, &["-s".to_owned(), "20".to_owned()]).unwrap();
    let ran = process.executed();
    assert_eq!(
        ran[0].args,
        [
            "v20.10.0",
            "node",
            "std",
            "source",
            "/n/versions/node/v20.10.0"
        ]
    );
}

#[test]
fn a_hook_that_fails_passes_its_own_status_on() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let failed = crate::ports::Completed {
        code: Some(7),
        ..crate::ports::Completed::default()
    };
    let process = FakeProcess::default().with_execution(
        "/opt/hook",
        "v20.10.0 node std binary /n/versions/node/v20.10.0",
        failed,
    );
    let context = Context::new(world.fs.as_ref(), &world.env)
        .with_http(&world.http)
        .with_process(&process);
    let output = super::super::run(&context, &["20".to_owned()]).unwrap();
    assert_eq!(output.status, NvmExitCode::Passed(7));
    assert_eq!(output.status.code(), 7);
}
