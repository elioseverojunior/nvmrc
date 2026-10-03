use std::rc::Rc;

use super::*;
use crate::fakes::FakeCpu;

const SRC_URL: &str = "https://nodejs.org/dist/v20.10.0/node-v20.10.0.tar.gz";
const SRC_TARBALL: &str = "/n/.cache/src/node-v20.10.0/node-v20.10.0.tar.gz";
const TOP: &str = "/n/.cache/src/node-v20.10.0/files";
const PREFIX: &str = "--prefix=/n/versions/node/v20.10.0";

/// A world where the binary cannot be had and the source can.
struct Built {
    fs: Rc<FakeFileSystem>,
    http: FakeHttp,
    digest: FakeDigest,
    env: FakeEnv,
    cpu: FakeCpu,
}

impl Built {
    fn new() -> Self {
        let node = index_text(&[("v20.10.0", "Iron")]);
        let sums =
            format!("{GOOD}  node-v20.10.0-linux-x64.tar.gz\n{GOOD}  node-v20.10.0.tar.gz\n");
        Self {
            fs: Rc::new(FakeFileSystem::default()),
            http: FakeHttp::default()
                .with_body(NODE_INDEX, &node)
                .with_body(IOJS_INDEX, &index_text(&[]))
                .with_body(SUMS, &sums)
                .with_status(TARBALL_URL, 404)
                .with_bytes(SRC_URL, b"source"),
            digest: FakeDigest::default().with_digest(SRC_TARBALL, GOOD),
            env: FakeEnv::default().with_var("NVM_DIR", "/n"),
            cpu: FakeCpu::with_cores(8),
        }
    }

    fn with_env(self, name: &str, value: &str) -> Self {
        let mut vars = vec![("NVM_DIR", "/n")];
        vars.push((name, value));
        let env = vars
            .iter()
            .fold(FakeEnv::default(), |env, (k, v)| env.with_var(k, v));
        Self { env, ..self }
    }

    fn run(&self, line: &str) -> Output {
        let fs = Rc::clone(&self.fs);
        let make_node = move || {
            fs.write_file(Path::new(NODE), "binary").unwrap();
            fs.set_executable(Path::new(NODE));
        };
        let process = FakeProcess::default()
            .with_success(&format!("{TOP}/configure"), PREFIX, "configured\n")
            .with_success("make", "-j 7", "built\n")
            .with_success("make", "-j 7 install", "installed\n")
            .with_effect("make", "-j 7 install", make_node)
            .with_success("make", "-j 4", "")
            .with_success("make", "-j 4 install", "")
            .with_effect("make", "-j 4 install", {
                let fs = Rc::clone(&self.fs);
                move || {
                    fs.write_file(Path::new(NODE), "binary").unwrap();
                    fs.set_executable(Path::new(NODE));
                }
            })
            .with_success(
                &format!("{TOP}/configure"),
                &format!("{PREFIX} --with-intl=full-icu"),
                "",
            );
        let archive = FakeArchive::new(&self.fs).with_archive(
            SRC_TARBALL,
            &[("node-v20.10.0/configure", "#!/bin/sh", true)],
        );
        let context = Context::new(self.fs.as_ref(), &self.env)
            .with_http(&self.http)
            .with_digest(&self.digest)
            .with_archive(&archive)
            .with_process(&process)
            .with_cpu(&self.cpu);
        let words: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
        super::super::run(&context, &words).unwrap()
    }
}

/// The expectations are what `nvm install` of the real `nvm.sh` printed with
/// a mirror that has no binary, and a fake `./configure` and `make`.
#[test]
fn a_failed_binary_falls_back_to_building_from_source() {
    let world = Built::new();
    let output = world.run("20");
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        lines(&output.stdout),
        [
            "Downloading and installing node v20.10.0...",
            "Detected that you have 8 CPU core(s)",
            "Running with 7 threads to speed up the build",
            "$>./configure --prefix=/n/versions/node/v20.10.0 <",
            "configured",
            "built",
            "installed",
            "Creating default alias: default -> 20 (-> v20.10.0 *)",
        ]
    );
    assert!(
        output
            .stderr
            .contains("Binary download failed, trying source.")
    );
    assert!(output.stderr.contains(&format!("Downloading {SRC_URL}...")));
    assert!(world.fs.file_info(Path::new(NODE)).is_ok());
}

#[test]
fn s_goes_straight_to_source_and_skips_the_binary() {
    let world = Built::new();
    let output = world.run("-s 20");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(!output.stdout.contains("Downloading and installing"));
    assert!(!world.http.requests().iter().any(|url| url == TARBALL_URL));
}

#[test]
fn b_ends_a_failed_binary_instead_of_building() {
    let world = Built::new();
    let output = world.run("-b 20");
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    assert!(
        output
            .stderr
            .ends_with("Binary download failed. Download from source aborted.")
    );
    assert!(!world.http.requests().iter().any(|url| url == SRC_URL));
}

#[test]
fn nvm_no_source_fallback_is_b_for_every_install() {
    let world = Built::new().with_env("NVM_NO_SOURCE_FALLBACK", "1");
    let output = world.run("20");
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    let value_that_is_not_one = Built::new()
        .with_env("NVM_NO_SOURCE_FALLBACK", "yes")
        .run("20");
    assert_eq!(value_that_is_not_one.status, NvmExitCode::Success);
}

#[test]
fn s_with_nvm_no_source_fallback_is_status_6() {
    let world = Built::new().with_env("NVM_NO_SOURCE_FALLBACK", "1");
    let fs = Rc::clone(&world.fs);
    let context = Context::new(&*fs, &world.env);
    let error = super::super::run(&context, &["-s".to_owned(), "20".to_owned()]).unwrap_err();
    assert_eq!(error.exit_code(), NvmExitCode::InvalidOptions);
    assert_eq!(
        error.to_string(),
        "-s cannot be combined with NVM_NO_SOURCE_FALLBACK=1 since that would skip install from both binary and source"
    );
}

#[test]
fn b_on_a_machine_without_binaries_is_status_3_with_the_binary_message() {
    let mut world = Built::new();
    world.http = FakeHttp::default()
        .with_body(NODE_INDEX, &index_text(&[("v20.10.0", "Iron")]))
        .with_body(IOJS_INDEX, &index_text(&[]))
        .with_body(SUMS, &format!("{GOOD}  node-v20.10.0.tar.gz\n"))
        .with_bytes(SRC_URL, b"source");
    let no_binary = Rc::clone(&world.fs);
    let context = Context::new(&*no_binary, &world.env)
        .with_platform(None)
        .with_http(&world.http);
    let words = ["-b".to_owned(), "20".to_owned()];
    let output = super::super::run(&context, &words).unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stderr,
        "Binary download is not available for v20.10.0"
    );
}

#[test]
fn j_sets_the_jobs_and_says_so_first() {
    let world = Built::new();
    let output = world.run("-j 4 -s 20");
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        output.stdout.lines().next(),
        Some("number of `make` jobs: 4")
    );
    assert!(!output.stdout.contains("Detected that you have"));
}

#[test]
fn j_that_is_not_natural_is_reported_and_the_cores_decide() {
    let world = Built::new();
    let output = world.run("-j abc -s 20");
    assert!(
        output
            .stderr
            .starts_with("abc is invalid for number of `make` jobs, must be a natural number")
    );
    assert!(
        output
            .stdout
            .contains("Running with 7 threads to speed up the build")
    );
}

#[test]
fn nvm_make_jobs_is_used_silently_when_it_is_a_natural_number() {
    let world = Built::new().with_env("NVM_MAKE_JOBS", "4");
    let output = world.run("-s 20");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(!output.stdout.contains("Detected that you have"));
    let ignored = Built::new().with_env("NVM_MAKE_JOBS", "many").run("-s 20");
    assert!(
        ignored
            .stdout
            .contains("Detected that you have 8 CPU core(s)")
    );
}

#[test]
fn the_words_after_the_version_are_passed_to_configure() {
    let world = Built::new();
    let output = world.run("-s 20 --with-intl=full-icu");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(
        output
            .stdout
            .contains("Additional options while compiling:  --with-intl=full-icu")
    );
}

#[test]
fn a_failed_build_is_status_1_and_installs_nothing() {
    let world = Built::new();
    let broken = World::new();
    let _ = broken;
    let process = FakeProcess::default();
    let archive = FakeArchive::new(&world.fs).with_archive(
        SRC_TARBALL,
        &[("node-v20.10.0/configure", "#!/bin/sh", true)],
    );
    let context = Context::new(world.fs.as_ref(), &world.env)
        .with_http(&world.http)
        .with_digest(&world.digest)
        .with_archive(&archive)
        .with_process(&process)
        .with_cpu(&world.cpu);
    let output = super::super::run(&context, &["-s".to_owned(), "20".to_owned()]).unwrap();
    assert_eq!(output.status, NvmExitCode::Failure);
    assert!(output.stderr.ends_with("nvm: install v20.10.0 failed!"));
    assert!(world.fs.file_info(Path::new(NODE)).is_err());
}

#[test]
fn a_hook_installs_in_place_of_nvm_and_is_told_how() {
    let world = Built::new().with_env("NVM_INSTALL_THIRD_PARTY_HOOK", "/opt/hook");
    let fs = Rc::clone(&world.fs);
    let process = FakeProcess::default()
        .with_success(
            "/opt/hook",
            "v20.10.0 node std binary /n/versions/node/v20.10.0",
            "",
        )
        .with_effect(
            "/opt/hook",
            "v20.10.0 node std binary /n/versions/node/v20.10.0",
            move || {
                fs.write_file(Path::new(NODE), "binary").unwrap();
                fs.set_executable(Path::new(NODE));
            },
        );
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
            .any(|url| url.contains(".tar.gz"))
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
