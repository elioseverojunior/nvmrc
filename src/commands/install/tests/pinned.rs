//! `nvm install` installs the `npm=`, `yarn=` and `pnpm=` pins of `.nvmrc`
//! into the shared store, after the node and before it is activated.

use super::*;

pub(super) const PNPM_DOWNLOAD: &str = "/n/package-managers/pnpm/9.12.0.partial/download.tgz";
const PNPM_BIN: &str = "/n/package-managers/pnpm/9.12.0/bin/pnpm";
const MANIFEST_URL: &str = "https://registry.npmjs.org/pnpm/9.12.0";
const PACKAGE_URL: &str = "https://t.test/pnpm.tgz";
const TARBALL_BYTES: &[u8] = b"tarball-bytes";
/// `sha512` of [`TARBALL_BYTES`].
const INTEGRITY: &str = "sha512-31GJ6SyG+dFBGDyQB1x8EHrdSW/FYRyb4mHdZIaXTme9WooJgWCCmKoHe70PgTrvis7oBfnw5WLWBp8KVj59oA==";

fn manifest() -> String {
    format!(
        r#"{{"bin": {{"pnpm": "bin/pnpm.cjs"}}, "dist": {{"tarball": "{PACKAGE_URL}", "integrity": "{INTEGRITY}"}}}}"#
    )
}

/// A world whose `/p/.nvmrc` holds `nvmrc`, with the pnpm package on offer
/// as `bytes`.
fn world_offering(nvmrc: &str, bytes: &[u8]) -> World {
    let base = World::new();
    World {
        fs: FakeFileSystem::default().with_file("/p/.nvmrc", nvmrc),
        http: base
            .http
            .with_body(MANIFEST_URL, &manifest())
            .with_bytes(PACKAGE_URL, bytes),
        ..base
    }
    .with_env(&[("PWD", "/p")])
}

fn pinned_world(nvmrc: &str) -> World {
    world_offering(nvmrc, TARBALL_BYTES)
}

fn pnpm_requests(world: &World) -> usize {
    let requests = world.http.requests();
    requests
        .iter()
        .filter(|url| url.contains("pnpm") || url.contains("t.test"))
        .count()
}

#[test]
fn install_installs_the_pinned_tools_after_node() {
    let world = pinned_world("20\npnpm=9.12.0\n");
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        lines(&output.stdout),
        [
            "Downloading and installing node v20.10.0...",
            "Installing pnpm 9.12.0...",
            "Creating default alias: default -> 20 (-> v20.10.0 *)",
        ]
    );
    assert!(world.installed());
    assert!(world.fs.is_file(Path::new(PNPM_BIN)));
}

#[test]
fn an_installed_tool_is_not_downloaded_again() {
    let world = pinned_world("20\npnpm=9.12.0\n");
    world.run("20").unwrap();
    let before = pnpm_requests(&world);
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(
        lines(&output.stderr),
        [
            "v20.10.0 is already installed.",
            "pnpm 9.12.0 is already installed."
        ]
    );
    assert_eq!(pnpm_requests(&world), before);
}

#[test]
fn offline_with_a_missing_tool_fails() {
    let world = pinned_world("20\npnpm=9.12.0\n");
    world.run("20").unwrap();
    let store = Path::new("/n/package-managers");
    world.fs.remove_dir_all(store).unwrap();
    let before = pnpm_requests(&world);
    let output = world.run("--offline 20").unwrap();
    assert_eq!(output.status, NvmExitCode::Failure);
    assert!(
        output
            .stderr
            .ends_with("pnpm 9.12.0 is not installed, and --offline does not download."),
        "{}",
        output.stderr
    );
    assert_eq!(pnpm_requests(&world), before);
}

#[test]
fn pins_come_from_nvmrc_even_with_a_version_argument() {
    let world = pinned_world("18\npnpm=9.12.0\n");
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(output.stdout.contains("Installing pnpm 9.12.0..."));
    assert!(world.fs.is_file(Path::new(PNPM_BIN)));
}

#[test]
fn a_pin_error_stops_before_any_download() {
    let world = pinned_world("20\npnpm=9.12\n");
    let error = world.run("20").unwrap_err();
    assert_eq!(error.exit_code(), NvmExitCode::InvalidVersion);
    assert!(world.http.requests().is_empty());
    assert!(!world.installed());
}

#[test]
fn a_failed_tool_install_exits_1_with_the_reason() {
    let world = world_offering("20\npnpm=9.12.0\n", b"tampered");
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::Failure);
    assert!(
        output.stderr.contains("nvm: cannot install pnpm 9.12.0: "),
        "{}",
        output.stderr
    );
}

#[test]
fn an_automatic_path_only_warns_when_a_tool_fails() {
    let world = World {
        advisory: true,
        ..world_offering("20\npnpm=9.12.0\n", b"tampered")
    };
    let output = world.run("20").unwrap();
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(
        output
            .stderr
            .contains("nvm: warning: cannot install pnpm 9.12.0: "),
        "{}",
        output.stderr
    );
}

#[test]
fn no_pins_change_nothing() {
    let world = World::new().with_env(&[("PWD", "/p")]);
    let output = world.run("20").unwrap();
    assert_eq!(
        lines(&output.stdout),
        [
            "Downloading and installing node v20.10.0...",
            "Creating default alias: default -> 20 (-> v20.10.0 *)",
        ]
    );
    assert_eq!(
        lines(&output.stderr),
        [
            format!("Downloading {TARBALL_URL}..."),
            "Checksums matched!".to_owned()
        ]
    );
}
