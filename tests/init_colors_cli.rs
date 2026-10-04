//! End-to-end: the color settings a shell holds without exporting them
//! (`NVM_COLORS`, `NVM_HAS_COLORS`) reach the binary through the `nvm`
//! function, as nvm.sh (a shell function) sees them, and unset ones stay
//! unset for the programs the binary starts.
#![cfg(unix)]

mod init_lab;

use init_lab::{Lab, each_shell, in_dialect, with_function};

const ALIAS_UNEXPORTED: &str = "NVM_COLORS=rgbcm\nNVM_HAS_COLORS=1\nnvm alias foo 20";
const ALIAS_UNEXPORTED_FISH: &str = "set NVM_COLORS rgbcm\nset NVM_HAS_COLORS 1\nnvm alias foo 20";
/// `v20.11.1` is installed and not current: the installed role, `r` here.
const ALIAS_IN_RED: &str = "\x1b[0;31mfoo\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31m20\x1b[0m \
(\x1b[0;90m->\x1b[0m \x1b[0;31mv20.11.1\x1b[0m)\n";

#[test]
fn unexported_color_settings_reach_alias() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let commands = in_dialect(name, ALIAS_UNEXPORTED, ALIAS_UNEXPORTED_FISH);
        let run = lab.run(shell, &with_function(name, commands));
        assert_eq!(run.stdout, ALIAS_IN_RED, "{name}: {}", run.stderr);
    });
}

#[test]
fn an_unexported_color_setting_reaches_ls() {
    let lab = Lab::new();
    let ls = "nvm ls >/dev/null";
    let invalid = format!("NVM_COLORS=rgbzm\n{ls}");
    let invalid_fish = format!("set NVM_COLORS rgbzm\n{ls}");
    each_shell(|name, shell| {
        let cases = [
            (ls, ""),
            (
                in_dialect(name, &invalid, &invalid_fish),
                "Invalid color code: z\n",
            ),
        ];
        for (commands, expected) in cases {
            let run = lab.run(shell, &with_function(name, commands));
            assert_eq!(run.stderr, expected, "{name}: {commands}");
        }
    });
}

#[test]
fn unset_color_settings_stay_unset_for_children() {
    let lab = Lab::new();
    let child = "echo \"${NVM_COLORS-unset} ${NVM_HAS_COLORS-unset} ${NVM_NO_COLORS-unset}\"";
    let commands = format!("nvm exec --silent 18 /bin/sh -c '{child}'");
    each_shell(|name, shell| {
        let run = lab.run(shell, &with_function(name, &commands));
        assert_eq!(run.stdout, "unset unset unset\n", "{name}: {}", run.stderr);
    });
}
