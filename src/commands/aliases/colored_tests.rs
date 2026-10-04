//! `nvm alias` (list) against the alias blocks of the golden captures of the
//! digest: 4.1 (pty, default colors), 4.2 (plain), 4.3 (`rgbcm`), 4.4.

use super::fixture::{GOLDEN, PLAIN, RGBCM, Setup, fixture};
use super::*;

#[test]
fn a_terminal_gets_the_golden_rows_sorted_on_their_color_codes() {
    let output = Setup::tty().alias(&[]);
    assert_eq!(output, Output::stdout(GOLDEN.join("\n")));
}

#[test]
fn a_pipe_or_no_colors_anywhere_gives_the_plain_rows_sorted_by_name() {
    let expected = Output::stdout(PLAIN.join("\n"));
    assert_eq!(Setup::pipe().alias(&[]), expected);
    assert_eq!(Setup::tty().alias(&["--no-colors"]), expected);
    assert_eq!(Setup::tty().alias(&["--", "--no-colors"]), expected);
}

#[test]
fn nvm_colors_picks_the_role_colors() {
    let output = Setup::tty().var("NVM_COLORS", "rgbcm").alias(&[]);
    assert_eq!(output, Output::stdout(RGBCM.join("\n")));
}

#[test]
fn nvm_no_colors_and_nvm_has_colors_in_the_environment_do_not_change_the_list() {
    let shadowed = Setup::tty().var("NVM_NO_COLORS", "--no-colors").alias(&[]);
    assert_eq!(shadowed, Output::stdout(GOLDEN.join("\n")));
    let forced = Setup::pipe().var("NVM_HAS_COLORS", "1").alias(&[]);
    assert_eq!(forced, Output::stdout(PLAIN.join("\n")));
}

#[test]
fn a_current_io_js_colors_the_iojs_row_as_current() {
    let output = Setup::tty()
        .in_use("/n/versions/io.js/v3.3.1/bin")
        .alias(&["iojs"]);
    let expected = "\x1b[0;32miojs\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;32miojs-v3.3\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;32miojs-v3.3.1\x1b[0m) \x1b[0;37m(default)\x1b[0m";
    assert_eq!(output, Output::stdout(expected));
}

#[test]
fn black_turns_the_lts_names_bold_red() {
    let output = Setup::tty().var("NVM_COLORS", "kKkKk").alias(&[]);
    let lines: Vec<&str> = output.stdout.lines().collect();
    assert!(lines[2].ends_with(" \x1b[0;30m(default)\x1b[0m"));
    assert!(lines[8].starts_with("\x1b[1;31mlts/iron\x1b[0m "));
}

#[test]
fn a_prefix_is_colored_too() {
    let output = Setup::tty().alias(&["my"]);
    assert_eq!(output, Output::stdout(GOLDEN[0]));
}

#[test]
fn an_lts_name_prints_the_raw_target_without_color() {
    assert_eq!(
        Setup::tty().alias(&["lts/iron"]),
        Output::stdout("v20.11.1")
    );
}

#[test]
fn no_match_prints_nothing_not_even_the_invalid_color_warning() {
    let output = Setup::tty().var("NVM_COLORS", "zzzzz").alias(&["nope"]);
    assert_eq!(output, Output::default());
}

#[test]
fn an_invalid_setting_leaves_the_rows_plain_with_gray_arrows_and_warns_once() {
    let output = Setup::tty().var("NVM_COLORS", "zzzzz").alias(&["default"]);
    let row = "default \x1b[0;90m->\x1b[0m 18 (\x1b[0;90m->\x1b[0m v18.20.4)";
    let expected = Output::stdout(row).with_stderr("Invalid color code: z");
    assert_eq!(output, expected);
    let plain = Setup::pipe().var("NVM_COLORS", "zzzzz").alias(&[]);
    assert_eq!(plain.stdout, PLAIN.join("\n"));
    assert_eq!(plain.stderr, "Invalid color code: z");
}

#[test]
fn other_options_are_still_exit_55() {
    let error = Setup::tty()
        .run(&fixture(), |context| {
            run(context, &["--no-colors".to_owned(), "--x".to_owned()])
        })
        .unwrap_err();
    assert_eq!(error.to_string(), "Unsupported option \"--x\".");
    assert_eq!(error.exit_code(), NvmExitCode::UnsupportedOption);
}
