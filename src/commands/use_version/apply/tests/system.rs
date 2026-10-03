//! `nvm use system` (digest 4.2 step 6): a silent deactivate, then the
//! version of the `node` (or `iojs`) left on the stripped `PATH`.

use super::{BASE_PATH, N18, env_on, lab, npm_versions, use_in};
use crate::error::NvmExitCode;

const DEACTIVATED: &str = "export PATH='/usr/bin:/bin'\nhash -r 2>/dev/null || true\n\
unset NVM_BIN\nunset NVM_INC\n";

fn active_path() -> String {
    format!("{N18}/bin:{BASE_PATH}")
}

#[test]
fn use_system_deactivates_and_names_the_system_node_and_its_npm() {
    let fs = lab()
        .with_executable("/usr/bin/node", "")
        .with_executable("/usr/bin/npm", "");
    let process = npm_versions()
        .with_output("/usr/bin/node", "v16.0.0\n")
        .with_success("/usr/bin/npm", "--version", "8.0.0\n");
    let output = use_in(&fs, &env_on(&active_path()), &process, &["system"]);
    assert_eq!(
        output.stdout,
        "Now using system version of node: v16.0.0 (npm v8.0.0)"
    );
    assert_eq!(output.stderr, "");
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(output.script.render(), DEACTIVATED);
}

#[test]
fn use_system_with_nothing_active_discards_the_deactivate_messages() {
    let fs = lab().with_executable("/bin/node", "");
    let process = npm_versions().with_output("/bin/node", "v16.0.0\n");
    let output = use_in(&fs, &env_on(BASE_PATH), &process, &["system"]);
    assert_eq!(output.stdout, "Now using system version of node: v16.0.0");
    assert_eq!(output.stderr, "");
    assert_eq!(output.script.render(), "unset NVM_BIN\nunset NVM_INC\n");
}

#[test]
fn use_system_silent_prints_nothing_and_still_deactivates() {
    let fs = lab().with_executable("/usr/bin/node", "");
    let process = npm_versions().with_output("/usr/bin/node", "v16.0.0\n");
    let output = use_in(
        &fs,
        &env_on(&active_path()),
        &process,
        &["system", "--silent"],
    );
    assert_eq!((output.stdout.as_str(), output.stderr.as_str()), ("", ""));
    assert_eq!(output.status, NvmExitCode::Success);
    assert_eq!(output.script.render(), DEACTIVATED);
}

#[test]
fn use_system_iojs_names_io_js() {
    let fs = lab().with_executable("/usr/bin/iojs", "");
    let process = npm_versions().with_output("/usr/bin/iojs", "v3.3.1\n");
    let output = use_in(&fs, &env_on(&active_path()), &process, &["system"]);
    assert_eq!(output.stdout, "Now using system version of io.js: v3.3.1");
    assert_eq!(output.script.render(), DEACTIVATED);
}

#[test]
fn use_system_without_a_system_node_halts_with_127_and_no_code() {
    let output = use_in(
        &lab(),
        &env_on(&active_path()),
        &npm_versions(),
        &["system"],
    );
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, "System version of node not found.");
    assert_eq!(output.status, NvmExitCode::NotFound);
    assert!(output.script.is_empty());
}
