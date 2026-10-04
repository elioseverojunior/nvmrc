//! `--offline`: the cached archive, taken without a checksum, or nothing.

use super::tests::{TARBALL, artifact_of, env, stderr, version};
use super::*;
use crate::fakes::{FakeDigest, FakeFileSystem, FakeHttp};

fn offline(fs: &FakeFileSystem, http: &FakeHttp) -> (Result<PathBuf, Failed>, Transcript) {
    let env = env();
    let digest = FakeDigest::default();
    let context = Context::new(fs, &env).with_http(http).with_digest(&digest);
    let mut transcript = Transcript::default();
    let result = fetch(
        &context,
        &artifact_of(&context),
        &version(),
        true,
        &mut transcript,
    );
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
            "Offline: using cached archive ${NVM_DIR}/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.xz"
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

#[test]
fn offline_does_not_take_a_directory_for_the_cached_archive() {
    let fs = FakeFileSystem::default().with_dir(TARBALL);
    let (result, transcript) = offline(&fs, &FakeHttp::default());
    assert_eq!(result, Err(Failed));
    assert_eq!(
        stderr(transcript),
        ["Offline: no cached archive found for node-v20.10.0-linux-x64"]
    );
}
