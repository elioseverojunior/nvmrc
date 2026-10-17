//! `nvm upgrade` installs what it wrote, as `nvm install` would.

use std::path::Path;

use super::run;
use crate::commands::Output;
use crate::commands::project::test_support::{file, project, served};
use crate::context::Context;
use crate::error::NvmExitCode;
use crate::fakes::{
    FakeArchive, FakeDigest, FakeEnv, FakeFileSystem, FakeHttp, FakeProcess, FakeSleeper,
};
use crate::ports::FileSystem;

const SUMS_URL: &str = "https://nodejs.org/dist/v20.18.0/SHASUMS256.txt";
const NODE_URL: &str = "https://nodejs.org/dist/v20.18.0/node-v20.18.0-linux-x64.tar.xz";
const NODE_TARBALL: &str = "/n/.cache/bin/node-v20.18.0-linux-x64/node-v20.18.0-linux-x64.tar.xz";
const NODE_BIN: &str = "/n/versions/node/v20.18.0/bin/node";
const PNPM_DOWNLOAD: &str = "/n/package-managers/pnpm/9.12.0.partial/download.tgz";
const PNPM_BIN: &str = "/n/package-managers/pnpm/9.12.0/bin/pnpm";
const MANIFEST_URL: &str = "https://registry.npmjs.org/pnpm/9.12.0";
const PACKAGE_URL: &str = "https://t.test/pnpm.tgz";
const NPM_PACKUMENT_URL: &str = "https://registry.npmjs.org/npm";
const NPM_MANIFEST_URL: &str = "https://registry.npmjs.org/npm/10.9.0";
const NPM_PACKAGE_URL: &str = "https://t.test/npm.tgz";
const NPM_DOWNLOAD: &str = "/n/package-managers/npm/10.9.0.partial/download.tgz";
const NPM_BIN: &str = "/n/package-managers/npm/10.9.0/bin/npm";
/// `sha512` of `tarball-bytes`.
const INTEGRITY: &str = "sha512-31GJ6SyG+dFBGDyQB1x8EHrdSW/FYRyb4mHdZIaXTme9WooJgWCCmKoHe70PgTrvis7oBfnw5WLWBp8KVj59oA==";
const PINNED: &str = "v20.11.1\npnpm=9.1.0\n";
const PNPM_9: &str = "{ \"packageManager\": \"pnpm@9.1.0\" }";

/// The registry manifest of a package whose `name` bin is at `bin`.
fn manifest(name: &str, bin: &str, tarball: &str) -> String {
    format!(
        r#"{{"bin": {{"{name}": "{bin}"}}, "dist": {{"tarball": "{tarball}", "integrity": "{INTEGRITY}"}}}}"#
    )
}

/// The mirror and registry on offer; the node tarball only when `with_node`.
fn mirror(with_node: bool) -> FakeHttp {
    let npm_packument = r#"{"versions": {"9.0.0": {}, "10.4.0": {}, "10.9.0": {}}}"#;
    let http = served()
        .with_body("https://iojs.org/dist/index.tab", "")
        .with_body(MANIFEST_URL, &manifest("pnpm", "bin/pnpm.cjs", PACKAGE_URL))
        .with_bytes(PACKAGE_URL, b"tarball-bytes")
        .with_body(NPM_PACKUMENT_URL, npm_packument)
        .with_body(
            NPM_MANIFEST_URL,
            &manifest("npm", "bin/npm-cli.js", NPM_PACKAGE_URL),
        )
        .with_bytes(NPM_PACKAGE_URL, b"tarball-bytes")
        .with_body(SUMS_URL, "aa11  node-v20.18.0-linux-x64.tar.xz\n");
    if with_node {
        return http.with_bytes(NODE_URL, b"tarball");
    }
    http
}

/// Runs `nvm upgrade args` with everything an install needs.
fn upgrade(fs: &FakeFileSystem, env: &FakeEnv, http: &FakeHttp, args: &[&str]) -> Output {
    let digest = FakeDigest::default().with_digest(NODE_TARBALL, "aa11");
    let node_files = [
        ("node-v20.18.0-linux-x64/bin/node", "binary", true),
        ("node-v20.18.0-linux-x64/bin/npm", "npm", true),
    ];
    let pnpm_files = [
        ("package/package.json", "{}", false),
        ("package/bin/pnpm.cjs", "pnpm", false),
    ];
    let npm_files = [
        ("package/package.json", "{}", false),
        ("package/bin/npm-cli.js", "npm", false),
    ];
    let archive = FakeArchive::new(fs)
        .with_archive(NODE_TARBALL, &node_files)
        .with_archive(PNPM_DOWNLOAD, &pnpm_files)
        .with_archive(NPM_DOWNLOAD, &npm_files);
    let (sleeper, process) = (FakeSleeper::default(), FakeProcess::default());
    let context = Context::new(fs, env)
        .with_http(http)
        .with_third_party_http(http, http)
        .with_digest(&digest)
        .with_archive(&archive)
        .with_sleeper(&sleeper)
        .with_process(&process);
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    run(&context, &args).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn an_upgrade_installs_the_new_versions() {
    let files = [("/p/.nvmrc", PINNED), ("/p/package.json", PNPM_9)];
    let (fs, env) = project(&files, "");
    let output = upgrade(&fs, &env, &mirror(true), &[]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert!(
        output
            .stdout
            .starts_with("node v20.11.1 -> v20.18.0\npnpm 9.1.0 -> 9.12.0\n"),
        "{}",
        output.stdout
    );
    assert!(fs.is_file(Path::new(NODE_BIN)));
    assert!(fs.is_file(Path::new(PNPM_BIN)));
}

#[test]
fn an_nvmrc_without_a_node_line_installs_only_the_pins() {
    let files = [("/p/.nvmrc", "pnpm=9.1.0\n"), ("/p/package.json", PNPM_9)];
    let (fs, env) = project(&files, "");
    let output = upgrade(&fs, &env, &mirror(false), &[]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert_eq!(
        output.stdout,
        "pnpm 9.1.0 -> 9.12.0\nInstalling pnpm 9.12.0..."
    );
    assert!(fs.is_file(Path::new(PNPM_BIN)));
    assert!(!fs.is_file(Path::new(NODE_BIN)));
}

#[test]
fn a_failed_install_keeps_the_files_and_says_so() {
    let files = [("/p/.nvmrc", PINNED), ("/p/package.json", PNPM_9)];
    let (fs, env) = project(&files, "");
    let output = upgrade(&fs, &env, &mirror(false), &[]);
    assert_ne!(output.status, NvmExitCode::Success);
    assert!(
        output.stderr.ends_with(
            "nvm upgrade: /p/.nvmrc is updated, but the install failed; run nvm install to finish."
        ),
        "{}",
        output.stderr
    );
    assert_eq!(
        file(&fs, "/p/.nvmrc").as_deref(),
        Some("v20.18.0\npnpm=9.12.0\n")
    );
}

#[test]
fn an_install_that_errors_keeps_the_upgrade_report() {
    // Upgrading pnpm alone never needs $NVM_DIR; the install after it does.
    let (fs, _) = project(&[("/p/.nvmrc", "v20.18.0\npnpm=9.1.0\n")], "");
    let env = FakeEnv::default()
        .with_var("PWD", "/p")
        .with_current_dir("/p");
    let http = mirror(true);
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_third_party_http(&http, &http);
    let output = run(&context, &["--pnpm".to_owned()]).unwrap();
    assert_eq!(output.status, NvmExitCode::Failure, "{output:?}");
    assert_eq!(output.stdout, "pnpm 9.1.0 -> 9.12.0");
    let notice =
        "nvm upgrade: /p/.nvmrc is updated, but the install failed; run nvm install to finish.";
    assert!(output.stderr.ends_with(notice), "{}", output.stderr);
    let error = "Neither NVM_DIR nor HOME is set; cannot locate the nvm directory.";
    assert!(output.stderr.starts_with(error), "{}", output.stderr);
    assert_eq!(
        file(&fs, "/p/.nvmrc").as_deref(),
        Some("v20.18.0\npnpm=9.12.0\n")
    );
}

#[test]
fn nothing_to_upgrade_installs_nothing() {
    let (fs, env) = project(&[("/p/.nvmrc", "v20.18.0\npnpm=9.12.0\n")], "");
    let http = mirror(true);
    let output = upgrade(&fs, &env, &http, &[]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    let requests = http.requests();
    let downloaded = |url: &&String| url.contains("SHASUMS") || url.contains("t.test");
    assert!(
        requests.iter().filter(downloaded).count() == 0,
        "{requests:?}"
    );
    assert!(!fs.is_file(Path::new(NODE_BIN)));
}

#[test]
fn a_pins_only_upgrade_installs_only_the_pins_it_moved() {
    let policy = r#"{ "schemaVersion": 1, "tools": { "npm": { "minimum": "10.0.0" } } }"#;
    let nvmrc = "npm=9.0.0\npnpm=9.1.0\n";
    let (fs, env) = project(&[("/p/.nvmrc", nvmrc), ("/p/package.json", PNPM_9)], policy);
    let http = mirror(false);
    let output = upgrade(&fs, &env, &http, &["--pnpm"]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert_eq!(
        output.stdout,
        "pnpm 9.1.0 -> 9.12.0\nInstalling pnpm 9.12.0..."
    );
    assert!(fs.is_file(Path::new(PNPM_BIN)));
    let requests = http.requests();
    let npm = |url: &&String| url.starts_with(NPM_PACKUMENT_URL);
    assert!(requests.iter().filter(npm).count() == 0, "{requests:?}");
}

#[test]
fn a_pins_only_upgrade_installs_every_pin_it_moved() {
    let nvmrc = "npm=10.4.0\npnpm=9.1.0\n";
    let (fs, env) = project(&[("/p/.nvmrc", nvmrc), ("/p/package.json", PNPM_9)], "");
    let output = upgrade(&fs, &env, &mirror(false), &[]);
    assert_eq!(output.status, NvmExitCode::Success, "{}", output.stderr);
    assert_eq!(
        output.stdout,
        "npm 10.4.0 -> 10.9.0\npnpm 9.1.0 -> 9.12.0\n\
         Installing npm 10.9.0...\nInstalling pnpm 9.12.0..."
    );
    assert!(fs.is_file(Path::new(NPM_BIN)));
    assert!(fs.is_file(Path::new(PNPM_BIN)));
}
