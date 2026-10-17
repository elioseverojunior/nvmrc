use super::cve::{CveLayer, DEFAULT_INDEX, merge_cve};
use super::document::Severity;
use super::source::{Source, Sourced};

fn layer(source: Source, limit: Option<Severity>, index: Option<&str>) -> CveLayer {
    CveLayer {
        source,
        limit,
        index: index.map(Into::into),
    }
}

fn central(limit: Option<Severity>, index: Option<&str>) -> CveLayer {
    layer(Source::CentralPolicy("/p.json".into()), limit, index)
}

fn repository(limit: Option<Severity>, index: Option<&str>) -> CveLayer {
    layer(
        Source::RepositoryPolicy("/w/.nvmrc-policy.json".into()),
        limit,
        index,
    )
}

fn flag(limit: Option<Severity>) -> CveLayer {
    layer(Source::Flag("--fail-on-severity"), limit, None)
}

fn variable(limit: Option<Severity>) -> CveLayer {
    layer(Source::Variable("NVMRC_FAIL_ON_SEVERITY"), limit, None)
}

#[test]
fn without_any_setting_the_limit_is_high_and_the_index_is_the_default() {
    let (policy, warnings) = merge_cve(vec![]);
    assert_eq!(
        policy.limit,
        Sourced {
            value: Severity::High,
            source: Source::Default
        }
    );
    assert_eq!(policy.index.value, DEFAULT_INDEX);
    assert!(warnings.is_empty());
}

#[test]
fn the_strictest_limit_wins_and_a_looser_override_warns() {
    let (policy, warnings) = merge_cve(vec![
        central(Some(Severity::High), None),
        flag(Some(Severity::Critical)),
    ]);
    assert_eq!(policy.limit.value, Severity::High);
    assert_eq!(
        warnings,
        vec![
            "nvm: --fail-on-severity critical is looser than high from policy /p.json; using high."
        ]
    );
    let (policy, warnings) = merge_cve(vec![
        central(Some(Severity::High), None),
        variable(Some(Severity::Low)),
    ]);
    assert_eq!(
        policy.limit,
        Sourced {
            value: Severity::Low,
            source: Source::Variable("NVMRC_FAIL_ON_SEVERITY")
        }
    );
    assert!(warnings.is_empty());
}

#[test]
fn none_is_the_loosest_limit() {
    let (policy, warnings) = merge_cve(vec![
        central(Some(Severity::Medium), None),
        repository(Some(Severity::None), None),
    ]);
    assert_eq!(policy.limit.value, Severity::Medium);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn none_alone_is_kept_and_ties_go_to_the_first_layer() {
    let (policy, _) = merge_cve(vec![central(Some(Severity::None), None)]);
    assert_eq!(policy.limit.value, Severity::None);
    let (policy, warnings) = merge_cve(vec![
        central(Some(Severity::High), None),
        flag(Some(Severity::High)),
    ]);
    assert_eq!(policy.limit.source, Source::CentralPolicy("/p.json".into()));
    assert!(warnings.is_empty());
}

#[test]
fn the_central_index_wins_and_the_repository_index_counts_only_alone() {
    let (with_central, _) = merge_cve(vec![
        central(None, None),
        repository(None, Some("/w/vuln.json")),
    ]);
    assert_eq!(with_central.index.value, DEFAULT_INDEX);
    let (alone, _) = merge_cve(vec![repository(None, Some("/w/vuln.json"))]);
    assert_eq!(
        alone.index,
        Sourced {
            value: "/w/vuln.json".into(),
            source: Source::RepositoryPolicy("/w/.nvmrc-policy.json".into())
        }
    );
    let (set, _) = merge_cve(vec![central(None, Some("/c/index.json"))]);
    assert_eq!(set.index.value, "/c/index.json");
}
