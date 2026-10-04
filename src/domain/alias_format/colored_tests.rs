//! The alias rows of the golden captures of the digest (4.1, 4.3, 4.4) and
//! the examples of `nvm alias <name> <target>` (3.5).

use super::colored::{AliasKind, AliasRow, VersionState, paint_line};
use crate::domain::colors::Palette;

fn palette(setting: &str) -> Palette {
    Palette::from_setting(Some(setting)).0
}

fn row<'a>(alias: &'a str, target: &'a str, version: &'a str, state: VersionState) -> AliasRow<'a> {
    AliasRow {
        alias,
        target,
        version,
        state,
        kind: AliasKind::File,
    }
}

fn implicit(row: AliasRow<'_>) -> AliasRow<'_> {
    AliasRow {
        kind: AliasKind::Implicit,
        ..row
    }
}

fn lts(row: AliasRow<'_>) -> AliasRow<'_> {
    AliasRow {
        kind: AliasKind::Lts,
        ..row
    }
}

fn on(row: &AliasRow<'_>, setting: &str) -> String {
    paint_line(row, &palette(setting), true)
}

fn off(row: &AliasRow<'_>) -> String {
    paint_line(row, &palette("bygre"), false)
}

const ARROW: &str = "\x1b[0;90m->\x1b[0m";

#[test]
fn the_current_version_colors_alias_target_and_version_green() {
    let myalias = row("myalias", "20", "v20.11.1", VersionState::Current);
    let expected = format!(
        "\x1b[0;32mmyalias\x1b[0m {ARROW} \x1b[0;32m20\x1b[0m ({ARROW} \x1b[0;32mv20.11.1\x1b[0m)"
    );
    assert_eq!(on(&myalias, "bygre"), expected);
}

#[test]
fn an_installed_version_colors_the_row_blue() {
    let default = row("default", "18", "v18.20.4", VersionState::Installed);
    let expected = format!(
        "\x1b[0;34mdefault\x1b[0m {ARROW} \x1b[0;34m18\x1b[0m ({ARROW} \x1b[0;34mv18.20.4\x1b[0m)"
    );
    assert_eq!(on(&default, "bygre"), expected);
}

#[test]
fn implicit_aliases_end_with_the_default_marker() {
    let unstable = implicit(row("unstable", "N/A", "N/A", VersionState::Missing));
    let expected = format!(
        "\x1b[0;31munstable\x1b[0m {ARROW} \x1b[0;31mN/A\x1b[0m \x1b[0;37m(default)\x1b[0m"
    );
    assert_eq!(on(&unstable, "bygre"), expected);
    let iojs = implicit(row(
        "iojs",
        "iojs-v3.3",
        "iojs-v3.3.1",
        VersionState::Installed,
    ));
    let expected = format!(
        "\x1b[0;34miojs\x1b[0m {ARROW} \x1b[0;34miojs-v3.3\x1b[0m ({ARROW} \x1b[0;34miojs-v3.3.1\x1b[0m) \x1b[0;37m(default)\x1b[0m"
    );
    assert_eq!(on(&iojs, "bygre"), expected);
}

#[test]
fn lts_names_and_lts_targets_take_the_lts_color_but_the_version_keeps_its_own() {
    let star = lts(row("lts/*", "lts/iron", "v20.11.1", VersionState::Current));
    let expected = format!(
        "\x1b[1;33mlts/*\x1b[0m {ARROW} \x1b[1;33mlts/iron\x1b[0m ({ARROW} \x1b[0;32mv20.11.1\x1b[0m)"
    );
    assert_eq!(on(&star, "bygre"), expected);
    let hydrogen = lts(row(
        "lts/hydrogen",
        "v18.20.4",
        "v18.20.4",
        VersionState::Installed,
    ));
    let expected = format!("\x1b[1;33mlts/hydrogen\x1b[0m {ARROW} \x1b[0;34mv18.20.4\x1b[0m");
    assert_eq!(on(&hydrogen, "bygre"), expected);
}

#[test]
fn the_roles_follow_the_setting() {
    let default = row("default", "18", "v18.20.4", VersionState::Installed);
    let expected = format!(
        "\x1b[0;31mdefault\x1b[0m {ARROW} \x1b[0;31m18\x1b[0m ({ARROW} \x1b[0;31mv18.20.4\x1b[0m)"
    );
    assert_eq!(on(&default, "rgbcm"), expected);
    let unstable = implicit(row("unstable", "N/A", "N/A", VersionState::Missing));
    let expected = format!(
        "\x1b[0;36munstable\x1b[0m {ARROW} \x1b[0;36mN/A\x1b[0m \x1b[0;35m(default)\x1b[0m"
    );
    assert_eq!(on(&unstable, "rgbcm"), expected);
    let iron = lts(row(
        "lts/iron",
        "v20.11.1",
        "v20.11.1",
        VersionState::Current,
    ));
    let expected = format!("\x1b[1;32mlts/iron\x1b[0m {ARROW} \x1b[0;34mv20.11.1\x1b[0m");
    assert_eq!(on(&iron, "rgbcm"), expected);
}

#[test]
fn black_lts_turns_bold_red_like_the_tr_quirk() {
    let iron = lts(row("lts/iron", "v20.11.1", "v20.11.1", VersionState::Other));
    assert_eq!(
        on(&iron, "kKkKk"),
        format!("\x1b[1;31mlts/iron\x1b[0m {ARROW} v20.11.1")
    );
    let node = implicit(row("node", "stable", "v22.3.0", VersionState::Other));
    assert!(on(&node, "kKkKk").ends_with(" \x1b[0;30m(default)\x1b[0m"));
}

#[test]
fn the_examples_of_creating_an_alias() {
    let unknown = row("bar", "99", "N/A", VersionState::Missing);
    let expected =
        format!("\x1b[0;31mbar\x1b[0m {ARROW} \x1b[0;31m99\x1b[0m ({ARROW} \x1b[0;31mN/A\x1b[0m)");
    assert_eq!(on(&unknown, "bygre"), expected);
    assert_eq!(off(&unknown), "bar -> 99 (-> N/A)");
    let system = row("sys", "system", "system", VersionState::Other);
    assert_eq!(on(&system, "bygre"), format!("sys {ARROW} system"));
    assert_eq!(off(&system), "sys -> system *");
    let baz = row("baz", "lts/iron", "v20.11.1", VersionState::Current);
    let expected = format!(
        "\x1b[0;32mbaz\x1b[0m {ARROW} \x1b[1;33mlts/iron\x1b[0m ({ARROW} \x1b[0;32mv20.11.1\x1b[0m)"
    );
    assert_eq!(on(&baz, "bygre"), expected);
    let circular = row("a", "b", "∞", VersionState::Missing);
    let expected =
        format!("\x1b[0;31ma\x1b[0m {ARROW} \x1b[0;31mb\x1b[0m ({ARROW} \x1b[0;31m∞\x1b[0m)");
    assert_eq!(on(&circular, "bygre"), expected);
    assert_eq!(off(&circular), "a -> b (-> ∞)");
}

#[test]
fn colors_off_gives_the_plain_lines_with_stars() {
    let node = implicit(row("node", "stable", "v22.3.0", VersionState::Installed));
    assert_eq!(off(&node), "node -> stable (-> v22.3.0 *) (default)");
    let iron = lts(row(
        "lts/iron",
        "v20.11.1",
        "v20.11.1",
        VersionState::Current,
    ));
    assert_eq!(off(&iron), "lts/iron -> v20.11.1 *");
    let unstable = implicit(row("unstable", "N/A", "N/A", VersionState::Missing));
    assert_eq!(off(&unstable), "unstable -> N/A (default)");
}

#[test]
fn only_a_target_whose_parent_is_lts_is_lts_colored() {
    let bare = row("x", "lts", "N/A", VersionState::Missing);
    assert!(on(&bare, "bygre").contains(" \x1b[1;33mlts\x1b[0m ("));
    let deep = row("x", "lts/a/b", "N/A", VersionState::Missing);
    assert!(on(&deep, "bygre").contains(" \x1b[0;31mlts/a/b\x1b[0m ("));
}

#[test]
fn a_role_without_a_color_is_plain() {
    let myalias = row("myalias", "20", "v20.11.1", VersionState::Current);
    assert_eq!(
        on(&myalias, "rg"),
        format!("myalias {ARROW} 20 ({ARROW} v20.11.1)")
    );
    let node = implicit(row("node", "stable", "v22.3.0", VersionState::Installed));
    let expected = format!("node {ARROW} stable ({ARROW} v22.3.0) (default)");
    assert_eq!(on(&node, "zzzzz"), expected);
    let iron = lts(row(
        "lts/iron",
        "v20.11.1",
        "v20.11.1",
        VersionState::Installed,
    ));
    assert_eq!(
        on(&iron, "bzgre"),
        format!("lts/iron {ARROW} \x1b[0;34mv20.11.1\x1b[0m")
    );
}
