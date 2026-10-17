//! The pinned package managers in the environment of the child.

use super::*;

#[test]
fn exec_gives_the_child_the_pinned_tools() {
    let nvmrc = lab().with_file("/proj/.nvmrc", "20\npnpm=9.12.0\n");
    let fs = with_tool(nvmrc, "pnpm", "9.12.0");
    let output = exec_in(&fs, &env_on(BASE_PATH), &["20", "node", "-v"]);
    let mut expected = child(N20, "v20.11.1", &["node", "-v"]);
    expected.env[0].1 = format!("/n/package-managers/pnpm/9.12.0/bin:{N20}/bin:{BASE_PATH}");
    assert_runs(&output, "Running node v20.11.1 (npm v10.2.4)", expected);
}

#[test]
fn exec_without_pins_drops_the_tool_bins_of_the_parent() {
    let path = format!("/n/package-managers/pnpm/9.12.0/bin:{BASE_PATH}");
    let output = exec_in(&lab(), &env_on(&path), &["20", "node", "-v"]);
    let expected = child(N20, "v20.11.1", &["node", "-v"]);
    assert_runs(&output, "Running node v20.11.1 (npm v10.2.4)", expected);
}

#[test]
fn a_missing_pinned_tool_stops_exec_with_3() {
    let fs = lab().with_file("/proj/.nvmrc", "20\npnpm=9.12.0\n");
    let output = exec_in(&fs, &env_on(BASE_PATH), &["20", "node", "-v"]);
    let text = "nvm: pnpm 9.12.0 (from /proj/.nvmrc) is not installed. Run `nvm install`.";
    assert_stops(&output, "", text, NvmExitCode::InvalidVersion);
}

#[test]
fn a_bad_pin_stops_exec_with_3_after_the_earlier_messages() {
    let fs = lab().with_file("/proj/.nvmrc", "20\npnpm=9.12\n");
    let output = exec_in(&fs, &env_on(BASE_PATH), &["npm", "-v"]);
    let text = "nvm: /proj/.nvmrc: 'pnpm=9.12' is not an exact version (major.minor.patch).";
    let found = "Found '/proj/.nvmrc' with version <20>";
    assert_stops(&output, found, text, NvmExitCode::InvalidVersion);
}

#[test]
fn exec_on_the_system_node_drops_tool_bins_and_adds_none() {
    let nvmrc = lab().with_file("/proj/.nvmrc", "system\npnpm=9.12.0\n");
    let fs = with_tool(nvmrc, "pnpm", "9.12.0");
    let path = format!("/n/package-managers/pnpm/9.12.0/bin:/sys/bin:{BASE_PATH}");
    let output = exec_in(&fs, &env_on(&path), &["system", "node", "-v"]);
    let spawn = output.spawn.expect("the command runs");
    let value = spawn.env.iter().find(|(name, _)| name == "PATH");
    let expected = format!("/sys/bin:{BASE_PATH}");
    assert_eq!(
        value.map(|(_, path)| path.as_str()),
        Some(expected.as_str())
    );
}
