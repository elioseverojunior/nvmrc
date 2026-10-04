//! `nvm ls` with its alias section, against the golden captures of the
//! digest (4.1, 4.2, 4.3): the same colors as `nvm alias`, one warning.

use super::*;
use crate::commands::aliases::fixture::{GOLDEN, PLAIN, RGBCM, Setup, fixture};

fn ls(setup: &Setup, args: &[&str]) -> Output {
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    setup
        .run(&fixture(), |context| run_command(context, &args))
        .unwrap()
}

fn joined(versions: &[&str], aliases: &[&str]) -> String {
    [versions, aliases].concat().join("\n")
}

#[test]
fn a_terminal_gets_the_golden_listing() {
    let versions = [
        "\x1b[0;34m    iojs-v3.3.1\x1b[0m",
        "\x1b[0;34m       v18.20.4\x1b[0m",
        "\x1b[0;32m->     v20.11.1\x1b[0m",
        "\x1b[0;34m        v22.3.0\x1b[0m",
        "\x1b[0;33m         system\x1b[0m (\x1b[0;33m-> v16.0.0\x1b[0m)",
    ];
    let expected = Output::stdout(joined(&versions, &GOLDEN));
    assert_eq!(ls(&Setup::tty(), &[]), expected);
}

#[test]
fn a_pipe_or_no_colors_gives_the_plain_listing() {
    let versions = [
        "    iojs-v3.3.1 *",
        "       v18.20.4 *",
        "->     v20.11.1 *",
        "        v22.3.0 *",
        "         system * (-> v16.0.0)",
    ];
    let expected = Output::stdout(joined(&versions, &PLAIN));
    assert_eq!(ls(&Setup::pipe(), &[]), expected);
    assert_eq!(ls(&Setup::tty(), &["--no-colors"]), expected);
    let forced = Setup::pipe().var("NVM_HAS_COLORS", "1");
    assert_eq!(ls(&forced, &[]), expected);
}

#[test]
fn nvm_colors_colors_both_parts() {
    let versions = [
        "\x1b[0;31m    iojs-v3.3.1\x1b[0m",
        "\x1b[0;31m       v18.20.4\x1b[0m",
        "\x1b[0;34m->     v20.11.1\x1b[0m",
        "\x1b[0;31m        v22.3.0\x1b[0m",
        "\x1b[0;32m         system\x1b[0m (\x1b[0;32m-> v16.0.0\x1b[0m)",
    ];
    let output = ls(&Setup::tty().var("NVM_COLORS", "rgbcm"), &[]);
    assert_eq!(output, Output::stdout(joined(&versions, &RGBCM)));
}

#[test]
fn an_invalid_setting_warns_once_for_the_whole_command() {
    let output = ls(&Setup::tty().var("NVM_COLORS", "zzzzz"), &[]);
    assert_eq!(output.stderr, "Invalid color code: z");
    let lines: Vec<&str> = output.stdout.lines().collect();
    assert_eq!(lines[2], "       v20.11.1");
    assert_eq!(
        lines[5],
        "default \x1b[0;90m->\x1b[0m 18 (\x1b[0;90m->\x1b[0m v18.20.4)"
    );
    let plain = ls(&Setup::pipe().var("NVM_COLORS", "rg"), &[]);
    assert_eq!(plain.stderr, "Invalid color code: ");
}
