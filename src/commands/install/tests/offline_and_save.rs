use super::*;

fn offline_world(fs: FakeFileSystem) -> World {
    World { fs, ..World::new() }
}

/// The expectations are what the real `nvm install --offline` printed.
#[test]
fn offline_nothing_installed_or_cached_is_status_3_and_asks_for_nothing_online() {
    let world = World::new();
    let output = world.run("--offline 20").unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stderr,
        "Version '20' not found locally or in cache - try `nvm ls` to browse available versions."
    );
    assert!(world.http.requests().is_empty());
}

#[test]
fn offline_installs_from_the_cached_archive_without_a_checksum() {
    let world = offline_world(FakeFileSystem::default().with_file(TARBALL, "cached"));
    let output = world.run("--offline 20").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        lines(&output.stdout),
        [
            "Downloading and installing node v20.10.0...",
            "Creating default alias: default -> 20 (-> v20.10.0 *)",
        ]
    );
    assert!(
        output
            .stderr
            .starts_with("Offline: using cached archive ${NVM_DIR}/.cache/bin")
    );
    assert!(world.installed());
    assert!(world.http.requests().is_empty());
}

#[test]
fn offline_an_installed_version_is_just_reported() {
    let world = World::new();
    world.run("20").unwrap();
    let requests = world.http.requests().len();
    let output = world.run("--offline 20").unwrap();
    assert_eq!(output.stderr, "v20.10.0 is already installed.");
    assert_eq!(world.http.requests().len(), requests);
}

#[test]
fn offline_lts_needs_the_local_alias_and_then_the_installed_or_cached_version() {
    let missing = World::new().run("--offline --lts").unwrap();
    assert_eq!(missing.status, NvmExitCode::InvalidVersion);
    assert_eq!(missing.stdout, "Installing latest LTS version.");
    assert_eq!(
        missing.stderr,
        "LTS alias '*' not found locally. Run `nvm ls-remote --lts` first to populate LTS aliases."
    );
    let fs = FakeFileSystem::default()
        .with_file("/n/alias/lts/*", "lts/iron\n")
        .with_file("/n/alias/lts/iron", "v20.10.0\n")
        .with_file(TARBALL, "cached");
    let world = offline_world(fs);
    let output = world.run("--offline lts/iron").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(world.installed());
}

#[test]
fn offline_with_a_missing_archive_fails_like_a_failed_download() {
    let world = offline_world(FakeFileSystem::default().with_dir("/n/.cache/src/node-v20.10.0"));
    let output = world.run("--offline 20").unwrap();
    assert_eq!(output.status, NvmExitCode::MissingTarget);
    assert!(
        output
            .stderr
            .starts_with("Offline: no cached archive found for node-v20.10.0-linux-x64")
    );
}

fn with_pwd(world: World) -> World {
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PWD", "/work");
    World { env, ..world }
}

#[test]
fn save_writes_the_resolved_version_of_an_installed_version_to_nvmrc() {
    let world = with_pwd(World::new());
    world.run("20").unwrap();
    let output = world.run("--save 20").unwrap();
    assert_eq!(output.stdout, "Wrote version number (v20.10.0) to .nvmrc");
    assert_eq!(world.text("/work/.nvmrc").as_deref(), Some("v20.10.0\n"));
}

#[test]
fn save_also_writes_after_a_fresh_install_unlike_nvm_sh() {
    let world = with_pwd(World::new());
    let output = world.run("-w 20").unwrap();
    assert!(
        output
            .stdout
            .ends_with("Wrote version number (v20.10.0) to .nvmrc")
    );
    assert_eq!(world.text("/work/.nvmrc").as_deref(), Some("v20.10.0\n"));
}

#[test]
fn save_after_the_version_does_nothing_like_nvm_sh() {
    let world = with_pwd(World::new());
    world.run("20 --save").unwrap();
    assert_eq!(world.text("/work/.nvmrc"), None);
}

#[test]
fn a_save_that_cannot_write_is_status_3_with_a_warning_and_skips_the_alias() {
    let world = World::new();
    world.run("20").unwrap();
    let output = world.run("--save --alias=work 20").unwrap();
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(
        output.stderr,
        "v20.10.0 is already installed.\nWarning: Unable to write version number (v20.10.0) to .nvmrc"
    );
    assert_eq!(world.text("/n/alias/work"), None);
}
