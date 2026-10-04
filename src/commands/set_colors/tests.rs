use super::*;
use crate::error::NvmExitCode;
use crate::fakes::{FakeEnv, FakeFileSystem, FakeProcess, FakeTerminal};

const TPUT: &str = "/usr/bin/tput";
const COLORED: &str = "Setting colors to: \x1b[0;31mr\x1b[0m\x1b[0;32mg\x1b[0m\
\x1b[0;34mb\x1b[0m\x1b[0;36mc\x1b[0m\x1b[0;35mm\x1b[0m";
const PLAIN: &str = "Setting colors to: r g b c m\n\
WARNING: Colors may not display because they are not supported in this shell.";
const INVALID: &str = "\x1b[1;37mPlease pass in five \x1b[1;31mvalid color codes\x1b[1;37m. \
Choose from: rRgGbBcCyYmMkKeW\x1b[0m";

/// A machine whose stdout is `terminal`, with a `tput` reporting 256 colors
/// for `xterm-256color`, and `variables` in the environment.
fn set_colors(terminal: &FakeTerminal, variables: &[(&str, &str)], args: &[&str]) -> Output {
    let fs = FakeFileSystem::default().with_file(TPUT, "");
    let env = variables.iter().fold(
        FakeEnv::default()
            .with_var("PATH", "/usr/bin")
            .with_var("TERM", "xterm-256color"),
        |env, (name, value)| env.with_var(name, value),
    );
    let process = FakeProcess::default().with_run(TPUT, "-T xterm-256color colors", true, "256\n");
    let context = Context::new(&fs, &env)
        .with_terminal(terminal)
        .with_process(&process);
    let args: Vec<String> = args.iter().map(|&argument| argument.to_owned()).collect();
    run(&context, &args).unwrap()
}

fn on_a_terminal(args: &[&str]) -> Output {
    set_colors(&FakeTerminal::terminal(), &[], args)
}

fn exported(setting: &str) -> String {
    format!("export NVM_COLORS='{setting}'\n")
}

#[test]
fn a_valid_setting_on_a_terminal_shows_each_letter_in_its_own_color() {
    let output = on_a_terminal(&["rgbcm"]);
    assert_eq!(output.stdout, COLORED);
    assert_eq!(output.stderr, "");
    assert_eq!(output.status, NvmExitCode::Success);
    assert!(!output.blank_stdout);
}

#[test]
fn a_valid_setting_exports_nvm_colors_for_the_shell() {
    assert_eq!(on_a_terminal(&["rgbcm"]).script.render(), exported("rgbcm"));
    let pipe = set_colors(&FakeTerminal::pipe(), &[], &["RGBCM"]);
    assert_eq!(pipe.script.render(), exported("RGBCM"));
}

#[test]
fn bold_letters_and_the_whites_take_their_own_codes() {
    let output = on_a_terminal(&["YkKeW"]);
    let expected = "Setting colors to: \x1b[1;33mY\x1b[0m\x1b[0;30mk\x1b[0m\
\x1b[1;30mK\x1b[0m\x1b[0;37me\x1b[0m\x1b[1;37mW\x1b[0m";
    assert_eq!(output.stdout, expected);
}

#[test]
fn without_colors_the_letters_are_spaced_and_a_warning_follows() {
    let output = set_colors(&FakeTerminal::pipe(), &[], &["rgbcm"]);
    assert_eq!(output.stdout, PLAIN);
    assert_eq!(output.stderr, "");
    assert_eq!(output.status, NvmExitCode::Success);
}

#[test]
fn nvm_no_colors_set_to_the_flag_turns_the_colors_off() {
    let output = set_colors(
        &FakeTerminal::terminal(),
        &[("NVM_NO_COLORS", "--no-colors")],
        &["rgbcm"],
    );
    assert_eq!(output.stdout, PLAIN);
    assert_eq!(output.script.render(), exported("rgbcm"));
}

#[test]
fn other_values_of_nvm_no_colors_are_ignored() {
    for value in ["1", "", "--no-color", "true"] {
        let output = set_colors(
            &FakeTerminal::terminal(),
            &[("NVM_NO_COLORS", value)],
            &["rgbcm"],
        );
        assert_eq!(output.stdout, COLORED, "{value:?}");
    }
}

#[test]
fn nvm_has_colors_does_not_force_the_colors() {
    let output = set_colors(
        &FakeTerminal::pipe(),
        &[("NVM_HAS_COLORS", "1")],
        &["rgbcm"],
    );
    assert_eq!(output.stdout, PLAIN);
}

#[test]
fn arguments_after_the_first_are_ignored_as_in_nvm_sh() {
    let output = on_a_terminal(&["rgbcm", "zzz"]);
    assert_eq!(output.stdout, COLORED);
    assert_eq!(output.script.render(), exported("rgbcm"));
}

fn assert_invalid(output: &Output, shape: &str) {
    assert_eq!(output.stdout, "", "{shape}");
    assert!(output.blank_stdout, "{shape}");
    assert_eq!(output.stderr, INVALID, "{shape}");
    assert_eq!(output.status, NvmExitCode::Success, "{shape}");
    assert!(output.script.is_empty(), "{shape}");
}

#[test]
fn a_missing_setting_is_invalid_with_status_0_and_no_shell_code() {
    assert_invalid(&on_a_terminal(&[]), "missing");
}

#[test]
fn every_invalid_shape_prints_the_forced_color_error_and_exports_nothing() {
    let shapes: [&[&str]; 9] = [
        &[""],
        &["rgbc"],
        &["rgbcmy"],
        &["rgbc0"],
        &["00000"],
        &["rgbcz"],
        &["rgbcé"],
        &["--no-colors"],
        &["x", "rgbcm"],
    ];
    for shape in shapes {
        let label = format!("{shape:?}");
        assert_invalid(&on_a_terminal(shape), &label);
        assert_invalid(&set_colors(&FakeTerminal::pipe(), &[], shape), &label);
    }
}

#[test]
fn the_error_is_colored_even_when_colors_are_off() {
    let output = set_colors(
        &FakeTerminal::pipe(),
        &[("NVM_NO_COLORS", "--no-colors")],
        &["nope"],
    );
    assert_invalid(&output, "nope");
}
