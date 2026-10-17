use super::*;
use crate::domain::policy::rules::{ToolLayer, merge};
use crate::domain::policy::source::Source;
use crate::domain::timestamp::days_from_iso_date;
use crate::domain::tool::Tool;
use crate::domain::version::{Version, VersionPattern};

fn v(text: &str) -> Version {
    text.parse().unwrap()
}

fn central() -> Source {
    Source::CentralPolicy("/p.json".into())
}

fn day(text: &str) -> Option<i64> {
    days_from_iso_date(text)
}

fn lts(name: &str) -> ReleaseInfo {
    ReleaseInfo {
        lts: Some(name.into()),
        published_day: day("2024-01-01"),
    }
}

fn policy_with_all_rules() -> ToolPolicy {
    let mut layer = ToolLayer::minimum_only(central(), v("20.11.1"));
    layer.allow = nodejs_semver::Range::parse(">=20 <25").ok();
    layer.allow_text = Some(">=20 <25".into());
    layer.deny = vec![v("20.12.0")];
    layer.lts_only = Some(true);
    layer.minimum_release_age_days = Some(14);
    merge(Tool::Node, vec![layer]).0
}

fn minimum_only(text: &str) -> ToolPolicy {
    let pattern: VersionPattern = text.parse().unwrap();
    merge(
        Tool::Node,
        vec![ToolLayer::minimum_only(central(), pattern.lowest())],
    )
    .0
}

#[test]
fn each_rule_reports_its_own_violation() {
    let policy = policy_with_all_rules();
    let today = day("2024-11-01").unwrap();
    let release = ReleaseInfo {
        lts: None,
        published_day: day("2024-10-29"),
    };
    let found = policy
        .violations(&v("v25.1.0"), Some(&release), today)
        .unwrap();
    let text: Vec<String> = found.iter().map(ToString::to_string).collect();
    assert_eq!(
        text,
        vec![
            "outside the allowed range >=20 <25 (from policy /p.json)",
            "not an LTS release (from policy /p.json)",
            "published 3 days ago; at least 14 required (from policy /p.json)",
        ]
    );
    assert_eq!(
        policy
            .violations(&v("v18.0.0"), Some(&lts("Hydrogen")), today)
            .unwrap()[0]
            .to_string(),
        "below the minimum v20.11.1 (from policy /p.json)"
    );
    assert_eq!(
        policy
            .violations(&v("v20.12.0"), Some(&lts("Iron")), today)
            .unwrap()[0]
            .to_string(),
        "denied (from policy /p.json)"
    );
}

#[test]
fn an_allowed_version_has_no_violations() {
    let today = day("2024-11-01").unwrap();
    let old_lts = ReleaseInfo {
        lts: Some("Iron".into()),
        published_day: day("2024-07-08"),
    };
    assert!(
        policy_with_all_rules()
            .violations(&v("v20.15.1"), Some(&old_lts), today)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn release_info_is_needed_only_for_lts_and_age_rules() {
    assert!(!minimum_only("20").needs_release_info());
    assert!(policy_with_all_rules().needs_release_info());
    assert_eq!(
        policy_with_all_rules().violations(&v("v20.15.1"), None, 0),
        Err(MissingReleaseInfo)
    );
}

#[test]
fn an_undated_release_cannot_satisfy_an_age_rule() {
    let undated = ReleaseInfo {
        lts: Some("Iron".into()),
        published_day: None,
    };
    assert_eq!(
        policy_with_all_rules().violations(&v("v20.15.1"), Some(&undated), 0),
        Err(MissingReleaseInfo)
    );
}

#[test]
fn an_iojs_version_is_never_allowed() {
    let found = minimum_only("1")
        .violations(&v("iojs-v3.3.1"), None, 0)
        .unwrap();
    assert_eq!(found, vec![Violation::NotNode]);
    assert_eq!(found[0].to_string(), "io.js is not covered by policy");
}

#[test]
fn a_vulnerable_verdict_names_each_finding_and_the_limit() {
    use crate::domain::policy::{Finding, Severity};
    let finding = |name: &str| Finding {
        name: name.into(),
        severity: Severity::High,
    };
    let violation = Violation::Vulnerable {
        findings: vec![finding("CVE-2024-27983"), finding("CVE-2024-22019")],
        limit: Sourced {
            value: Severity::High,
            source: central(),
        },
    };
    assert_eq!(
        violation.to_string(),
        "CVE-2024-27983 (high), CVE-2024-22019 (high); the limit is high (from policy /p.json)"
    );
}

#[test]
fn an_npm_minimum_is_shown_without_a_v() {
    let layer = ToolLayer::minimum_only(Source::CentralPolicy("p".into()), v("10.2.4"));
    let (policy, _) = merge(Tool::Npm, vec![layer]);
    let found = policy.violations(&v("9.0.0"), None, 0).unwrap();
    assert_eq!(
        found[0].to_string(),
        "below the minimum 10.2.4 (from policy p)"
    );
}
