use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem};

fn cached(fs: &FakeFileSystem, pattern: &str) -> Option<String> {
    let env = FakeEnv::default().with_var("NVM_DIR", "/n");
    let context = Context::new(fs, &env);
    cached_version(&context, pattern).map(|version| version.to_string())
}

fn cache() -> FakeFileSystem {
    FakeFileSystem::default()
        .with_file(
            "/n/.cache/bin/node-v20.10.0-linux-x64/node-v20.10.0-linux-x64.tar.gz",
            "t",
        )
        .with_file(
            "/n/.cache/bin/node-v18.19.0-linux-x64/node-v18.19.0-linux-x64.tar.gz",
            "t",
        )
        .with_file(
            "/n/.cache/bin/node-v18.18.0-linux-x64/node-v18.18.0-linux-x64.tar.gz",
            "t",
        )
        .with_file(
            "/n/.cache/bin/node-v22.0.0-darwin-arm64/node-v22.0.0-darwin-arm64.tar.gz",
            "t",
        )
        .with_file(
            "/n/.cache/bin/iojs-v3.3.1-linux-x64/iojs-v3.3.1-linux-x64.tar.gz",
            "t",
        )
        .with_file("/n/.cache/src/node-v16.20.2/node-v16.20.2.tar.gz", "t")
}

/// The expectations are what `nvm_ls_cached` of the real `nvm.sh` lists.
#[test]
fn the_newest_cached_match_wins() {
    let fs = cache();
    assert_eq!(cached(&fs, "18").as_deref(), Some("v18.19.0"));
    assert_eq!(cached(&fs, "20").as_deref(), Some("v20.10.0"));
    assert_eq!(cached(&fs, "v18.18").as_deref(), Some("v18.18.0"));
}

#[test]
fn only_this_platform_and_source_entries_count() {
    let fs = cache();
    assert_eq!(cached(&fs, "22"), None);
    assert_eq!(cached(&fs, "16").as_deref(), Some("v16.20.2"));
}

#[test]
fn io_js_is_found_by_its_name_and_a_pattern_is_a_substring() {
    let fs = cache();
    assert_eq!(cached(&fs, "iojs").as_deref(), Some("iojs-v3.3.1"));
    assert_eq!(cached(&fs, "20.1").as_deref(), Some("v20.10.0"));
    assert_eq!(cached(&fs, "99"), None);
}

#[test]
fn an_empty_cache_has_nothing() {
    assert_eq!(cached(&FakeFileSystem::default(), "20"), None);
}
