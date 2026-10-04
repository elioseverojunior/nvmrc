//! The colored rows, against the golden `nvm ls-remote` blocks of the digest
//! (mirror: node v22.3.0, v22.2.0, v20.11.1 Iron, v20.11.0 Iron, v18.20.4
//! Hydrogen, v4.0.0, v0.12.18, v0.11.16; io.js v3.3.1, v3.3.0; installed:
//! v18.20.4, v20.11.1 (current), v22.3.0, iojs-v3.3.1; aliases
//! `default -> 18`, `myalias -> 20`).

use super::*;
use crate::domain::colors::Palette;

fn row(version: &str, lts: Option<&str>, latest_lts: bool) -> RemoteRow {
    RemoteRow {
        version: version.parse().unwrap(),
        lts: lts.map(str::to_owned),
        latest_lts,
    }
}

fn mirror_rows() -> Vec<RemoteRow> {
    vec![
        row("v0.11.16", None, false),
        row("v0.12.18", None, false),
        row("iojs-v3.3.0", None, false),
        row("iojs-v3.3.1", None, false),
        row("v4.0.0", None, false),
        row("v18.20.4", Some("Hydrogen"), true),
        row("v20.11.0", Some("Iron"), false),
        row("v20.11.1", Some("Iron"), true),
        row("v22.2.0", None, false),
        row("v22.3.0", None, false),
    ]
}

fn lts_rows() -> Vec<RemoteRow> {
    vec![
        row("v18.20.4", Some("Hydrogen"), true),
        row("v20.11.0", Some("Iron"), false),
        row("v20.11.1", Some("Iron"), true),
    ]
}

fn installed() -> Vec<Version> {
    ["iojs-v3.3.1", "v18.20.4", "v20.11.1", "v22.3.0"]
        .iter()
        .map(|name| name.parse().unwrap())
        .collect()
}

fn aliases() -> Vec<(String, String)> {
    vec![
        ("v18".to_owned(), "default".to_owned()),
        ("v20".to_owned(), "myalias".to_owned()),
    ]
}

fn palette(setting: &str) -> Palette {
    Palette::from_setting(Some(setting)).0
}

/// The full listing (`latest_alias` and aliases) or a filtered one.
fn paint(rows: &[RemoteRow], full: bool, colors: Option<RemoteColors<'_>>) -> Vec<String> {
    let installed = installed();
    let named = if full { aliases() } else { Vec::new() };
    let input = FormatInput {
        rows,
        installed: &installed,
        current: "v20.11.1",
        latest_alias: full.then_some("node"),
        named_aliases: &named,
    };
    paint_remote_rows(&input, colors)
}

fn on(palette: &Palette, italics: bool) -> Option<RemoteColors<'_>> {
    Some(RemoteColors { palette, italics })
}

/// Removes every `\e[...m`, leaving what the terminal shows.
fn visible(line: &str) -> String {
    let mut shown = String::new();
    let mut rest = line;
    while let Some(start) = rest.find('\x1b') {
        shown.push_str(&rest[..start]);
        let end = rest[start..]
            .find('m')
            .map_or(rest.len(), |at| start + at + 1);
        rest = &rest[end..];
    }
    shown.push_str(rest);
    shown
}

const GOLDEN_COLORED: [&str; 10] = [
    "       v0.11.16",
    "       v0.12.18",
    "    iojs-v3.3.0",
    "\x1b[0;34m    iojs-v3.3.1\x1b[0m",
    "         v4.0.0",
    "\x1b[0;34m       v18.20.4\x1b[0m  \x1b[1;32m (Latest LTS: Hydrogen)\x1b[0m                   \x1b[0;32m (Aliases: \x1b[3mdefault\x1b[23m)\x1b[0m",
    "       v20.11.0  \x1b[0;37m (LTS: Iron)\x1b[0m",
    "\x1b[0;32m->     v20.11.1\x1b[0m  \x1b[1;32m (Latest LTS: Iron)\x1b[0m                       \x1b[0;32m (Aliases: \x1b[3mmyalias\x1b[23m)\x1b[0m",
    "        v22.2.0",
    "\x1b[0;34m        v22.3.0\x1b[0m                           \x1b[0;32m (Latest: \x1b[3mnode\x1b[23m)\x1b[0m",
];

const GOLDEN_PLAIN: [&str; 10] = [
    "       v0.11.16",
    "       v0.12.18",
    "    iojs-v3.3.0",
    "    iojs-v3.3.1 *",
    "         v4.0.0",
    "       v18.20.4 * (Latest LTS: Hydrogen)                    (Aliases: default)",
    "       v20.11.0   (LTS: Iron)",
    "->     v20.11.1 * (Latest LTS: Iron)                        (Aliases: myalias)",
    "        v22.2.0",
    "        v22.3.0 *                          (Latest: node)",
];

#[test]
fn the_default_palette_with_italics_is_the_golden_block() {
    let colors = palette("bygre");
    assert_eq!(
        paint(&mirror_rows(), true, on(&colors, true)),
        GOLDEN_COLORED
    );
}

#[test]
fn without_italics_the_names_are_not_wrapped() {
    let colors = palette("bygre");
    let expected: Vec<String> = GOLDEN_COLORED
        .iter()
        .map(|line| line.replace("\x1b[3m", "").replace("\x1b[23m", ""))
        .collect();
    assert_eq!(paint(&mirror_rows(), true, on(&colors, false)), expected);
}

#[test]
fn colors_off_is_the_plain_listing() {
    assert_eq!(paint(&mirror_rows(), true, None), GOLDEN_PLAIN);
    let rows = mirror_rows();
    let installed = installed();
    let named = aliases();
    let input = FormatInput {
        rows: &rows,
        installed: &installed,
        current: "v20.11.1",
        latest_alias: Some("node"),
        named_aliases: &named,
    };
    assert_eq!(format_remote_rows(&input), GOLDEN_PLAIN);
}

#[test]
fn the_visible_layout_is_the_same_with_and_without_colors() {
    let colors = palette("bygre");
    for italics in [true, false] {
        let shown: Vec<String> = paint(&mirror_rows(), true, on(&colors, italics))
            .iter()
            .map(|line| visible(line))
            .collect();
        let plain: Vec<String> = GOLDEN_PLAIN
            .iter()
            .map(|line| line.replacen(" *", "  ", 1).trim_end().to_owned())
            .collect();
        assert_eq!(shown, plain);
    }
}

#[test]
fn rgbcm_moves_every_role() {
    let colors = palette("rgbcm");
    assert_eq!(
        paint(&mirror_rows(), true, on(&colors, true)),
        [
            "       v0.11.16",
            "       v0.12.18",
            "    iojs-v3.3.0",
            "\x1b[0;31m    iojs-v3.3.1\x1b[0m",
            "         v4.0.0",
            "\x1b[0;31m       v18.20.4\x1b[0m  \x1b[1;34m (Latest LTS: Hydrogen)\x1b[0m                   \x1b[0;34m (Aliases: \x1b[3mdefault\x1b[23m)\x1b[0m",
            "       v20.11.0  \x1b[0;35m (LTS: Iron)\x1b[0m",
            "\x1b[0;34m->     v20.11.1\x1b[0m  \x1b[1;34m (Latest LTS: Iron)\x1b[0m                       \x1b[0;34m (Aliases: \x1b[3mmyalias\x1b[23m)\x1b[0m",
            "        v22.2.0",
            "\x1b[0;31m        v22.3.0\x1b[0m                           \x1b[0;34m (Latest: \x1b[3mnode\x1b[23m)\x1b[0m",
        ]
    );
}

#[test]
fn an_lts_listing_never_has_italics() {
    let colors = palette("bygre");
    assert_eq!(
        paint(&lts_rows(), false, on(&colors, true)),
        [
            "\x1b[0;34m       v18.20.4\x1b[0m  \x1b[1;32m (Latest LTS: Hydrogen)\x1b[0m",
            "       v20.11.0  \x1b[0;37m (LTS: Iron)\x1b[0m",
            "\x1b[0;32m->     v20.11.1\x1b[0m  \x1b[1;32m (Latest LTS: Iron)\x1b[0m",
        ]
    );
}

/// What nvm.sh printed with `NVM_COLORS=rg`: no current or default color, so
/// the current row loses its arrow, the LTS columns and the annotations are
/// plain, yet the alias names keep their italics.
#[test]
fn a_role_without_color_is_plain_and_keeps_the_layout() {
    let colors = palette("rg");
    assert_eq!(
        paint(&mirror_rows(), true, on(&colors, true)),
        [
            "       v0.11.16",
            "       v0.12.18",
            "    iojs-v3.3.0",
            "\x1b[0;31m    iojs-v3.3.1\x1b[0m",
            "         v4.0.0",
            "\x1b[0;31m       v18.20.4\x1b[0m   (Latest LTS: Hydrogen)                    (Aliases: \x1b[3mdefault\x1b[23m)",
            "       v20.11.0   (LTS: Iron)",
            "       v20.11.1   (Latest LTS: Iron)                        (Aliases: \x1b[3mmyalias\x1b[23m)",
            "        v22.2.0",
            "\x1b[0;31m        v22.3.0\x1b[0m                            (Latest: \x1b[3mnode\x1b[23m)",
        ]
    );
}

#[test]
fn several_aliases_on_a_release_are_each_italic_and_joined_by_commas() {
    let rows = vec![row("v18.20.4", Some("Hydrogen"), true)];
    let installed = installed();
    let named = vec![
        ("v18".to_owned(), "default".to_owned()),
        ("v18.20.4".to_owned(), "work".to_owned()),
    ];
    let input = FormatInput {
        rows: &rows,
        installed: &installed,
        current: "none",
        latest_alias: Some("node"),
        named_aliases: &named,
    };
    let colors = palette("bygre");
    assert_eq!(
        paint_remote_rows(&input, on(&colors, true)),
        [
            "\x1b[0;34m       v18.20.4\x1b[0m  \x1b[1;32m (Latest LTS: Hydrogen)\x1b[0m  \x1b[0;32m (Latest: \x1b[3mnode\x1b[23m)\x1b[0m  \x1b[0;32m (Aliases: \x1b[3mdefault\x1b[23m, \x1b[3mwork\x1b[23m)\x1b[0m"
        ]
    );
}
