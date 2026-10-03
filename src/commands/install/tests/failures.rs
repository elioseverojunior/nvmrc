use super::*;

#[test]
fn a_version_that_does_not_exist_is_status_3_with_the_hint() {
    let output = World::new().run("99").unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stderr,
        "Version '99' not found - try `nvm ls-remote` to browse available versions."
    );
}

#[test]
fn no_version_at_all_is_the_usage_with_status_127() {
    let error = World::new().run("").unwrap_err();
    assert_eq!(error.exit_code(), NvmExitCode::NotFound);
    assert!(error.to_string().starts_with(
        "No version provided and no .nvmrc file found\nUsage: nvm install [<version>]"
    ));
}

#[test]
fn a_version_below_the_floor_is_status_7_with_both_lines() {
    let mut world = World::new();
    world.env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("NVM_MIN_VERSION", "v22");
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::BelowVersionFloor);
    assert_eq!(
        lines(&output.stderr),
        [
            "Version v20.10.0 is below the minimum allowed version v22.0.0.",
            "Lower or unset NVM_MIN_VERSION (or edit $NVM_DIR/min-version) to install it.",
        ]
    );
    assert!(!world.installed());
}

#[test]
fn the_floor_can_come_from_the_min_version_file() {
    let world = World::new();
    world
        .fs
        .write_file(Path::new("/n/min-version"), "v22\n")
        .unwrap();
    assert_eq!(
        world.run("20").unwrap().status,
        NvmExitCode::BelowVersionFloor
    );
}

#[test]
fn a_failed_download_is_status_2_and_installs_nothing() {
    let mut world = World::new();
    world.http = FakeHttp::default()
        .with_body(NODE_INDEX, &index_text(&[("v20.10.0", "Iron")]))
        .with_body(IOJS_INDEX, &index_text(&[]))
        .with_status(TARBALL_URL, 404);
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    assert!(
        output
            .stderr
            .ends_with("Binary download failed. Download from source aborted.")
    );
    assert!(!world.installed());
}

#[test]
fn a_wrong_checksum_is_status_2_and_installs_nothing() {
    let mut world = World::new();
    world.digest = FakeDigest::default().with_digest(TARBALL, "bad0");
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    assert!(
        output
            .stderr
            .contains("Checksums do not match: 'bad0' found, 'aa11' expected.")
    );
    assert!(!world.installed());
}

#[test]
fn a_machine_without_binaries_is_status_3() {
    let world = World::new();
    let archive = FakeArchive::new(&world.fs);
    let context = Context::new(&world.fs, &world.env)
        .with_http(&world.http)
        .with_archive(&archive)
        .with_platform(None);
    let output = super::run(&context, &["20".to_owned()]).unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stderr,
        "Binary download is not available for v20.10.0"
    );
}

#[test]
fn a_held_lock_that_never_clears_stops_the_install_with_status_1() {
    let mut world = World::new();
    world.fs = FakeFileSystem::default().with_dir("/n/.cache/locks/v20.10.0");
    world.env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("NVM_INSTALL_LOCK_TIMEOUT", "1");
    let error = world.run("20").unwrap_err();
    assert_eq!(error.exit_code(), NvmExitCode::Failure);
    assert!(
        error
            .to_string()
            .starts_with("Timed out after 1s waiting for another install of v20.10.0")
    );
}
