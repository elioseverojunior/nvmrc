use super::*;
use crate::domain::tool::Tool;

struct Rules(NodeRules);

impl Rules {
    fn with(edit: impl FnOnce(&mut NodeRules)) -> Self {
        let mut rules = NodeRules {
            minimum: None,
            allow: None,
            deny: None,
            lts_only: None,
            minimum_release_age_days: None,
        };
        edit(&mut rules);
        Self(rules)
    }

    fn deny(mut self, versions: &[&str]) -> Self {
        self.0.deny = Some(versions.iter().map(|text| (*text).to_owned()).collect());
        self
    }
}

fn minimum(text: &str) -> Rules {
    Rules::with(|rules| rules.minimum = Some(text.into()))
}

fn allow(range: &str) -> Rules {
    Rules::with(|rules| rules.allow = Some(range.into()))
}

fn lts_only(value: bool) -> Rules {
    Rules::with(|rules| rules.lts_only = Some(value))
}

fn age(days: u32) -> Rules {
    Rules::with(|rules| rules.minimum_release_age_days = Some(days))
}

fn layer(source: Source, rules: Rules) -> ToolLayer {
    ToolLayer::from_node(source, &rules.0).unwrap()
}

fn central(rules: Rules) -> ToolLayer {
    layer(Source::CentralPolicy("/p.json".into()), rules)
}

fn flag(rules: Rules) -> ToolLayer {
    layer(Source::Flag("--min-node"), rules)
}

fn variable(rules: Rules) -> ToolLayer {
    layer(Source::Variable("NVM_MIN_VERSION"), rules)
}

fn repository(rules: Rules) -> ToolLayer {
    layer(
        Source::RepositoryPolicy("/w/.nvmrc-policy.json".into()),
        rules,
    )
}

#[test]
fn the_highest_minimum_wins_and_names_its_source() {
    let (policy, _) = merge(
        Tool::Node,
        vec![central(minimum("20.11.1")), variable(minimum("22"))],
    );
    let winner = policy.minimum.unwrap();
    assert_eq!(winner.value.to_string(), "v22.0.0");
    assert_eq!(winner.source, Source::Variable("NVM_MIN_VERSION"));
}

#[test]
fn a_lower_override_loses_with_a_warning() {
    let (policy, warnings) = merge(
        Tool::Node,
        vec![central(minimum("20.11.1")), flag(minimum("16"))],
    );
    assert_eq!(policy.minimum.unwrap().value.to_string(), "v20.11.1");
    assert_eq!(
        warnings,
        vec![
            "nvm: --min-node v16.0.0 is looser than v20.11.1 from policy /p.json; using v20.11.1."
        ]
    );
}

#[test]
fn allow_ranges_all_apply_and_deny_lists_add_up() {
    let (policy, _) = merge(
        Tool::Node,
        vec![
            central(allow(">=20 <25").deny(&["20.12.0"])),
            repository(allow("^22").deny(&["22.3.0"])),
        ],
    );
    assert_eq!(policy.allow.len(), 2);
    assert_eq!(policy.deny.len(), 2);
}

#[test]
fn lts_only_is_on_when_any_source_sets_it_and_an_explicit_false_warns() {
    let (policy, warnings) = merge(
        Tool::Node,
        vec![central(lts_only(true)), repository(lts_only(false))],
    );
    assert!(policy.lts_only.is_some());
    assert_eq!(warnings.len(), 1);
}

#[test]
fn the_largest_release_age_wins() {
    let (policy, warnings) = merge(Tool::Node, vec![central(age(14)), repository(age(7))]);
    assert_eq!(policy.minimum_release_age_days.unwrap().value, 14);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn the_central_policy_never_warns_about_itself() {
    let (_, warnings) = merge(
        Tool::Node,
        vec![central(minimum("18")), repository(minimum("20"))],
    );
    assert!(warnings.is_empty());
}

#[test]
fn sources_display_as_users_know_them() {
    assert_eq!(Source::Flag("--min-node").to_string(), "--min-node");
    assert_eq!(
        Source::CentralPolicy("https://sec/p.json".into()).to_string(),
        "policy https://sec/p.json"
    );
    assert_eq!(
        Source::RepositoryPolicy("/w/.nvmrc-policy.json".into()).to_string(),
        "repository policy /w/.nvmrc-policy.json"
    );
    assert_eq!(
        Source::MinVersionFile("/n/min-version".into()).to_string(),
        "/n/min-version"
    );
    assert!(!Source::CentralPolicy(String::new()).is_override());
}

#[test]
fn a_layer_rejects_a_blank_allow_and_iojs_versions() {
    let source = || Source::CentralPolicy("/p.json".into());
    for rules in [
        allow(""),
        minimum("iojs-v3"),
        Rules::with(|_| {}).deny(&["iojs-v3.0.0"]),
    ] {
        assert!(ToolLayer::from_node(source(), &rules.0).is_err());
    }
}

fn package_layer(source: Source, rules: &PackageRules) -> ToolLayer {
    ToolLayer::from_package(Tool::Npm, source, rules).unwrap()
}

fn package_minimum(text: &str) -> PackageRules {
    PackageRules {
        minimum: Some(text.into()),
        allow: None,
        deny: None,
        minimum_release_age_days: None,
    }
}

#[test]
fn a_package_layer_names_its_own_fields() {
    let rules = PackageRules {
        deny: Some(vec!["10.x".into()]),
        ..package_minimum("10")
    };
    let error = ToolLayer::from_package(Tool::Npm, Source::Default, &rules).unwrap_err();
    assert!(error.to_string().starts_with("tools.npm.deny[0]:"));
}

#[test]
fn package_minimums_tighten_like_node() {
    let central = Source::CentralPolicy("/p.json".into());
    let (policy, warnings) = merge(
        Tool::Npm,
        vec![
            package_layer(central, &package_minimum("10.8.0")),
            package_layer(Source::Flag("--min-npm"), &package_minimum("10.2.4")),
        ],
    );
    assert_eq!(policy.tool, Tool::Npm);
    assert_eq!(policy.minimum.unwrap().value.to_string(), "v10.8.0");
    assert_eq!(
        warnings,
        vec!["nvm: --min-npm 10.2.4 is looser than 10.8.0 from policy /p.json; using 10.8.0."]
    );
}

#[test]
fn an_unrestricted_policy_has_no_rule() {
    assert!(ToolPolicy::unrestricted(Tool::Yarn).is_unrestricted());
    let (policy, _) = merge(Tool::Node, vec![central(minimum("20"))]);
    assert!(!policy.is_unrestricted());
}
