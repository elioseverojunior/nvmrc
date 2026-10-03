use super::*;
use crate::fakes::{FakeArchive, FakeEnv, FakeFileSystem};
use crate::ports::FileSystem;

const TARBALL: &str = "/n/.cache/bin/s/s.tar.gz";
const FILES: &str = "/n/.cache/bin/s/files";
const TARGET: &str = "/n/versions/node/v20.10.0";

fn version() -> Version {
    "v20.10.0".parse().unwrap()
}

fn env() -> FakeEnv {
    FakeEnv::default().with_var("NVM_DIR", "/n")
}

fn read(fs: &FakeFileSystem, path: &str) -> Option<String> {
    fs.read_to_string(Path::new(path)).ok()
}

#[test]
fn the_version_path_is_per_flavor() {
    let (fs, env) = (FakeFileSystem::default(), env());
    let context = Context::new(&fs, &env);
    assert_eq!(
        version_path(&context, &version()).unwrap(),
        PathBuf::from(TARGET)
    );
    let iojs = "iojs-v3.3.1".parse().unwrap();
    let expected = PathBuf::from("/n/versions/io.js/v3.3.1");
    assert_eq!(version_path(&context, &iojs).unwrap(), expected);
}

#[test]
fn node_below_0_12_lives_directly_in_nvm_dir_as_nvm_version_path_says() {
    let (fs, env) = (FakeFileSystem::default(), env());
    let context = Context::new(&fs, &env);
    let path_of = |text: &str| version_path(&context, &text.parse().unwrap()).unwrap();
    assert_eq!(path_of("v0.10.48"), PathBuf::from("/n/v0.10.48"));
    assert_eq!(path_of("v0.11.16"), PathBuf::from("/n/v0.11.16"));
    assert_eq!(
        path_of("v0.12.0"),
        PathBuf::from("/n/versions/node/v0.12.0")
    );
    assert_eq!(
        path_of("iojs-v1.0.0"),
        PathBuf::from("/n/versions/io.js/v1.0.0")
    );
}

#[test]
fn an_install_needs_a_runnable_node_that_is_not_empty() {
    let env = env();
    let check = |fs: &FakeFileSystem| is_valid_install(&Context::new(fs, &env), Path::new(TARGET));
    let good = FakeFileSystem::default().with_executable("/n/versions/node/v20.10.0/bin/node", "x");
    assert!(check(&good));
    let empty = FakeFileSystem::default().with_executable("/n/versions/node/v20.10.0/bin/node", "");
    assert!(!check(&empty));
    let not_runnable =
        FakeFileSystem::default().with_file("/n/versions/node/v20.10.0/bin/node", "x");
    assert!(!check(&not_runnable));
    assert!(!check(&FakeFileSystem::default()));
}

#[test]
fn placing_moves_the_top_level_directory_to_the_version_path() {
    let fs = FakeFileSystem::default();
    let archive = FakeArchive::new(&fs).with_archive(
        TARBALL,
        &[
            ("node-v20.10.0-linux-x64/bin/node", "binary", true),
            ("node-v20.10.0-linux-x64/README", "hi", false),
        ],
    );
    let env = env();
    let context = Context::new(&fs, &env).with_archive(&archive);
    place(
        &context,
        Path::new(TARBALL),
        Path::new(FILES),
        Path::new(TARGET),
    )
    .unwrap();
    assert_eq!(
        read(&fs, "/n/versions/node/v20.10.0/README").as_deref(),
        Some("hi")
    );
    assert!(is_valid_install(&context, Path::new(TARGET)));
    assert!(fs.file_info(Path::new(FILES)).is_err());
}

#[test]
fn a_broken_install_that_was_there_is_replaced_whole() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.10.0/stale", "old");
    let archive = FakeArchive::new(&fs).with_archive(TARBALL, &[("top/bin/node", "binary", true)]);
    let env = env();
    let context = Context::new(&fs, &env).with_archive(&archive);
    place(
        &context,
        Path::new(TARBALL),
        Path::new(FILES),
        Path::new(TARGET),
    )
    .unwrap();
    assert!(
        fs.file_info(Path::new("/n/versions/node/v20.10.0/stale"))
            .is_err()
    );
    assert!(
        fs.file_info(Path::new("/n/versions/node/v20.10.0/bin/node"))
            .is_ok()
    );
}

#[test]
fn leftovers_in_the_unpack_directory_do_not_confuse_the_layout() {
    let fs = FakeFileSystem::default().with_file("/n/.cache/bin/s/files/old-run/f", "x");
    let archive = FakeArchive::new(&fs).with_archive(TARBALL, &[("top/bin/node", "b", true)]);
    let env = env();
    let context = Context::new(&fs, &env).with_archive(&archive);
    place(
        &context,
        Path::new(TARBALL),
        Path::new(FILES),
        Path::new(TARGET),
    )
    .unwrap();
    assert!(is_valid_install(&context, Path::new(TARGET)));
}

#[test]
fn an_archive_with_two_top_level_entries_is_refused_and_leaves_the_install_alone() {
    let fs = FakeFileSystem::default().with_file("/n/versions/node/v20.10.0/keep", "x");
    let archive =
        FakeArchive::new(&fs).with_archive(TARBALL, &[("a/f", "1", false), ("b/f", "2", false)]);
    let env = env();
    let context = Context::new(&fs, &env).with_archive(&archive);
    let error = place(
        &context,
        Path::new(TARBALL),
        Path::new(FILES),
        Path::new(TARGET),
    )
    .unwrap_err();
    assert_eq!(
        error,
        "the archive does not hold exactly one top-level directory"
    );
    assert!(
        fs.file_info(Path::new("/n/versions/node/v20.10.0/keep"))
            .is_ok()
    );
}

#[test]
fn an_archive_that_cannot_be_unpacked_is_an_error() {
    let (fs, env) = (FakeFileSystem::default(), env());
    let context = Context::new(&fs, &env);
    assert!(
        place(
            &context,
            Path::new(TARBALL),
            Path::new(FILES),
            Path::new(TARGET)
        )
        .is_err()
    );
}
