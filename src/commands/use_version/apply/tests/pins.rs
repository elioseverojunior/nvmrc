//! The pinned package managers on the `PATH` of `nvm use`.

use super::{BASE_PATH, HASH, env_on, lab, npm_versions, use_in, with_tool};
use crate::error::NvmExitCode;

const PNPM_BIN: &str = "/n/package-managers/pnpm/9.12.0/bin";
const N20: &str = "/n/versions/node/v20.11.1";

#[test]
fn use_puts_pinned_pnpm_on_path() {
    let nvmrc = lab().with_file("/proj/.nvmrc", "20\npnpm=9.12.0\n");
    let fs = with_tool(nvmrc, "pnpm", "9.12.0");
    let output = use_in(&fs, &env_on(BASE_PATH), &npm_versions(), &[]);
    assert_eq!(output.status, NvmExitCode::Success);
    let expected = format!("export PATH='{PNPM_BIN}:{N20}/bin:{BASE_PATH}'\n{HASH}");
    let script = output.script.render();
    assert!(script.starts_with(&expected), "{script}");
    assert!(
        output
            .stdout
            .ends_with("Now using node v20.11.1 (npm v10.2.4)")
    );
}

#[test]
fn use_drops_the_tool_bins_of_another_project() {
    let path = format!("{PNPM_BIN}:{BASE_PATH}");
    let output = use_in(&lab(), &env_on(&path), &npm_versions(), &["18"]);
    assert_eq!(output.status, NvmExitCode::Success);
    let expected = format!("export PATH='/n/versions/node/v18.20.4/bin:{BASE_PATH}'\n{HASH}");
    let script = output.script.render();
    assert!(script.starts_with(&expected), "{script}");
}

#[test]
fn now_using_shows_the_pinned_npm() {
    let nvmrc = lab().with_file("/proj/.nvmrc", "20\nnpm=10.9.0\n");
    let fs = with_tool(nvmrc, "npm", "10.9.0");
    let process = npm_versions().with_success(
        "/n/package-managers/npm/10.9.0/bin/npm",
        "--version",
        "10.9.0\n",
    );
    let output = use_in(&fs, &env_on(BASE_PATH), &process, &[]);
    assert!(
        output
            .stdout
            .ends_with("Now using node v20.11.1 (npm v10.9.0)")
    );
}

#[test]
fn use_system_adds_no_pins() {
    let nvmrc = lab().with_file("/proj/.nvmrc", "system\npnpm=9.12.0\n");
    let fs = with_tool(nvmrc, "pnpm", "9.12.0");
    let env = env_on(&format!("{PNPM_BIN}:/sys/bin:/usr/bin"));
    let output = use_in(&fs, &env, &npm_versions(), &["system"]);
    assert!(!output.script.render().contains("package-managers"));
}

#[test]
fn a_missing_pinned_tool_keeps_the_earlier_messages() {
    let fs = lab().with_file("/proj/.nvmrc", "20\npnpm=9.12.0\n");
    let output = use_in(&fs, &env_on(BASE_PATH), &npm_versions(), &[]);
    assert_eq!(output.status, NvmExitCode::InvalidVersion);
    assert_eq!(output.stdout, "Found '/proj/.nvmrc' with version <20>");
    let text = "nvm: pnpm 9.12.0 (from /proj/.nvmrc) is not installed. Run `nvm install`.";
    assert_eq!(output.stderr, text);
    assert!(output.script.is_empty());
}
