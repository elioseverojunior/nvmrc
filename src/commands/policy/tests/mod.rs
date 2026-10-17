mod advisories;
mod enforce;
mod load;
mod mode;

use crate::context::Context;
use crate::fakes::{FakeEnv, FakeFileSystem};

pub const POLICY: &str = r#"{ "schemaVersion": 1, "tools": { "node": { "minimum": "22" } } }"#;

pub fn policy_with_minimum(minimum: &str) -> String {
    format!(r#"{{ "schemaVersion": 1, "tools": {{ "node": {{ "minimum": "{minimum}" }} }} }}"#)
}

pub fn context<'a>(fs: &'a FakeFileSystem, env: &'a FakeEnv) -> Context<'a> {
    Context::new(fs, env)
}
