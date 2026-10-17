//! A project in `/p` and the registry it reads, for the project-mode tests.

use std::path::Path;

use crate::commands::Output;
use crate::commands::init::run;
use crate::context::Context;
use crate::domain::fixtures::index_text_dated;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeHttp};
use crate::ports::FileSystem;

pub const INDEX_URL: &str = "https://nodejs.org/dist/index.tab";
pub const PNPM_URL: &str = "https://registry.npmjs.org/pnpm";
pub const PNPM_PACKUMENT: &str = r#"{"versions": {"9.1.0": {}, "9.12.0": {}, "10.0.0-rc.1": {}}}"#;
pub const MIN_PNPM: &str =
    r#"{ "schemaVersion": 1, "tools": { "pnpm": { "minimum": "9.12.0" } } }"#;

fn node_index() -> String {
    index_text_dated(&[
        ("v23.0.0", "2024-05-01", "-"),
        ("v22.11.0", "2024-10-29", "Jod"),
        ("v20.18.0", "2024-10-03", "Iron"),
    ])
}

/// A project in `/p` with `files` in it and, unless empty, a policy.
pub fn project(files: &[(&str, &str)], policy: &str) -> (FakeFileSystem, FakeEnv) {
    let mut fs = FakeFileSystem::default().with_dir("/p");
    for (path, contents) in files {
        fs = fs.with_file(path, contents);
    }
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/n")
        .with_var("PWD", "/p")
        .with_current_dir("/p");
    if policy.is_empty() {
        return (fs, env);
    }
    let fs = fs.with_file("/policy.json", policy);
    (fs, env.with_var("NVMRC_POLICY", "/policy.json"))
}

pub fn served() -> FakeHttp {
    FakeHttp::default()
        .with_body(INDEX_URL, &node_index())
        .with_body(PNPM_URL, PNPM_PACKUMENT)
        .with_empty_advisories()
}

pub fn init(fs: &FakeFileSystem, env: &FakeEnv, http: &FakeHttp, args: &[&str]) -> Output {
    let context = Context::new(fs, env)
        .with_http(http)
        .with_third_party_http(http, http);
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    run(&context, &args).unwrap_or_else(|error| panic!("{error}"))
}

pub fn file(fs: &FakeFileSystem, path: &str) -> Option<String> {
    fs.read_to_string(Path::new(path)).ok()
}
