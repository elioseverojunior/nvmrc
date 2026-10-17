//! The mirror fixture the real `nvm.sh` was run against, for tests of the
//! remote listing.

use crate::domain::index::{Release, parse_index};
use crate::domain::version::Flavor;

#[must_use]
pub fn index_text(rows: &[(&str, &str)]) -> String {
    let header = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity";
    let body: Vec<String> = rows
        .iter()
        .map(|(version, lts)| {
            format!(
                "{version}\t2023-01-01\tlinux-x64\t10.0.0\t11.0.0\t1.0.0\t1.3\t3.0\t100\t{lts}\t-"
            )
        })
        .collect();
    format!("{header}\n{}\n", body.join("\n"))
}

/// Like [`index_text`], with a release date per row: `(version, date, lts)`.
#[must_use]
pub fn index_text_dated(rows: &[(&str, &str, &str)]) -> String {
    let bundling: Vec<(&str, &str, &str, &str)> = rows
        .iter()
        .map(|(version, date, lts)| (*version, *date, *lts, "10.0.0"))
        .collect();
    index_text_bundling(&bundling)
}

/// Like [`index_text_dated`], with the npm each release bundles:
/// `(version, date, lts, npm)`.
#[must_use]
pub fn index_text_bundling(rows: &[(&str, &str, &str, &str)]) -> String {
    let header = "version\tdate\tfiles\tnpm\tv8\tuv\tzlib\topenssl\tmodules\tlts\tsecurity";
    let body: Vec<String> = rows
        .iter()
        .map(|(version, date, lts, npm)| {
            format!("{version}\t{date}\tlinux-x64\t{npm}\t11.0.0\t1.0.0\t1.3\t3.0\t100\t{lts}\t-")
        })
        .collect();
    format!("{header}\n{}\n", body.join("\n"))
}

#[must_use]
pub fn node_releases() -> Vec<Release> {
    parse_index(
        &index_text(&[
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
        ]),
        Flavor::Node,
    )
}

#[must_use]
pub fn iojs_releases() -> Vec<Release> {
    parse_index(
        &index_text(&[
            ("v3.3.1", "-"),
            ("v3.0.0", "-"),
            ("v2.5.0", "-"),
            ("v1.0.0", "-"),
        ]),
        Flavor::IoJs,
    )
}

/// Real entries of the Node Security WG vulnerability index (downloaded
/// 2026-10-17), keeping the shapes the parser has to handle: a plain range, a
/// `win32`-only advisory, no CVE ids, an `unknown` severity, ranges with a
/// leading space and a `<=` comparator. The long texts are dropped.
#[must_use]
pub fn advisories_json() -> &'static str {
    r#"{
  "6": {"cve": ["CVE-2016-9551"], "vulnerable": "^7.1.0", "patched": "^7.2.0",
        "affectedEnvironments": ["all"], "severity": "unknown"},
  "9": {"cve": [], "vulnerable": "6.x", "patched": "^6.9.0",
        "affectedEnvironments": ["all"], "severity": "medium"},
  "54": {"cve": ["CVE-2018-12115"], "vulnerable": "<= 10",
         "patched": "^6.14.4 || ^8.11.4 || >= 10.9.0",
         "affectedEnvironments": ["all"], "severity": "high"},
  "69": {"cve": ["CVE-2020-8277"], "vulnerable": " 12.x || 14.x || 15.x",
         "patched": " ^12.19.1 || ^14.15.1 || 15.2.1",
         "affectedEnvironments": ["all"], "severity": "high"},
  "80": {"cve": ["CVE-2021-22921"], "vulnerable": " 12.x || 14.x || 16.x",
         "patched": " ^12.22.2 || ^14.17.2 || ^16.4.1",
         "affectedEnvironments": ["win32"], "severity": "medium"},
  "192": {"cve": ["CVE-2026-58043"], "vulnerable": "22.x || 24.x || 26.x",
          "patched": "^22.23.2 || ^24.18.1 || ^26.5.1",
          "affectedEnvironments": ["all"], "severity": "high"}
}"#
}
