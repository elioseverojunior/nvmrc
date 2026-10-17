use super::*;

fn installed() -> FakeFileSystem {
    FakeFileSystem::default().with_executable("/n/versions/node/v20.1.0/bin/node", "")
}

/// Runs the CLI with v20.1.0 installed and runnable.
fn run_installed(args: &[&str]) -> (u8, String, String) {
    run_in(
        &installed(),
        &FakeEnv::default().with_var("NVM_DIR", "/n"),
        args,
    )
}

/// Runs the CLI in `fs` and `env`, with an empty vulnerability index served.
fn run_in(fs: &FakeFileSystem, env: &FakeEnv, args: &[&str]) -> (u8, String, String) {
    let http = FakeHttp::default().with_empty_advisories();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let context = Context::new(fs, env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let code = run(args, &context, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn a_flag_error_is_a_usage_error() {
    let (code, _, err) = run_cli(&["nvm", "--policy"]);
    assert_eq!(code, 127);
    assert_eq!(err.trim_end(), "nvm: --policy needs a value.");
}

#[test]
fn stripped_flags_do_not_reach_clap() {
    let (code, out, _) = run_cli(&["nvm", "--min-node", "18", "version", "20"]);
    assert_eq!((code, out.trim_end()), (0, "v20.1.0"));
}

#[test]
fn stripped_package_manager_flags_do_not_reach_clap() {
    let (code, out, _) = run_cli(&["nvm", "--min-npm", "10", "version", "20"]);
    assert_eq!((code, out.trim_end()), (0, "v20.1.0"));
}

#[test]
fn a_policy_flag_after_another_option_of_use_is_enforced() {
    let (code, _, err) = run_installed(&["nvm", "use", "--silent", "--policy", "/nonexist", "20"]);
    assert_eq!(code, 9, "{err}");
}

#[test]
fn a_policy_flag_after_the_version_of_use_is_enforced() {
    let (code, _, err) = run_installed(&["nvm", "use", "20", "--policy=/nonexist"]);
    assert_eq!(code, 9, "{err}");
}

#[test]
fn a_floor_flag_after_save_is_enforced() {
    let (code, _, err) = run_installed(&["nvm", "use", "--save", "--min-node=22", "20"]);
    assert_eq!(code, 7, "{err}");
}

#[test]
fn a_floor_flag_after_the_version_of_install_is_enforced() {
    // Offline, the CVE check needs a local index: a repository policy names one.
    let policy = r#"{ "schemaVersion": 1, "cve": { "source": "vuln.json" } }"#;
    let fs = installed()
        .with_file("/w/.nvmrc-policy.json", policy)
        .with_file("/w/vuln.json", "{}");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PWD", "/w")
        .with_current_dir("/w");
    let args = ["nvm", "install", "--offline", "20", "--min-node=22"];
    let (code, _, err) = run_in(&fs.with_dir("/w"), &env, &args);
    assert_eq!(code, 7, "{err}");
}

#[test]
fn a_policy_flag_after_the_program_of_exec_reaches_the_program() {
    let fs = FakeFileSystem::default().with_executable("/n/versions/node/v20.1.0/bin/node", "");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PATH", "/usr/bin");
    let process = crate::fakes::FakeProcess::default().with_spawn("node", "--policy x.js", 0);
    let context = Context::new(&fs, &env).with_process(&process);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args = ["nvm", "exec", "--silent", "20", "node", "--policy", "x.js"];
    let code = run(args, &context, &mut out, &mut err);
    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert_eq!(process.spawned().len(), 1);
}

#[test]
fn policy_flags_reach_init() {
    let packument = r#"{"versions": {"9.1.0": {}, "9.11.0": {}}}"#;
    let http = FakeHttp::default().with_body("https://registry.npmjs.org/pnpm", packument);
    let fs = FakeFileSystem::default().with_dir("/w");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PWD", "/w")
        .with_current_dir("/w");
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args = ["nvm", "--min-pnpm", "9.12.0", "init", "--pnpm"];
    let code = run(args, &context, &mut out, &mut err);
    let err = String::from_utf8(err).unwrap();
    assert_eq!(
        (code, err.trim_end()),
        (7, "nvm init: no allowed pnpm release.")
    );
    assert!(!fs.is_file(std::path::Path::new("/w/.nvmrc")));
}
