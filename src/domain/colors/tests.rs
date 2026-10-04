use super::*;

const TABLE: [(char, &str); 16] = [
    ('r', "0;31m"),
    ('R', "1;31m"),
    ('g', "0;32m"),
    ('G', "1;32m"),
    ('b', "0;34m"),
    ('B', "1;34m"),
    ('c', "0;36m"),
    ('C', "1;36m"),
    ('m', "0;35m"),
    ('M', "1;35m"),
    ('y', "0;33m"),
    ('Y', "1;33m"),
    ('k', "0;30m"),
    ('K', "1;30m"),
    ('e', "0;37m"),
    ('W', "1;37m"),
];

fn roles(palette: &Palette) -> Vec<Option<&str>> {
    [
        Role::Installed,
        Role::System,
        Role::Current,
        Role::NotInstalled,
        Role::Default,
    ]
    .into_iter()
    .map(|role| palette.code(role))
    .collect()
}

#[test]
fn every_letter_of_the_table_has_its_code() {
    for (letter, code) in TABLE {
        assert_eq!(sgr(letter), Some(code), "letter {letter}");
    }
}

#[test]
fn zero_and_unknown_letters_have_no_code() {
    for letter in ['0', 'x', 'E', 'w', ' ', 'z'] {
        assert_eq!(sgr(letter), None, "letter {letter}");
    }
}

#[test]
fn constants_are_the_exact_byte_sequences() {
    assert_eq!(RESET, "\x1b[0m");
    assert_eq!(ARROW, "\x1b[0;90m");
    assert_eq!(ITALIC_ON, "\x1b[3m");
    assert_eq!(ITALIC_OFF, "\x1b[23m");
}

#[test]
fn unset_and_empty_settings_use_bygre() {
    for setting in [None, Some("")] {
        let (palette, offending) = Palette::from_setting(setting);
        assert!(offending.is_empty());
        assert_eq!(
            roles(&palette),
            [
                Some("0;34m"),
                Some("0;33m"),
                Some("0;32m"),
                Some("0;31m"),
                Some("0;37m")
            ]
        );
    }
}

#[test]
fn custom_settings_map_by_position() {
    let (palette, offending) = Palette::from_setting(Some("rgbcm"));
    assert!(offending.is_empty());
    assert_eq!(
        roles(&palette),
        [
            Some("0;31m"),
            Some("0;32m"),
            Some("0;34m"),
            Some("0;36m"),
            Some("0;35m")
        ]
    );
    let (bold, _) = Palette::from_setting(Some("RGBCM"));
    assert_eq!(bold.code(Role::Installed), Some("1;31m"));
    assert_eq!(bold.code(Role::Default), Some("1;35m"));
}

#[test]
fn extra_characters_are_ignored() {
    let (palette, offending) = Palette::from_setting(Some("bygreXX"));
    assert!(offending.is_empty());
    assert_eq!(palette, Palette::from_setting(None).0);
}

#[test]
fn a_short_setting_leaves_missing_roles_without_color() {
    let (palette, offending) = Palette::from_setting(Some("rg"));
    assert_eq!(
        roles(&palette),
        [Some("0;31m"), Some("0;32m"), None, None, None]
    );
    assert_eq!(offending, [None, None, None]);
}

#[test]
fn invalid_letters_are_reported_and_uncolored() {
    let (palette, offending) = Palette::from_setting(Some("zzzzz"));
    assert_eq!(roles(&palette), [None; 5]);
    assert_eq!(offending, [Some('z'); 5]);
    assert_eq!(palette.lts(), None);
    assert_eq!(palette.latest_lts(), None);
    assert_eq!(palette.annotation(), None);
    assert_eq!(palette.old_lts(), None);
}

#[test]
fn zero_means_no_color_without_a_warning() {
    let (palette, offending) = Palette::from_setting(Some("b0gre"));
    assert_eq!(palette.code(Role::System), None);
    assert!(offending.is_empty());
}

#[test]
fn default_palette_derives_the_documented_colors() {
    let (palette, _) = Palette::from_setting(None);
    assert_eq!(palette.lts().as_deref(), Some("1;33m"));
    assert_eq!(palette.latest_lts().as_deref(), Some("1;32m"));
    assert_eq!(palette.annotation().as_deref(), Some("0;32m"));
    assert_eq!(palette.old_lts(), Some("0;37m"));
}

#[test]
fn black_lts_turns_bold_red_but_latest_lts_stays_black() {
    let (palette, _) = Palette::from_setting(Some("kKkKk"));
    assert_eq!(palette.lts().as_deref(), Some("1;31m"));
    assert_eq!(palette.latest_lts().as_deref(), Some("1;30m"));
    let (bold_black, _) = Palette::from_setting(Some("bKbbb"));
    assert_eq!(bold_black.lts().as_deref(), Some("1;31m"));
}

#[test]
fn bold_current_keeps_latest_lts_and_unbolds_the_annotation() {
    let (palette, _) = Palette::from_setting(Some("byGre"));
    assert_eq!(palette.latest_lts().as_deref(), Some("1;32m"));
    assert_eq!(palette.annotation().as_deref(), Some("0;32m"));
}

#[test]
fn wrap_surrounds_the_text_with_the_code_and_a_reset() {
    assert_eq!(wrap(Some("0;32m"), "v18"), "\x1b[0;32mv18\x1b[0m");
    assert_eq!(wrap(None, "v18"), "v18");
}

#[test]
fn set_colors_validation_accepts_exactly_five_settable_letters() {
    for setting in ["bygre", "rRgGb", "BcCyY", "mMkKe", "WWWWW", "rgbcm"] {
        assert!(is_valid_setting(setting), "{setting}");
    }
}

#[test]
fn set_colors_validation_rejects_zero_bad_letters_and_wrong_lengths() {
    for setting in [
        "b0gre", "00000", "bygrx", "byg", "bygrey", "", "bygr ", "bygrE",
    ] {
        assert!(!is_valid_setting(setting), "{setting:?}");
    }
}
