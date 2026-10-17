use std::path::Path;

use super::*;
use crate::fakes::{FakeArchive, FakeEnv, FakeFileSystem, FakeHttp};
use crate::ports::FileSystem;

const TARBALL_BYTES: &[u8] = b"tarball-bytes";
/// `sha512` of [`TARBALL_BYTES`].
const INTEGRITY: &str = "sha512-31GJ6SyG+dFBGDyQB1x8EHrdSW/FYRyb4mHdZIaXTme9WooJgWCCmKoHe70PgTrvis7oBfnw5WLWBp8KVj59oA==";
const PNPM_DIR: &str = "/nvm/package-managers/pnpm/9.12.0";

fn manifest(tarball: &str, bin: &str) -> String {
    format!(r#"{{"bin": {bin}, "dist": {{"tarball": "{tarball}", "integrity": "{INTEGRITY}"}}}}"#)
}

fn pnpm_manifest() -> String {
    let bin = r#"{"pnpm": "bin/pnpm.cjs", "pnpx": "bin/pnpx.cjs"}"#;
    manifest("https://t.test/pnpm.tgz", bin)
}

fn pnpm_archive(fs: &FakeFileSystem) -> FakeArchive<'_> {
    FakeArchive::new(fs).with_archive(
        &format!("{PNPM_DIR}.partial/download.tgz"),
        &[
            ("package/package.json", "{}", false),
            ("package/bin/pnpm.cjs", "pnpm", false),
            ("package/bin/pnpx.cjs", "pnpx", false),
        ],
    )
}

fn pnpm_http() -> FakeHttp {
    FakeHttp::default()
        .with_body("https://registry.npmjs.org/pnpm/9.12.0", &pnpm_manifest())
        .with_bytes("https://t.test/pnpm.tgz", TARBALL_BYTES)
}

fn version(text: &str) -> Version {
    text.parse().unwrap()
}

fn partial_is_gone(fs: &FakeFileSystem) -> bool {
    fs.read_dir(Path::new(&format!("{PNPM_DIR}.partial")))
        .is_err()
}

#[test]
fn installs_pnpm_with_its_commands_linked() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVM_DIR", "/nvm");
    let (http, archive) = (pnpm_http(), pnpm_archive(&fs));
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_archive(&archive);
    let pinned = version("v9.12.0");
    assert!(!is_installed(&context, Tool::Pnpm, &pinned));
    install_tool(&context, Tool::Pnpm, &pinned).unwrap();
    assert!(is_installed(&context, Tool::Pnpm, &pinned));
    let bin = bin_dir(&context, Tool::Pnpm, &pinned).unwrap();
    assert_eq!(bin, Path::new("/nvm/package-managers/pnpm/9.12.0/bin"));
    assert_eq!(
        fs.read_link(&bin.join("pnpm")).unwrap(),
        Path::new("../package/bin/pnpm.cjs")
    );
    assert_eq!(
        fs.read_link(&bin.join("pnpx")).unwrap(),
        Path::new("../package/bin/pnpx.cjs")
    );
    let target = fs.file_info(&Path::new(PNPM_DIR).join("package/bin/pnpm.cjs"));
    assert!(target.unwrap().executable);
    assert!(fs.file_info(&bin.join("pnpm")).unwrap().executable);
    assert!(partial_is_gone(&fs));
}

#[test]
fn yarn_berry_comes_from_cli_dist() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVM_DIR", "/nvm");
    let url = "https://registry.npmjs.org/@yarnpkg%2fcli-dist/4.5.0";
    let body = manifest("https://t.test/y.tgz", r#"{"yarn": "bin/yarn.js"}"#);
    let http = FakeHttp::default()
        .with_body(url, &body)
        .with_bytes("https://t.test/y.tgz", TARBALL_BYTES);
    let partial = "/nvm/package-managers/yarn/4.5.0.partial/download.tgz";
    let files = [
        ("package/package.json", "{}", false),
        ("package/bin/yarn.js", "yarn", false),
    ];
    let archive = FakeArchive::new(&fs).with_archive(partial, &files);
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_archive(&archive);
    let pinned = version("v4.5.0");
    install_tool(&context, Tool::Yarn, &pinned).unwrap();
    assert!(is_installed(&context, Tool::Yarn, &pinned));
    assert_eq!(http.requests()[0], url);
}

#[test]
fn a_tarball_that_fails_integrity_leaves_nothing_in_the_store() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default().with_var("NVM_DIR", "/nvm");
    let http = pnpm_http().with_bytes("https://t.test/pnpm.tgz", b"tampered");
    let archive = pnpm_archive(&fs);
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_archive(&archive);
    let error = install_tool(&context, Tool::Pnpm, &version("v9.12.0")).unwrap_err();
    assert!(error.contains("sha512"), "{error}");
    assert!(fs.read_dir(Path::new("/nvm")).is_err());
}

#[test]
fn a_broken_install_is_replaced() {
    let fs = FakeFileSystem::default().with_file(&format!("{PNPM_DIR}/stale"), "old");
    let env = FakeEnv::default().with_var("NVM_DIR", "/nvm");
    let (http, archive) = (pnpm_http(), pnpm_archive(&fs));
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_archive(&archive);
    let pinned = version("v9.12.0");
    assert!(!is_installed(&context, Tool::Pnpm, &pinned));
    install_tool(&context, Tool::Pnpm, &pinned).unwrap();
    assert!(is_installed(&context, Tool::Pnpm, &pinned));
    assert!(!fs.is_file(&Path::new(PNPM_DIR).join("stale")));
}

#[test]
fn a_failed_install_keeps_the_existing_one() {
    let fs = FakeFileSystem::default().with_file(&format!("{PNPM_DIR}/stale"), "old");
    let env = FakeEnv::default().with_var("NVM_DIR", "/nvm");
    let http = pnpm_http().with_bytes("https://t.test/pnpm.tgz", b"tampered");
    let archive = pnpm_archive(&fs);
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_archive(&archive);
    install_tool(&context, Tool::Pnpm, &version("v9.12.0")).unwrap_err();
    assert!(fs.is_file(&Path::new(PNPM_DIR).join("stale")));
    assert!(partial_is_gone(&fs));
}

#[test]
fn the_registry_variable_is_used() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/nvm")
        .with_var("NVMRC_NPM_REGISTRY", "https://mirror.test/npm/");
    let http = FakeHttp::default()
        .with_body("https://mirror.test/npm/pnpm/9.12.0", &pnpm_manifest())
        .with_bytes("https://t.test/pnpm.tgz", TARBALL_BYTES);
    let archive = pnpm_archive(&fs);
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_archive(&archive);
    install_tool(&context, Tool::Pnpm, &version("v9.12.0")).unwrap();
    assert_eq!(http.requests()[0], "https://mirror.test/npm/pnpm/9.12.0");
}

/// An archive whose `bin/pnpm.cjs` is a link to a file outside the package,
/// as `tar` would unpack it.
struct LinkingArchive<'a> {
    fs: &'a FakeFileSystem,
    inner: FakeArchive<'a>,
}

impl crate::ports::Archive for LinkingArchive<'_> {
    fn extract(&self, archive: &Path, destination: &Path) -> std::io::Result<()> {
        self.inner.extract(archive, destination)?;
        let link = destination.join("package/bin/pnpm.cjs");
        self.fs.symlink(Path::new("/outside/secret"), &link)
    }
}

#[test]
fn a_command_linked_outside_the_package_refuses_the_install() {
    let fs = FakeFileSystem::default().with_file("/outside/secret", "x");
    let env = FakeEnv::default().with_var("NVM_DIR", "/nvm");
    let inner = FakeArchive::new(&fs).with_archive(
        &format!("{PNPM_DIR}.partial/download.tgz"),
        &[
            ("package/package.json", "{}", false),
            ("package/bin/pnpx.cjs", "pnpx", false),
        ],
    );
    let archive = LinkingArchive { fs: &fs, inner };
    let http = pnpm_http();
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_archive(&archive);
    let error = install_tool(&context, Tool::Pnpm, &version("v9.12.0")).unwrap_err();
    assert!(error.contains("outside"), "{error}");
    assert!(
        !fs.file_info(Path::new("/outside/secret"))
            .unwrap()
            .executable
    );
    assert!(fs.file_info(Path::new(PNPM_DIR)).is_err());
    assert!(partial_is_gone(&fs));
}

#[test]
fn a_package_without_the_tools_own_command_is_refused() {
    let fs = FakeFileSystem::default().with_file(&format!("{PNPM_DIR}/stale"), "old");
    let env = FakeEnv::default().with_var("NVM_DIR", "/nvm");
    let body = manifest("https://t.test/pnpm.tgz", r#"{"pnpx": "bin/pnpx.cjs"}"#);
    let http = FakeHttp::default()
        .with_body("https://registry.npmjs.org/pnpm/9.12.0", &body)
        .with_bytes("https://t.test/pnpm.tgz", TARBALL_BYTES);
    let archive = pnpm_archive(&fs);
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_archive(&archive);
    let error = install_tool(&context, Tool::Pnpm, &version("v9.12.0")).unwrap_err();
    assert!(error.contains("pnpm"), "{error}");
    assert!(fs.is_file(&Path::new(PNPM_DIR).join("stale")));
}

#[test]
fn stale_work_areas_are_cleared_and_the_old_copy_does_not_linger() {
    let fs = FakeFileSystem::default()
        .with_file(&format!("{PNPM_DIR}.partial/junk"), "x")
        .with_file(&format!("{PNPM_DIR}.old/junk"), "x")
        .with_file(&format!("{PNPM_DIR}/stale"), "old");
    let env = FakeEnv::default().with_var("NVM_DIR", "/nvm");
    let (http, archive) = (pnpm_http(), pnpm_archive(&fs));
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_archive(&archive);
    install_tool(&context, Tool::Pnpm, &version("v9.12.0")).unwrap();
    assert!(fs.read_dir(Path::new(&format!("{PNPM_DIR}.old"))).is_err());
    assert!(partial_is_gone(&fs));
    assert!(!fs.is_file(&Path::new(PNPM_DIR).join("stale")));
}
