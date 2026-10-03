use super::*;
use crate::domain::fixtures::{index_text, iojs_releases, node_releases};
use crate::domain::index::parse_index;

fn query(pattern: Option<&str>, lts: Option<&str>) -> Query {
    Query {
        pattern: pattern.map(str::to_owned),
        lts: lts.map(str::to_owned),
    }
}

fn listing(pattern: Option<&str>, lts: Option<&str>) -> (Vec<String>, bool) {
    let (node, iojs) = (node_releases(), iojs_releases());
    let result = list(Some(&node), Some(&iojs), &query(pattern, lts)).unwrap();
    let lines = result.rows.iter().map(RemoteRow::line).collect();
    (lines, result.missing)
}

#[test]
fn everything_is_listed_with_io_js_between_node_0_12_and_4_0() {
    let (lines, missing) = listing(None, None);
    assert_eq!(
        lines,
        [
            "v0.10.48",
            "v0.12.18",
            "iojs-v1.0.0",
            "iojs-v2.5.0",
            "iojs-v3.0.0",
            "iojs-v3.3.1",
            "v4.0.0",
            "v4.9.1 Argon *",
            "v14.21.3 Fermium *",
            "v16.20.2 Gallium *",
            "v18.18.0 Hydrogen",
            "v18.19.0 Hydrogen *",
            "v20.9.0 Iron",
            "v20.10.0 Iron *",
            "v21.1.0",
            "v21.2.0",
        ]
    );
    assert!(!missing);
}

#[test]
fn lts_lists_only_lts_releases_and_never_asks_io_js() {
    let (lines, missing) = listing(None, Some("*"));
    assert_eq!(
        lines,
        [
            "v4.9.1 Argon *",
            "v14.21.3 Fermium *",
            "v16.20.2 Gallium *",
            "v18.18.0 Hydrogen",
            "v18.19.0 Hydrogen *",
            "v20.9.0 Iron",
            "v20.10.0 Iron *",
        ]
    );
    assert!(!missing);
}

#[test]
fn an_lts_name_filters_by_substring_ignoring_case() {
    let expected = ["v20.9.0 Iron", "v20.10.0 Iron *"];
    assert_eq!(listing(None, Some("iron")).0, expected);
    assert_eq!(listing(None, Some("ir")).0, expected);
    assert_eq!(listing(None, Some("IRON")).0, expected);
}

#[test]
fn an_unknown_lts_name_finds_nothing() {
    assert_eq!(listing(None, Some("foo")), (Vec::new(), true));
}

#[test]
fn a_pattern_matches_whole_version_words() {
    let both = ["v20.9.0 Iron", "v20.10.0 Iron *"];
    assert_eq!(listing(Some("20"), None).0, both);
    assert_eq!(listing(Some("v20"), None).0, both);
    assert_eq!(listing(Some("20."), None).0, both);
    assert_eq!(listing(Some("18.19"), None).0, ["v18.19.0 Hydrogen *"]);
    assert_eq!(listing(Some("18.18"), None).0, ["v18.18.0 Hydrogen"]);
    assert_eq!(listing(Some("v20.10.0"), None).0, ["v20.10.0 Iron *"]);
    // `v2` is a whole word in `iojs-v2.5.0`, so io.js answers; node has no v2.
    assert_eq!(
        listing(Some("2"), None),
        (vec!["iojs-v2.5.0".to_owned()], true)
    );
    assert_eq!(
        listing(Some("1"), None),
        (vec!["iojs-v1.0.0".to_owned()], true)
    );
    assert_eq!(listing(Some("v"), None), (Vec::new(), true));
    assert_eq!(listing(Some("99"), None), (Vec::new(), true));
}

#[test]
fn a_pattern_without_io_js_matches_exits_missing_even_with_rows() {
    // nvm.sh exits 3 here: its io.js part found nothing for `20`.
    let (lines, missing) = listing(Some("20"), None);
    assert_eq!(lines.len(), 2);
    assert!(missing);
    // With --lts the io.js part does not run, so the same pattern succeeds.
    assert!(!listing(Some("20"), Some("*")).1);
}

#[test]
fn a_plain_number_also_finds_io_js() {
    let (lines, missing) = listing(Some("3"), None);
    assert_eq!(lines, ["iojs-v3.0.0", "iojs-v3.3.1"]);
    assert!(missing, "the node part found nothing");
}

#[test]
fn the_words_node_and_iojs_pick_one_flavor_and_succeed() {
    let (node_lines, node_missing) = listing(Some("node"), None);
    assert_eq!(node_lines.len(), 12);
    assert!(node_lines.iter().all(|line| !line.starts_with("iojs-")));
    assert!(!node_missing);
    let expected = ["iojs-v1.0.0", "iojs-v2.5.0", "iojs-v3.0.0", "iojs-v3.3.1"];
    assert_eq!(
        listing(Some("iojs"), None),
        (expected.map(String::from).to_vec(), false)
    );
    assert_eq!(listing(Some("io.js"), None).0, expected);
}

#[test]
fn an_iojs_flavor_with_an_lts_filter_finds_nothing() {
    assert_eq!(listing(Some("iojs"), Some("*")), (Vec::new(), true));
}

#[test]
fn stable_and_unstable_are_not_supported_remotely() {
    let (node, iojs) = (node_releases(), iojs_releases());
    for word in ["stable", "unstable"] {
        let result = list(Some(&node), Some(&iojs), &query(Some(word), None));
        assert_eq!(result, Err(RemoteError::ImplicitAlias), "{word}");
    }
}

#[test]
fn an_unreachable_index_is_missing_and_the_other_one_still_lists() {
    let iojs = iojs_releases();
    let result = list(None, Some(&iojs), &query(None, None)).unwrap();
    assert_eq!(result.rows.len(), 4);
    assert!(result.missing);
    let neither = list(None, None, &query(None, None)).unwrap();
    assert_eq!(
        neither,
        Listing {
            rows: Vec::new(),
            missing: true
        }
    );
}

#[test]
fn without_a_v4_row_io_js_comes_after_all_node_rows() {
    let node = parse_index(
        &index_text(&[("v20.0.0", "-"), ("v18.0.0", "-")]),
        Flavor::Node,
    );
    let iojs = iojs_releases();
    let result = list(Some(&node), Some(&iojs), &query(None, None)).unwrap();
    let lines: Vec<String> = result.rows.iter().map(RemoteRow::line).collect();
    assert_eq!(lines[..2], ["v18.0.0", "v20.0.0"]);
    assert_eq!(lines[2], "iojs-v1.0.0");
}

fn names() -> Vec<String> {
    ["*", "argon", "fermium", "gallium", "hydrogen", "iron"]
        .map(String::from)
        .to_vec()
}

#[test]
fn a_back_count_picks_the_codename_before_the_last() {
    assert_eq!(normalize_lts("-1", &names()).unwrap(), "hydrogen");
    assert_eq!(normalize_lts("-2", &names()).unwrap(), "gallium");
    assert_eq!(normalize_lts("-4", &names()).unwrap(), "argon");
}

#[test]
fn going_back_past_the_first_codename_is_an_error_with_status_2() {
    for back in ["-5", "-9", "-12"] {
        let error = normalize_lts(back, &names()).unwrap_err();
        assert_eq!(error, LtsError::TooFarBack, "{back}");
        assert_eq!(error.status(), 2);
    }
    assert_eq!(normalize_lts("-1", &[]), Err(LtsError::TooFarBack));
}

#[test]
fn names_must_be_lowercase_and_stay_as_they_are() {
    let error = normalize_lts("Iron", &names()).unwrap_err();
    assert_eq!(error, LtsError::NotLowercase);
    assert_eq!(error.status(), 3);
    assert_eq!(error.to_string(), "LTS names must be lowercase");
    assert_eq!(normalize_lts("iron", &names()).unwrap(), "iron");
    assert_eq!(normalize_lts("*", &names()).unwrap(), "*");
    assert_eq!(normalize_lts("-0", &names()).unwrap(), "-0");
}
