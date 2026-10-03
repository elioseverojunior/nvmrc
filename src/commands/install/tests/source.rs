use std::rc::Rc;

use super::built::*;
use super::*;

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
        .with_body(SUMS, &format!("{GOOD}  node-v20.10.0.tar.xz\n"))
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
