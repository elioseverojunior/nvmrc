use serde_json::{Map, Value};

use super::advisories::{Advisory, IndexError, findings, parse_advisories};
use super::document::Severity;
use crate::domain::fixtures::advisories_json;
use crate::domain::version::Version;

const WIN_VERSION: &str = "v12.0.0";
const WIN_CVE: &str = "CVE-2021-22921";
const NO_CVE_VERSION: &str = "v6.0.0";

fn v(text: &str) -> Version {
    text.parse().unwrap()
}

fn fixture() -> Vec<Advisory> {
    parse_advisories(advisories_json()).unwrap()
}

/// One advisory with the given JSON members merged over a valid base.
fn entry_with(members: &str) -> String {
    let mut entry: Map<String, Value> =
        serde_json::from_str(r#"{"cve": ["CVE-1"], "vulnerable": "22.x", "patched": "^22.1.0"}"#)
            .unwrap();
    let extra: Map<String, Value> = serde_json::from_str(&format!("{{{members}}}")).unwrap();
    entry.extend(extra);
    serde_json::json!({ "1": entry }).to_string()
}

fn entry_without_severity() -> String {
    entry_with(r#""affectedEnvironments": ["all"]"#)
}

#[test]
fn a_vulnerable_unpatched_version_is_affected() {
    let found = findings(&fixture(), &v("v22.20.0"), Some("linux"));
    assert!(
        found
            .iter()
            .any(|f| f.name == "CVE-2026-58043" && f.severity == Severity::High)
    );
}

#[test]
fn a_patched_version_is_not_affected() {
    let found = findings(&fixture(), &v("v22.23.2"), Some("linux"));
    assert!(found.iter().all(|f| f.name != "CVE-2026-58043"));
}

#[test]
fn a_platform_specific_advisory_counts_only_on_its_platform() {
    let on_linux = findings(&fixture(), &v(WIN_VERSION), Some("linux"));
    assert!(on_linux.iter().all(|f| f.name != WIN_CVE));
    let anywhere = findings(&fixture(), &v(WIN_VERSION), None);
    assert!(anywhere.iter().any(|f| f.name == WIN_CVE));
    let on_windows = findings(&fixture(), &v(WIN_VERSION), Some("win32"));
    assert!(on_windows.iter().any(|f| f.name == WIN_CVE));
}

#[test]
fn an_advisory_without_cve_ids_is_named_by_id() {
    let found = findings(&fixture(), &v(NO_CVE_VERSION), Some("linux"));
    assert!(found.iter().any(|f| f.name == "advisory 9"));
}

#[test]
fn one_finding_is_made_per_cve_id() {
    let text = entry_with(r#""cve": ["CVE-1", "CVE-2"]"#);
    let advisories = parse_advisories(&text).unwrap();
    let found = findings(&advisories, &v("v22.0.0"), None);
    assert_eq!(found.len(), 2);
}

#[test]
fn a_comparator_range_covers_old_versions_until_patched() {
    let old = findings(&fixture(), &v("v8.0.0"), None);
    assert!(old.iter().any(|f| f.name == "CVE-2018-12115"));
    let patched = findings(&fixture(), &v("v10.9.0"), None);
    assert!(patched.iter().all(|f| f.name != "CVE-2018-12115"));
}

#[test]
fn io_js_versions_never_match() {
    let found = findings(&fixture(), &v("iojs-v3.0.0"), None);
    assert!(found.is_empty());
}

#[test]
fn unknown_and_missing_severities_count_as_high() {
    let unknown = parse_advisories(&entry_with(r#""severity": "unknown""#)).unwrap();
    let missing = parse_advisories(&entry_without_severity()).unwrap();
    assert_eq!(
        (unknown[0].severity, missing[0].severity),
        (Severity::High, Severity::High)
    );
}

#[test]
fn anything_else_is_a_failure_to_know() {
    assert!(matches!(
        parse_advisories("[]"),
        Err(IndexError::NotAnIndex(_))
    ));
    assert!(matches!(
        parse_advisories(&entry_with(r#""severity": "urgent""#)),
        Err(IndexError::BadEntry { .. })
    ));
    assert!(matches!(
        parse_advisories(&entry_with(r#""vulnerable": ">>22""#)),
        Err(IndexError::BadEntry { .. })
    ));
    assert!(matches!(
        parse_advisories(r#"{"1": {"cve": []}}"#),
        Err(IndexError::BadEntry { .. })
    ));
}

#[test]
fn an_empty_patched_range_means_no_fix() {
    let text = entry_with(r#""patched": """#);
    assert!(parse_advisories(&text).unwrap()[0].patched.is_none());
}

#[test]
fn ranges_with_spaces_and_comparators_parse() {
    assert_eq!(fixture().len(), 6);
}

#[test]
fn severities_rank_from_none_to_critical() {
    assert!(Severity::None.rank() < Severity::Low.rank());
    assert!(Severity::High.rank() < Severity::Critical.rank());
}
