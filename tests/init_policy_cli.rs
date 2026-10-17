//! End-to-end: the `nvm` function routes past leading policy flags, so
//! `nvm --min-node 18 use 20` still switches the calling shell.
#![cfg(unix)]

mod init_lab;

use init_lab::{Lab, each_shell, in_dialect, with_function};

const POLICY: &str = r#"{"schemaVersion":1,"tools":{"node":{"allow":">=20"}}}"#;

#[test]
fn use_after_leading_policy_flags_switches_the_shell() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let run = lab.run(
            shell,
            &with_function(name, "nvm --min-node 18 use 20 >/dev/null\nnode -v"),
        );
        assert_eq!(
            (run.status, run.stdout.as_str()),
            (0, "v20.11.1\n"),
            "{name}: {}",
            run.stderr
        );
    });
}

#[test]
fn use_after_a_joined_policy_flag_switches_the_shell() {
    let lab = Lab::new().file("proj/p.json", POLICY);
    each_shell(|name, shell| {
        let run = lab.run(
            shell,
            &with_function(name, "nvm --policy=p.json use 20 >/dev/null\nnode -v"),
        );
        assert_eq!(
            (run.status, run.stdout.as_str()),
            (0, "v20.11.1\n"),
            "{name}: {}",
            run.stderr
        );
    });
}

#[test]
fn a_disallowed_use_leaves_the_shell_alone() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let status = in_dialect(name, "echo $?", "echo $status");
        let commands = format!(
            "nvm use 18 >/dev/null\nnvm --min-node 22 use 20 >/dev/null 2>&1\n{status}\nnode -v"
        );
        let run = lab.run(shell, &with_function(name, &commands));
        assert_eq!(
            (run.status, run.stdout.as_str()),
            (0, "7\nv18.20.4\n"),
            "{name}: {}",
            run.stderr
        );
    });
}
