//! Every word probed against the real `nvm_is_valid_version`.

use super::is_valid_version;

#[test]
fn the_implicit_aliases_and_the_two_prefixes_are_valid() {
    for word in ["stable", "unstable", "node", "iojs"] {
        assert!(is_valid_version(word), "{word}");
    }
}

#[test]
fn partial_and_full_versions_are_valid_with_or_without_a_v() {
    for word in [
        "0",
        "99",
        "18",
        "v18",
        "18.",
        "18.20",
        "v18.20.4",
        "v18.20.4.",
        "1.2.3-rc.1",
    ] {
        assert!(is_valid_version(word), "{word}");
    }
}

#[test]
fn an_iojs_prefixed_version_is_valid() {
    assert!(is_valid_version("iojs-v3"));
    assert!(is_valid_version("iojs-3.3.1"));
    assert!(is_valid_version("iojs-v3.3.1-rc.1"));
}

#[test]
fn aliases_lts_names_and_commands_are_not_valid() {
    let words = [
        "lts/argon",
        "lts/hydrogen",
        "lts/*",
        "missing",
        "io.js",
        "system",
        "app.js",
        "∞",
        "-",
        "--",
        "v",
        "",
        "npm",
    ];
    for word in words {
        assert!(!is_valid_version(word), "{word}");
    }
}

#[test]
fn too_many_parts_or_stray_dots_are_not_valid() {
    for word in ["18.20.4.5", ".18", "18..2", "18.x", "iojs-"] {
        assert!(!is_valid_version(word), "{word}");
    }
}

#[test]
fn a_prerelease_needs_a_full_version_and_a_clean_suffix() {
    assert!(!is_valid_version("1.2-rc.1"));
    assert!(!is_valid_version("1.2.3-rc.1a"));
    assert!(!is_valid_version("1.2.3-"));
    assert!(!is_valid_version("1.2.3-.rc"));
    assert!(!is_valid_version("1.2.3-rc."));
    assert!(!is_valid_version("1.2.3-rc..1"));
    assert!(!is_valid_version("1.2.3-rc_1"));
    assert!(is_valid_version("1.2.3-nightly2024"));
    assert!(is_valid_version("1.2.3-v8-canary"));
}
