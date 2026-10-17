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
