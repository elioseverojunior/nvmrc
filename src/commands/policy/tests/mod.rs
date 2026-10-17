mod advisories;
mod clients;
mod enforce;
mod load;
mod mode;
mod tool_mode;
mod tools;
mod unknown_index;

use std::ffi::OsString;

use crate::commands::policy::{EnforcedPolicy, Mode};
use crate::context::Context;
use crate::domain::policy::{PolicyFlags, extract};
use crate::fakes::{FakeEnv, FakeFileSystem};

pub const POLICY: &str = r#"{ "schemaVersion": 1, "tools": { "node": { "minimum": "22" } } }"#;

pub fn policy_with_minimum(minimum: &str) -> String {
    format!(r#"{{ "schemaVersion": 1, "tools": {{ "node": {{ "minimum": "{minimum}" }} }} }}"#)
}

pub fn context<'a>(fs: &'a FakeFileSystem, env: &'a FakeEnv) -> Context<'a> {
    Context::new(fs, env)
}

/// The policy flags `values` give, as the command line would.
pub fn flags(values: &[&str]) -> PolicyFlags {
    let args = std::iter::once("nvm").chain(values.iter().copied());
    extract(args.map(OsString::from).collect()).unwrap().0
}

pub fn enforced(mode: Mode) -> EnforcedPolicy {
    match mode {
        Mode::Enforced(policy) => policy,
        Mode::Legacy => panic!("expected an enforced policy"),
    }
}
