use std::path::Path;

use super::*;
use crate::domain::fixtures::index_text;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};
use crate::ports::FileSystem;

const NODE_INDEX: &str = "https://nodejs.org/dist/index.tab";

fn node_text() -> String {
    index_text(&[
        ("v20.10.0", "Iron"),
        ("v20.9.0", "Iron"),
        ("v18.19.0", "Hydrogen"),
    ])
}

fn env() -> FakeEnv {
    FakeEnv::default().with_var("NVM_DIR", "/n")
}

#[test]
fn it_returns_the_releases_and_writes_the_lts_aliases() {
    let fs = FakeFileSystem::default();
    let env = env();
    let http = FakeHttp::default().with_body(NODE_INDEX, &node_text());
    let context = Context::new(&fs, &env).with_http(&http);
    let fetched = fetch(&context, Flavor::Node).unwrap();
    assert_eq!(fetched.releases.unwrap().len(), 3);
    assert_eq!(fetched.warning, None);
    let read = |name: &str| fs.read_to_string(Path::new(&format!("/n/alias/lts/{name}")));
    assert_eq!(read("*").unwrap(), "lts/iron\n");
    assert_eq!(read("iron").unwrap(), "v20.10.0\n");
    assert_eq!(read("hydrogen").unwrap(), "v18.19.0\n");
}

#[test]
fn a_failed_download_gives_no_releases_and_no_aliases() {
    let fs = FakeFileSystem::default();
    let env = env();
    let http = FakeHttp::default().with_status(NODE_INDEX, 404);
    let context = Context::new(&fs, &env).with_http(&http);
    let fetched = fetch(&context, Flavor::Node).unwrap();
    assert!(fetched.releases.is_none());
    assert_eq!(fetched.warning, None);
    assert!(fs.read_dir(Path::new("/n/alias/lts")).unwrap().is_empty());
}

#[test]
fn a_mirror_that_is_not_a_url_is_a_warning_and_no_request() {
    let fs = FakeFileSystem::default();
    let env = env().with_var("NVM_NODEJS_ORG_MIRROR", "not a url");
    let http = FakeHttp::default();
    let context = Context::new(&fs, &env).with_http(&http);
    let fetched = fetch(&context, Flavor::Node).unwrap();
    assert!(fetched.releases.is_none());
    assert!(fetched.warning.unwrap().contains("may only contain a URL"));
    assert!(http.requests().is_empty());
}

#[test]
fn the_iojs_index_is_read_from_its_own_mirror() {
    let fs = FakeFileSystem::default();
    let env = env().with_var("NVM_IOJS_ORG_MIRROR", "https://m.example/iojs");
    let body = index_text(&[("v3.3.1", "-")]);
    let http = FakeHttp::default().with_body("https://m.example/iojs/index.tab", &body);
    let context = Context::new(&fs, &env).with_http(&http);
    let releases = fetch(&context, Flavor::IoJs).unwrap().releases.unwrap();
    assert_eq!(releases[0].version.to_string(), "iojs-v3.3.1");
}

#[test]
fn fetch_if_skips_the_download_when_not_wanted() {
    let fs = FakeFileSystem::default();
    let env = env();
    let http = FakeHttp::default();
    let context = Context::new(&fs, &env).with_http(&http);
    let mut warnings = Vec::new();
    let releases = fetch_if(&context, false, Flavor::Node, &mut warnings).unwrap();
    assert_eq!(releases, None);
    assert!(warnings.is_empty());
}

#[test]
fn fetch_if_keeps_the_warning_about_an_unusable_mirror() {
    let fs = FakeFileSystem::default();
    let env = env().with_var("NVM_NODEJS_ORG_MIRROR", "not a url");
    let http = FakeHttp::default();
    let context = Context::new(&fs, &env).with_http(&http);
    let mut warnings = Vec::new();
    let releases = fetch_if(&context, true, Flavor::Node, &mut warnings).unwrap();
    assert_eq!(releases, None);
    assert_eq!(warnings.len(), 1);
}
