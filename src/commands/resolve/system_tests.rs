//! `system_node` finds the `node` left on `PATH` once nvm's own entries are
//! stripped from it, as nvm.sh's `nvm_has_system_node` does.

use std::path::PathBuf;

use super::system_node;
use crate::context::Context;
use crate::fakes::{FakeEnv, FakeFileSystem};

fn found_on(path: &str) -> Option<PathBuf> {
    let fs = FakeFileSystem::default()
        .with_file("/w/nvm/versions/node/v20.1.0/bin/node", "")
        // The fake normalises `..` when it looks a file up.
        .with_file("/w/sys/node", "")
        .with_file("/w/nvm/tools/bin/node", "");
    let env = FakeEnv::default()
        .with_var("NVM_DIR", "/w/nvm")
        .with_var("PATH", path);
    system_node(&Context::new(&fs, &env)).unwrap()
}

#[test]
fn a_directory_whose_text_starts_with_nvm_dir_is_the_system() {
    let found = found_on("/w/nvm/versions/node/v20.1.0/bin:/w/nvm/../sys:/usr/bin");
    assert_eq!(found, Some(PathBuf::from("/w/nvm/../sys/node")));
}

#[test]
fn the_entries_nvm_strips_are_never_the_system() {
    assert_eq!(found_on("/w/nvm/versions/node/v20.1.0/bin"), None);
    assert_eq!(found_on("/w/nvm/tools/bin"), None);
}
