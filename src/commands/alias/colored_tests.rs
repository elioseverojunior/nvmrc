//! `nvm alias <name> <target>` against the examples of the digest (3.5, 4.1,
//! 4.3, 4.4): one colored row, the alias name never in the LTS color.

use crate::commands::Output;
use crate::commands::aliases::fixture::{GOLDEN, RGBCM, Setup, fixture};

const WARNING: &str = "! WARNING: Version '99' does not exist.";

#[test]
fn creating_myalias_prints_its_golden_row() {
    assert_eq!(
        Setup::tty().alias(&["myalias", "20"]),
        Output::stdout(GOLDEN[0])
    );
    let rgbcm = Setup::tty().var("NVM_COLORS", "rgbcm");
    assert_eq!(rgbcm.alias(&["myalias", "20"]), Output::stdout(RGBCM[1]));
}

#[test]
fn an_unknown_target_is_not_installed_and_warns() {
    let output = Setup::tty().alias(&["bar", "99"]);
    let row = "\x1b[0;31mbar\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31m99\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;31mN/A\x1b[0m)";
    assert_eq!(output, Output::stdout(row).with_stderr(WARNING));
    let plain = Setup::tty().alias(&["bar", "99", "--no-colors"]);
    assert_eq!(
        plain,
        Output::stdout("bar -> 99 (-> N/A)").with_stderr(WARNING)
    );
}

#[test]
fn a_system_target_not_in_use_is_plain_but_for_the_arrow() {
    let output = Setup::tty().alias(&["sys", "system"]);
    assert_eq!(output, Output::stdout("sys \x1b[0;90m->\x1b[0m system"));
    let plain = Setup::pipe().alias(&["sys", "system"]);
    assert_eq!(plain, Output::stdout("sys -> system *"));
}

#[test]
fn a_system_target_in_use_is_current() {
    let output = Setup::tty().in_use("/sys").alias(&["sys", "system"]);
    let row = "\x1b[0;32msys\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;32msystem\x1b[0m";
    assert_eq!(output, Output::stdout(row));
}

#[test]
fn an_lts_target_takes_the_lts_color_and_the_name_does_not() {
    let output = Setup::tty().alias(&["baz", "lts/iron"]);
    let row = "\x1b[0;32mbaz\x1b[0m \x1b[0;90m->\x1b[0m \x1b[1;33mlts/iron\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;32mv20.11.1\x1b[0m)";
    assert_eq!(output, Output::stdout(row));
}

#[test]
fn a_circular_target_is_infinite() {
    let fs = fixture()
        .with_file("/n/alias/a", "b\n")
        .with_file("/n/alias/b", "a\n");
    let output = Setup::tty()
        .run(&fs, |context| {
            super::run_with_colors(context, "a", "b", false)
        })
        .unwrap();
    let row = "\x1b[0;31ma\x1b[0m \x1b[0;90m->\x1b[0m \x1b[0;31mb\x1b[0m (\x1b[0;90m->\x1b[0m \x1b[0;31m∞\x1b[0m)";
    assert_eq!(output, Output::stdout(row));
}

#[test]
fn an_exported_nvm_has_colors_forces_the_colors() {
    let forced = Setup::pipe().var("NVM_HAS_COLORS", "1");
    assert_eq!(forced.alias(&["myalias", "20"]), Output::stdout(GOLDEN[0]));
    assert_eq!(
        forced.alias(&["--no-colors", "myalias", "20"]),
        Output::stdout(GOLDEN[0])
    );
    let other = Setup::pipe().var("NVM_HAS_COLORS", "0");
    assert_eq!(
        other.alias(&["myalias", "20"]),
        Output::stdout("myalias -> 20 (-> v20.11.1 *)")
    );
}

#[test]
fn an_invalid_setting_warns_once_after_the_missing_version_warning() {
    let output = Setup::tty()
        .var("NVM_COLORS", "zzzzz")
        .alias(&["bar", "99"]);
    let row = "bar \x1b[0;90m->\x1b[0m 99 (\x1b[0;90m->\x1b[0m N/A)";
    let stderr = format!("{WARNING}\nInvalid color code: z");
    assert_eq!(output, Output::stdout(row).with_stderr(stderr));
}
