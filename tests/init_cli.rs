//! End-to-end: the function `nvm init <shell>` prints, evaluated by the real
//! shells (each one skipped when it is not installed), against a temporary
//! `$NVM_DIR` and the built binary on `PATH` as `nvm`.
#![cfg(unix)]

mod init_lab;

use init_lab::{Lab, USING_18, each_shell, with_function};

#[test]
fn use_switches_the_shell_and_says_so_on_stdout() {
    let lab = Lab::new();
    let bin = lab.version_bin("v18.20.4");
    each_shell(|name, shell| {
        let run = lab.run(
            shell,
            &with_function(
                name,
                "nvm use 18\necho \"PATH=$PATH\"\necho \"BIN=$NVM_BIN\"",
            ),
        );
        let expected = format!("{USING_18}PATH={bin}:{}\nBIN={bin}\n", lab.base_path());
        assert_eq!(
            (run.status, run.stdout.as_str()),
            (0, expected.as_str()),
            "{name}: {}",
            run.stderr
        );
        assert_eq!(run.stderr, "", "{name}");
    });
}

#[test]
fn a_silenced_use_still_switches() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let run = lab.run(
            shell,
            &with_function(name, "nvm use 18 >/dev/null\necho \"$NVM_BIN\""),
        );
        assert_eq!(
            run.stdout,
            format!("{}\n", lab.version_bin("v18.20.4")),
            "{name}: {}",
            run.stderr
        );
    });
}

#[test]
fn a_failing_use_returns_its_status_and_changes_nothing() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let commands = "nvm use 99\necho \"status=$?\"\necho \"PATH=$PATH\"";
        let run = lab.run(shell, &with_function(name, commands));
        let tail = format!("status=3\nPATH={}\n", lab.base_path());
        assert!(run.stdout.ends_with(&tail), "{name}: {}", run.stdout);
        let quiet = lab.run(
            shell,
            &with_function(name, "nvm use 99 2>/dev/null >/dev/null; echo $?"),
        );
        assert_eq!(
            (quiet.stdout.as_str(), quiet.stderr.as_str()),
            ("3\n", ""),
            "{name}"
        );
    });
}

#[test]
fn deactivate_undoes_use_and_leaves_no_temporaries() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let commands = "nvm use 18 >/dev/null\nnvm deactivate\n\
echo \"PATH=$PATH\"\necho \"BIN=${NVM_BIN-unset} ${__nvmrc_code-gone} ${__nvmrc_status-gone} ${NVMRC_SCRIPT_FD-none}\"";
        let run = lab.run(shell, &with_function(name, commands));
        let removed = format!("{}/*/bin removed from ${{PATH}}\n", lab.at("nvm"));
        let tail = format!("PATH={}\nBIN=unset gone gone none\n", lab.base_path());
        assert!(run.stdout.starts_with(&removed), "{name}: {}", run.stdout);
        assert!(run.stdout.ends_with(&tail), "{name}: {}", run.stdout);
    });
}

#[test]
fn other_commands_pass_straight_through() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let run = lab.run(
            shell,
            &with_function(name, "nvm use 20 >/dev/null\nnvm current\nnvm ls"),
        );
        assert!(
            run.stdout.starts_with("v20.11.1\n"),
            "{name}: {}",
            run.stdout
        );
        assert!(
            run.stdout.contains("->     v20.11.1"),
            "{name}: {}",
            run.stdout
        );
        assert_eq!(run.status, 0, "{name}: {}", run.stderr);
    });
}

#[test]
fn the_function_survives_set_u_set_e_and_an_alias_named_nvm() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let commands = format!(
            "set -eu\nshopt -s expand_aliases 2>/dev/null || true\nalias nvm=false\n\
eval \"$(\\nvm init {name} --no-use)\"\nnvm use 18 >/dev/null\necho \"$NVM_BIN\"\n\
nvm bogus 2>/dev/null || echo \"bogus=$?\""
        );
        let run = lab.run(shell, &commands);
        let expected = format!("{}\nbogus=127\n", lab.version_bin("v18.20.4"));
        assert_eq!(
            (run.status, run.stdout.as_str()),
            (0, expected.as_str()),
            "{name}: {}",
            run.stderr
        );
    });
}

const AUTO: &str = "echo \"status=$?\"\nnvm current";

fn init_with(name: &str, options: &str) -> String {
    format!("eval \"$(nvm init {name}{options})\"\n{AUTO}")
}

#[test]
fn init_applies_the_default_alias_silently() {
    let lab = Lab::new().file("nvm/alias/default", "18\n");
    each_shell(|name, shell| {
        let run = lab.run(shell, &init_with(name, ""));
        assert_eq!(run.stdout, "status=0\nv18.20.4\n", "{name}: {}", run.stderr);
        let skipped = lab.run(shell, &init_with(name, " --no-use"));
        assert_eq!(skipped.stdout, "status=0\nnone\n", "{name}");
    });
}

#[test]
fn init_applies_the_nvmrc_version_without_a_default() {
    let lab = Lab::new().file("proj/.nvmrc", "20\n");
    each_shell(|name, shell| {
        let run = lab.run(shell, &init_with(name, ""));
        assert_eq!(run.stdout, "status=0\nv20.11.1\n", "{name}: {}", run.stderr);
    });
}

#[test]
fn an_init_whose_nvmrc_names_a_missing_version_has_status_3() {
    let lab = Lab::new().file("proj/.nvmrc", "99\n");
    each_shell(|name, shell| {
        let run = lab.run(shell, &init_with(name, ""));
        assert_eq!(run.stdout, "status=3\nnone\n", "{name}: {}", run.stderr);
    });
}
