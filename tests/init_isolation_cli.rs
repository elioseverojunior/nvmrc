//! End-to-end: what the `nvm` function hands the binary (descriptor 3,
//! `NVMRC_SCRIPT_FD`, unexported variables) stays between the two, and the
//! programs the binary starts never see the channel.
#![cfg(unix)]

mod init_lab;

use std::time::{Duration, Instant};

use init_lab::{Lab, each_shell, find_shell, with_function};

#[test]
fn a_program_nvm_starts_cannot_write_shell_code_for_the_caller() {
    let npm = "echo 'echo INJECTED-FROM-CHILD' >&3 2>/dev/null\necho 10.7.0";
    let lab = Lab::new().npm("v18.20.4", npm);
    each_shell(|name, shell| {
        let run = lab.run(
            shell,
            &with_function(name, "nvm use 18\necho \"status=$?\""),
        );
        assert!(!run.stdout.contains("INJECTED"), "{name}: {}", run.stdout);
        assert!(run.stdout.ends_with("status=0\n"), "{name}: {}", run.stdout);
    });
}

#[test]
fn a_daemon_left_by_a_program_does_not_hold_use_up() {
    let npm = "(/bin/sleep 4 >/dev/null 2>&1 &)\necho 10.7.0";
    let lab = Lab::new().npm("v18.20.4", npm);
    let shell = find_shell("sh").expect("sh is installed");
    let started = Instant::now();
    let run = lab.run(
        &shell,
        &with_function("sh", "nvm use 18 >/dev/null\necho \"$NVM_BIN\""),
    );
    let elapsed = started.elapsed();
    assert_eq!(run.stdout, format!("{}\n", lab.version_bin("v18.20.4")));
    assert!(elapsed < Duration::from_secs(3), "use took {elapsed:?}");
}

#[test]
fn exec_children_get_neither_the_descriptor_nor_its_variable() {
    let lab = Lab::new();
    let shell = find_shell("sh").expect("sh is installed");
    let child = "echo \"fd=${NVMRC_SCRIPT_FD-none}\"; echo leaked >&3";
    let commands = format!(
        "NVMRC_SCRIPT_FD=3 nvm exec --silent 18 /bin/sh -c '{child}' 3>channel 2>/dev/null \\\n\
&& echo \"write: ok\" || echo \"write: failed\"\nwhile IFS= read -r line; do echo \"channel: $line\"; done <channel"
    );
    let run = lab.run(&shell, &commands);
    // Only that the write failed: dash says 2 for a failed redirection, bash 1.
    assert_eq!(run.stdout, "fd=none\nwrite: failed\n", "{}", run.stderr);
}

#[test]
fn exec_links_current_with_an_unexported_symlink_setting() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let commands = "NVM_SYMLINK_CURRENT=true\nnvm exec 18 /bin/sh -c : >/dev/null\n\
[ -L \"$NVM_DIR/current\" ] && \"$NVM_DIR/current/bin/node\"";
        let run = lab.run(shell, &with_function(name, commands));
        assert_eq!(run.stdout, "v18.20.4\n", "{name}: {}", run.stderr);
        std::fs::remove_file(lab.at("nvm/current")).ok();
    });
}

#[test]
fn install_children_see_prefix_only_when_the_shell_has_it() {
    let npm = "echo \"prefix=${PREFIX-unset}\" >> \"$HOME/env\"\necho 10.7.0";
    let lab = Lab::new().npm("v18.20.4", npm);
    let install = "nvm install --offline 18 >/dev/null 2>&1";
    let exported = format!("export PREFIX=\"$NVM_DIR/versions/node/v18.20.4\"\n{install}");
    let directory = lab.at("nvm/versions/node/v18.20.4");
    let cases = [
        (install.to_owned(), "prefix=unset\n".to_owned()),
        (exported, format!("prefix={directory}\n")),
    ];
    each_shell(|name, shell| {
        for (commands, expected) in &cases {
            lab.run(shell, &with_function(name, commands));
            let seen = std::fs::read_to_string(lab.at("home/env")).unwrap_or_default();
            std::fs::remove_file(lab.at("home/env")).ok();
            assert_eq!(&seen, expected, "{name}: {commands}");
        }
    });
}
