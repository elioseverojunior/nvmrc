use super::*;
use crate::fakes::{FakeDigest, FakeEnv, FakeFileSystem, FakeHttp};
use crate::ports::FileSystem;

const SUMS: &str = "http://127.0.0.1:1/v20.10.0/SHASUMS256.txt";
const TARBALL_URL: &str = "http://127.0.0.1:1/v20.10.0/node-v20.10.0-linux-x64.tar.gz";
const TARBALL: &str =
    "/home/me/.nvm/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz";
const GOOD: &str = "aa11";

fn version() -> Version {
    "v20.10.0".parse().unwrap()
}

fn env() -> FakeEnv {
    FakeEnv::default()
        .with_var("NVM_DIR", "/home/me/.nvm")
        .with_var("HOME", "/home/me")
        .with_var("NVM_NODEJS_ORG_MIRROR", "http://127.0.0.1:1")
}

fn sums(digest: &str) -> String {
    format!("{digest}  node-v20.10.0-linux-x64.tar.gz\nff00  node-v20.10.0-linux-x64.tar.xz\n")
}

fn run(
    fs: &FakeFileSystem,
    http: &FakeHttp,
    digest: &FakeDigest,
) -> (Result<PathBuf, Failed>, Transcript) {
    let env = env();
    let context = Context::new(fs, &env).with_http(http).with_digest(digest);
    let mut transcript = Transcript::default();
    let result = fetch(&context, &version(), false, &mut transcript);
    (result, transcript)
}

fn stderr(transcript: Transcript) -> Vec<String> {
    transcript
        .finish(crate::error::NvmExitCode::Success)
        .stderr
        .lines()
        .map(str::to_owned)
        .collect()
}

#[test]
fn it_downloads_checks_and_keeps_the_archive_in_the_cache() {
    let fs = FakeFileSystem::default();
    let http = FakeHttp::default()
        .with_body(SUMS, &sums(GOOD))
        .with_bytes(TARBALL_URL, b"tarball");
    let digest = FakeDigest::default().with_digest(TARBALL, GOOD);
    let (result, transcript) = run(&fs, &http, &digest);
    assert_eq!(result.unwrap(), PathBuf::from(TARBALL));
    assert_eq!(fs.file_info(Path::new(TARBALL)).unwrap().len, 7);
    assert_eq!(
        stderr(transcript),
        [
            format!("Downloading {TARBALL_URL}..."),
            "Checksums matched!".to_owned()
        ]
    );
}

#[test]
fn a_matching_cached_archive_is_used_without_downloading() {
    let fs = FakeFileSystem::default().with_file(TARBALL, "cached");
    let http = FakeHttp::default().with_body(SUMS, &sums(GOOD));
    let digest = FakeDigest::default().with_digest(TARBALL, GOOD);
    let (result, transcript) = run(&fs, &http, &digest);
    assert!(result.is_ok());
    assert_eq!(http.requests(), [SUMS]);
    assert_eq!(
        stderr(transcript),
        [
            "Local cache found: ${NVM_DIR}/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz",
            "Checksums match! Using existing downloaded archive ${NVM_DIR}/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz",
        ]
    );
}

#[test]
fn a_broken_cached_archive_is_removed_and_downloaded_again() {
    let fs = FakeFileSystem::default().with_file(TARBALL, "corrupt");
    let http = FakeHttp::default()
        .with_body(SUMS, &sums(GOOD))
        .with_bytes(TARBALL_URL, b"fresh");
    let digest = FakeDigest::default().with_digest(TARBALL, "bad0");
    let (result, transcript) = run(&fs, &http, &digest);
    // The fake digest answers the same for the new file, so it fails again.
    assert_eq!(result, Err(Failed));
    let lines = stderr(transcript);
    assert_eq!(
        lines[1],
        "Checksums do not match: 'bad0' found, 'aa11' expected."
    );
    assert_eq!(lines[2], "Checksum check failed!");
    assert_eq!(lines[3], "Removing the broken local cache...");
    assert_eq!(lines[4], format!("Downloading {TARBALL_URL}..."));
}

#[test]
fn a_wrong_checksum_fails_and_leaves_the_archive_but_not_the_unpack_directory() {
    let fs = FakeFileSystem::default();
    let http = FakeHttp::default()
        .with_body(SUMS, &sums(GOOD))
        .with_bytes(TARBALL_URL, b"tarball");
    let digest = FakeDigest::default().with_digest(TARBALL, "bad0");
    let (result, transcript) = run(&fs, &http, &digest);
    assert_eq!(result, Err(Failed));
    assert_eq!(
        stderr(transcript)[1],
        "Checksums do not match: 'bad0' found, 'aa11' expected."
    );
    assert!(fs.file_info(Path::new(TARBALL)).is_ok());
    let files = Path::new("/home/me/.nvm/.cache/bin/node-v20.10.0-linux-x64/files");
    assert!(fs.file_info(files).is_err());
}

#[test]
fn no_listed_checksum_is_a_failure() {
    let fs = FakeFileSystem::default();
    let http = FakeHttp::default().with_bytes(TARBALL_URL, b"tarball");
    let digest = FakeDigest::default().with_digest(TARBALL, GOOD);
    let (result, transcript) = run(&fs, &http, &digest);
    assert_eq!(result, Err(Failed));
    assert_eq!(
        stderr(transcript)[1],
        "Provided checksum to compare to is empty."
    );
}

#[test]
fn a_failed_download_removes_the_cache_directory_and_says_so() {
    let fs = FakeFileSystem::default();
    let http = FakeHttp::default()
        .with_body(SUMS, &sums(GOOD))
        .with_status(TARBALL_URL, 404);
    let (result, transcript) = run(&fs, &http, &FakeDigest::default());
    assert_eq!(result, Err(Failed));
    assert_eq!(
        stderr(transcript),
        [
            format!("Downloading {TARBALL_URL}..."),
            format!("download from {TARBALL_URL} failed")
        ]
    );
    assert!(
        fs.file_info(Path::new(
            "/home/me/.nvm/.cache/bin/node-v20.10.0-linux-x64"
        ))
        .is_err()
    );
}

#[test]
fn a_mirror_that_is_not_a_url_is_reported_and_nothing_is_requested() {
    let fs = FakeFileSystem::default();
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/home/me/.nvm")
        .with_var("NVM_NODEJS_ORG_MIRROR", "not a url");
    let http = FakeHttp::default();
    let digest = FakeDigest::default();
    let context = Context::new(&fs, &env)
        .with_http(&http)
        .with_digest(&digest);
    let mut transcript = Transcript::default();
    assert_eq!(
        fetch(&context, &version(), false, &mut transcript),
        Err(Failed)
    );
    assert!(stderr(transcript)[0].contains("may only contain a URL"));
    assert!(http.requests().is_empty());
}

#[test]
fn the_artifact_is_named_after_the_platform() {
    let fs = FakeFileSystem::default();
    let env = env();
    let mac = crate::domain::platform::Platform::from_host("macos", "aarch64", false);
    let context = Context::new(&fs, &env).with_platform(mac);
    let artifact = Artifact::of(&context, &"v20.10.0".parse().unwrap()).unwrap();
    assert_eq!(artifact.file_name, "node-v20.10.0-darwin-arm64.tar.gz");
    assert_eq!(
        artifact.files(),
        PathBuf::from("/home/me/.nvm/.cache/bin/node-v20.10.0-darwin-arm64/files")
    );
    assert!(Artifact::of(&context.with_platform(None), &"v20.10.0".parse().unwrap()).is_none());
}

fn offline(fs: &FakeFileSystem, http: &FakeHttp) -> (Result<PathBuf, Failed>, Transcript) {
    let env = env();
    let digest = FakeDigest::default();
    let context = Context::new(fs, &env).with_http(http).with_digest(&digest);
    let mut transcript = Transcript::default();
    let result = fetch(&context, &version(), true, &mut transcript);
    (result, transcript)
}

/// The expectations are what the real `nvm install --offline` printed.
#[test]
fn offline_uses_the_cached_archive_without_a_checksum_or_the_network() {
    let fs = FakeFileSystem::default().with_file(TARBALL, "cached");
    let http = FakeHttp::default();
    let (result, transcript) = offline(&fs, &http);
    assert_eq!(result.unwrap(), PathBuf::from(TARBALL));
    assert!(http.requests().is_empty());
    assert_eq!(
        stderr(transcript),
        [
            "Offline: using cached archive ${NVM_DIR}/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz"
        ]
    );
}

#[test]
fn offline_without_a_cached_archive_fails_naming_the_slug() {
    let (result, transcript) = offline(&FakeFileSystem::default(), &FakeHttp::default());
    assert_eq!(result, Err(Failed));
    assert_eq!(
        stderr(transcript),
        ["Offline: no cached archive found for node-v20.10.0-linux-x64"]
    );
}
