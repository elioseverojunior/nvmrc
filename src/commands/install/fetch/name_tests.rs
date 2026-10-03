use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem};

fn env() -> FakeEnv {
    FakeEnv::default().with_var("NVM_DIR", "/home/me/.nvm")
}

#[test]
fn a_node_without_an_xz_archive_gets_the_gzip_one() {
    let fs = FakeFileSystem::default();
    let env = env();
    let context = Context::new(&fs, &env);
    let old = Artifact::of(&context, &"v0.10.41".parse().unwrap()).unwrap();
    assert_eq!(old.file_name, "node-v0.10.41-linux-x64.tar.gz");
    let iojs = Artifact::of(&context, &"iojs-v3.3.1".parse().unwrap()).unwrap();
    assert_eq!(iojs.file_name, "iojs-v3.3.1-linux-x64.tar.xz");
    let mac = crate::domain::platform::Platform::from_host("macos", "x86_64", false);
    let mac = context.with_platform(mac);
    let iojs = Artifact::of(&mac, &"iojs-v2.3.1".parse().unwrap()).unwrap();
    assert_eq!(iojs.file_name, "iojs-v2.3.1-darwin-x64.tar.gz");
}

#[test]
fn the_source_archive_follows_the_same_rule() {
    let fs = FakeFileSystem::default();
    let env = env();
    let context = Context::new(&fs, &env);
    let new = Artifact::source_of(&context, &"v20.10.0".parse().unwrap()).unwrap();
    assert_eq!(new.file_name, "node-v20.10.0.tar.xz");
    let old = Artifact::source_of(&context, &"v0.10.41".parse().unwrap()).unwrap();
    assert_eq!(old.file_name, "node-v0.10.41.tar.gz");
}
