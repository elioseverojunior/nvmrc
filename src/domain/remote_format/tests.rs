use super::*;
use crate::domain::fixtures::{iojs_releases, node_releases};
use crate::domain::remote::{Query, list};

fn rows(pattern: Option<&str>) -> Vec<RemoteRow> {
    let (node, iojs) = (node_releases(), iojs_releases());
    let query = Query {
        pattern: pattern.map(str::to_owned),
        lts: None,
    };
    list(Some(&node), Some(&iojs), &query).unwrap().rows
}

fn versions(names: &[&str]) -> Vec<Version> {
    names.iter().map(|name| name.parse().unwrap()).collect()
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(target, name)| ((*target).to_owned(), (*name).to_owned()))
        .collect()
}

/// Every expected line below is what the real nvm.sh printed for the same
/// mirror, installed versions and aliases.
#[test]
fn a_plain_listing_matches_nvm_sh() {
    let rows = rows(None);
    let input = FormatInput {
        rows: &rows,
        installed: &[],
        current: "none",
        latest_alias: Some("node"),
        named_aliases: &[],
    };
    assert_eq!(
        format_remote_rows(&input),
        [
            "       v0.10.48",
            "       v0.12.18",
            "    iojs-v1.0.0",
            "    iojs-v2.5.0",
            "    iojs-v3.0.0",
            "    iojs-v3.3.1",
            "         v4.0.0",
            "         v4.9.1   (Latest LTS: Argon)",
            "       v14.21.3   (Latest LTS: Fermium)",
            "       v16.20.2   (Latest LTS: Gallium)",
            "       v18.18.0   (LTS: Hydrogen)",
            "       v18.19.0   (Latest LTS: Hydrogen)",
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0   (Latest LTS: Iron)",
            "        v21.1.0",
            "        v21.2.0                            (Latest: node)",
        ]
    );
}

#[test]
fn installed_versions_and_aliases_match_nvm_sh() {
    let rows = rows(None);
    let installed = versions(&["v20.10.0", "v18.19.0", "iojs-v3.3.1"]);
    let aliases = pairs(&[
        ("node", "default"),
        ("v99.0.0", "future"),
        ("v21.1.0", "pinned"),
        ("v20.10.0", "prod"),
        ("v18", "work"),
    ]);
    let input = FormatInput {
        rows: &rows,
        installed: &installed,
        current: "none",
        latest_alias: Some("node"),
        named_aliases: &aliases,
    };
    assert_eq!(
        format_remote_rows(&input),
        [
            "       v0.10.48",
            "       v0.12.18",
            "    iojs-v1.0.0",
            "    iojs-v2.5.0",
            "    iojs-v3.0.0",
            "    iojs-v3.3.1 *",
            "         v4.0.0",
            "         v4.9.1   (Latest LTS: Argon)",
            "       v14.21.3   (Latest LTS: Fermium)",
            "       v16.20.2   (Latest LTS: Gallium)",
            "       v18.18.0   (LTS: Hydrogen)",
            "       v18.19.0 * (Latest LTS: Hydrogen)                    (Aliases: work)",
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0 * (Latest LTS: Iron)                        (Aliases: prod)",
            "        v21.1.0                                             (Aliases: pinned)",
            "        v21.2.0                            (Latest: node)   (Aliases: default)",
        ]
    );
}

#[test]
fn the_current_version_gets_an_arrow() {
    let rows = rows(None);
    let installed = versions(&["v20.10.0", "v18.19.0", "iojs-v3.3.1"]);
    let aliases = pairs(&[("node", "default")]);
    let input = FormatInput {
        rows: &rows,
        installed: &installed,
        current: "v18.19.0",
        latest_alias: Some("node"),
        named_aliases: &aliases,
    };
    assert_eq!(
        format_remote_rows(&input),
        [
            "       v0.10.48",
            "       v0.12.18",
            "    iojs-v1.0.0",
            "    iojs-v2.5.0",
            "    iojs-v3.0.0",
            "    iojs-v3.3.1 *",
            "         v4.0.0",
            "         v4.9.1   (Latest LTS: Argon)",
            "       v14.21.3   (Latest LTS: Fermium)",
            "       v16.20.2   (Latest LTS: Gallium)",
            "       v18.18.0   (LTS: Hydrogen)",
            "->     v18.19.0 * (Latest LTS: Hydrogen)",
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0 * (Latest LTS: Iron)",
            "        v21.1.0",
            "        v21.2.0                            (Latest: node)   (Aliases: default)",
        ]
    );
}

#[test]
fn several_aliases_on_one_release_are_joined_in_order() {
    let rows = rows(None);
    let aliases = pairs(&[
        ("node", "default"),
        ("node", "latest"),
        ("v20.10.0", "prod"),
        ("v20", "twenty"),
    ]);
    let input = FormatInput {
        rows: &rows,
        installed: &[],
        current: "none",
        latest_alias: Some("node"),
        named_aliases: &aliases,
    };
    assert_eq!(
        format_remote_rows(&input),
        [
            "       v0.10.48",
            "       v0.12.18",
            "    iojs-v1.0.0",
            "    iojs-v2.5.0",
            "    iojs-v3.0.0",
            "    iojs-v3.3.1",
            "         v4.0.0",
            "         v4.9.1   (Latest LTS: Argon)",
            "       v14.21.3   (Latest LTS: Fermium)",
            "       v16.20.2   (Latest LTS: Gallium)",
            "       v18.18.0   (LTS: Hydrogen)",
            "       v18.19.0   (Latest LTS: Hydrogen)",
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0   (Latest LTS: Iron)                        (Aliases: prod, twenty)",
            "        v21.1.0",
            "        v21.2.0                            (Latest: node)   (Aliases: default, latest)",
        ]
    );
}

#[test]
fn a_pattern_listing_has_no_latest_or_alias_columns() {
    let rows = rows(Some("20"));
    let installed = versions(&["v20.10.0", "v18.19.0", "iojs-v3.3.1"]);
    let input = FormatInput {
        rows: &rows,
        installed: &installed,
        current: "system",
        latest_alias: None,
        named_aliases: &[],
    };
    assert_eq!(
        format_remote_rows(&input),
        [
            "        v20.9.0   (LTS: Iron)",
            "       v20.10.0 * (Latest LTS: Iron)",
        ]
    );
}

#[test]
fn old_unstable_releases_are_not_the_latest_stable() {
    assert!(is_old_unstable("v0.11.16"));
    assert!(is_old_unstable("v0.9.1"));
    assert!(!is_old_unstable("v0.10.48"));
    assert!(!is_old_unstable("v0.12.18"));
    assert!(!is_old_unstable("v4.0.0"));
    assert!(!is_old_unstable("v0.1"));
}

#[test]
fn no_rows_format_to_nothing() {
    let input = FormatInput {
        rows: &[],
        installed: &[],
        current: "none",
        latest_alias: Some("node"),
        named_aliases: &[],
    };
    assert!(format_remote_rows(&input).is_empty());
}
