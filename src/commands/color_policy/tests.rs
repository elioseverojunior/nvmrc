use super::*;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess, FakeTerminal};

const TPUT: &str = "/usr/bin/tput";

fn tput_process(term: &str, colors: &str) -> FakeProcess {
    FakeProcess::default().with_run(TPUT, &format!("-T {term} colors"), true, colors)
}

fn italic_process(term: &str, colors: &str) -> FakeProcess {
    tput_process(term, colors).with_run(TPUT, &format!("-T {term} sitm"), true, "\x1b[3m")
}

fn policy(
    env: &FakeEnv,
    terminal: &FakeTerminal,
    process: &FakeProcess,
    flag: bool,
    has_tput: bool,
) -> ColorPolicy {
    let fs = if has_tput {
        FakeFileSystem::default().with_file(TPUT, "")
    } else {
        FakeFileSystem::default()
    };
    let context = Context::new(&fs, env)
        .with_terminal(terminal)
        .with_process(process);
    detect(&context, flag)
}

fn env_with(term: Option<&str>) -> FakeEnv {
    let env = FakeEnv::default().with_var("PATH", "/usr/bin");
    match term {
        Some(term) => env.with_var("TERM", term),
        None => env,
    }
}

fn on(term: &str, colors: &str) -> bool {
    let env = env_with(Some(term));
    policy(
        &env,
        &FakeTerminal::terminal(),
        &tput_process(term, colors),
        false,
        true,
    )
    .enabled
}

#[test]
fn a_terminal_with_256_colors_is_on() {
    assert!(on("xterm-256color", "256\n"));
}

#[test]
fn a_terminal_with_exactly_8_colors_is_on() {
    assert!(on("xterm", "8\n"));
    assert!(!on("xterm", "7\n"));
}

#[test]
fn dumb_and_vt100_have_no_colors() {
    assert!(!on("dumb", "-1\n"));
    assert!(!on("vt100", "-1\n"));
}

#[test]
fn an_unset_or_empty_term_asks_for_vt100() {
    for term in [None, Some("")] {
        let env = env_with(term);
        let process = tput_process("vt100", "256\n");
        let off = tput_process("xterm", "256\n");
        let terminal = FakeTerminal::terminal();
        assert!(policy(&env, &terminal, &process, false, true).enabled);
        assert!(!policy(&env, &terminal, &off, false, true).enabled);
    }
}

#[test]
fn a_pipe_is_off_even_with_colors() {
    let env = env_with(Some("xterm"));
    let process = tput_process("xterm", "256");
    assert_eq!(
        policy(&env, &FakeTerminal::pipe(), &process, false, true),
        ColorPolicy::off()
    );
}

#[test]
fn the_no_colors_flag_turns_it_off() {
    let env = env_with(Some("xterm"));
    let process = italic_process("xterm", "256");
    let terminal = FakeTerminal::terminal();
    assert_eq!(
        policy(&env, &terminal, &process, true, true),
        ColorPolicy::off()
    );
}

#[test]
fn a_missing_tput_is_off() {
    let env = env_with(Some("xterm"));
    let process = italic_process("xterm", "256");
    let terminal = FakeTerminal::terminal();
    assert_eq!(
        policy(&env, &terminal, &process, false, false),
        ColorPolicy::off()
    );
}

#[test]
fn a_tput_that_cannot_run_or_fails_is_off() {
    let env = env_with(Some("xterm"));
    let terminal = FakeTerminal::terminal();
    let unrunnable = FakeProcess::default();
    let failing = FakeProcess::default().with_run(TPUT, "-T xterm colors", false, "256");
    assert!(!policy(&env, &terminal, &unrunnable, false, true).enabled);
    assert!(!policy(&env, &terminal, &failing, false, true).enabled);
}

#[test]
fn tput_garbage_is_off() {
    for garbage in [
        "",
        "\n",
        "many",
        "8 9",
        "8\n9",
        "0x10",
        "-1",
        "99999999999999999999",
    ] {
        assert!(!on("xterm", garbage), "{garbage:?}");
    }
}

#[test]
fn tput_numbers_are_read_like_test_ge_does() {
    for number in [" 256 \n", "08", "+8", "\t16\n"] {
        assert!(on("xterm", number), "{number:?}");
    }
}

#[test]
fn italics_need_sitm_to_succeed() {
    let env = env_with(Some("xterm"));
    let terminal = FakeTerminal::terminal();
    let with = policy(&env, &terminal, &italic_process("xterm", "8"), false, true);
    let without = policy(&env, &terminal, &tput_process("xterm", "8"), false, true);
    assert_eq!(
        with,
        ColorPolicy {
            enabled: true,
            italics: true
        }
    );
    assert_eq!(
        without,
        ColorPolicy {
            enabled: true,
            italics: false
        }
    );
    let failing = tput_process("xterm", "8").with_run(TPUT, "-T xterm sitm", false, "");
    assert!(!policy(&env, &terminal, &failing, false, true).italics);
}

#[test]
fn italics_are_off_when_colors_are_off() {
    let env = env_with(Some("xterm"));
    let process = italic_process("xterm", "256");
    assert!(!policy(&env, &FakeTerminal::pipe(), &process, false, true).italics);
    assert!(!policy(&env, &FakeTerminal::terminal(), &process, true, true).italics);
}

#[test]
fn no_color_variables_change_nothing() {
    let terminal = FakeTerminal::terminal();
    let process = italic_process("xterm", "256");
    let fs = FakeFileSystem::default().with_file(TPUT, "");
    for (name, value) in [
        ("NVM_NO_COLORS", "1"),
        ("NVM_NO_COLORS", "--no-colors"),
        ("NO_COLOR", "1"),
    ] {
        let env = env_with(Some("xterm")).with_var(name, value);
        let context = Context::new(&fs, &env)
            .with_terminal(&terminal)
            .with_process(&process);
        let expected = ColorPolicy {
            enabled: true,
            italics: true,
        };
        assert_eq!(detect(&context, false), expected, "{name}={value}");
    }
}

fn forced_policy(value: Option<&str>, terminal: &FakeTerminal, flag: bool) -> (bool, ColorPolicy) {
    let mut env = env_with(Some("dumb"));
    if let Some(value) = value {
        env = env.with_var("NVM_HAS_COLORS", value);
    }
    let fs = FakeFileSystem::default().with_file(TPUT, "");
    let process = FakeProcess::default().with_run(TPUT, "-T dumb colors", true, "-1\n");
    let context = Context::new(&fs, &env)
        .with_terminal(terminal)
        .with_process(&process);
    (forced(&context), detect_or_forced(&context, flag))
}

#[test]
fn has_colors_one_forces_on_a_pipe_and_over_the_flag() {
    let forced_on = ColorPolicy {
        enabled: true,
        italics: false,
    };
    assert_eq!(
        forced_policy(Some("1"), &FakeTerminal::pipe(), false),
        (true, forced_on)
    );
    assert_eq!(
        forced_policy(Some("1"), &FakeTerminal::terminal(), true),
        (true, forced_on)
    );
}

#[test]
fn has_colors_other_values_do_not_force() {
    for value in [None, Some("0"), Some(""), Some("true"), Some("11")] {
        let (is_forced, policy) = forced_policy(value, &FakeTerminal::pipe(), false);
        assert!(!is_forced, "{value:?}");
        assert_eq!(policy, ColorPolicy::off());
    }
}

#[test]
fn forcing_does_not_add_italics_that_detection_would_not() {
    let env = env_with(Some("xterm")).with_var("NVM_HAS_COLORS", "1");
    let fs = FakeFileSystem::default().with_file(TPUT, "");
    let process = italic_process("xterm", "256");
    let context = Context::new(&fs, &env).with_process(&process);
    let piped = detect_or_forced(&context, false);
    assert_eq!(
        piped,
        ColorPolicy {
            enabled: true,
            italics: false
        }
    );
    let terminal = FakeTerminal::terminal();
    let context = context.with_terminal(&terminal);
    let real = detect_or_forced(&context, false);
    assert_eq!(
        real,
        ColorPolicy {
            enabled: true,
            italics: true
        }
    );
}
