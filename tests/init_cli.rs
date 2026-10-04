//! End-to-end: the function `nvm init <shell>` prints, evaluated by the real
//! shells (each one skipped when it is not installed), against a temporary
//! `$NVM_DIR` and the built binary on `PATH` as `nvm`.
#![cfg(unix)]

mod init_lab;

use init_lab::{Lab, USING_18, each_shell, find_shell, in_dialect, init_line, with_function};

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
        let commands = in_dialect(
            name,
            "nvm use 99\necho \"status=$?\"\necho \"PATH=$PATH\"",
            "nvm use 99\necho \"status=$status\"\necho \"PATH=$PATH\"",
        );
        let run = lab.run(shell, &with_function(name, commands));
        let tail = format!("status=3\nPATH={}\n", lab.base_path());
        assert!(run.stdout.ends_with(&tail), "{name}: {}", run.stdout);
        let quiet = lab.run(
            shell,
            &with_function(
                name,
                in_dialect(
                    name,
                    "nvm use 99 2>/dev/null >/dev/null; echo $?",
                    "nvm use 99 2>/dev/null >/dev/null; echo $status",
                ),
            ),
        );
        assert_eq!(
            (quiet.stdout.as_str(), quiet.stderr.as_str()),
            ("3\n", ""),
            "{name}"
        );
    });
}

/// What `deactivate` leaves behind: `NVM_BIN` and the temporaries of the
/// function, each printed as `gone` when it is not set.
const LEFT_BEHIND: &str = "echo \"left: ${NVM_BIN-gone} ${__nvmrc_code-gone} \
${__nvmrc_status-gone} ${NVMRC_SCRIPT_FD-gone} ${NVMRC_SHELL_KIND-gone}\"";
const LEFT_BEHIND_FISH: &str = "echo left: (for name in NVM_BIN nvmrc_code nvmrc_status \
NVMRC_SCRIPT_FD NVMRC_SHELL_KIND; set -q $name; and echo $name; or echo gone; end)";

#[test]
fn deactivate_undoes_use_and_leaves_no_temporaries() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let commands = format!(
            "nvm use 18 >/dev/null\nnvm deactivate\necho \"PATH=$PATH\"\n{}",
            in_dialect(name, LEFT_BEHIND, LEFT_BEHIND_FISH)
        );
        let run = lab.run(shell, &with_function(name, &commands));
        let removed = format!("{}/*/bin removed from ${{PATH}}\n", lab.at("nvm"));
        let tail = format!("PATH={}\nleft: gone gone gone gone gone\n", lab.base_path());
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

/// fish has no `set -eu`; its aliases are functions, and `command` skips them.
const ALIASED_FISH: &str = "alias nvm false\ncommand nvm init fish --no-use | source\n\
nvm use 18 >/dev/null\necho \"$NVM_BIN\"\nnvm bogus 2>/dev/null; or echo \"bogus=$status\"";

#[test]
fn the_function_survives_set_u_set_e_and_an_alias_named_nvm() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let posix = format!(
            "set -eu\nshopt -s expand_aliases 2>/dev/null || true\nalias nvm=false\n\
eval \"$(\\nvm init {name} --no-use)\"\nnvm use 18 >/dev/null\necho \"$NVM_BIN\"\n\
nvm bogus 2>/dev/null || echo \"bogus=$?\""
        );
        let run = lab.run(shell, in_dialect(name, &posix, ALIASED_FISH));
        let expected = format!("{}\nbogus=127\n", lab.version_bin("v18.20.4"));
        assert_eq!(
            (run.status, run.stdout.as_str()),
            (0, expected.as_str()),
            "{name}: {}",
            run.stderr
        );
    });
}

fn init_with(name: &str, options: &str) -> String {
    let status = in_dialect(name, "$?", "$status");
    format!(
        "{}\necho \"status={status}\"\nnvm current",
        init_line(name, options)
    )
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

#[test]
fn install_activates_the_version_in_the_shell() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let status = in_dialect(name, "$?", "$status");
        let commands = format!(
            "nvm install --offline 18 >/dev/null 2>&1\necho \"status={status}\"\necho \"BIN=$NVM_BIN\""
        );
        let run = lab.run(shell, &with_function(name, &commands));
        let expected = format!("status=0\nBIN={}\n", lab.version_bin("v18.20.4"));
        assert_eq!(run.stdout, expected, "{name}: {}", run.stderr);
    });
}

const SPACED: &str = "PATH=\"$PATH:/sp ace\"\nMANPATH='/m a::/n:'\n";
const SPACED_FISH: &str = "set PATH $PATH '/sp ace'\nset MANPATH '/m a::/n:'\n";

#[test]
fn path_and_manpath_keep_spaces_empty_entries_and_the_trailing_colon() {
    let lab = Lab::new().program("path/manpath", "echo /usr/share/man");
    let bin = lab.version_bin("v18.20.4");
    let man = lab.at("nvm/versions/node/v18.20.4/share/man");
    each_shell(|name, shell| {
        let commands = format!(
            "{}nvm use 18 >/dev/null\necho \"PATH=$PATH\"\necho \"MANPATH=$MANPATH\"",
            in_dialect(name, SPACED, SPACED_FISH)
        );
        let run = lab.run(shell, &with_function(name, &commands));
        let expected = format!(
            "PATH={bin}:{}:/sp ace\nMANPATH={man}:/m a::/n:\n",
            lab.base_path()
        );
        assert_eq!(run.stdout, expected, "{name}: {}", run.stderr);
    });
}

#[test]
fn standalone_code_for_fish_evaluates_in_fish() {
    let Some(fish) = find_shell("fish") else {
        eprintln!("skipped: fish is not installed");
        return;
    };
    let lab = Lab::new();
    let commands = "NVMRC_SHELL_KIND=fish nvm use 18 2>/dev/null | source\n\
echo \"BIN=$NVM_BIN\"\necho \"PATH=$PATH\"";
    let run = lab.run(&fish, commands);
    let bin = lab.version_bin("v18.20.4");
    let expected = format!("BIN={bin}\nPATH={bin}:{}\n", lab.base_path());
    assert_eq!(run.stdout, expected, "{}", run.stderr);
}

const SET_COLORS_PLAIN: &str = "Setting colors to: r g b c m\n\
WARNING: Colors may not display because they are not supported in this shell.\n";

#[test]
fn set_colors_keeps_nvm_colors_in_the_shell() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let commands = "nvm set-colors rgbcm\n/usr/bin/printenv NVM_COLORS\necho \"$NVM_COLORS\"";
        let run = lab.run(shell, &with_function(name, commands));
        let expected = format!("{SET_COLORS_PLAIN}rgbcm\nrgbcm\n");
        assert_eq!(
            (run.status, run.stdout.as_str(), run.stderr.as_str()),
            (0, expected.as_str(), ""),
            "{name}"
        );
    });
}

#[test]
fn an_invalid_set_colors_leaves_nvm_colors_unchanged() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let commands = format!(
            "nvm set-colors rgbcm >/dev/null\nnvm set-colors rgbc0 2>/dev/null\necho \"status={}\"\n\
/usr/bin/printenv NVM_COLORS",
            in_dialect(name, "$?", "$status")
        );
        let run = lab.run(shell, &with_function(name, &commands));
        assert_eq!(run.stdout, "\nstatus=0\nrgbcm\n", "{name}: {}", run.stderr);
    });
}
