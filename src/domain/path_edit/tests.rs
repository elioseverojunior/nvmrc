use super::*;

const NVM_DIR: &str = "/home/me/.nvm";
const N18: &str = "/home/me/.nvm/versions/node/v18.20.4";

/// Expands the digest's shorthands: `V20`, `V1819`, `OLD`, `N18` and `$NVM_DIR`.
fn expand(template: &str) -> String {
    template
        .replace("V1819", "/home/me/.nvm/versions/node/v18.19.0")
        .replace("V20", "/home/me/.nvm/versions/node/v20.11.1")
        .replace("OLD", "/home/me/.nvm/v0.10.48")
        .replace("N18", N18)
        .replace("$NVM_DIR", NVM_DIR)
}

fn assert_change_rows(suffix: &str, rows: &[(&str, &str)]) {
    for (before, after) in rows {
        let actual = change_path(&expand(before), suffix, N18, NVM_DIR);
        assert_eq!(actual, expand(after), "change_path({before:?}, {suffix:?})");
    }
}

/// The PATH table of the digest (2.2): (before, after `nvm use 18`).
const DIGEST_PATH_TABLE: &[(&str, &str)] = &[
    ("/a:V20/bin:/usr/bin:/bin", "/a:N18/bin:/usr/bin:/bin"),
    (
        "/opt/bin:V20/bin:/usr/bin:/bin",
        "/opt/bin:N18/bin:/usr/bin:/bin",
    ),
    (
        "/x/bin:V20/bin:/usr/bin:/bin",
        "/x/bin:N18/bin:/usr/bin:/bin",
    ),
    (
        "/usr/bin:/bin:V20/bin:/b",
        "N18/bin:/usr/bin:/bin:V20/bin:/b",
    ),
    (
        "/usr/local/bin:V20/bin:/usr/bin:/bin",
        "N18/bin:/usr/local/bin:V20/bin:/usr/bin:/bin",
    ),
    (
        "V20/bin:/usr/bin:/bin:/b:V1819/bin",
        "N18/bin:V20/bin:/usr/bin:/bin:/b:V1819/bin",
    ),
    ("/a:OLD/bin:/usr/bin:/bin", "/a:N18/bin:/usr/bin:/bin"),
    ("/a:V20/bin/sub:/usr/bin:/bin", "/a:N18/bin:/usr/bin:/bin"),
    ("/a:V20/binx:/usr/bin:/bin", "/a:N18/bin:/usr/bin:/bin"),
    (
        "/a:$NVM_DIR/foo/bin:/usr/bin:/bin",
        "/a:N18/bin:/usr/bin:/bin",
    ),
    (
        "/a:V20/bin:/b:V1819/bin:OLD/bin:/usr/bin:/bin",
        "/a:N18/bin:/b:V1819/bin:N18/bin:/usr/bin:/bin",
    ),
    ("/usr/bin:/bin:", "N18/bin:/usr/bin:/bin:"),
    ("", "N18/bin"),
];

#[test]
fn the_digest_path_table_is_reproduced_row_by_row() {
    assert_change_rows("/bin", DIGEST_PATH_TABLE);
}

#[test]
fn the_digest_manpath_table_is_reproduced_row_by_row() {
    let rows = [
        ("", "N18/share/man:"),
        ("/m1:/m2", "N18/share/man:/m1:/m2:"),
        ("/m1:V20/share/man:/m2", "/m1:N18/share/man:/m2:"),
        (":/m1", "N18/share/man::/m1"),
    ];
    for (before, after) in rows {
        let changed = change_path(&expand(before), "/share/man", N18, NVM_DIR);
        let actual = manpath_with_trailing_colon(&changed);
        assert_eq!(actual, expand(after), "MANPATH {before:?}");
    }
}

/// PATH values probed against the real `nvm_change_path`: (before, after).
const REAL_CHANGE_PATH_PROBES: &[(&str, &str)] = &[
    ("$NVM_DIR/foo:/bin", "N18/bin"),
    (
        "/a:$NVM_DIR/foo:/x/bin:/c",
        "N18/bin:/a:$NVM_DIR/foo:/x/bin:/c",
    ),
    (
        "/a:$NVM_DIR/v1/bin:$NVM_DIR/v2/bin",
        "/a:N18/bin:$NVM_DIR/v2/bin",
    ),
    ("/a:OLD/bin:V20/bin", "/a:N18/bin:V20/bin"),
    ("/a:V20/bin:V20/bin", "/a:N18/bin:V20/bin"),
    ("$NVM_DIR/v1/bin:V20/bin", "N18/bin:V20/bin"),
    ("/bin", "N18/bin:/bin"),
    ("/a:", "N18/bin:/a:"),
    (":/a", "N18/bin::/a"),
    ("/a:V20/bin:", "/a:N18/bin:"),
    ("$NVM_DIR/versions/io.js/v3.3.1/bin:/a", "N18/bin:/a"),
    ("/a:$NVM_DIR/versions/bin:/b", "/a:N18/bin:/b"),
    ("/sbin:V20/bin", "/sbin:N18/bin"),
    ("/opt/bin:/usr/bin/x:V20/bin", "/opt/bin:/usr/bin/x:N18/bin"),
    (
        "/a:/usr/local/bin:V20/bin",
        "N18/bin:/a:/usr/local/bin:V20/bin",
    ),
    ("/a:/bin/:V20/bin", "/a:/bin/:N18/bin"),
    ("/a/usr/bin:$NVM_DIR/v1/bin", "/a/usr/bin:N18/bin"),
    (
        "/usr/bin:$NVM_DIR/foo/bin",
        "N18/bin:/usr/bin:$NVM_DIR/foo/bin",
    ),
    ("$NVM_DIR/a/b/bin:/c", "N18/bin:$NVM_DIR/a/b/bin:/c"),
    ("V20/binary/bin", "N18/bin"),
    ("$NVM_DIR/versions/node/a:b/bin:/c", "N18/bin:/c"),
    ("/a:$NVM_DIR/v1/sbin:/b", "N18/bin:/a:$NVM_DIR/v1/sbin:/b"),
    (
        "/a:$NVM_DIR/v1/sbin/bin:/b",
        "N18/bin:/a:$NVM_DIR/v1/sbin/bin:/b",
    ),
    ("/a:$NVM_DIR/:/bin:/b", "/a:N18/bin:/b"),
];

#[test]
fn probes_of_the_real_change_path_agree() {
    assert_change_rows("/bin", REAL_CHANGE_PATH_PROBES);
}

#[test]
fn a_dot_in_nvm_dir_matches_any_character_like_the_regex_does() {
    assert_change_rows(
        "/bin",
        &[
            ("/home/me/xnvm/v1/bin:/a", "N18/bin:/a"),
            ("/a:/home/me/Xnvm/versions/node/v2/bin:/b", "/a:N18/bin:/b"),
            ("/a:/home/me/énvm/v1/bin:/b", "/a:N18/bin:/b"),
            (
                "/usr/local/bin:/home/me/xnvm/v1/bin",
                "N18/bin:/usr/local/bin:/home/me/xnvm/v1/bin",
            ),
        ],
    );
}

#[test]
fn probes_of_the_real_change_path_for_manpath_agree() {
    assert_change_rows(
        "/share/man",
        &[
            (
                "/usr/share/man:V20/share/man",
                "N18/share/man:/usr/share/man:V20/share/man",
            ),
            ("/m:V20/share/man/x:/n", "/m:N18/share/man:/n"),
            ("/a:$NVM_DIR/v1/bin", "N18/share/man:/a:$NVM_DIR/v1/bin"),
        ],
    );
}

fn assert_strip_rows(suffix: &str, rows: &[(&str, &str)]) {
    for (before, after) in rows {
        let actual = strip_path(&expand(before), suffix, NVM_DIR);
        assert_eq!(actual, expand(after), "strip_path({before:?}, {suffix:?})");
    }
}

#[test]
fn the_digest_strip_example_drops_every_nvm_entry() {
    assert_strip_rows(
        "/bin",
        &[(
            ":/a:V20/bin/sub::$NVM_DIR/foo/bin:$NVM_DIR/versions/bin:$NVM_DIR/bin:\
             $NVM_DIR/x/y/bin:/usr/bin:/bin:",
            ":/a::$NVM_DIR/bin:$NVM_DIR/x/y/bin:/usr/bin:/bin:",
        )],
    );
}

#[test]
fn probes_of_the_real_strip_path_agree() {
    assert_strip_rows(
        "/bin",
        &[
            (
                "/a:V20/bin:$NVM_DIR/v1/bin:$NVM_DIR/versions/io.js/v3/bin/x:/b",
                "/a:/b",
            ),
            (":/a::", ":/a::"),
            ("", ""),
            (":", ":"),
            ("::", "::"),
            ("$NVM_DIR/bin/x:$NVM_DIR/v1/binx", "$NVM_DIR/bin/x"),
            ("$NVM_DIR/versions/a/b/c/bin", "$NVM_DIR/versions/a/b/c/bin"),
            ("/home/me/xnvm/v1/bin", "/home/me/xnvm/v1/bin"),
        ],
    );
    assert_strip_rows(
        "/share/man",
        &[(
            "/home/me/.nvmx/v1/bin:$NVM_DIR/versions/node/v1/share/man",
            "/home/me/.nvmx/v1/bin",
        )],
    );
}

#[test]
fn manpath_gets_a_trailing_colon_only_without_an_empty_entry() {
    let rows = [
        ("", ":"),
        ("/m", "/m:"),
        ("/m1:/m2", "/m1:/m2:"),
        (":/m", ":/m"),
        ("/m:", "/m:"),
        ("/m1::/m2", "/m1::/m2"),
        (":", ":"),
    ];
    for (before, after) in rows {
        assert_eq!(manpath_with_trailing_colon(before), after, "{before:?}");
    }
}
