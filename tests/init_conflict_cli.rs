//! End-to-end: the init snippet warns, in an interactive shell, when nvm.sh
//! is still loaded (its `nvm_has` helper is defined, or `nvm` already is a
//! function that is not nvmrc's), and stays silent otherwise. Real shells
//! (each one skipped when missing), started with `-i` without a terminal;
//! nvm.sh is faked by the functions it would leave behind, never sourced.
#![cfg(unix)]

mod init_lab;

use init_lab::{Lab, Run, each_shell, in_dialect, load_line};

const ADVICE: &str = "  nvmrc now answers `nvm`, but the old loader keeps running on every \
shell start;\n  run `nvm doctor` to see where, and `nvm migrate` to move it\n";
const HELPERS: &str =
    "nvm: nvm.sh is still loaded in this shell (its helper functions are defined)\n";
const FUNCTION: &str = "nvm: nvm.sh is still loaded in this shell (nvm was already a function)\n";

/// What nvm.sh leaves behind: its helpers, and its `nvm` function.
fn fake_nvm_sh(name: &str) -> &'static str {
    in_dialect(
        name,
        "nvm_has() { :; }\nnvm() { :; }\n",
        "function nvm_has; end\nfunction nvm; end\n",
    )
}

/// A lazy loader's stub: `nvm` is a function, the helpers are absent.
fn lazy_stub(name: &str) -> &'static str {
    in_dialect(name, "nvm() { :; }\n", "function nvm; end\n")
}

/// `before`, the init (with the automatic `use`), then `nvm use 18`.
fn init_then_use(name: &str, before: &str) -> String {
    format!(
        "{before}{}\nnvm use 18 >/dev/null\necho \"BIN=$NVM_BIN\"",
        load_line(name)
    )
}

fn assert_used_18(lab: &Lab, name: &str, run: &Run) {
    let expected = format!("BIN={}\n", lab.version_bin("v18.20.4"));
    assert_eq!(
        (run.status, run.stdout.as_str()),
        (0, expected.as_str()),
        "{name}: {}",
        run.stderr
    );
}

fn assert_silent(name: &str, run: &Run) {
    assert!(!run.stderr.contains("nvm:"), "{name}: {}", run.stderr);
}

#[test]
fn nvm_sh_helpers_loaded_before_the_init_are_reported_once_on_stderr() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let run = lab.run_interactive(shell, &init_then_use(name, fake_nvm_sh(name)));
        let warning = format!("{HELPERS}{ADVICE}");
        assert!(run.stderr.contains(&warning), "{name}: {}", run.stderr);
        assert_eq!(
            run.stderr.matches("nvm:").count(),
            1,
            "{name}: {}",
            run.stderr
        );
        assert_used_18(&lab, name, &run);
    });
}

#[test]
fn a_lazy_nvm_stub_defined_before_the_init_is_reported() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let run = lab.run_interactive(shell, &init_then_use(name, lazy_stub(name)));
        let warning = format!("{FUNCTION}{ADVICE}");
        assert!(run.stderr.contains(&warning), "{name}: {}", run.stderr);
        assert_eq!(
            run.stderr.matches("nvm:").count(),
            1,
            "{name}: {}",
            run.stderr
        );
        assert_used_18(&lab, name, &run);
    });
}

#[test]
fn a_second_init_and_a_clean_shell_stay_silent() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        let clean = lab.run_interactive(shell, &init_then_use(name, ""));
        assert_silent(name, &clean);
        assert_used_18(&lab, name, &clean);
        let twice = format!("{}\n", load_line(name));
        let again = lab.run_interactive(shell, &init_then_use(name, &twice));
        assert_silent(name, &again);
        assert_used_18(&lab, name, &again);
    });
}

#[test]
fn a_non_interactive_shell_never_warns() {
    let lab = Lab::new();
    each_shell(|name, shell| {
        for before in [fake_nvm_sh(name), lazy_stub(name)] {
            let run = lab.run(shell, &init_then_use(name, before));
            assert_eq!(run.stderr, "", "{name}");
            assert_used_18(&lab, name, &run);
        }
    });
}

#[test]
fn the_warning_keeps_set_eu_and_the_status_of_the_automatic_use() {
    let lab = Lab::new().file("proj/.nvmrc", "99\n");
    each_shell(|name, shell| {
        let posix = format!(
            "set -eu\nnvm() {{ :; }}\n{} || echo \"status=$?\"\necho done",
            load_line(name)
        );
        let fish = format!(
            "function nvm; end\n{}\necho \"status=$status\"\necho done",
            load_line(name)
        );
        let run = lab.run_interactive(shell, in_dialect(name, &posix, &fish));
        assert!(run.stderr.contains(FUNCTION), "{name}: {}", run.stderr);
        assert_eq!(run.stdout, "status=3\ndone\n", "{name}: {}", run.stderr);
    });
}

#[test]
fn conflict_prints_on_stderr_only_and_rejects_an_unknown_reason() {
    let run = |reason: &str| {
        std::process::Command::new(init_lab::BINARY)
            .args(["__conflict", reason])
            .output()
            .unwrap()
    };
    let helpers = run("helpers");
    assert_eq!(helpers.status.code(), Some(0));
    assert_eq!(helpers.stdout, b"");
    assert_eq!(
        String::from_utf8_lossy(&helpers.stderr),
        format!("{HELPERS}{ADVICE}")
    );
    let unknown = run("nvm.sh");
    assert_eq!(unknown.status.code(), Some(127));
    assert_eq!(unknown.stdout, b"");
}
