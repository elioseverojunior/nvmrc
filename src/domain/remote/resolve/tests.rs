use super::*;
use crate::domain::fixtures::{iojs_releases, node_releases};

fn resolve_text(pattern: Option<&str>, lts: Option<&str>) -> Option<String> {
    let (node, iojs) = (node_releases(), iojs_releases());
    let query = Query {
        pattern: pattern.map(str::to_owned),
        lts: lts.map(str::to_owned),
    };
    resolve(Some(&node), Some(&iojs), &query).map(|version| version.to_string())
}

fn pattern(text: &str) -> Option<String> {
    resolve_text(Some(text), None)
}

/// Every expectation is what the real `nvm version-remote` printed against
/// the same mirrors.
#[test]
fn no_pattern_and_node_and_stable_are_the_newest_release() {
    for text in [None, Some("node"), Some("stable")] {
        assert_eq!(resolve_text(text, None).as_deref(), Some("v21.2.0"));
    }
}

#[test]
fn unstable_is_n_a_when_no_release_line_is_unstable() {
    assert_eq!(pattern("unstable"), None);
}

#[test]
fn iojs_is_the_newest_iojs_release() {
    assert_eq!(pattern("iojs").as_deref(), Some("iojs-v3.3.1"));
}

#[test]
fn lts_narrows_the_implicit_aliases() {
    assert_eq!(resolve_text(None, Some("*")).as_deref(), Some("v20.10.0"));
    assert_eq!(
        resolve_text(Some("stable"), Some("gallium")).as_deref(),
        Some("v16.20.2")
    );
    assert_eq!(
        resolve_text(None, Some("hydrogen")).as_deref(),
        Some("v18.19.0")
    );
    assert_eq!(resolve_text(Some("unstable"), Some("iron")), None);
}

#[test]
fn lts_has_no_iojs_release() {
    assert_eq!(resolve_text(Some("iojs"), Some("*")), None);
}

#[test]
fn a_partial_version_is_its_newest_release() {
    assert_eq!(pattern("20").as_deref(), Some("v20.10.0"));
    assert_eq!(pattern("v4").as_deref(), Some("v4.9.1"));
    assert_eq!(pattern("0.10").as_deref(), Some("v0.10.48"));
    assert_eq!(pattern("20.9").as_deref(), Some("v20.9.0"));
    assert_eq!(pattern("v20.9.0").as_deref(), Some("v20.9.0"));
    assert_eq!(pattern("4.0").as_deref(), Some("v4.0.0"));
    assert_eq!(pattern("18.").as_deref(), Some("v18.19.0"));
    assert_eq!(pattern("0").as_deref(), Some("v0.12.18"));
}

#[test]
fn io_js_releases_are_found_by_their_number() {
    assert_eq!(pattern("3").as_deref(), Some("iojs-v3.3.1"));
    assert_eq!(pattern("2").as_deref(), Some("iojs-v2.5.0"));
    assert_eq!(pattern("iojs-v2").as_deref(), Some("iojs-v2.5.0"));
}

#[test]
fn a_pattern_with_lts_keeps_only_lts_releases() {
    assert_eq!(
        resolve_text(Some("18"), Some("*")).as_deref(),
        Some("v18.19.0")
    );
    assert_eq!(resolve_text(Some("21"), Some("*")), None);
}

#[test]
fn what_matches_nothing_is_n_a() {
    for text in ["99", "x", "v", "99.1", "lts/foo"] {
        assert_eq!(pattern(text), None, "{text}");
    }
}

#[test]
fn io_js_is_not_the_alias_iojs_and_is_n_a() {
    assert_eq!(pattern("io.js"), None);
}

#[test]
fn a_missing_index_resolves_to_nothing() {
    let query = Query::default();
    assert_eq!(resolve(None, None, &query), None);
}
