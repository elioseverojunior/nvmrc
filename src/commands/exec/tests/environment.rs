//! What the child sees: `MANPATH` when `manpath` exists, the system node,
//! the prefix checks `nvm-exec`'s `nvm use` makes, and the `current` link.

use std::path::{Path, PathBuf};

use super::*;
use crate::ports::FileSystem;

const PREFIX_MESSAGE: &str = "nvm is not compatible with the \"PREFIX\" environment variable: \
currently set to \"/x\"\nRun `unset PREFIX` to unset it.";

#[test]
fn manpath_is_switched_too_when_a_manpath_program_exists() {
    let fs = lab().with_executable("/usr/bin/manpath", "");
    let env = env_on(BASE_PATH).with_var("MANPATH", "/m1:/m2");
    let output = exec_in(&fs, &env, &["18", "node"]);
    let manpath = format!("{N18}/share/man:/m1:/m2:");
    let expected = child(N18, "v18.20.4", &["node"]);
    let mut env = expected.env.clone();
    env.insert(1, ("MANPATH".to_owned(), manpath));
    assert_eq!(output.spawn, Some(Invocation { env, ..expected }));
}

#[test]
fn the_system_node_runs_without_the_nvm_entries() {
    let path = format!("{N18}/bin:/sys/bin:/usr/bin");
    let env = env_on(&path).with_var("MANPATH", &format!("{N18}/share/man:/m"));
    let output = exec_in(&lab(), &env, &["system", "node", "a"]);
    let expected = Invocation::new("node")
        .args(&["a"])
        .env("PATH", "/sys/bin:/usr/bin")
        .env("MANPATH", "/m")
        .env("NODE_VERSION", "system")
        .env("NVM_CD_FLAGS", "")
        .env("NVM_DIR", "/n")
        .env_remove("NVM_BIN")
        .env_remove("NVM_INC");
    assert_runs(&output, "Running node system (npm v8.0.0)", expected);
}

#[test]
fn a_refused_prefix_is_reported_twice_and_stops_with_127() {
    let env = env_on(BASE_PATH).with_var("PREFIX", "/x");
    let output = exec_in(&lab(), &env, &["18", "node", "a"]);
    let stderr = format!("{PREFIX_MESSAGE}\n{PREFIX_MESSAGE}");
    assert_stops(
        &output,
        "Running node v18.20.4",
        &stderr,
        NvmExitCode::NotFound,
    );
}

#[test]
fn a_refused_prefix_is_reported_once_when_silent() {
    let env = env_on(BASE_PATH).with_var("PREFIX", "/x");
    let output = exec_in(&lab(), &env, &["--silent", "18", "node", "a"]);
    assert_stops(&output, "", PREFIX_MESSAGE, NvmExitCode::NotFound);
}

#[test]
fn symlink_current_true_points_current_at_the_version() {
    let fs = lab();
    let env = env_on(BASE_PATH).with_var("NVM_SYMLINK_CURRENT", "true");
    let output = exec_in(&fs, &env, &["18", "node"]);
    assert_eq!(output.status, NvmExitCode::Success);
    let link = fs.read_link(Path::new("/n/current")).unwrap();
    assert_eq!(link, PathBuf::from(N18));
}

#[test]
fn the_parent_environment_is_left_alone() {
    let output = exec(&["18", "node"]);
    assert!(output.script.is_empty());
}
