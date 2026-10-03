//! The release index of a mirror, `<mirror>/index.tab`: a tab-separated table
//! with a header line, newest release first. Column 1 is the version, column
//! 10 the LTS codename (`-` when there is none).

use std::collections::HashSet;

use crate::domain::version::{Flavor, Version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// The LTS codename as the index spells it (`Iron`).
    pub lts: Option<String>,
}

/// The releases of `index_tab`, in file order. The header, blank lines and
/// rows whose version is not one are skipped.
#[must_use]
pub fn parse_index(index_tab: &str, flavor: Flavor) -> Vec<Release> {
    index_tab
        .lines()
        .skip(1)
        .filter_map(|line| parse_row(line, flavor))
        .collect()
}

fn parse_row(line: &str, flavor: Flavor) -> Option<Release> {
    let columns: Vec<&str> = line.split_whitespace().collect();
    let raw = columns.first()?;
    let text = match flavor {
        Flavor::Node => (*raw).to_owned(),
        Flavor::IoJs => format!("iojs-{raw}"),
    };
    let lts = columns
        .get(9)
        .filter(|name| **name != "-")
        .map(|name| (*name).to_owned());
    Some(Release {
        version: text.parse().ok()?,
        lts,
    })
}

fn is_alias_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
        && characters.all(|rest| rest.is_ascii_alphanumeric() || "._-".contains(rest))
}

/// The `lts/*` aliases a listing refreshes, as `(alias, target)` in the order
/// `nvm.sh` writes them: `lts/*` first (the newest codename), then one alias
/// per codename pointing at the newest release of that codename. Names that
/// are not plain lowercase words are skipped.
#[must_use]
pub fn lts_aliases(releases: &[Release]) -> Vec<(String, String)> {
    let mut seen = HashSet::new();
    let mut aliases: Vec<(String, String)> = Vec::new();
    for release in releases {
        let Some(name) = release.lts.as_deref().map(str::to_lowercase) else {
            continue;
        };
        if !is_alias_name(&name) || !seen.insert(name.clone()) {
            continue;
        }
        let alias = format!("lts/{name}");
        if aliases.is_empty() {
            aliases.push(("lts/*".to_owned(), alias.clone()));
        }
        aliases.push((alias, release.version.to_string()));
    }
    aliases
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(version: &str, lts: &str) -> String {
        format!("{version}\t2023-01-01\tlinux-x64\t10.0.0\t11.0.0\t1.0.0\t1.3\t3.0\t100\t{lts}\t-")
    }

    fn index(rows: &[(&str, &str)]) -> String {
        let header = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity";
        let body: Vec<String> = rows.iter().map(|(v, l)| row(v, l)).collect();
        format!("{header}\n{}\n", body.join("\n"))
    }

    fn node_index() -> String {
        index(&[
            ("v21.2.0", "-"),
            ("v21.1.0", "-"),
            ("v20.10.0", "Iron"),
            ("v20.9.0", "Iron"),
            ("v18.19.0", "Hydrogen"),
            ("v18.18.0", "Hydrogen"),
            ("v16.20.2", "Gallium"),
            ("v14.21.3", "Fermium"),
            ("v4.9.1", "Argon"),
            ("v4.0.0", "-"),
            ("v0.12.18", "-"),
            ("v0.10.48", "-"),
        ])
    }

    #[test]
    fn the_header_is_skipped_and_file_order_is_kept() {
        let releases = parse_index(&node_index(), Flavor::Node);
        assert_eq!(releases.len(), 12);
        assert_eq!(releases[0].version.to_string(), "v21.2.0");
        assert_eq!(releases[11].version.to_string(), "v0.10.48");
    }

    #[test]
    fn the_lts_column_is_read_and_a_dash_means_none() {
        let releases = parse_index(&node_index(), Flavor::Node);
        assert_eq!(releases[0].lts, None);
        assert_eq!(releases[2].lts.as_deref(), Some("Iron"));
    }

    #[test]
    fn io_js_rows_get_the_iojs_flavor() {
        let releases = parse_index(&index(&[("v3.3.1", "-"), ("v1.0.0", "-")]), Flavor::IoJs);
        let versions: Vec<String> = releases.iter().map(|r| r.version.to_string()).collect();
        assert_eq!(versions, ["iojs-v3.3.1", "iojs-v1.0.0"]);
    }

    #[test]
    fn blank_lines_short_rows_and_junk_are_tolerated() {
        let text = "header\n\nv20.0.0\nnot-a-version\tx\nv18.0.0\t2023\r\n";
        let releases = parse_index(text, Flavor::Node);
        let versions: Vec<String> = releases.iter().map(|r| r.version.to_string()).collect();
        assert_eq!(versions, ["v20.0.0", "v18.0.0"]);
        assert!(releases.iter().all(|release| release.lts.is_none()));
    }

    #[test]
    fn lts_aliases_follow_the_order_nvm_sh_writes_them() {
        let aliases = lts_aliases(&parse_index(&node_index(), Flavor::Node));
        let pairs: Vec<(&str, &str)> = aliases
            .iter()
            .map(|(alias, target)| (alias.as_str(), target.as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                ("lts/*", "lts/iron"),
                ("lts/iron", "v20.10.0"),
                ("lts/hydrogen", "v18.19.0"),
                ("lts/gallium", "v16.20.2"),
                ("lts/fermium", "v14.21.3"),
                ("lts/argon", "v4.9.1"),
            ]
        );
    }

    #[test]
    fn a_codename_points_at_its_newest_release_and_names_are_lowercased() {
        let aliases = lts_aliases(&parse_index(
            &index(&[
                ("v20.10.0", "Iron"),
                ("v20.9.0", "iron"),
                ("v18.1.0", "Hydrogen"),
            ]),
            Flavor::Node,
        ));
        assert_eq!(aliases[1], ("lts/iron".to_owned(), "v20.10.0".to_owned()));
        assert_eq!(aliases.len(), 3);
    }

    #[test]
    fn names_that_are_not_plain_words_are_skipped() {
        let aliases = lts_aliases(&parse_index(
            &index(&[
                ("v20.0.0", "Iron!"),
                ("v18.0.0", "-bad"),
                ("v16.0.0", "Gallium"),
            ]),
            Flavor::Node,
        ));
        assert_eq!(
            aliases,
            [
                ("lts/*".to_owned(), "lts/gallium".to_owned()),
                ("lts/gallium".to_owned(), "v16.0.0".to_owned()),
            ]
        );
    }

    #[test]
    fn without_any_lts_release_there_are_no_aliases() {
        let aliases = lts_aliases(&parse_index(&index(&[("v21.0.0", "-")]), Flavor::Node));
        assert!(aliases.is_empty());
    }
}
